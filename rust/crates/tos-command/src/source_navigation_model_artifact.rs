//! Bounded create-only publication of a compiled native source-navigation model.
//!
//! This owner installs only a byte-for-byte candidate named by a genuine
//! compiler stage receipt. It does not admit the source cut or replace an
//! existing artifact.

use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_creation_store::active;
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags};
use rustix::io::Errno;
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_compiler::NativeFsVerityMeasurement;
use tos_compiler::knowledge_stage::StageReceipt;
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, canonical_bytes_v1,
    parse_json,
};

#[derive(Debug)]
pub(crate) enum NativeModelArtifactPublishError {
    Source(SourceCommandError),
    Compiler(tos_compiler::Error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PublishedCanonicalJson {
    pub sha256: Digest256,
    pub size_bytes: u64,
}

impl From<SourceCommandError> for NativeModelArtifactPublishError {
    fn from(error: SourceCommandError) -> Self {
        Self::Source(error)
    }
}

impl From<tos_compiler::Error> for NativeModelArtifactPublishError {
    fn from(error: tos_compiler::Error) -> Self {
        Self::Compiler(error)
    }
}

type PublishResult<T> = Result<T, NativeModelArtifactPublishError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

fn identity(file: &File) -> SourceCommandResult<FileIdentity> {
    let metadata = file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("native model artifact metadata"))?;
    Ok(FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

fn safe_leaf(leaf: &str) -> bool {
    !leaf.is_empty()
        && leaf != "."
        && leaf != ".."
        && !leaf.bytes().any(|byte| byte == b'/' || byte == 0)
}

fn same_named_directory(path: &Path, pinned: &File) -> SourceCommandResult<()> {
    let named = tos_fd_open::open_absolute_directory(path)
        .map_err(|_| SourceCommandError::Conflict("native model artifact directory changed"))?;
    if identity(&named)? != identity(pinned)? {
        return Err(SourceCommandError::Conflict(
            "native model artifact directory changed",
        ));
    }
    Ok(())
}

fn temporary_name() -> SourceCommandResult<String> {
    let mut random = [0u8; 16];
    File::open("/dev/urandom")
        .and_then(|mut source| source.read_exact(&mut random))
        .map_err(|_| SourceCommandError::Invalid("native model artifact temporary name"))?;
    Ok(format!(
        ".tos-native-model-{}-{}",
        std::process::id(),
        Digest256::of_bytes(&random).to_hex()
    ))
}

struct PendingTemporary<'a> {
    parent: &'a File,
    name: String,
    identity: FileIdentity,
    published: bool,
}

impl PendingTemporary<'_> {
    fn publish(&mut self) {
        self.published = true;
    }
}

impl Drop for PendingTemporary<'_> {
    fn drop(&mut self) {
        if self.published {
            return;
        }
        let Ok(named) = tos_fd_open::open_regular_at(self.parent, Path::new(&self.name)) else {
            return;
        };
        if identity(&named).ok() == Some(self.identity) {
            let _ = rustix::fs::unlinkat(self.parent, self.name.as_str(), AtFlags::empty());
        }
    }
}

/// Copy and install an exact staged model under a protected directory.
///
/// The source and output byte counts are capped and cancellation is checked
/// throughout the copy. A unique mode-0600 temporary is created and renamed
/// relative to the pinned directory descriptor. The absolute directory path
/// exists only because the compiler's fs-verity preparation API takes a path.
pub(crate) fn publish_completed_native_model_artifact(
    source_candidate_path: &Path,
    destination_directory: &Path,
    leaf: &str,
    receipt: &StageReceipt,
    byte_cap: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> PublishResult<NativeFsVerityMeasurement> {
    active(deadline, cancelled)?;
    if !safe_leaf(leaf) {
        return Err(SourceCommandError::Invalid("native model artifact leaf").into());
    }
    if receipt.sqlite_size_bytes == 0 || receipt.sqlite_size_bytes > byte_cap {
        return Err(SourceCommandError::Unsupported("native model artifact byte cap").into());
    }
    if !destination_directory.is_absolute() {
        return Err(SourceCommandError::Invalid("native model artifact directory").into());
    }

    let mut source = tos_fd_open::open_absolute_regular(source_candidate_path, byte_cap)
        .map_err(|_| SourceCommandError::Invalid("native model candidate path"))?;
    if source
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("native model candidate metadata"))?
        .len()
        != receipt.sqlite_size_bytes
    {
        return Err(SourceCommandError::Conflict("native model candidate size differs").into());
    }

    let parent = tos_fd_open::open_absolute_directory(destination_directory)
        .map_err(|_| SourceCommandError::Invalid("native model artifact directory"))?;
    let mut created = None;
    for _ in 0..4 {
        active(deadline, cancelled)?;
        let name = temporary_name()?;
        let fd = match rustix::fs::openat(
            &parent,
            name.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        ) {
            Ok(fd) => fd,
            Err(error) if error == Errno::EXIST => continue,
            Err(_) => {
                return Err(SourceCommandError::Invalid("native model temporary creation").into());
            }
        };
        let file = File::from(fd);
        let file_identity = match identity(&file) {
            Ok(identity) => identity,
            Err(error) => {
                let _ = rustix::fs::unlinkat(&parent, name.as_str(), AtFlags::empty());
                return Err(error.into());
            }
        };
        created = Some((name, file, file_identity));
        break;
    }
    let (temporary_name, mut output, file_identity) = created.ok_or_else(|| {
        NativeModelArtifactPublishError::Source(SourceCommandError::Conflict(
            "native model temporary name collision",
        ))
    })?;
    let mut pending = PendingTemporary {
        parent: &parent,
        name: temporary_name.clone(),
        identity: file_identity,
        published: false,
    };
    rustix::fs::fchmod(&output, Mode::from_raw_mode(0o600))
        .map_err(|_| SourceCommandError::Invalid("native model temporary permissions"))?;

    let mut digest = Digest256Hasher::new();
    let mut copied = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        active(deadline, cancelled)?;
        let remaining = receipt.sqlite_size_bytes.saturating_sub(copied);
        if remaining == 0 {
            let mut extra = [0u8; 1];
            if source
                .read(&mut extra)
                .map_err(|_| SourceCommandError::Invalid("native model candidate read"))?
                != 0
            {
                return Err(
                    SourceCommandError::Unsupported("native model artifact byte cap").into(),
                );
            }
            break;
        }
        let read_limit = buffer
            .len()
            .min(remaining.min(buffer.len() as u64) as usize);
        let count = source
            .read(&mut buffer[..read_limit])
            .map_err(|_| SourceCommandError::Invalid("native model candidate read"))?;
        if count == 0 {
            break;
        }
        let next = copied
            .checked_add(count as u64)
            .ok_or(SourceCommandError::Unsupported(
                "native model artifact byte cap",
            ))?;
        if next > byte_cap || next > receipt.sqlite_size_bytes {
            return Err(SourceCommandError::Unsupported("native model artifact byte cap").into());
        }
        output
            .write_all(&buffer[..count])
            .map_err(|_| SourceCommandError::Invalid("native model temporary write"))?;
        digest.update(&buffer[..count]);
        copied = next;
    }
    if copied != receipt.sqlite_size_bytes || digest.finalize().to_hex() != receipt.sqlite_sha256 {
        return Err(SourceCommandError::Conflict("native model candidate fixity differs").into());
    }
    active(deadline, cancelled)?;

    rustix::fs::fchmod(&output, Mode::from_raw_mode(0o600))
        .map_err(|_| SourceCommandError::Invalid("native model temporary permissions"))?;
    output
        .sync_all()
        .map_err(|_| SourceCommandError::Invalid("native model temporary fsync"))?;
    let output_metadata = output
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("native model temporary metadata"))?;
    if output_metadata.len() != receipt.sqlite_size_bytes
        || output_metadata.mode() & 0o777 != 0o600
        || identity(&output)? != pending.identity
    {
        return Err(SourceCommandError::Conflict("native model temporary changed").into());
    }
    drop(output);

    same_named_directory(destination_directory, &parent)?;
    let temporary_path: PathBuf = destination_directory.join(&temporary_name);
    active(deadline, cancelled)?;
    let measurement = tos_compiler::prepare_native_knowledge_artifact(&temporary_path, receipt)?;
    active(deadline, cancelled)?;
    same_named_directory(destination_directory, &parent)?;

    let named_temporary = tos_fd_open::open_regular_at(&parent, Path::new(&temporary_name))
        .map_err(|_| SourceCommandError::Conflict("native model temporary changed"))?;
    let named_metadata = named_temporary
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("native model temporary metadata"))?;
    if identity(&named_temporary)? != pending.identity
        || named_metadata.len() != receipt.sqlite_size_bytes
        || named_metadata.mode() & 0o777 != 0o600
    {
        return Err(SourceCommandError::Conflict("native model temporary changed").into());
    }

    active(deadline, cancelled)?;
    rustix::fs::renameat_with(
        &parent,
        temporary_name.as_str(),
        &parent,
        leaf,
        RenameFlags::NOREPLACE,
    )
    .map_err(|error| {
        if error == Errno::EXIST {
            SourceCommandError::Conflict("native model artifact destination exists")
        } else {
            SourceCommandError::Invalid("native model artifact publication")
        }
    })?;
    pending.publish();
    parent
        .sync_all()
        .map_err(|_| SourceCommandError::Invalid("native model artifact directory fsync"))?;
    Ok(measurement)
}

/// Validate and atomically publish one canonical owner JSON document.
///
/// The pinned directory descriptor is both the publication root and the
/// boundary for `atomic_write`; no path re-resolution or replacement is used.
pub(crate) fn publish_canonical_owner_json(
    parent: &File,
    leaf: &str,
    canonical_bytes: &[u8],
    max_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PublishedCanonicalJson> {
    active(deadline, cancelled)?;
    if !safe_leaf(leaf) || canonical_bytes.is_empty() || canonical_bytes.len() > max_bytes {
        return Err(SourceCommandError::Invalid("canonical owner JSON bounds"));
    }
    let limits = JsonLimits::new(max_bytes, 64, 300_000, 4_300)
        .map_err(|_| SourceCommandError::Invalid("canonical owner JSON limits"))?;
    let parsed = parse_json(canonical_bytes, JsonMode::PublishedStrict, limits)
        .map_err(|_| SourceCommandError::Invalid("canonical owner JSON parsing"))?;
    let verified = canonical_bytes_v1(
        parsed.root(),
        CanonicalProfile::SourceCommandInputV1,
        limits,
    )
    .map_err(|_| SourceCommandError::Unsupported("canonical owner JSON byte cap"))?;
    if verified.as_slice() != canonical_bytes {
        return Err(SourceCommandError::Invalid("owner JSON is not canonical"));
    }
    let size_bytes = u64::try_from(canonical_bytes.len())
        .map_err(|_| SourceCommandError::Unsupported("canonical owner JSON byte cap"))?;
    let sha256 = Digest256::of_bytes(canonical_bytes);

    crate::source_work_transaction::atomic_write(
        parent,
        leaf,
        canonical_bytes,
        true,
        deadline,
        cancelled,
    )?;
    let mut installed = tos_fd_open::open_regular_at(parent, Path::new(leaf))
        .map_err(|_| SourceCommandError::Conflict("published owner JSON unavailable"))?;
    let metadata = installed
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("published owner JSON metadata"))?;
    if metadata.len() != size_bytes || metadata.mode() & 0o777 != 0o600 {
        return Err(SourceCommandError::Conflict(
            "published owner JSON mode or size differs",
        ));
    }
    let mut installed_digest = Digest256Hasher::new();
    let mut installed_size = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        active(deadline, cancelled)?;
        let count = installed
            .read(&mut buffer)
            .map_err(|_| SourceCommandError::Invalid("published owner JSON readback"))?;
        if count == 0 {
            break;
        }
        installed_size = installed_size
            .checked_add(count as u64)
            .filter(|size| *size <= size_bytes)
            .ok_or(SourceCommandError::Conflict(
                "published owner JSON size changed",
            ))?;
        installed_digest.update(&buffer[..count]);
    }
    if installed_size != size_bytes || installed_digest.finalize() != sha256 {
        return Err(SourceCommandError::Conflict(
            "published owner JSON bytes differ",
        ));
    }
    Ok(PublishedCanonicalJson { sha256, size_bytes })
}
