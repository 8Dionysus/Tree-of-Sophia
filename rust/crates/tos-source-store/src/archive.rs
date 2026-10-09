//! Existing Git capture v1/v2 transport. Restoring bytes grants no admission.
use crate::software::read_capture_index;
use crate::{ReadLimits, Result, SoftwareCaptureSelectionV1, StoreError, StoreErrorCode as Code};
use sha1::{Digest, Sha1};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{Digest256Hasher, JsonValue};

#[derive(Clone, Copy, Debug)]
pub struct CaptureRestoreLimits {
    pub metadata: ReadLimits,
    pub max_archive_bytes: u64,
    pub max_decoded_bytes: u64,
    pub max_source_bytes: u64,
}

/// Actual successfully consumed bytes in the SAME capture traversal.
#[derive(Clone, Copy, Debug, Default)]
pub struct CaptureReadUsage {
    pub metadata_bytes: u64,
    pub archive_read_bytes: u64,
    pub decoded_bytes: u64,
}
impl CaptureReadUsage {
    pub fn total_read_bytes(self) -> Result<u64> {
        self.metadata_bytes
            .checked_add(self.archive_read_bytes)
            .and_then(|n| n.checked_add(self.decoded_bytes))
            .ok_or_else(|| StoreError::new(Code::BudgetExceeded, "capture read count overflow"))
    }
}
pub struct CaptureVerification {
    pub manifest: JsonValue,
    pub usage: CaptureReadUsage,
}

fn fail(detail: &'static str) -> StoreError {
    StoreError::new(Code::CorruptSelectedObject, detail)
}
fn io_error(e: io::Error) -> StoreError {
    StoreError::io("capture restore IO", e)
}
fn check(deadline: Instant, cancelled: &AtomicBool) -> io::Result<()> {
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "capture restore interrupted",
        ))
    } else {
        Ok(())
    }
}
struct Limited<'a, R> {
    inner: R,
    remaining: u64,
    consumed: u64,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
impl<R: Read> Read for Limited<'_, R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        check(self.deadline, self.cancelled)?;
        if out.is_empty() {
            return Ok(0);
        }
        let count = out
            .len()
            .min(self.remaining.saturating_add(1).min(65536) as usize);
        let n = self.inner.read(&mut out[..count])?;
        if n as u64 > self.remaining {
            return Err(io::Error::other("capture byte budget exceeded"));
        }
        self.remaining -= n as u64;
        self.consumed = self
            .consumed
            .checked_add(n as u64)
            .ok_or_else(|| io::Error::other("capture read count overflow"))?;
        Ok(n)
    }
}
fn fd_path(dir: &File, name: &str) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", dir.as_raw_fd())).join(name)
}
fn directory(parent: &File, name: &str) -> Result<File> {
    tos_fd_open::open_directory_at(parent, Path::new(name))
        .map_err(|_| fail("unsafe restore directory"))
}
pub(crate) fn new_file(
    root: &File,
    path: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<File> {
    let mut dir = root.try_clone().map_err(io_error)?;
    let mut parts = path.split('/').peekable();
    while let Some(part) = parts.next() {
        check(deadline, cancelled).map_err(io_error)?;
        if parts.peek().is_none() {
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(fd_path(&dir, part))
                .map_err(io_error)?;
            // Persist this name before a later success receipt can survive.
            dir.sync_all().map_err(io_error)?;
            return Ok(file);
        }
        match fs::DirBuilder::new()
            .mode(0o700)
            .create(fd_path(&dir, part))
        {
            Ok(()) => dir.sync_all().map_err(io_error)?,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(io_error(e)),
        }
        dir = directory(&dir, part)?;
    }
    Err(fail("empty restore member path"))
}

pub(crate) fn fresh_destination(destination: &Path) -> Result<File> {
    let parent_path = destination
        .parent()
        .ok_or_else(|| fail("restore parent absent"))?;
    let name = destination
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or_else(|| fail("invalid restore name"))?;
    if matches!(name, "." | ".." | "") {
        return Err(fail("invalid restore name"));
    }
    let parent = tos_fd_open::open_absolute_directory(parent_path)
        .map_err(|_| fail("unsafe restore parent"))?;
    fs::DirBuilder::new()
        .mode(0o700)
        .create(fd_path(&parent, name))
        .map_err(io_error)?;
    parent.sync_all().map_err(io_error)?;
    let output = directory(&parent, name)?;
    Ok(output)
}

/// Restore into a new private directory. On failure it may contain partial bytes,
/// and must not be consumed. Caller owns cleanup and physical reservation.
/// Input capture and destination parent must remain exclusively owner-controlled.
/// The selected manifest digest is supplied by the caller, not trusted from input.
pub fn restore_capture(
    capture_root: &Path,
    destination: &Path,
    selection: &SoftwareCaptureSelectionV1,
    limits: CaptureRestoreLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<()> {
    walk_capture(
        capture_root,
        Some(destination),
        selection,
        limits,
        deadline,
        cancelled,
    )
    .map(|_| ())
}

/// Verify the selected capture without creating files or granting source admission.
pub fn verify_capture(
    capture_root: &Path,
    selection: &SoftwareCaptureSelectionV1,
    limits: CaptureRestoreLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<JsonValue> {
    verify_capture_with_usage(capture_root, selection, limits, deadline, cancelled)
        .map(|verified| verified.manifest)
}
/// Verify without extraction and report actual consumed read costs. Caps remain
/// enforced per chunk; usage is not a resource grant and never resets a ledger.
pub fn verify_capture_with_usage(
    capture_root: &Path,
    selection: &SoftwareCaptureSelectionV1,
    limits: CaptureRestoreLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<CaptureVerification> {
    walk_capture(capture_root, None, selection, limits, deadline, cancelled)
}

fn walk_capture(
    capture_root: &Path,
    destination: Option<&Path>,
    selection: &SoftwareCaptureSelectionV1,
    limits: CaptureRestoreLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<CaptureVerification> {
    let metadata = limits.metadata.validate()?;
    for n in [
        limits.max_archive_bytes,
        limits.max_decoded_bytes,
        limits.max_source_bytes,
    ] {
        if n == 0 || n == u64::MAX {
            return Err(StoreError::new(
                Code::BudgetExceeded,
                "invalid capture restore limits",
            ));
        }
    }
    check(deadline, cancelled).map_err(io_error)?;
    let capture = tos_fd_open::open_absolute_directory(capture_root)
        .map_err(|_| fail("unsafe capture root"))?;
    let mut names = Vec::new();
    for entry in fs::read_dir(fd_path(&capture, ".")).map_err(io_error)? {
        if names.len() == 3 {
            return Err(fail("extra capture member"));
        }
        names.push(entry.map_err(io_error)?.file_name());
    }
    names.sort();
    if names != ["capture.json", "members.jsonl", "source.tar.gz"].map(std::ffi::OsString::from) {
        return Err(fail("capture file set differs"));
    }
    let index = read_capture_index(&capture, selection, metadata, deadline, cancelled)?;
    if index.archive_size_bytes > limits.max_archive_bytes
        || index.source_bytes > limits.max_source_bytes
    {
        return Err(StoreError::new(
            Code::BudgetExceeded,
            "capture totals exceed limits",
        ));
    }
    if destination.is_some()
        && index.members.keys().any(|p| {
            p.as_str() == "restore-receipt.json" || p.as_str().starts_with("restore-receipt.json/")
        })
    {
        return Err(fail("capture conflicts with restore receipt"));
    }
    let mut archive_file = tos_fd_open::open_regular_at(&capture, Path::new("source.tar.gz"))
        .map_err(|_| fail("unsafe capture archive"))?;
    let initial = archive_file.metadata().map_err(io_error)?;
    if initial.len() != index.archive_size_bytes {
        return Err(fail("archive size differs"));
    }
    let mut hash = Digest256Hasher::new();
    let mut buffer = [0u8; 65536];
    let mut input = Limited {
        inner: &mut archive_file,
        remaining: limits.max_archive_bytes,
        consumed: 0,
        deadline,
        cancelled,
    };
    loop {
        let n = input.read(&mut buffer).map_err(io_error)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    if hash.finalize() != index.archive_sha256 {
        return Err(fail("archive digest differs"));
    }
    let hash_pass_bytes = input.consumed;
    drop(input);
    archive_file.seek(SeekFrom::Start(0)).map_err(io_error)?;
    let output = if let Some(destination) = destination {
        let output = fresh_destination(destination)?;
        Some(output)
    } else {
        None
    };
    let compressed = Limited {
        inner: &mut archive_file,
        remaining: limits.max_archive_bytes,
        consumed: 0,
        deadline,
        cancelled,
    };
    let gzip = flate2::read::MultiGzDecoder::new(compressed);
    let decoded = Limited {
        inner: gzip,
        remaining: limits.max_decoded_bytes,
        consumed: 0,
        deadline,
        cancelled,
    };
    let mut archive = tar::Archive::new(decoded);
    let mut expected = index.members.iter();
    for item in archive.entries().map_err(io_error)? {
        check(deadline, cancelled).map_err(io_error)?;
        let mut item = item.map_err(io_error)?;
        let (path, member) = expected
            .next()
            .ok_or_else(|| fail("extra archive member"))?;
        if !item.header().entry_type().is_file()
            || item.path_bytes().as_ref() != path.as_str().as_bytes()
            || item.size() != member.size_bytes
            || item.header().mode().map_err(io_error)? != member.mode
        {
            return Err(fail("archive member metadata differs"));
        }
        let mut target = output
            .as_ref()
            .map(|output| new_file(output, path.as_str(), deadline, cancelled))
            .transpose()?;
        let mut sha256 = Digest256Hasher::new();
        let mut sha1 = Sha1::new();
        sha1.update(format!("blob {}\0", member.size_bytes).as_bytes());
        let mut count = 0u64;
        loop {
            check(deadline, cancelled).map_err(io_error)?;
            let n = item.read(&mut buffer).map_err(io_error)?;
            if n == 0 {
                break;
            }
            count = count
                .checked_add(n as u64)
                .ok_or_else(|| fail("member size overflow"))?;
            if count > member.size_bytes {
                return Err(fail("member grew"));
            }
            sha256.update(&buffer[..n]);
            sha1.update(&buffer[..n]);
            if let Some(target) = &mut target {
                target.write_all(&buffer[..n]).map_err(io_error)?;
            }
        }
        if count != member.size_bytes
            || sha256.finalize() != member.sha256
            || format!("{:x}", sha1.finalize()) != index.git_blob_oids[path]
        {
            return Err(fail("member bytes differ"));
        }
        if let Some(target) = &mut target {
            target
                .set_permissions(fs::Permissions::from_mode(member.mode))
                .map_err(io_error)?;
            target.sync_all().map_err(io_error)?;
        }
    }
    if expected.next().is_some() {
        return Err(fail("missing archive member"));
    }
    let mut decoded = archive.into_inner();
    loop {
        let n = decoded.read(&mut buffer).map_err(io_error)?;
        if n == 0 {
            break;
        }
        if buffer[..n].iter().any(|b| *b != 0) {
            return Err(fail("nonzero trailing tar bytes"));
        }
    }
    let usage = CaptureReadUsage {
        metadata_bytes: index.metadata_read_bytes,
        archive_read_bytes: hash_pass_bytes
            .checked_add(decoded.inner.get_ref().consumed)
            .ok_or_else(|| fail("archive read count overflow"))?,
        decoded_bytes: decoded.consumed,
    };
    usage.total_read_bytes()?;
    drop(decoded);
    let final_meta = archive_file.metadata().map_err(io_error)?;
    if (
        initial.dev(),
        initial.ino(),
        initial.len(),
        initial.mtime(),
        initial.mtime_nsec(),
        initial.ctime(),
        initial.ctime_nsec(),
    ) != (
        final_meta.dev(),
        final_meta.ino(),
        final_meta.len(),
        final_meta.mtime(),
        final_meta.mtime_nsec(),
        final_meta.ctime(),
        final_meta.ctime_nsec(),
    ) {
        return Err(fail("capture archive changed during restore"));
    }
    check(deadline, cancelled).map_err(io_error)?;
    let Some(output) = output else {
        return Ok(CaptureVerification {
            manifest: index.manifest,
            usage,
        });
    };
    let receipt = format!(
        "{{\"manifest_sha256\":\"{}\",\"member_count\":{},\"schema_version\":\"tos_corpus_restore_receipt_v1\",\"source_bytes\":{},\"source_git_commit\":\"{}\"}}\n",
        selection.capture_manifest_sha256.to_hex(),
        index.member_count,
        index.source_bytes,
        selection.source_git_commit
    );
    let mut file = new_file(&output, "restore-receipt.json", deadline, cancelled)?;
    let result = (|| {
        file.write_all(receipt.as_bytes()).map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        output.sync_all().map_err(io_error)?;
        Ok(())
    })();
    if result.is_err() {
        // A failed receipt write must not leave a parseable success marker.
        // Cleanup can itself fail; callers must always honor the returned error.
        drop(file);
        let _ = fs::remove_file(fd_path(&output, "restore-receipt.json"));
    }
    result.map(|()| CaptureVerification {
        manifest: index.manifest,
        usage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tos_foundation::{
        CanonicalProfile, Digest256, JsonLimits, JsonNumber, JsonNumberKind, JsonString,
        canonical_bytes_v1,
    };

    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn json_string(value: &str) -> JsonValue {
        JsonValue::String(JsonString::from_utf8(value))
    }

    fn json_number(value: u64) -> JsonValue {
        JsonValue::Number(JsonNumber {
            kind: JsonNumberKind::Int,
            lexeme: value.to_string(),
        })
    }

    fn json_array(values: &[String]) -> JsonValue {
        JsonValue::Array(values.iter().map(|value| json_string(value)).collect())
    }

    fn json_object(values: Vec<(&str, JsonValue)>) -> JsonValue {
        JsonValue::Object(
            values
                .into_iter()
                .map(|(key, value)| (JsonString::from_utf8(key), value))
                .collect(),
        )
    }

    fn canonical(value: &JsonValue) -> Vec<u8> {
        canonical_bytes_v1(
            value,
            CanonicalProfile::CorpusSnapshotV1,
            JsonLimits::default(),
        )
        .unwrap()
    }

    fn write_capture_metadata(root: &Path, path: &str, data: &[u8], archive: &[u8]) -> Vec<u8> {
        let mut git_blob = Sha1::new();
        git_blob.update(format!("blob {}\0", data.len()).as_bytes());
        git_blob.update(data);
        let member = json_object(vec![
            ("path", json_string(path)),
            (
                "git_blob_oid",
                json_string(&format!("{:x}", git_blob.finalize())),
            ),
            ("size_bytes", json_number(data.len() as u64)),
            ("sha256", json_string(&Digest256::of_bytes(data).to_hex())),
            ("mode", json_number(0o755)),
        ]);
        let members = canonical(&member);
        fs::write(root.join("members.jsonl"), &members).unwrap();

        let commit = "1".repeat(40);
        let tree = "2".repeat(40);
        let manifest = json_object(vec![
            ("schema_version", json_string("tos_corpus_capture_v2")),
            ("source_git_commit", json_string(&commit)),
            ("source_git_tree", json_string(&tree)),
            ("include_prefixes", json_array(&["scripts".to_owned()])),
            ("exclude_prefixes", json_array(&[])),
            ("exclude_path_parts", json_array(&[])),
            ("member_count", json_number(1)),
            ("source_bytes", json_number(data.len() as u64)),
            (
                "members_sha256",
                json_string(&Digest256::of_bytes(&members).to_hex()),
            ),
            (
                "archive_sha256",
                json_string(&Digest256::of_bytes(archive).to_hex()),
            ),
            ("archive_size_bytes", json_number(archive.len() as u64)),
        ]);
        let raw = canonical(&manifest);
        fs::write(root.join("capture.json"), &raw).unwrap();
        raw
    }

    fn synthetic_capture(root: &Path) -> (String, Vec<u8>, Vec<u8>) {
        fs::create_dir(root).unwrap();
        let path = format!("scripts/{}δοκιμή.py", "nested/".repeat(18));
        let data = b"old bytes\x00\xff\n".to_vec();
        // Keep restore coverage independent of the native Git capture path.
        let gzip = flate2::GzBuilder::new()
            .mtime(0)
            .operating_system(255)
            .write(Vec::new(), flate2::Compression::best());
        let mut tar = tar::Builder::new(gzip);
        let pax_path = !path.is_ascii() || path.len() > 100;
        if pax_path {
            tar.append_pax_extensions([("path", path.as_bytes())])
                .unwrap();
        }
        let mut header = tar::Header::new_ustar();
        header
            .set_path(if pax_path { "PaxMember" } else { path.as_str() })
            .unwrap();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_mode(0o755);
        header.set_size(data.len() as u64);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_username("").unwrap();
        header.set_groupname("").unwrap();
        header.set_cksum();
        tar.append(&header, data.as_slice()).unwrap();
        let archive = tar.into_inner().unwrap().finish().unwrap();
        fs::write(root.join("source.tar.gz"), &archive).unwrap();
        write_capture_metadata(root, &path, &data, &archive);
        (path, data, archive)
    }

    #[test]
    fn native_pax_capture_restores_exact_bytes_and_rejects_corruption() {
        let dir = std::env::temp_dir().join(format!(
            "tos-capture-restore-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let scratch = Scratch(dir);
        let capture = scratch.0.join("capture");
        let (path, data, original_archive) = synthetic_capture(&capture);
        let selection = SoftwareCaptureSelectionV1 {
            source_git_commit: "1".repeat(40),
            source_git_tree: "2".repeat(40),
            capture_manifest_sha256: Digest256::of_bytes(
                &fs::read(capture.join("capture.json")).unwrap(),
            ),
        };
        let limits = CaptureRestoreLimits {
            metadata: ReadLimits {
                max_manifest_bytes: 16384,
                max_manifest_entries: 8,
                max_selected_object_bytes: 16384,
                json: JsonLimits {
                    max_bytes: 16384,
                    ..JsonLimits::default()
                },
            },
            max_archive_bytes: 65536,
            max_decoded_bytes: 65536,
            max_source_bytes: 16384,
        };
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + std::time::Duration::from_secs(10);
        let output = scratch.0.join("restored");
        restore_capture(&capture, &output, &selection, limits, deadline, &cancelled).unwrap();
        let reader = crate::SoftwareCaptureReader::open(
            &capture,
            &output,
            selection.clone(),
            limits.metadata,
            deadline,
            &cancelled,
        )
        .unwrap();
        let member = reader.members().next().unwrap();
        assert_eq!(member.path.as_str(), path);
        assert_eq!(
            reader
                .read_current(&member.path, 16384, deadline, &cancelled)
                .unwrap()
                .unwrap(),
            data
        );
        assert_eq!(
            fs::metadata(output.join(member.path.as_str()))
                .unwrap()
                .mode()
                & 0o777,
            0o755
        );
        assert!(
            restore_capture(&capture, &output, &selection, limits, deadline, &cancelled).is_err()
        );
        let mut archive = original_archive.clone();
        archive[10] ^= 1;
        fs::write(capture.join("source.tar.gz"), archive).unwrap();
        let rejected = scratch.0.join("rejected");
        assert!(
            restore_capture(
                &capture, &rejected, &selection, limits, deadline, &cancelled
            )
            .is_err()
        );
        assert!(!rejected.exists());
        // Restore the stream, corrupt only gzip CRC, then honestly rebind the
        // outer digest. The decoder still must reject it before receipting.
        let mut corrupt_crc = original_archive;
        let crc_offset = corrupt_crc.len() - 8;
        corrupt_crc[crc_offset] ^= 1;
        fs::write(capture.join("source.tar.gz"), &corrupt_crc).unwrap();
        let rebound_manifest = write_capture_metadata(&capture, &path, &data, &corrupt_crc);
        let mut rebound = selection.clone();
        rebound.capture_manifest_sha256 = Digest256::of_bytes(&rebound_manifest);
        let crc_output = scratch.0.join("bad-crc");
        assert!(
            restore_capture(
                &capture,
                &crc_output,
                &rebound,
                limits,
                deadline,
                &cancelled
            )
            .is_err()
        );
        assert!(!crc_output.join("restore-receipt.json").exists());
    }
}
