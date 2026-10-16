//! Custody-only import from one exact Git capture and an optional retained cut
//! into a fresh immutable V1 corpus.
//!
//! This writes byte custody and one exact manifest revision. It does not run
//! source semantic validation, establish rights, or select a current pointer.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonNumber, JsonNumberKind,
    JsonString, JsonValue, SourceRevision, canonical_bytes_v1,
};

use crate::archive::{
    CaptureMemberSink, CaptureReadUsage, CaptureRestoreLimits, walk_capture_for_import,
};
use crate::cut::is_authored_source_path_v1;
use crate::{
    CorpusReader, CutReadLimits, MemberMetadata, Result, SoftwareCaptureSelectionV1, StoreError,
    StoreErrorCode as Code, feed_source_membership_v1,
};

const IMPORT_RECEIPT_SCHEMA: &str = "tos_corpus_capture_import_v1";
const RETAINED_IMPORT_RECEIPT_SCHEMA: &str = "tos_corpus_capture_import_v2";
const MECHANICAL_VALIDATOR_ROLE: &str = "capture-import-byte-integrity-v1";
const MAX_EXECUTABLE_BYTES: u64 = 536_870_912;

/// Exact capture selection and a new store directory beneath a private parent.
/// `expected_validator_sha256` is checked against the executing ELF image; it
/// is retained as the snapshot's mechanical byte-validator identity only.
pub struct CaptureImportRequestV1<'a> {
    pub capture_root: &'a Path,
    pub output: &'a Path,
    pub selection: &'a SoftwareCaptureSelectionV1,
    pub limits: CaptureRestoreLimits,
    pub expected_validator_sha256: Digest256,
    pub retained: Option<RetainedCaptureSelectionV1<'a>>,
}

/// One explicitly selected immutable historical subtree. Container revision
/// and source membership are distinct: the binding also pins portable modes.
pub struct RetainedCaptureSelectionV1<'a> {
    pub root: &'a Path,
    pub revision: SourceRevision,
    pub receipt_file: &'a str,
    pub scope: tos_foundation::RelativePath,
    pub membership_sha256: Digest256,
    pub member_count: u64,
    pub source_bytes: u64,
    pub member_mode: u32,
    pub binding_sha256: Digest256,
}

/// Verified retained input, with no source or publication grant.
#[derive(Clone, Debug)]
pub struct RetainedCaptureReceiptV1 {
    pub revision: SourceRevision,
    pub validator_sha256: Digest256,
    pub scope: tos_foundation::RelativePath,
    pub membership_sha256: Digest256,
    pub member_count: u64,
    pub source_bytes: u64,
    pub member_mode: u32,
    pub binding_sha256: Digest256,
    pub original_receipt_sha256: Digest256,
    pub receipt_ref: String,
    pub member_bytes_read: u64,
}

/// Durable custody evidence for an exact Git capture and optional retained input.
#[derive(Clone, Debug)]
pub struct CaptureImportReceipt {
    pub schema_version: &'static str,
    pub capture_manifest_sha256: Digest256,
    pub source_git_commit: String,
    pub source_git_tree: String,
    pub include_prefixes: Vec<String>,
    pub exclude_prefixes: Vec<String>,
    pub exclude_path_parts: Vec<String>,
    pub validator_sha256: Digest256,
    pub revision: SourceRevision,
    pub membership_sha256: Digest256,
    pub member_count: u64,
    pub source_bytes: u64,
    pub import_receipt_sha256: Digest256,
    pub usage: CaptureReadUsage,
    pub retained: Option<RetainedCaptureReceiptV1>,
}

fn fail(detail: &'static str) -> StoreError {
    StoreError::new(Code::CorruptSelectedObject, detail)
}

fn io_error(detail: &'static str, error: io::Error) -> StoreError {
    StoreError::io(detail, error)
}

fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
        Err(StoreError::new(
            Code::BudgetExceeded,
            "capture import interrupted",
        ))
    } else {
        Ok(())
    }
}

fn fd_path(dir: &File, name: &str) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", dir.as_raw_fd())).join(name)
}

fn create_directory(parent: &File, name: &str) -> Result<File> {
    fs::DirBuilder::new()
        .mode(0o700)
        .create(fd_path(parent, name))
        .map_err(|error| io_error("cannot create private corpus directory", error))?;
    parent
        .sync_all()
        .map_err(|error| io_error("cannot persist private corpus directory", error))?;
    tos_fd_open::open_directory_at(parent, Path::new(name))
        .map_err(|_| fail("unsafe new corpus directory"))
}

fn open_regular(parent: &File, name: &str) -> Result<File> {
    tos_fd_open::open_regular_at(parent, Path::new(name))
        .map_err(|_| fail("unsafe imported corpus file"))
}

fn image_identity(metadata: &fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}

fn same_linked_file(before: &fs::Metadata, after: &fs::Metadata) -> bool {
    // Adding a hard link changes ctime, so compare stable inode/content mode
    // fields while preserving the exact file identity and sealed length.
    before.dev() == after.dev()
        && before.ino() == after.ino()
        && before.len() == after.len()
        && before.mode() & 0o777 == after.mode() & 0o777
        && before.mtime() == after.mtime()
        && before.mtime_nsec() == after.mtime_nsec()
}

/// Hash the kernel-selected running image, not a caller-selected pathname.
fn verify_executing_validator(
    expected: Digest256,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Digest256> {
    check(deadline, cancelled)?;
    let mut image = File::open("/proc/self/exe")
        .map_err(|error| io_error("cannot open executing capture importer", error))?;
    let before = image
        .metadata()
        .map_err(|error| io_error("cannot stat executing capture importer", error))?;
    if !before.is_file() || before.len() == 0 || before.len() > MAX_EXECUTABLE_BYTES {
        return Err(StoreError::new(
            Code::BudgetExceeded,
            "executing capture importer image is outside its finite bound",
        ));
    }
    let mut digest = Digest256Hasher::new();
    let mut total = 0u64;
    let mut first = true;
    let mut buffer = [0u8; 65536];
    loop {
        check(deadline, cancelled)?;
        let count = image
            .read(&mut buffer)
            .map_err(|error| io_error("cannot read executing capture importer", error))?;
        if count == 0 {
            break;
        }
        if first && !buffer[..count].starts_with(b"\x7fELF") {
            return Err(fail("executing capture importer is not an ELF image"));
        }
        first = false;
        total = total
            .checked_add(count as u64)
            .filter(|value| *value <= MAX_EXECUTABLE_BYTES)
            .ok_or_else(|| StoreError::new(Code::BudgetExceeded, "importer image read bound"))?;
        digest.update(&buffer[..count]);
    }
    let after = image
        .metadata()
        .map_err(|error| io_error("cannot restat executing capture importer", error))?;
    let current = File::open("/proc/self/exe")
        .map_err(|error| io_error("cannot re-open executing capture importer", error))?;
    let named = current
        .metadata()
        .map_err(|error| io_error("cannot restat executing capture importer path", error))?;
    if total != before.len()
        || image_identity(&before) != image_identity(&after)
        || image_identity(&before) != image_identity(&named)
    {
        return Err(StoreError::new(
            Code::DescriptorMismatch,
            "executing capture importer changed during identity check",
        ));
    }
    let actual = digest.finalize();
    if actual != expected {
        return Err(StoreError::new(
            Code::DescriptorMismatch,
            "pinned capture importer image differs from the executing image",
        ));
    }
    check(deadline, cancelled)?;
    Ok(actual)
}

struct PendingObject {
    metadata: MemberMetadata,
    file: Option<File>,
    temporary_name: Option<String>,
    digest: Digest256Hasher,
    bytes: u64,
}

struct ImportMemberSink<'a> {
    output: &'a Path,
    root: Option<File>,
    objects: Option<File>,
    revisions: Option<File>,
    pending: Option<PendingObject>,
    installed: BTreeMap<Digest256, u64>,
    next_temporary: u64,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    forbidden_scope: Option<tos_foundation::RelativePath>,
}

impl<'a> ImportMemberSink<'a> {
    fn new(output: &'a Path, deadline: Instant, cancelled: &'a AtomicBool) -> Self {
        Self {
            output,
            root: None,
            objects: None,
            revisions: None,
            pending: None,
            installed: BTreeMap::new(),
            next_temporary: 0,
            deadline,
            cancelled,
            forbidden_scope: None,
        }
    }

    fn ensure_layout(&mut self) -> Result<()> {
        if self.root.is_some() {
            return Ok(());
        }
        check(self.deadline, self.cancelled)?;
        let root = crate::archive::fresh_destination(self.output)?;
        let objects = create_directory(&root, "objects")?;
        let revisions = create_directory(&root, "revisions")?;
        self.root = Some(root);
        self.objects = Some(objects);
        self.revisions = Some(revisions);
        Ok(())
    }

    fn objects(&self) -> Result<&File> {
        self.objects
            .as_ref()
            .ok_or_else(|| fail("capture importer object directory is absent"))
    }

    fn temp_name(&mut self) -> Result<String> {
        let value = self.next_temporary;
        self.next_temporary = value.checked_add(1).ok_or_else(|| {
            StoreError::new(Code::BudgetExceeded, "object stage sequence overflow")
        })?;
        Ok(format!(
            ".capture-import-{}-{value:016x}",
            std::process::id()
        ))
    }

    fn verify_file_readback(
        file: &mut File,
        expected_size: u64,
        expected_sha256: Digest256,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<()> {
        file.seek(SeekFrom::Start(0))
            .map_err(|error| io_error("cannot rewind imported object", error))?;
        let mut digest = Digest256Hasher::new();
        let mut total = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            check(deadline, cancelled)?;
            let count = file
                .read(&mut buffer)
                .map_err(|error| io_error("cannot verify imported object", error))?;
            if count == 0 {
                break;
            }
            total = total
                .checked_add(count as u64)
                .filter(|value| *value <= expected_size)
                .ok_or_else(|| fail("imported object readback grew"))?;
            digest.update(&buffer[..count]);
        }
        if total != expected_size || digest.finalize() != expected_sha256 {
            return Err(fail("imported object readback differs"));
        }
        Ok(())
    }

    fn publish_snapshot(
        &self,
        revision: Digest256,
        bytes: &[u8],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<()> {
        check(deadline, cancelled)?;
        let revisions = self
            .revisions
            .as_ref()
            .ok_or_else(|| fail("capture importer revision directory is absent"))?;
        let stage_name = format!(".capture-import-snapshot-{}", std::process::id());
        let stage = create_directory(revisions, &stage_name)?;
        let mut published = false;
        let result = (|| {
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(0x20000)
                .open(fd_path(&stage, "snapshot.json"))
                .map_err(|error| io_error("cannot create staged corpus snapshot", error))?;
            file.write_all(bytes)
                .map_err(|error| io_error("cannot write staged corpus snapshot", error))?;
            file.sync_all()
                .map_err(|error| io_error("cannot persist staged corpus snapshot", error))?;
            file.set_permissions(fs::Permissions::from_mode(0o444))
                .map_err(|error| io_error("cannot seal staged corpus snapshot", error))?;
            file.sync_all()
                .map_err(|error| io_error("cannot persist sealed corpus snapshot", error))?;
            Self::verify_file_readback(
                &mut file,
                bytes.len() as u64,
                Digest256::of_bytes(bytes),
                deadline,
                cancelled,
            )?;
            drop(file);
            stage
                .set_permissions(fs::Permissions::from_mode(0o555))
                .map_err(|error| io_error("cannot seal staged revision directory", error))?;
            stage
                .sync_all()
                .map_err(|error| io_error("cannot persist staged revision directory", error))?;
            let name = revision.to_hex();
            let target = fd_path(revisions, &name);
            match fs::symlink_metadata(&target) {
                Ok(_) => return Err(fail("target revision already exists in the fresh store")),
                Err(error) if error.kind() == io::ErrorKind::NotFound => (),
                Err(error) => return Err(io_error("cannot inspect target revision", error)),
            }
            fs::rename(fd_path(revisions, &stage_name), target)
                .map_err(|error| io_error("cannot install exact corpus revision", error))?;
            published = true;
            revisions
                .sync_all()
                .map_err(|error| io_error("cannot persist exact corpus revision", error))
        })();
        if result.is_err() && !published {
            // Remove only this invocation's exact snapshot staging directory.
            let _ = stage.set_permissions(fs::Permissions::from_mode(0o700));
            let _ = fs::remove_file(fd_path(&stage, "snapshot.json"));
            let _ = fs::remove_dir(fd_path(revisions, &stage_name));
        }
        result
    }

    fn write_receipt(
        &self,
        name: &str,
        bytes: &[u8],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<()> {
        check(deadline, cancelled)?;
        let root = self
            .root
            .as_ref()
            .ok_or_else(|| fail("capture importer root is absent"))?;
        let temporary_name = format!(".capture-import-receipt-{}", std::process::id());
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(0x20000)
            .open(fd_path(root, &temporary_name))
            .map_err(|error| io_error("cannot create staged capture import receipt", error))?;
        let mut receipt_linked = false;
        let result = (|| {
            file.write_all(bytes)
                .map_err(|error| io_error("cannot write capture import receipt", error))?;
            file.sync_all()
                .map_err(|error| io_error("cannot persist capture import receipt", error))?;
            file.set_permissions(fs::Permissions::from_mode(0o444))
                .map_err(|error| io_error("cannot seal capture import receipt", error))?;
            file.sync_all()
                .map_err(|error| io_error("cannot persist sealed capture import receipt", error))?;
            Self::verify_file_readback(
                &mut file,
                bytes.len() as u64,
                Digest256::of_bytes(bytes),
                deadline,
                cancelled,
            )?;
            let before = file
                .metadata()
                .map_err(|error| io_error("cannot stat staged capture import receipt", error))?;
            drop(file);
            fs::hard_link(fd_path(root, &temporary_name), fd_path(root, name))
                .map_err(|error| io_error("cannot install capture import receipt", error))?;
            receipt_linked = true;
            let installed = open_regular(root, name)?;
            let after = installed
                .metadata()
                .map_err(|error| io_error("cannot stat installed capture import receipt", error))?;
            if !same_linked_file(&before, &after)
                || after.mode() & 0o777 != 0o444
                || after.len() != bytes.len() as u64
            {
                return Err(fail(
                    "installed capture import receipt differs from its stage",
                ));
            }
            root.sync_all()
                .map_err(|error| io_error("cannot persist capture import receipt name", error))?;
            fs::remove_file(fd_path(root, &temporary_name))
                .map_err(|error| io_error("cannot remove exact receipt stage", error))?;
            root.sync_all()
                .map_err(|error| io_error("cannot persist receipt stage removal", error))
        })();
        if result.is_err() {
            let _ = fs::remove_file(fd_path(root, &temporary_name));
            if receipt_linked {
                let _ = fs::remove_file(fd_path(root, name));
            }
        }
        result
    }

    fn finish_pending_object(
        &mut self,
        member: &MemberMetadata,
        mut pending: PendingObject,
    ) -> Result<()> {
        if pending.metadata != *member
            || pending.bytes != member.size_bytes
            || pending.digest.finalize() != member.sha256
        {
            return Err(fail("capture member sink digest or size differs"));
        }
        let Some(mut file) = pending.file.take() else {
            return Ok(());
        };
        file.sync_all()
            .map_err(|error| io_error("cannot persist private object stage", error))?;
        file.set_permissions(fs::Permissions::from_mode(0o444))
            .map_err(|error| io_error("cannot seal immutable source object", error))?;
        file.sync_all()
            .map_err(|error| io_error("cannot persist sealed source object", error))?;
        Self::verify_file_readback(
            &mut file,
            member.size_bytes,
            member.sha256,
            self.deadline,
            self.cancelled,
        )?;
        let before = file
            .metadata()
            .map_err(|error| io_error("cannot stat sealed source object", error))?;
        let objects = self
            .objects()?
            .try_clone()
            .map_err(|error| io_error("cannot clone private object directory", error))?;
        let temporary_name = pending
            .temporary_name
            .as_deref()
            .ok_or_else(|| fail("source object stage name is absent"))?;
        let digest_name = member.sha256.to_hex();
        fs::hard_link(
            fd_path(&objects, temporary_name),
            fd_path(&objects, &digest_name),
        )
        .map_err(|error| io_error("cannot install immutable source object", error))?;
        objects
            .sync_all()
            .map_err(|error| io_error("cannot persist immutable source object", error))?;
        let linked = open_regular(&objects, &digest_name)?;
        let after = linked
            .metadata()
            .map_err(|error| io_error("cannot stat installed source object", error))?;
        if !same_linked_file(&before, &after)
            || after.len() != member.size_bytes
            || after.mode() & 0o777 != 0o444
        {
            return Err(fail("installed object differs from its sealed stage"));
        }
        fs::remove_file(fd_path(&objects, temporary_name))
            .map_err(|error| io_error("cannot remove exact source-object stage", error))?;
        objects
            .sync_all()
            .map_err(|error| io_error("cannot persist source-object stage removal", error))?;
        self.installed.insert(member.sha256, member.size_bytes);
        Ok(())
    }
}

impl CaptureMemberSink for ImportMemberSink<'_> {
    fn begin_member(&mut self, member: &MemberMetadata) -> Result<()> {
        check(self.deadline, self.cancelled)?;
        if self.pending.is_some() || !is_authored_source_path_v1(member.path.as_str()) {
            return Err(StoreError::new(
                Code::InvalidMemberIndex,
                "capture member is outside the authored ToS carrier",
            ));
        }
        if self.forbidden_scope.as_ref().is_some_and(|scope| {
            member.path == *scope
                || member
                    .path
                    .as_str()
                    .strip_prefix(scope.as_str())
                    .is_some_and(|suffix| suffix.starts_with('/'))
        }) {
            return Err(fail("Git capture shadows the selected retained subtree"));
        }
        self.ensure_layout()?;
        let duplicate = self.installed.get(&member.sha256).copied();
        if duplicate.is_some_and(|size| size != member.size_bytes) {
            return Err(fail("identical object digest has a different source size"));
        }
        let (file, temporary_name) = if duplicate.is_some() {
            (None, None)
        } else {
            let name = self.temp_name()?;
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(0x20000)
                .open(fd_path(self.objects()?, &name))
                .map_err(|error| io_error("cannot create private object stage", error))?;
            (Some(file), Some(name))
        };
        self.pending = Some(PendingObject {
            metadata: member.clone(),
            file,
            temporary_name,
            digest: Digest256Hasher::new(),
            bytes: 0,
        });
        Ok(())
    }

    fn write_chunk(&mut self, bytes: &[u8]) -> Result<()> {
        check(self.deadline, self.cancelled)?;
        let pending = self
            .pending
            .as_mut()
            .ok_or_else(|| fail("capture member sink has no active member"))?;
        pending.bytes = pending
            .bytes
            .checked_add(bytes.len() as u64)
            .filter(|value| *value <= pending.metadata.size_bytes)
            .ok_or_else(|| fail("capture member sink exceeded declared size"))?;
        pending.digest.update(bytes);
        if let Some(file) = pending.file.as_mut() {
            file.write_all(bytes)
                .map_err(|error| io_error("cannot write private object stage", error))?;
        }
        Ok(())
    }

    fn finish_member(&mut self, member: &MemberMetadata) -> Result<()> {
        check(self.deadline, self.cancelled)?;
        let pending = self
            .pending
            .take()
            .ok_or_else(|| fail("capture member sink has no active member"))?;
        let temporary_name = pending.temporary_name.clone();
        let result = self.finish_pending_object(member, pending);
        if result.is_err()
            && let (Some(objects), Some(name)) = (self.objects.as_ref(), temporary_name)
        {
            let _ = fs::remove_file(fd_path(objects, &name));
        }
        result
    }
}

impl Drop for ImportMemberSink<'_> {
    fn drop(&mut self) {
        if let (Some(objects), Some(pending)) = (self.objects.as_ref(), self.pending.as_ref()) {
            if let Some(name) = pending.temporary_name.as_deref() {
                // Remove only this invocation's exact private stage name; the
                // store root and already installed content-addressed objects
                // remain as explicitly unselected partial custody on failure.
                let _ = fs::remove_file(fd_path(objects, name));
            }
        }
    }
}

fn string(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}

fn number(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}

fn array(values: &[String]) -> JsonValue {
    JsonValue::Array(values.iter().map(|value| string(value)).collect())
}

fn object(values: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        values
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(key), value))
            .collect(),
    )
}

fn canonical(value: &JsonValue, limits: JsonLimits) -> Result<Vec<u8>> {
    canonical_bytes_v1(value, CanonicalProfile::CorpusSnapshotV1, limits)
        .map_err(|_| StoreError::new(Code::BudgetExceeded, "capture import JSON budget"))
}

fn snapshot_bytes(
    members: &BTreeMap<tos_foundation::RelativePath, MemberMetadata>,
    validator_sha256: Digest256,
    limits: JsonLimits,
) -> Result<(Digest256, Vec<u8>)> {
    let files = JsonValue::Array(
        members
            .values()
            .map(|member| {
                object(vec![
                    ("path", string(member.path.as_str())),
                    ("sha256", string(&member.sha256.to_hex())),
                    ("size_bytes", number(member.size_bytes)),
                    ("mode", number(member.mode as u64)),
                ])
            })
            .collect(),
    );
    let body = object(vec![
        ("schema_version", string("tos_corpus_snapshot_v1")),
        ("base_revision", JsonValue::Null),
        ("validator_sha256", string(&validator_sha256.to_hex())),
        ("files", files),
        ("identities", JsonValue::Object(Vec::new())),
        ("dependencies", JsonValue::Object(Vec::new())),
        ("retirements", JsonValue::Array(Vec::new())),
    ]);
    let body_bytes = canonical(&body, limits)?;
    let revision = Digest256::of_bytes(&body_bytes);
    let mut full = body;
    let JsonValue::Object(fields) = &mut full else {
        return Err(fail("snapshot body is not an object"));
    };
    fields.push((
        JsonString::from_utf8("revision"),
        string(&revision.to_hex()),
    ));
    Ok((revision, canonical(&full, limits)?))
}

fn source_membership(members: impl Iterator<Item = MemberMetadata>) -> Result<(u64, Digest256)> {
    let mut digest = Digest256Hasher::new();
    digest.update(b"tos-val-full-membership-v1\0");
    let mut count = 0u64;
    for member in members {
        count = count
            .checked_add(1)
            .ok_or_else(|| StoreError::new(Code::BudgetExceeded, "membership count overflow"))?;
        feed_source_membership_v1(
            &mut digest,
            member.path.as_str(),
            member.size_bytes,
            member.sha256,
        );
    }
    Ok((count, digest.finalize()))
}

struct SelectedRetainedInput<'a> {
    selection: RetainedCaptureSelectionV1<'a>,
    root: File,
    cut: crate::CorpusCutReader,
    members: BTreeMap<tos_foundation::RelativePath, MemberMetadata>,
    snapshot_raw: Vec<u8>,
    receipt_raw: Vec<u8>,
    observation: RetainedCaptureReceiptV1,
}

fn bounded_metadata_file(
    parent: &File,
    name: &str,
    cap: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    check(deadline, cancelled)?;
    let mut file = open_regular(parent, name)?;
    let before = file
        .metadata()
        .map_err(|e| io_error("retained metadata stat", e))?;
    if before.mode() & 0o777 != 0o444 || before.len() > cap as u64 {
        return Err(fail("retained metadata is not a bounded immutable file"));
    }
    let mut bytes = Vec::new();
    let mut buffer = [0; 65536];
    loop {
        check(deadline, cancelled)?;
        let count = file
            .read(&mut buffer)
            .map_err(|e| io_error("retained metadata read", e))?;
        if count == 0 {
            break;
        }
        if bytes.len().checked_add(count).is_none_or(|size| size > cap) {
            return Err(fail("retained metadata grew beyond its bound"));
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    let after = file
        .metadata()
        .map_err(|e| io_error("retained metadata restat", e))?;
    let named = open_regular(parent, name)?
        .metadata()
        .map_err(|e| io_error("retained metadata name stat", e))?;
    if bytes.len() as u64 != before.len()
        || image_identity(&before) != image_identity(&after)
        || image_identity(&before) != image_identity(&named)
    {
        return Err(fail("retained metadata changed during its bounded read"));
    }
    Ok(bytes)
}

fn select_retained_input<'a>(
    selection: RetainedCaptureSelectionV1<'a>,
    limits: CaptureRestoreLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<SelectedRetainedInput<'a>> {
    check(deadline, cancelled)?;
    let receipt_ref = format!("capture-import.{}.json", selection.revision.0.to_hex());
    if !is_authored_source_path_v1(selection.scope.as_str())
        || selection.member_count == 0
        || selection.member_count > limits.metadata.max_manifest_entries as u64
        || selection.source_bytes == 0
        || selection.source_bytes > limits.max_source_bytes
        || !matches!(selection.member_mode, 0o644 | 0o755)
        || (selection.receipt_file != "capture-import.json"
            && selection.receipt_file != receipt_ref)
    {
        return Err(fail("retained source binding or receipt selector differs"));
    }
    let root = tos_fd_open::open_absolute_directory(selection.root)
        .map_err(|_| fail("unsafe retained corpus root"))?;
    let reader = CorpusReader::open_existing_at(&root, limits.metadata)?;
    let cut = reader.open_source_cut(
        selection.revision,
        CutReadLimits {
            max_revisions: 1,
            max_members: selection.member_count,
            max_total_bytes: selection.source_bytes,
            max_member_bytes: limits.metadata.max_selected_object_bytes,
        },
        deadline,
        cancelled,
    )?;
    let snapshot = cut.current();
    let members = snapshot
        .members()
        .cloned()
        .map(|member| (member.path.clone(), member))
        .collect::<BTreeMap<_, _>>();
    let mut source_bytes = 0u64;
    for member in members.values() {
        check(deadline, cancelled)?;
        if member.mode != selection.member_mode
            || !member
                .path
                .as_str()
                .strip_prefix(selection.scope.as_str())
                .is_some_and(|suffix| suffix.starts_with('/'))
        {
            return Err(fail("retained member mode or subtree differs"));
        }
        source_bytes = source_bytes
            .checked_add(member.size_bytes)
            .ok_or_else(|| fail("retained source byte count overflow"))?;
    }
    let (member_count, membership_sha256) = source_membership(members.values().cloned())?;
    if member_count != selection.member_count
        || source_bytes != selection.source_bytes
        || membership_sha256 != selection.membership_sha256
    {
        return Err(fail("retained source membership differs from its binding"));
    }
    // This input is a mechanical capture snapshot. Rebuilding its exact V1
    // body also proves that no base, identity, dependency or retirement layer
    // would be discarded by the composition.
    let (revision, expected_raw) =
        snapshot_bytes(&members, snapshot.validator_sha256(), limits.metadata.json)?;
    if revision != selection.revision.0 {
        return Err(fail("retained snapshot contains stronger indexed state"));
    }
    let revisions = tos_fd_open::open_directory_at(&root, Path::new("revisions"))
        .map_err(|_| fail("retained revisions directory is unsafe"))?;
    let revision_dir =
        tos_fd_open::open_directory_at(&revisions, Path::new(&selection.revision.0.to_hex()))
            .map_err(|_| fail("retained exact revision is unsafe"))?;
    let snapshot_raw = bounded_metadata_file(
        &revision_dir,
        "snapshot.json",
        limits.metadata.max_manifest_bytes,
        deadline,
        cancelled,
    )?;
    if snapshot_raw != expected_raw {
        return Err(fail(
            "retained snapshot bytes are not its exact capture form",
        ));
    }
    let receipt_raw = bounded_metadata_file(
        &root,
        selection.receipt_file,
        limits.metadata.max_manifest_bytes,
        deadline,
        cancelled,
    )?;
    let receipt = tos_foundation::parse_json(
        &receipt_raw,
        tos_foundation::JsonMode::PublishedStrict,
        limits.metadata.json,
    )
    .map_err(|_| fail("retained capture receipt is invalid"))?
    .into_root();
    let field = |name| receipt.object_get(name);
    if field("schema_version").and_then(JsonValue::as_str) != Some(IMPORT_RECEIPT_SCHEMA)
        || field("validator_role").and_then(JsonValue::as_str) != Some(MECHANICAL_VALIDATOR_ROLE)
        || field("revision").and_then(JsonValue::as_str)
            != Some(selection.revision.0.to_hex().as_str())
        || field("validator_sha256").and_then(JsonValue::as_str)
            != Some(snapshot.validator_sha256().to_hex().as_str())
        || field("membership_sha256").and_then(JsonValue::as_str)
            != Some(membership_sha256.to_hex().as_str())
        || field("member_count").and_then(JsonValue::as_u64) != Some(member_count)
        || field("source_bytes").and_then(JsonValue::as_u64) != Some(source_bytes)
        || field("custody_only").and_then(JsonValue::as_bool) != Some(true)
        || [
            "semantic_admission",
            "rights_change",
            "source_admitted",
            "source_currentness_selected",
            "publication_authorized",
            "whole_authored_inventory_established",
            "identity_index_derived",
            "dependency_index_derived",
            "current_pointer_written",
        ]
        .into_iter()
        .any(|name| field(name).and_then(JsonValue::as_bool) != Some(false))
        || canonical(&receipt, limits.metadata.json)? != receipt_raw
    {
        return Err(fail(
            "retained capture receipt does not bind the selected byte-only cut",
        ));
    }
    let observation = RetainedCaptureReceiptV1 {
        revision: selection.revision,
        validator_sha256: snapshot.validator_sha256(),
        scope: selection.scope.clone(),
        membership_sha256,
        member_count,
        source_bytes,
        member_mode: selection.member_mode,
        binding_sha256: selection.binding_sha256,
        original_receipt_sha256: Digest256::of_bytes(&receipt_raw),
        receipt_ref,
        member_bytes_read: 0,
    };
    Ok(SelectedRetainedInput {
        selection,
        root,
        cut,
        members,
        snapshot_raw,
        receipt_raw,
        observation,
    })
}

fn retained_receipt_value(retained: &RetainedCaptureReceiptV1) -> JsonValue {
    object(vec![
        ("revision", string(&retained.revision.0.to_hex())),
        (
            "validator_sha256",
            string(&retained.validator_sha256.to_hex()),
        ),
        ("scope", string(retained.scope.as_str())),
        (
            "membership_sha256",
            string(&retained.membership_sha256.to_hex()),
        ),
        ("member_count", number(retained.member_count)),
        ("source_bytes", number(retained.source_bytes)),
        ("member_mode", number(retained.member_mode as u64)),
        ("binding_sha256", string(&retained.binding_sha256.to_hex())),
        (
            "original_receipt_sha256",
            string(&retained.original_receipt_sha256.to_hex()),
        ),
        ("receipt_ref", string(&retained.receipt_ref)),
        ("member_bytes_read", number(retained.member_bytes_read)),
        ("complete_physical_io", JsonValue::Bool(false)),
    ])
}

fn import_receipt_bytes(
    selection: &SoftwareCaptureSelectionV1,
    index: &crate::software::CaptureIndex,
    validator_sha256: Digest256,
    revision: Digest256,
    membership_sha256: Digest256,
    usage: CaptureReadUsage,
    member_count: u64,
    source_bytes: u64,
    retained: Option<&RetainedCaptureReceiptV1>,
    limits: JsonLimits,
) -> Result<Vec<u8>> {
    let capture_total_read_bytes = usage.total_read_bytes()?;
    let mut fields = vec![
        (
            "schema_version",
            string(if retained.is_some() {
                RETAINED_IMPORT_RECEIPT_SCHEMA
            } else {
                IMPORT_RECEIPT_SCHEMA
            }),
        ),
        (
            "capture_manifest_sha256",
            string(&selection.capture_manifest_sha256.to_hex()),
        ),
        ("source_git_commit", string(&selection.source_git_commit)),
        ("source_git_tree", string(&selection.source_git_tree)),
        ("include_prefixes", array(&index.includes)),
        ("exclude_prefixes", array(&index.excludes)),
        ("exclude_path_parts", array(&index.excluded_parts)),
        ("revision", string(&revision.to_hex())),
        ("membership_sha256", string(&membership_sha256.to_hex())),
        ("member_count", number(member_count)),
        ("source_bytes", number(source_bytes)),
        ("validator_sha256", string(&validator_sha256.to_hex())),
        ("validator_role", string(MECHANICAL_VALIDATOR_ROLE)),
        ("custody_only", JsonValue::Bool(true)),
        ("semantic_admission", JsonValue::Bool(false)),
        ("rights_change", JsonValue::Bool(false)),
        ("source_admitted", JsonValue::Bool(false)),
        ("source_currentness_selected", JsonValue::Bool(false)),
        ("publication_authorized", JsonValue::Bool(false)),
        (
            "whole_authored_inventory_established",
            JsonValue::Bool(false),
        ),
        ("identity_index_derived", JsonValue::Bool(false)),
        ("dependency_index_derived", JsonValue::Bool(false)),
        ("current_pointer_written", JsonValue::Bool(false)),
        (
            "capture_read_usage",
            object(vec![
                (
                    "measurement",
                    string("capture-parser-logical-read-bytes-v1"),
                ),
                ("metadata_bytes", number(usage.metadata_bytes)),
                ("archive_read_bytes", number(usage.archive_read_bytes)),
                ("decoded_bytes", number(usage.decoded_bytes)),
                ("capture_total_read_bytes", number(capture_total_read_bytes)),
                ("complete_physical_io", JsonValue::Bool(false)),
            ]),
        ),
    ];
    if let Some(retained) = retained {
        fields.push(("retained_source", retained_receipt_value(retained)));
    }
    canonical(&object(fields), limits)
}

/// Stream one exact verified capture, optionally composed with an explicitly
/// selected byte-only retained cut, into a fresh, unselected immutable V1 store.
/// No current pointer is written; on error, exact private stages are cleaned
/// where possible and installed bytes remain unselected.
pub fn import_git_capture_corpus_v1(
    request: CaptureImportRequestV1<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<CaptureImportReceipt> {
    check(deadline, cancelled)?;
    let metadata = request.limits.metadata.validate()?;
    if request.limits.max_archive_bytes == 0
        || request.limits.max_archive_bytes == u64::MAX
        || request.limits.max_decoded_bytes == 0
        || request.limits.max_decoded_bytes == u64::MAX
        || request.limits.max_source_bytes == 0
        || request.limits.max_source_bytes == u64::MAX
    {
        return Err(StoreError::new(
            Code::BudgetExceeded,
            "invalid capture import limits",
        ));
    }
    if !request.output.is_absolute() {
        return Err(StoreError::new(
            Code::InvalidRoot,
            "capture import output must be absolute",
        ));
    }
    let validator_sha256 =
        verify_executing_validator(request.expected_validator_sha256, deadline, cancelled)?;
    if request
        .retained
        .as_ref()
        .is_some_and(|retained| request.output.starts_with(retained.root))
    {
        return Err(fail(
            "fresh import output must be disjoint from retained input",
        ));
    }
    let mut retained = request
        .retained
        .map(|selection| select_retained_input(selection, request.limits, deadline, cancelled))
        .transpose()?;
    let mut capture_limits = request.limits;
    if let Some(input) = retained.as_ref() {
        // The retained subtree consumes the same original member/source and
        // decoded-input ceilings, rather than receiving a second allowance.
        capture_limits.metadata.max_manifest_entries = capture_limits
            .metadata
            .max_manifest_entries
            .checked_sub(input.members.len())
            .filter(|value| *value > 0)
            .ok_or_else(|| fail("retained source exhausts original member allowance"))?;
        capture_limits.max_source_bytes = capture_limits
            .max_source_bytes
            .checked_sub(input.observation.source_bytes)
            .filter(|value| *value > 0)
            .ok_or_else(|| fail("retained source exhausts original source allowance"))?;
        capture_limits.max_decoded_bytes = capture_limits
            .max_decoded_bytes
            .checked_sub(input.observation.source_bytes)
            .filter(|value| *value > 0)
            .ok_or_else(|| fail("retained source exhausts original decoded allowance"))?;
    }
    let mut sink = ImportMemberSink::new(request.output, deadline, cancelled);
    if let Some(input) = retained.as_mut() {
        for member in input.members.values() {
            check(deadline, cancelled)?;
            let selected = input.cut.read_member(
                input.selection.revision,
                &member.path,
                request.limits.metadata.max_selected_object_bytes,
                deadline,
                cancelled,
            )?;
            sink.begin_member(member)?;
            sink.write_chunk(&selected.raw)?;
            sink.finish_member(member)?;
            input.observation.member_bytes_read = input
                .observation
                .member_bytes_read
                .checked_add(selected.raw.len() as u64)
                .ok_or_else(|| fail("retained member read count overflow"))?;
        }
        if input.observation.member_bytes_read != input.observation.source_bytes {
            return Err(fail("retained member read coverage differs"));
        }
        sink.publish_snapshot(
            input.selection.revision.0,
            &input.snapshot_raw,
            deadline,
            cancelled,
        )?;
        sink.write_receipt(
            &input.observation.receipt_ref,
            &input.receipt_raw,
            deadline,
            cancelled,
        )?;
        sink.forbidden_scope = Some(input.selection.scope.clone());
    }
    let walked = walk_capture_for_import(
        request.capture_root,
        request.selection,
        capture_limits,
        &mut sink,
        deadline,
        cancelled,
    )?;
    if walked.index.member_count == 0 || sink.pending.is_some() || sink.root.is_none() {
        return Err(StoreError::new(
            Code::InvalidMemberIndex,
            "empty or incomplete capture import",
        ));
    }
    let mut members = walked.index.members.clone();
    let mut source_bytes = walked.index.source_bytes;
    if let Some(input) = retained.as_ref() {
        for (path, member) in &input.members {
            if members.insert(path.clone(), member.clone()).is_some() {
                return Err(fail("retained and Git capture member sets overlap"));
            }
        }
        source_bytes = source_bytes
            .checked_add(input.observation.source_bytes)
            .filter(|value| *value <= request.limits.max_source_bytes)
            .ok_or_else(|| fail("composed source exceeds original byte allowance"))?;
    }
    if members.len() > metadata.max_manifest_entries {
        return Err(fail("composed source exceeds original member allowance"));
    }
    let names = members
        .keys()
        .map(|path| path.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for name in &names {
        check(deadline, cancelled)?;
        for (position, _) in name.match_indices('/') {
            if names.contains(&name[..position]) {
                return Err(fail("composed source contains a file/directory collision"));
            }
        }
    }
    if let Some(input) = retained.as_ref() {
        let named = tos_fd_open::open_absolute_directory(input.selection.root)
            .map_err(|_| fail("retained corpus name changed"))?;
        let before = input
            .root
            .metadata()
            .map_err(|e| io_error("retained root stat", e))?;
        let after = named
            .metadata()
            .map_err(|e| io_error("retained root name stat", e))?;
        if before.dev() != after.dev() || before.ino() != after.ino() {
            return Err(fail("retained corpus name changed"));
        }
    }
    let (revision, snapshot) = snapshot_bytes(&members, validator_sha256, metadata.json)?;
    if snapshot.len() > metadata.max_manifest_bytes {
        return Err(StoreError::new(
            Code::BudgetExceeded,
            "corpus snapshot exceeds metadata limit",
        ));
    }
    sink.publish_snapshot(revision, &snapshot, deadline, cancelled)?;

    // Re-open the exact unselected revision through the public read path. This
    // confirms canonical revision identity, source-path eligibility and the
    // membership transcript without materializing any member body.
    let root = sink
        .root
        .as_ref()
        .ok_or_else(|| fail("capture importer root is absent"))?;
    let reader = CorpusReader::open_existing_at(root, metadata)?;
    let cut = reader.open_source_cut(
        SourceRevision(revision),
        CutReadLimits {
            max_revisions: 1,
            max_members: members.len() as u64,
            max_total_bytes: source_bytes.max(1),
            max_member_bytes: metadata.max_selected_object_bytes,
        },
        deadline,
        cancelled,
    )?;
    let snapshot = cut.current();
    let (member_count, membership_sha256) = source_membership(snapshot.members().cloned())?;
    if member_count != members.len() as u64
        || member_count != snapshot.member_count() as u64
        || snapshot.base_revision().is_some()
        || snapshot.validator_sha256() != validator_sha256
    {
        return Err(StoreError::new(
            Code::DescriptorMismatch,
            "reopened exact corpus revision differs from capture import",
        ));
    }
    let receipt = import_receipt_bytes(
        request.selection,
        &walked.index,
        validator_sha256,
        revision,
        membership_sha256,
        walked.usage,
        member_count,
        source_bytes,
        retained.as_ref().map(|input| &input.observation),
        metadata.json,
    )?;
    if receipt.len() > metadata.max_manifest_bytes {
        return Err(StoreError::new(
            Code::BudgetExceeded,
            "capture import receipt exceeds metadata limit",
        ));
    }
    sink.write_receipt("capture-import.json", &receipt, deadline, cancelled)?;
    let import_receipt_sha256 = Digest256::of_bytes(&receipt);
    check(deadline, cancelled)?;
    Ok(CaptureImportReceipt {
        schema_version: if retained.is_some() {
            RETAINED_IMPORT_RECEIPT_SCHEMA
        } else {
            IMPORT_RECEIPT_SCHEMA
        },
        capture_manifest_sha256: request.selection.capture_manifest_sha256,
        source_git_commit: request.selection.source_git_commit.clone(),
        source_git_tree: request.selection.source_git_tree.clone(),
        include_prefixes: walked.index.includes,
        exclude_prefixes: walked.index.excludes,
        exclude_path_parts: walked.index.excluded_parts,
        validator_sha256,
        revision: SourceRevision(revision),
        membership_sha256,
        member_count,
        source_bytes,
        import_receipt_sha256,
        usage: walked.usage,
        retained: retained.map(|input| input.observation),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ReadLimits;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::process::Command;
    use tos_foundation::{JsonMode, RelativePath, parse_json};

    struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            fn make_directories_writable(path: &Path) {
                let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
                if let Ok(entries) = fs::read_dir(path) {
                    for entry in entries.flatten() {
                        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                            make_directories_writable(&entry.path());
                        }
                    }
                }
            }
            make_directories_writable(&self.0);
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn scratch() -> Scratch {
        let path = std::env::temp_dir().join(format!(
            "tos-capture-import-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Scratch(path)
    }

    fn git(repository: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(args)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_COMMON_DIR")
            .env_remove("GIT_INDEX_FILE")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "ToS capture fixture")
            .env("GIT_AUTHOR_EMAIL", "tos-capture-fixture@example.invalid")
            .env("GIT_COMMITTER_NAME", "ToS capture fixture")
            .env("GIT_COMMITTER_EMAIL", "tos-capture-fixture@example.invalid")
            .output()
            .expect("Git is required for the native capture fixture");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("Git fixture output must be UTF-8")
            .trim()
            .to_owned()
    }

    fn running_image_digest() -> Digest256 {
        let mut file = File::open("/proc/self/exe").unwrap();
        let mut digest = Digest256Hasher::new();
        let mut total = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            let count = file.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            total = total.checked_add(count as u64).unwrap();
            assert!(total <= MAX_EXECUTABLE_BYTES);
            digest.update(&buffer[..count]);
        }
        digest.finalize()
    }

    #[test]
    fn native_git_capture_import_reopens_exact_unselected_custody() {
        let scratch = scratch();
        let repository = scratch.0.join("repo");
        fs::create_dir(&repository).unwrap();
        git(&repository, &["init", "--quiet"]);
        let source_path = "ToS/capture-fixture.md";
        let source_bytes = b"authored fixture bytes\n";
        let source = repository.join(source_path);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, source_bytes).unwrap();
        git(&repository, &["add", "--", source_path]);
        git(&repository, &["commit", "--quiet", "-m", "capture fixture"]);
        let commit = git(&repository, &["rev-parse", "--verify", "HEAD"]);

        let capture = scratch.0.join("capture");
        let include_prefixes = vec!["ToS".to_owned()];
        let no_prefixes = Vec::new();
        let git_limits = crate::GitCaptureLimits {
            max_members: 8,
            max_member_bytes: 4096,
            max_source_bytes: 8192,
            max_metadata_bytes: 65536,
            max_tree_bytes: 65536,
            max_archive_bytes: 65536,
        };
        let deadline = Instant::now() + std::time::Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let capture_result = crate::capture_git(
            crate::CaptureGitRequest {
                repository: &repository,
                commit: &commit,
                include_prefixes: &include_prefixes,
                exclude_prefixes: &no_prefixes,
                exclude_path_parts: &no_prefixes,
                output: &capture,
            },
            git_limits,
            deadline,
            &cancelled,
        )
        .unwrap();
        let tree = capture_result
            .manifest
            .object_get("source_git_tree")
            .and_then(JsonValue::as_str)
            .unwrap()
            .to_owned();
        let selection = SoftwareCaptureSelectionV1 {
            source_git_commit: commit,
            source_git_tree: tree,
            capture_manifest_sha256: capture_result.manifest_sha256,
        };
        let limits = CaptureRestoreLimits {
            metadata: ReadLimits {
                max_manifest_bytes: 65536,
                max_manifest_entries: 16,
                max_selected_object_bytes: 4096,
                json: JsonLimits {
                    max_bytes: 65536,
                    ..JsonLimits::default()
                },
            },
            max_archive_bytes: 65536,
            max_decoded_bytes: 65536,
            max_source_bytes: 8192,
        };
        let validator_sha256 = running_image_digest();
        let store = scratch.0.join("corpus");
        let receipt = import_git_capture_corpus_v1(
            CaptureImportRequestV1 {
                capture_root: &capture,
                output: &store,
                selection: &selection,
                limits,
                expected_validator_sha256: validator_sha256,
                retained: None,
            },
            deadline,
            &cancelled,
        )
        .unwrap();

        assert_eq!(receipt.member_count, 1);
        assert_eq!(receipt.source_bytes, source_bytes.len() as u64);
        assert_eq!(receipt.validator_sha256, validator_sha256);
        assert!(!store.join("current.json").exists());
        let reader = CorpusReader::open_existing(&store, limits.metadata).unwrap();
        assert!(reader.select_current().unwrap().is_none());
        let snapshot = reader.load_exact(receipt.revision).unwrap();
        assert_eq!(snapshot.revision(), receipt.revision);
        assert_eq!(snapshot.base_revision(), None);
        assert_eq!(snapshot.validator_sha256(), validator_sha256);
        let path = RelativePath::parse(source_path).unwrap();
        let descriptor = reader
            .resolve(&snapshot, crate::Selector::Path(&path))
            .unwrap();
        let mut reopened = Vec::new();
        reader
            .read_selected(&snapshot, &descriptor, 4096, &mut reopened)
            .unwrap();
        assert_eq!(reopened.as_slice(), source_bytes);

        let sidecar = fs::read(store.join("capture-import.json")).unwrap();
        assert_eq!(Digest256::of_bytes(&sidecar), receipt.import_receipt_sha256);
        let sidecar = parse_json(&sidecar, JsonMode::PublishedStrict, limits.metadata.json)
            .unwrap()
            .into_root();
        assert_eq!(
            sidecar
                .object_get("custody_only")
                .and_then(JsonValue::as_bool),
            Some(true)
        );
        assert_eq!(
            sidecar
                .object_get("semantic_admission")
                .and_then(JsonValue::as_bool),
            Some(false)
        );
        assert_eq!(
            sidecar
                .object_get("source_currentness_selected")
                .and_then(JsonValue::as_bool),
            Some(false)
        );
        assert_eq!(
            sidecar
                .object_get("current_pointer_written")
                .and_then(JsonValue::as_bool),
            Some(false)
        );
        let usage = sidecar.object_get("capture_read_usage").unwrap();
        assert_eq!(
            usage.object_get("measurement").and_then(JsonValue::as_str),
            Some("capture-parser-logical-read-bytes-v1")
        );
        assert_eq!(
            usage
                .object_get("complete_physical_io")
                .and_then(JsonValue::as_bool),
            Some(false)
        );
        assert!(
            usage
                .object_get("capture_total_read_bytes")
                .and_then(JsonValue::as_u64)
                .is_some_and(|bytes| bytes > 0)
        );
    }

    #[test]
    fn retained_capture_keeps_old_cut_modes_receipt_and_one_object_pool_under_original_limits() {
        let scratch = scratch();
        let repository = scratch.0.join("repo");
        fs::create_dir(&repository).unwrap();
        git(&repository, &["init", "--quiet"]);
        let raw = b"same exact authored bytes\n";
        let old_paths = ["ToS/history/a.md", "ToS/history/b.md"];
        let current_path = "ToS/current/subject.md";
        for path in old_paths.into_iter().chain([current_path]) {
            let path = repository.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, raw).unwrap();
        }
        git(&repository, &["add", "--", "ToS"]);
        git(
            &repository,
            &["commit", "--quiet", "-m", "retained input fixture"],
        );
        let commit = git(&repository, &["rev-parse", "HEAD"]);
        let deadline = Instant::now() + std::time::Duration::from_secs(60);
        let cancelled = AtomicBool::new(false);
        let limits = CaptureRestoreLimits {
            metadata: ReadLimits {
                max_manifest_bytes: 65536,
                max_manifest_entries: 16,
                max_selected_object_bytes: 4096,
                json: JsonLimits {
                    max_bytes: 65536,
                    ..JsonLimits::default()
                },
            },
            max_archive_bytes: 65536,
            max_decoded_bytes: 65536,
            max_source_bytes: 8192,
        };
        let capture = |leaf: &str, include: &[String]| {
            let output = scratch.0.join(leaf);
            let result = crate::capture_git(
                crate::CaptureGitRequest {
                    repository: &repository,
                    commit: &commit,
                    include_prefixes: include,
                    exclude_prefixes: &[],
                    exclude_path_parts: &[],
                    output: &output,
                },
                crate::GitCaptureLimits {
                    max_members: 16,
                    max_member_bytes: 4096,
                    max_source_bytes: 8192,
                    max_metadata_bytes: 65536,
                    max_tree_bytes: 65536,
                    max_archive_bytes: 65536,
                },
                deadline,
                &cancelled,
            )
            .unwrap();
            let selection = SoftwareCaptureSelectionV1 {
                source_git_commit: commit.clone(),
                source_git_tree: result
                    .manifest
                    .object_get("source_git_tree")
                    .and_then(JsonValue::as_str)
                    .unwrap()
                    .to_owned(),
                capture_manifest_sha256: result.manifest_sha256,
            };
            (output, selection)
        };
        let (history_capture, history_selection) =
            capture("history-capture", &["ToS/history".into()]);
        let (current_capture, current_selection) =
            capture("current-capture", &["ToS/current".into()]);
        let (collision_capture, collision_selection) =
            capture("collision-capture", &["ToS".into()]);
        let validator = running_image_digest();
        let history = scratch.0.join("history-store");
        let history_receipt = import_git_capture_corpus_v1(
            CaptureImportRequestV1 {
                capture_root: &history_capture,
                output: &history,
                selection: &history_selection,
                limits,
                expected_validator_sha256: validator,
                retained: None,
            },
            deadline,
            &cancelled,
        )
        .unwrap();
        let old_snapshot = fs::read(
            history
                .join("revisions")
                .join(history_receipt.revision.0.to_hex())
                .join("snapshot.json"),
        )
        .unwrap();
        let old_receipt = fs::read(history.join("capture-import.json")).unwrap();
        let retained = |root, receipt_file| RetainedCaptureSelectionV1 {
            root,
            revision: history_receipt.revision,
            receipt_file,
            scope: RelativePath::parse("ToS/history").unwrap(),
            membership_sha256: history_receipt.membership_sha256,
            member_count: 2,
            source_bytes: 2 * raw.len() as u64,
            member_mode: 0o644,
            binding_sha256: Digest256::of_bytes(b"exact owned binding fixture"),
        };
        let mut receipts = Vec::new();
        for leaf in ["joined-first", "joined-repeat"] {
            let target = scratch.0.join(leaf);
            let receipt = import_git_capture_corpus_v1(
                CaptureImportRequestV1 {
                    capture_root: &current_capture,
                    output: &target,
                    selection: &current_selection,
                    limits,
                    expected_validator_sha256: validator,
                    retained: Some(retained(&history, "capture-import.json")),
                },
                deadline,
                &cancelled,
            )
            .unwrap();
            assert_eq!(receipt.schema_version, RETAINED_IMPORT_RECEIPT_SCHEMA);
            assert_eq!(receipt.member_count, 3);
            assert_eq!(receipt.source_bytes, 3 * raw.len() as u64);
            let preserved = receipt.retained.as_ref().unwrap();
            assert_eq!(preserved.member_bytes_read, 2 * raw.len() as u64);
            assert_eq!(
                fs::read(target.join(&preserved.receipt_ref)).unwrap(),
                old_receipt
            );
            assert_eq!(
                fs::read(
                    target
                        .join("revisions")
                        .join(history_receipt.revision.0.to_hex())
                        .join("snapshot.json")
                )
                .unwrap(),
                old_snapshot
            );
            assert_eq!(fs::read_dir(target.join("objects")).unwrap().count(), 1);
            assert!(!target.join("current.json").exists());
            let reader = CorpusReader::open_existing(&target, limits.metadata).unwrap();
            for (revision, paths) in [
                (history_receipt.revision, old_paths.as_slice()),
                (
                    receipt.revision,
                    [old_paths[0], old_paths[1], current_path].as_slice(),
                ),
            ] {
                let cut = reader
                    .open_source_cut(
                        revision,
                        CutReadLimits {
                            max_revisions: 1,
                            max_members: 3,
                            max_total_bytes: 8192,
                            max_member_bytes: 4096,
                        },
                        deadline,
                        &cancelled,
                    )
                    .unwrap();
                for path in paths {
                    let path = RelativePath::parse(path).unwrap();
                    assert_eq!(cut.current().member(&path).unwrap().mode, 0o644);
                    assert_eq!(
                        cut.read_member(revision, &path, 4096, deadline, &cancelled)
                            .unwrap()
                            .raw,
                        raw
                    );
                }
            }
            receipts.push(receipt);
        }
        assert_eq!(receipts[0].revision, receipts[1].revision);
        assert_eq!(
            receipts[0].import_receipt_sha256,
            receipts[1].import_receipt_sha256
        );
        let first = scratch.0.join("joined-first");
        let carried_receipt = &receipts[0].retained.as_ref().unwrap().receipt_ref;
        let target = scratch.0.join("joined-from-carried-history");
        let carried = import_git_capture_corpus_v1(
            CaptureImportRequestV1 {
                capture_root: &current_capture,
                output: &target,
                selection: &current_selection,
                limits,
                expected_validator_sha256: validator,
                retained: Some(retained(&first, carried_receipt)),
            },
            deadline,
            &cancelled,
        )
        .unwrap();
        assert_eq!(carried.revision, receipts[0].revision);
        assert_eq!(
            carried.import_receipt_sha256,
            receipts[0].import_receipt_sha256
        );

        for (name, supplied, changed_limits, source, selection) in [
            (
                "bad-membership",
                RetainedCaptureSelectionV1 {
                    membership_sha256: Digest256::of_bytes(b"wrong"),
                    ..retained(&history, "capture-import.json")
                },
                limits,
                &current_capture,
                &current_selection,
            ),
            (
                "bad-mode",
                RetainedCaptureSelectionV1 {
                    member_mode: 0o755,
                    ..retained(&history, "capture-import.json")
                },
                limits,
                &current_capture,
                &current_selection,
            ),
            (
                "member-cap",
                retained(&history, "capture-import.json"),
                CaptureRestoreLimits {
                    metadata: ReadLimits {
                        max_manifest_entries: 2,
                        ..limits.metadata
                    },
                    ..limits
                },
                &current_capture,
                &current_selection,
            ),
            (
                "source-cap",
                retained(&history, "capture-import.json"),
                CaptureRestoreLimits {
                    max_source_bytes: 3 * raw.len() as u64 - 1,
                    ..limits
                },
                &current_capture,
                &current_selection,
            ),
            (
                "decoded-cap",
                retained(&history, "capture-import.json"),
                CaptureRestoreLimits {
                    max_decoded_bytes: 2 * raw.len() as u64,
                    ..limits
                },
                &current_capture,
                &current_selection,
            ),
            (
                "namespace-collision",
                retained(&history, "capture-import.json"),
                limits,
                &collision_capture,
                &collision_selection,
            ),
        ] {
            let target = scratch.0.join(name);
            assert!(
                import_git_capture_corpus_v1(
                    CaptureImportRequestV1 {
                        capture_root: source,
                        output: &target,
                        selection,
                        limits: changed_limits,
                        expected_validator_sha256: validator,
                        retained: Some(supplied),
                    },
                    deadline,
                    &cancelled
                )
                .is_err(),
                "{name}"
            );
            assert!(!target.join("current.json").exists());
            if target.exists() {
                assert_eq!(
                    fs::read_dir(target.join("revisions")).unwrap().count(),
                    1,
                    "only the old unselected snapshot may survive refusal: {name}"
                );
            }
        }
        assert_eq!(
            fs::read(history.join("capture-import.json")).unwrap(),
            old_receipt
        );
        assert_eq!(
            fs::read(
                history
                    .join("revisions")
                    .join(history_receipt.revision.0.to_hex())
                    .join("snapshot.json")
            )
            .unwrap(),
            old_snapshot
        );
        assert_eq!(fs::read_dir(history.join("objects")).unwrap().count(), 1);
        assert_eq!(fs::read_dir(history.join("revisions")).unwrap().count(), 1);
    }
}
