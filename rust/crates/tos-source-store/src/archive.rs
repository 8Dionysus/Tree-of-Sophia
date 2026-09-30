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
use tos_foundation::Digest256Hasher;

#[derive(Clone, Copy, Debug)]
pub struct CaptureRestoreLimits {
    pub metadata: ReadLimits,
    pub max_archive_bytes: u64,
    pub max_decoded_bytes: u64,
    pub max_source_bytes: u64,
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
fn new_file(root: &File, path: &str, deadline: Instant, cancelled: &AtomicBool) -> Result<File> {
    let mut dir = root.try_clone().map_err(io_error)?;
    let mut parts = path.split('/').peekable();
    while let Some(part) = parts.next() {
        check(deadline, cancelled).map_err(io_error)?;
        if parts.peek().is_none() {
            return OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(fd_path(&dir, part))
                .map_err(io_error);
        }
        match fs::DirBuilder::new()
            .mode(0o700)
            .create(fd_path(&dir, part))
        {
            Ok(()) => (),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(io_error(e)),
        }
        dir = directory(&dir, part)?;
    }
    Err(fail("empty restore member path"))
}

/// Restore into a new private directory. On failure it may contain partial bytes,
/// but never a success receipt. Caller owns cleanup and physical reservation.
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
    if index.members.keys().any(|p| {
        p.as_str() == "restore-receipt.json" || p.as_str().starts_with("restore-receipt.json/")
    }) {
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
    archive_file.seek(SeekFrom::Start(0)).map_err(io_error)?;
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
    let output = directory(&parent, name)?;
    let compressed = Limited {
        inner: &mut archive_file,
        remaining: limits.max_archive_bytes,
        deadline,
        cancelled,
    };
    let gzip = flate2::read::MultiGzDecoder::new(compressed);
    let decoded = Limited {
        inner: gzip,
        remaining: limits.max_decoded_bytes,
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
        let mut target = new_file(&output, path.as_str(), deadline, cancelled)?;
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
            target.write_all(&buffer[..n]).map_err(io_error)?;
        }
        if count != member.size_bytes
            || sha256.finalize() != member.sha256
            || format!("{:x}", sha1.finalize()) != index.git_blob_oids[path]
        {
            return Err(fail("member bytes differ"));
        }
        target
            .set_permissions(fs::Permissions::from_mode(member.mode))
            .map_err(io_error)?;
        target.sync_all().map_err(io_error)?;
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
    let receipt = format!(
        "{{\"manifest_sha256\":\"{}\",\"member_count\":{},\"schema_version\":\"tos_corpus_restore_receipt_v1\",\"source_bytes\":{},\"source_git_commit\":\"{}\"}}\n",
        selection.capture_manifest_sha256.to_hex(),
        index.member_count,
        index.source_bytes,
        selection.source_git_commit
    );
    let mut file = new_file(&output, "restore-receipt.json", deadline, cancelled)?;
    file.write_all(receipt.as_bytes()).map_err(io_error)?;
    file.sync_all().map_err(io_error)?;
    output.sync_all().map_err(io_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tos_foundation::{Digest256, JsonLimits};
    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn python_pax_capture_restores_exact_bytes_and_rejects_corruption() {
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
        let script = r#"
import sys,io,json,gzip,tarfile,hashlib
from pathlib import Path
root=Path(sys.argv[1]); root.mkdir()
canon=lambda x:(json.dumps(x,sort_keys=True,separators=(',',':'),ensure_ascii=False)+'\n').encode()
path='scripts/'+('nested/'*18)+'δοκιμή.py'; data=b'old bytes\x00\xff\n'
row={'path':path,'git_blob_oid':hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest(),'size_bytes':len(data),'sha256':hashlib.sha256(data).hexdigest(),'mode':493}
with (root/'source.tar.gz').open('wb') as raw:
 with gzip.GzipFile(fileobj=raw,mode='wb',mtime=0,filename='') as gz:
  with tarfile.open(fileobj=gz,mode='w|',format=tarfile.PAX_FORMAT) as tar:
   info=tarfile.TarInfo(path); info.size=len(data); info.mode=493; tar.addfile(info,io.BytesIO(data))
members=canon(row); (root/'members.jsonl').write_bytes(members)
archive=(root/'source.tar.gz').read_bytes()
manifest={'schema_version':'tos_corpus_capture_v2','source_git_commit':'1'*40,'source_git_tree':'2'*40,'include_prefixes':['scripts'],'exclude_prefixes':[],'exclude_path_parts':[],'member_count':1,'source_bytes':len(data),'members_sha256':hashlib.sha256(members).hexdigest(),'archive_sha256':hashlib.sha256(archive).hexdigest(),'archive_size_bytes':len(archive)}
(root/'capture.json').write_bytes(canon(manifest))
"#;
        let capture = scratch.0.join("capture");
        assert!(
            std::process::Command::new("python3")
                .arg("-I")
                .arg("-c")
                .arg(script)
                .arg(&capture)
                .status()
                .unwrap()
                .success()
        );
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
        assert_eq!(
            reader
                .read_current(&member.path, 16384, deadline, &cancelled)
                .unwrap()
                .unwrap(),
            b"old bytes\x00\xff\n"
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
        let mut archive = fs::read(capture.join("source.tar.gz")).unwrap();
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
    }
}
