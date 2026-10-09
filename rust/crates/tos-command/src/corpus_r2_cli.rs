//! Native Cloudflare R2 byte boundary for raw and chunked source transfers.
//!
//! The command accepts only explicit paths, bucket/key selection, byte fixity,
//! and finite limits. Source rights, plans, receipts, publication, and canon
//! remain with their existing owners. Credentials are read only by Wrangler
//! and held only in memory when the REST transport is selected.

use serde_json::{Value, json};
use std::ffi::OsString;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, atomic::AtomicBool, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, canonical_bytes_v1,
    parse_json,
};
use tos_source_store::{
    ChunkedFileLimitsV1, ChunkedFileTransportV1, restore_chunked_file_v1, upload_chunked_file_v1,
};

pub const HELP: &str = "usage: tos-native-owner-command corpus-r2 < REQUEST_JSON\n\nRequest schema: tos_corpus_r2_request_v1. Operations: raw-transfer, raw-read, chunk-upload, chunk-restore.\nThe Wrangler transport uses an absolute Wrangler executable. The rest transport uses Wrangler's supported auth-token command and bounded curl requests; it requires an account_id.\nRaw transfer probes before upload and verifies exact readback. Chunked operations verify every object through the maintained source-store protocol. No operation grants rights, admission, or publication.\n";

const REQUEST_BYTES: usize = 64 * 1024;
const RESPONSE_BYTES: usize = 16 * 1024;
const MAX_OBJECT_BYTES: u64 = 300 * 1024 * 1024;
const MAX_CURL_OUTPUT: usize = 32 * 1024;
const MAX_PROCESS_STDERR: usize = 64 * 1024;
const MAX_AUTH_TOKEN_BYTES: usize = 16 * 1024;
const DEFAULT_TIMEOUT_SECONDS: u64 = 120;
const MAX_TIMEOUT_SECONDS: u64 = 600;
const MAX_RETRIES: usize = 3;
const REQUEST_SPACING: Duration = Duration::from_millis(300);
static SCRATCH_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R2TransportKindV1 {
    Wrangler,
    Rest,
}

/// One explicitly selected external R2 transport. A value can be reused for a
/// batch so the spacing clock and REST token survive between object requests.
pub struct R2TransportV1 {
    kind: R2TransportKindV1,
    bucket: String,
    account_id: Option<String>,
    wrangler: PathBuf,
    cwd: Option<PathBuf>,
    timeout: Duration,
    max_upload_bytes: u64,
    token: Option<String>,
    last_request_at: Option<Instant>,
}

impl R2TransportV1 {
    pub fn new(
        kind: R2TransportKindV1,
        bucket: &str,
        account_id: Option<&str>,
        wrangler: &Path,
        cwd: Option<&Path>,
        timeout_seconds: u64,
    ) -> Result<Self, String> {
        validate_bucket(bucket)?;
        if !wrangler.is_absolute() || wrangler.to_str().is_none() {
            return Err("Wrangler executable must be an absolute UTF-8 path".into());
        }
        if let Some(cwd) = cwd
            && (!cwd.is_absolute() || cwd.to_str().is_none())
        {
            return Err("working directory must be an absolute UTF-8 path".into());
        }
        if !(1..=MAX_TIMEOUT_SECONDS).contains(&timeout_seconds) {
            return Err("timeout_seconds is outside the finite 1..600 second bound".into());
        }
        let account_id = match (kind, account_id) {
            (R2TransportKindV1::Rest, Some(value)) => Some(validate_account_id(value)?),
            (R2TransportKindV1::Rest, None) => {
                return Err("REST transport requires account_id".into());
            }
            (R2TransportKindV1::Wrangler, Some(value)) => Some(validate_account_id(value)?),
            (R2TransportKindV1::Wrangler, None) => None,
        };
        Ok(Self {
            kind,
            bucket: bucket.to_owned(),
            account_id,
            wrangler: wrangler.to_owned(),
            cwd: cwd.map(Path::to_owned),
            timeout: Duration::from_secs(timeout_seconds),
            max_upload_bytes: MAX_OBJECT_BYTES,
            token: None,
            last_request_at: None,
        })
    }

    /// Narrow the per-object upload ceiling without changing transfer receipts.
    pub fn with_max_upload_bytes(mut self, max_upload_bytes: u64) -> Result<Self, String> {
        if max_upload_bytes > MAX_OBJECT_BYTES {
            return Err("max_upload_bytes exceeds the 300 MiB platform bound".into());
        }
        self.max_upload_bytes = max_upload_bytes;
        Ok(self)
    }

    fn throttle(&mut self) {
        if let Some(last) = self.last_request_at {
            let elapsed = last.elapsed();
            if elapsed < REQUEST_SPACING {
                thread::sleep(REQUEST_SPACING - elapsed);
            }
        }
        self.last_request_at = Some(Instant::now());
    }

    fn object_path(&self, key: &str) -> Result<String, String> {
        validate_object_key(key)?;
        Ok(format!("{}/{}", self.bucket, key))
    }

    fn rest_url(&self, key: &str) -> Result<String, String> {
        let account = self
            .account_id
            .as_deref()
            .ok_or("REST account ID is unavailable")?;
        validate_object_key(key)?;
        Ok(format!(
            "https://api.cloudflare.com/client/v4/accounts/{account}/r2/buckets/{}/objects/{key}",
            self.bucket
        ))
    }

    fn auth_token(&mut self) -> Result<&str, String> {
        if self.token.is_none() {
            let args = vec![
                OsString::from("auth"),
                OsString::from("token"),
                OsString::from("--json"),
            ];
            let output = run_bounded(
                &self.wrangler,
                &args,
                self.cwd.as_deref(),
                None,
                self.timeout,
                MAX_AUTH_TOKEN_BYTES,
                MAX_PROCESS_STDERR,
                ProcessEnvironment::Wrangler,
            )?;
            if !output.success {
                return Err("Wrangler auth token command failed".into());
            }
            let value: Value = serde_json::from_slice(&output.stdout)
                .map_err(|_| "Wrangler auth token output was invalid")?;
            let token_type = value.get("type").and_then(Value::as_str);
            let token = value.get("token").and_then(Value::as_str);
            if !matches!(token_type, Some("oauth" | "api_token"))
                || token.is_none_or(|token| token.is_empty() || token.contains(['\r', '\n']))
            {
                return Err("Wrangler auth token output lacked a usable token".into());
            }
            self.token = token.map(str::to_owned);
        }
        self.token
            .as_deref()
            .ok_or_else(|| "Wrangler auth token unavailable".into())
    }

    fn refresh_token(&mut self) -> Result<(), String> {
        self.token = None;
        let _ = self.auth_token()?;
        Ok(())
    }

    fn run_wrangler(&self, args: &[OsString]) -> Result<ProcessOutput, String> {
        run_bounded(
            &self.wrangler,
            args,
            self.cwd.as_deref(),
            None,
            self.timeout,
            MAX_CURL_OUTPUT,
            MAX_PROCESS_STDERR,
            ProcessEnvironment::Wrangler,
        )
    }

    fn run_curl(&self, args: &[OsString], stdin: Vec<u8>) -> Result<ProcessOutput, String> {
        run_bounded(
            Path::new("/usr/bin/curl"),
            args,
            self.cwd.as_deref(),
            Some(stdin),
            self.timeout,
            128,
            MAX_PROCESS_STDERR,
            ProcessEnvironment::Native,
        )
    }

    fn rest_request(
        &mut self,
        method: &str,
        key: &str,
        source: Option<&Path>,
        destination: Option<&Path>,
        max_download_bytes: u64,
        media_type: Option<&str>,
    ) -> Result<u16, String> {
        let url = self.rest_url(key)?;
        let mut refreshed = false;
        let mut retries = 0usize;
        loop {
            self.throttle();
            let token = self.auth_token()?.to_owned();
            let mut config = curl_config_header(&format!("Authorization: Bearer {token}"));
            if let Some(media_type) = media_type {
                if media_type.is_empty() || media_type.contains(['\r', '\n']) {
                    return Err("upload media type is invalid".into());
                }
                config.push_str(&curl_config_header(&format!("Content-Type: {media_type}")));
                config.push_str(&curl_config_header("Cache-Control: private,no-store"));
                config.push_str(&curl_config_header("cf-r2-storage-class: Standard"));
            }
            config.push_str(&curl_config_header("Accept-Encoding: identity"));
            let mut args = vec![
                OsString::from("-q"),
                OsString::from("--silent"),
                OsString::from("--max-time"),
                OsString::from(self.timeout.as_secs().to_string()),
                OsString::from("--proto"),
                OsString::from("=https"),
                OsString::from("--write-out"),
                OsString::from("%{http_code}"),
                OsString::from("--config"),
                OsString::from("-"),
                OsString::from("--url"),
                OsString::from(url.as_str()),
            ];
            if method == "GET" {
                args.push(OsString::from("--max-filesize"));
                args.push(OsString::from(max_download_bytes.max(1).to_string()));
                args.push(OsString::from("--output"));
                args.push(path_os(destination.ok_or("REST GET destination missing")?)?);
            } else {
                args.push(OsString::from("--request"));
                args.push(OsString::from("PUT"));
                args.push(OsString::from("--upload-file"));
                args.push(path_os(source.ok_or("REST PUT source missing")?)?);
                args.push(OsString::from("--output"));
                args.push(OsString::from("/dev/null"));
            }
            let output = self.run_curl(&args, config.into_bytes());
            let output = match output {
                Ok(output) => output,
                Err(_) => {
                    if let Some(destination) = destination {
                        remove_if_exists(destination);
                    }
                    if retries < MAX_RETRIES {
                        retries += 1;
                        thread::sleep(retry_delay(retries));
                        continue;
                    }
                    return Err("Cloudflare REST request exceeded retry budget".into());
                }
            };
            if !output.success {
                if let Some(destination) = destination {
                    remove_if_exists(destination);
                }
                if retries < MAX_RETRIES {
                    retries += 1;
                    thread::sleep(retry_delay(retries));
                    continue;
                }
                return Err("Cloudflare REST request exceeded retry budget".into());
            }
            let status = std::str::from_utf8(&output.stdout)
                .ok()
                .filter(|text| text.len() == 3 && text.bytes().all(|b| b.is_ascii_digit()))
                .and_then(|text| text.parse::<u16>().ok())
                .ok_or_else(|| {
                    if let Some(destination) = destination {
                        remove_if_exists(destination);
                    }
                    "Cloudflare REST response status was missing"
                })?;
            if status == 401 {
                if refreshed {
                    return Err("Cloudflare REST authentication failed".into());
                }
                refreshed = true;
                self.refresh_token()?;
                if let Some(destination) = destination {
                    remove_if_exists(destination);
                }
                continue;
            }
            if (status == 429 || status >= 500) && retries < MAX_RETRIES {
                retries += 1;
                if let Some(destination) = destination {
                    remove_if_exists(destination);
                }
                thread::sleep(retry_delay(retries));
                continue;
            }
            return Ok(status);
        }
    }
}

impl ChunkedFileTransportV1 for R2TransportV1 {
    fn fetch(&mut self, key: &str, destination: &Path, max_bytes: u64) -> io::Result<bool> {
        let result = self.fetch_inner(key, destination, max_bytes);
        result.map_err(io_error)
    }

    fn put(
        &mut self,
        key: &str,
        source: &Path,
        byte_size: u64,
        media_type: &str,
        storage_class: &str,
    ) -> io::Result<()> {
        self.put_inner(key, source, byte_size, media_type, storage_class)
            .map_err(io_error)
    }
}

impl R2TransportV1 {
    fn fetch_inner(
        &mut self,
        key: &str,
        destination: &Path,
        max_bytes: u64,
    ) -> Result<bool, String> {
        validate_object_key(key)?;
        if max_bytes > MAX_OBJECT_BYTES {
            return Err("remote object read exceeds the 300 MiB object bound".into());
        }
        if fs::symlink_metadata(destination).is_ok() {
            return Err("transport destination already exists".into());
        }
        match self.kind {
            R2TransportKindV1::Wrangler => {
                self.throttle();
                let object = self.object_path(key)?;
                let args = vec![
                    OsString::from("r2"),
                    OsString::from("object"),
                    OsString::from("get"),
                    OsString::from(object),
                    OsString::from("--remote"),
                    OsString::from("--file"),
                    path_os(destination)?,
                ];
                let output = self.run_wrangler(&args)?;
                if !output.success {
                    remove_if_exists(destination);
                    if missing_object_marker(&output.stdout, &output.stderr) {
                        return Ok(false);
                    }
                    return Err("Wrangler R2 read failed".into());
                }
                let size = regular_file_size(destination)?;
                if size > max_bytes {
                    remove_if_exists(destination);
                    return Err("Wrangler R2 read exceeded selected byte limit".into());
                }
                Ok(true)
            }
            R2TransportKindV1::Rest => {
                let status =
                    self.rest_request("GET", key, None, Some(destination), max_bytes, None)?;
                if status == 404 {
                    remove_if_exists(destination);
                    return Ok(false);
                }
                if !(200..300).contains(&status) {
                    remove_if_exists(destination);
                    return Err(format!("Cloudflare REST read failed with HTTP {status}"));
                }
                let size = regular_file_size(destination)?;
                if size > max_bytes {
                    remove_if_exists(destination);
                    return Err("Cloudflare REST read exceeded selected byte limit".into());
                }
                Ok(true)
            }
        }
    }

    fn put_inner(
        &mut self,
        key: &str,
        source: &Path,
        byte_size: u64,
        media_type: &str,
        storage_class: &str,
    ) -> Result<(), String> {
        validate_object_key(key)?;
        if storage_class != "Standard" {
            return Err("only R2 Standard storage is enabled by this adapter".into());
        }
        if byte_size > self.max_upload_bytes {
            return Err("R2 upload exceeds selected per-object byte limit".into());
        }
        if media_type.is_empty() || media_type.contains(['\r', '\n']) {
            return Err("upload media type is invalid".into());
        }
        if regular_file_size(source)? != byte_size {
            return Err("upload source byte-size does not match the plan".into());
        }
        match self.kind {
            R2TransportKindV1::Wrangler => {
                self.throttle();
                let object = self.object_path(key)?;
                let args = vec![
                    OsString::from("r2"),
                    OsString::from("object"),
                    OsString::from("put"),
                    OsString::from(object),
                    OsString::from("--remote"),
                    OsString::from("--file"),
                    path_os(source)?,
                    OsString::from("--content-type"),
                    OsString::from(media_type),
                    OsString::from("--cache-control"),
                    OsString::from("private,no-store"),
                    OsString::from("--storage-class"),
                    OsString::from("Standard"),
                ];
                let output = self.run_wrangler(&args)?;
                if !output.success {
                    return Err("Wrangler R2 upload failed".into());
                }
                Ok(())
            }
            R2TransportKindV1::Rest => {
                if self.rest_request("PUT", key, Some(source), None, 0, Some(media_type))? >= 300 {
                    return Err("Cloudflare REST upload failed".into());
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawTransferReceiptV1 {
    pub remote_status: &'static str,
    pub upload_attempted: bool,
    pub byte_size: u64,
    pub sha256: Digest256,
    pub readback_verified: bool,
}

/// Probe, upload only when absent, then verify the exact remote bytes.
pub fn raw_transfer_v1<T: ChunkedFileTransportV1>(
    transport: &mut T,
    object_key: &str,
    source: &Path,
    expected_size: u64,
    expected_sha256: Digest256,
    media_type: &str,
    scratch_root: &Path,
) -> Result<RawTransferReceiptV1, String> {
    validate_object_key(object_key)?;
    if expected_size > MAX_OBJECT_BYTES {
        return Err("raw object exceeds the 300 MiB object limit".into());
    }
    verify_file(source, expected_size, expected_sha256)?;
    let scratch = ScratchDirectory::create(scratch_root)?;
    let probe = scratch.0.join("probe.bin");
    let existing = transport
        .fetch(object_key, &probe, expected_size.max(1))
        .map_err(|_| "remote R2 probe failed")?;
    if existing {
        verify_file(&probe, expected_size, expected_sha256)?;
        return Ok(RawTransferReceiptV1 {
            remote_status: "already-matched",
            upload_attempted: false,
            byte_size: expected_size,
            sha256: expected_sha256,
            readback_verified: true,
        });
    }
    if fs::symlink_metadata(&probe).is_ok() {
        remove_if_exists(&probe);
        return Err("missing-object probe produced bytes".into());
    }
    transport
        .put(object_key, source, expected_size, media_type, "Standard")
        .map_err(|_| "remote R2 upload failed")?;
    let readback = scratch.0.join("readback.bin");
    let found = transport
        .fetch(object_key, &readback, expected_size.max(1))
        .map_err(|_| "remote R2 readback failed")?;
    if !found {
        return Err("uploaded R2 object was unavailable on readback".into());
    }
    verify_file(&readback, expected_size, expected_sha256)?;
    Ok(RawTransferReceiptV1 {
        remote_status: "uploaded",
        upload_attempted: true,
        byte_size: expected_size,
        sha256: expected_sha256,
        readback_verified: true,
    })
}

/// Verify a remote object before atomically publishing a new local output.
pub fn raw_read_verified_v1<T: ChunkedFileTransportV1>(
    transport: &mut T,
    object_key: &str,
    output: &Path,
    expected_size: u64,
    expected_sha256: Digest256,
    scratch_root: &Path,
) -> Result<(), String> {
    validate_object_key(object_key)?;
    if expected_size > MAX_OBJECT_BYTES {
        return Err("raw object exceeds the 300 MiB object limit".into());
    }
    if fs::symlink_metadata(output).is_ok() {
        return Err("read output already exists; refusing overwrite".into());
    }
    let scratch = ScratchDirectory::create(scratch_root)?;
    let readback = scratch.0.join("remote.bin");
    if !transport
        .fetch(object_key, &readback, expected_size.max(1))
        .map_err(|_| "remote R2 read failed")?
    {
        return Err("remote R2 object is unavailable".into());
    }
    verify_file(&readback, expected_size, expected_sha256)?;
    publish_no_clobber(&readback, output, expected_size, expected_sha256)?;
    Ok(())
}

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn create(root: &Path) -> Result<Self, String> {
        if !root.is_absolute() || root.to_str().is_none() {
            return Err("scratch_root must be an absolute UTF-8 directory".into());
        }
        fs::create_dir_all(root).map_err(|_| "cannot prepare transfer scratch root")?;
        let mut current = PathBuf::new();
        for component in root.components() {
            match component {
                Component::RootDir => current.push("/"),
                Component::Normal(part) => current.push(part),
                Component::CurDir => continue,
                Component::ParentDir => return Err("scratch_root is not normalized".into()),
                _ => return Err("scratch_root is invalid".into()),
            }
            let metadata = fs::symlink_metadata(&current)
                .map_err(|_| "cannot inspect transfer scratch root")?;
            if metadata.file_type().is_symlink() || (current != root && !metadata.is_dir()) {
                return Err("scratch_root or its ancestors are unsafe".into());
            }
        }
        for _ in 0..128 {
            let sequence = SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = root.join(format!(".tos-corpus-r2-{}-{sequence}", std::process::id()));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err("cannot create transfer scratch directory".into()),
            }
        }
        Err("cannot reserve transfer scratch directory".into())
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn verify_file(path: &Path, expected_size: u64, expected_sha256: Digest256) -> Result<(), String> {
    let before = regular_file_metadata(path)?;
    if before.len() != expected_size {
        return Err("source or remote byte-size mismatch".into());
    }
    let mut file = File::open(path).map_err(|_| "cannot open source or remote file")?;
    let opened_before = file
        .metadata()
        .map_err(|_| "cannot inspect source or remote file")?;
    if !opened_before.is_file() || file_identity(&before) != file_identity(&opened_before) {
        return Err("source or remote file changed before hash".into());
    }
    let mut hasher = Digest256Hasher::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| "cannot read source or remote file")?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or("source or remote file size overflow")?;
        if total > expected_size {
            return Err("source or remote file exceeds expected size".into());
        }
        hasher.update(&buffer[..count]);
    }
    let opened_after = file
        .metadata()
        .map_err(|_| "cannot restat source or remote file")?;
    let after = regular_file_metadata(path)?;
    if file_identity(&opened_after) != file_identity(&before)
        || file_identity(&after) != file_identity(&before)
        || total != expected_size
        || hasher.finalize() != expected_sha256
    {
        return Err("source or remote SHA-256 mismatch or changed during verification".into());
    }
    Ok(())
}

fn publish_no_clobber(
    source: &Path,
    output: &Path,
    expected_size: u64,
    expected_sha256: Digest256,
) -> Result<(), String> {
    if fs::symlink_metadata(output).is_ok() {
        return Err("read output already exists; refusing overwrite".into());
    }
    let parent = output.parent().ok_or("read output parent is missing")?;
    fs::create_dir_all(parent).map_err(|_| "cannot create read output parent")?;
    let leaf = output
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or("read output name is invalid")?;
    let sequence = SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = parent.join(format!(".{leaf}.{}.{}.tmp", std::process::id(), sequence));
    let result = (|| {
        let mut target = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|_| "cannot create private output temporary")?;
        let mut source_file = File::open(source).map_err(|_| "cannot open verified readback")?;
        io::copy(&mut source_file, &mut target).map_err(|_| "cannot copy verified readback")?;
        target
            .sync_all()
            .map_err(|_| "cannot sync verified output")?;
        drop(target);
        verify_file(&temporary, expected_size, expected_sha256)?;
        fs::hard_link(&temporary, output)
            .map_err(|_| "read output appeared or cannot be published without overwrite")?;
        if let Ok(directory) = File::open(parent) {
            let _ = directory.sync_all();
        }
        Ok(())
    })();
    let _ = fs::remove_file(&temporary);
    result
}

fn file_identity(metadata: &Metadata) -> (u64, u64, u64, u32, u64, i64, i64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mode(),
        metadata.nlink(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}

fn regular_file_metadata(path: &Path) -> Result<Metadata, String> {
    let metadata = fs::symlink_metadata(path).map_err(|_| "selected file is unavailable")?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("selected file must be regular and not a symlink".into());
    }
    Ok(metadata)
}

fn regular_file_size(path: &Path) -> Result<u64, String> {
    Ok(regular_file_metadata(path)?.len())
}

fn validate_bucket(value: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    if bytes.len() < 3
        || bytes.len() > 63
        || !bytes.first().is_some_and(u8::is_ascii_lowercase)
            && !bytes.first().is_some_and(u8::is_ascii_digit)
        || !bytes.last().is_some_and(u8::is_ascii_lowercase)
            && !bytes.last().is_some_and(u8::is_ascii_digit)
        || bytes
            .iter()
            .any(|byte| !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && *byte != b'-')
    {
        return Err("invalid R2 bucket name".into());
    }
    Ok(())
}

fn validate_account_id(value: &str) -> Result<String, String> {
    if value.len() != 32 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Cloudflare account ID must contain 32 hexadecimal characters".into());
    }
    Ok(value.to_ascii_lowercase())
}

fn validate_object_key(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.starts_with('/')
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/'))
        || value.split('/').any(|part| part == "..")
    {
        return Err("unsafe R2 object key".into());
    }
    Ok(())
}

fn path_os(path: &Path) -> Result<OsString, String> {
    if !path.is_absolute() {
        return Err("selected path must be absolute".into());
    }
    path.to_str()
        .map(OsString::from)
        .ok_or_else(|| "selected path must be UTF-8".into())
}

fn io_error(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::Other, message)
}

fn remove_if_exists(path: &Path) {
    let _ = fs::remove_file(path);
}

fn missing_object_marker(stdout: &[u8], stderr: &[u8]) -> bool {
    let output = format!(
        "{}\n{}",
        String::from_utf8_lossy(stdout).to_ascii_lowercase(),
        String::from_utf8_lossy(stderr).to_ascii_lowercase()
    );
    [
        "not found",
        "no such object",
        "does not exist",
        "object not found",
    ]
    .iter()
    .any(|marker| output.contains(marker))
}

fn retry_delay(retry: usize) -> Duration {
    Duration::from_millis((500u64.saturating_mul(1u64 << retry.saturating_sub(1))).min(30_000))
}

fn curl_config_header(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("header = \"{escaped}\"\n")
}

#[derive(Clone, Copy)]
enum ProcessEnvironment {
    Wrangler,
    Native,
}

struct ProcessOutput {
    success: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn run_bounded(
    program: &Path,
    args: &[OsString],
    cwd: Option<&Path>,
    stdin: Option<Vec<u8>>,
    timeout: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
    environment: ProcessEnvironment,
) -> Result<ProcessOutput, String> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    match environment {
        ProcessEnvironment::Wrangler => {
            command.env("WRANGLER_SEND_METRICS", "false");
        }
        ProcessEnvironment::Native => {
            command
                .env_clear()
                .env("PATH", "/usr/bin")
                .env("LC_ALL", "C.UTF-8");
            for name in [
                "HTTPS_PROXY",
                "https_proxy",
                "HTTP_PROXY",
                "http_proxy",
                "NO_PROXY",
                "no_proxy",
            ] {
                if let Some(value) = std::env::var_os(name) {
                    command.env(name, value);
                }
            }
        }
    }
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let mut child = command
        .spawn()
        .map_err(|_| "external R2 tool is unavailable".to_owned())?;
    let group = rustix::process::Pid::from_child(&child);
    let stdout = child
        .stdout
        .take()
        .ok_or("external R2 stdout unavailable")?;
    let stderr = child
        .stderr
        .take()
        .ok_or("external R2 stderr unavailable")?;
    let overflow = Arc::new(AtomicBool::new(false));
    let (sender, receiver) = mpsc::channel();
    for (index, mut pipe, limit) in [
        (
            0usize,
            Box::new(stdout) as Box<dyn Read + Send>,
            stdout_limit,
        ),
        (
            1usize,
            Box::new(stderr) as Box<dyn Read + Send>,
            stderr_limit,
        ),
    ] {
        let sender = sender.clone();
        let overflow = Arc::clone(&overflow);
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let mut block = [0u8; 8192];
            let mut exceeded = false;
            loop {
                match pipe.read(&mut block) {
                    Ok(0) => break,
                    Ok(count) => {
                        if !exceeded
                            && bytes
                                .len()
                                .checked_add(count)
                                .is_some_and(|len| len <= limit)
                        {
                            bytes.extend_from_slice(&block[..count]);
                        } else {
                            exceeded = true;
                            overflow.store(true, Ordering::Relaxed);
                        }
                    }
                    Err(_) => {
                        overflow.store(true, Ordering::Relaxed);
                        break;
                    }
                }
            }
            let _ = sender.send((index, bytes));
        });
    }
    let input_failed = Arc::new(AtomicBool::new(false));
    let has_stdin = stdin.is_some();
    if let Some(input) = stdin {
        let mut pipe = child.stdin.take().ok_or("external R2 stdin unavailable")?;
        let sender = sender.clone();
        let failed = Arc::clone(&input_failed);
        thread::spawn(move || {
            if pipe.write_all(&input).is_err() {
                failed.store(true, Ordering::Relaxed);
            }
            drop(pipe);
            let _ = sender.send((2usize, Vec::new()));
        });
    }
    drop(sender);
    let mut streams: [Option<Vec<u8>>; 3] =
        [None, None, if has_stdin { None } else { Some(Vec::new()) }];
    let deadline = Instant::now() + timeout;
    let mut status = None;
    let mut failed = None;
    while status.is_none() || streams.iter().any(Option::is_none) {
        if Instant::now() >= deadline {
            failed = Some("external R2 tool timed out");
            break;
        }
        if overflow.load(Ordering::Relaxed) {
            failed = Some("external R2 tool exceeded output bound");
            break;
        }
        if input_failed.load(Ordering::Relaxed) {
            failed = Some("external R2 tool input was incomplete");
            break;
        }
        if status.is_none() {
            match child.try_wait() {
                Ok(observed) => status = observed,
                Err(_) => {
                    failed = Some("cannot observe external R2 tool");
                    break;
                }
            }
        }
        while let Ok((index, bytes)) = receiver.try_recv() {
            streams[index] = Some(bytes);
        }
        if status.is_none() || streams.iter().any(Option::is_none) {
            thread::sleep(Duration::from_millis(5));
        }
    }
    if let Some(reason) = failed {
        let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
        let _ = child.wait();
        return Err(reason.into());
    }
    let status = status.ok_or("external R2 tool status unavailable")?;
    Ok(ProcessOutput {
        success: status.success(),
        stdout: streams[0].take().ok_or("external R2 stdout missing")?,
        stderr: streams[1].take().ok_or("external R2 stderr missing")?,
    })
}

fn exact_keys(value: &Value, required: &[&str], optional: &[&str]) -> Result<(), String> {
    let object = value.as_object().ok_or("request must be an object")?;
    for key in required {
        if !object.contains_key(*key) {
            return Err(format!("request is missing {key}"));
        }
    }
    if object
        .keys()
        .any(|key| !required.contains(&key.as_str()) && !optional.contains(&key.as_str()))
    {
        return Err("request contains an unsupported field".into());
    }
    Ok(())
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{key} must be text"))
}

fn optional_text<'a>(value: &'a Value, key: &str) -> Result<Option<&'a str>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        _ => Err(format!("{key} must be text or null")),
    }
}

fn unsigned(value: &Value, key: &str) -> Result<u64, String> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{key} must be a nonnegative integer"))
}

fn absolute_path(value: &Value, key: &str) -> Result<PathBuf, String> {
    let raw = text(value, key)?;
    let path = PathBuf::from(raw);
    if !path.is_absolute() || path.to_str() != Some(raw) {
        return Err(format!("{key} must be an absolute UTF-8 path"));
    }
    Ok(path)
}

fn parse_limits(value: &Value) -> Result<ChunkedFileLimitsV1, String> {
    exact_keys(
        value,
        &[
            "max_file_bytes",
            "max_chunk_count",
            "max_manifest_bytes",
            "max_chunk_bytes",
        ],
        &[],
    )?;
    let max_chunk_count = usize::try_from(unsigned(value, "max_chunk_count")?)
        .map_err(|_| "max_chunk_count exceeds address space")?;
    let max_manifest_bytes = usize::try_from(unsigned(value, "max_manifest_bytes")?)
        .map_err(|_| "max_manifest_bytes exceeds address space")?;
    ChunkedFileLimitsV1 {
        max_file_bytes: unsigned(value, "max_file_bytes")?,
        max_chunk_count,
        max_manifest_bytes,
        max_chunk_bytes: unsigned(value, "max_chunk_bytes")?,
    }
    .validate()
    .map_err(|error| error.to_string())
}

fn build_transport(value: &Value) -> Result<R2TransportV1, String> {
    let kind = match text(value, "transport")? {
        "wrangler" => R2TransportKindV1::Wrangler,
        "rest" => R2TransportKindV1::Rest,
        _ => return Err("transport must be wrangler or rest".into()),
    };
    let timeout_seconds = match value.get("timeout_seconds") {
        None => DEFAULT_TIMEOUT_SECONDS,
        Some(value) => value
            .as_u64()
            .ok_or("timeout_seconds must be a nonnegative integer")?,
    };
    let cwd = optional_text(value, "cwd")?.map(PathBuf::from);
    if cwd.as_ref().is_some_and(|path| !path.is_absolute()) {
        return Err("cwd must be an absolute path".into());
    }
    let transport = R2TransportV1::new(
        kind,
        text(value, "bucket")?,
        optional_text(value, "account_id")?,
        &absolute_path(value, "wrangler")?,
        cwd.as_deref(),
        timeout_seconds,
    )?;
    match value.get("max_upload_bytes") {
        None => Ok(transport),
        Some(value) => transport.with_max_upload_bytes(
            value
                .as_u64()
                .ok_or("max_upload_bytes must be a nonnegative integer")?,
        ),
    }
}

/// Validate and execute one strict native R2 request.
pub fn invoke(request: &Value) -> Result<Value, String> {
    if text(request, "schema_version")? != "tos_corpus_r2_request_v1" {
        return Err("unsupported corpus-r2 request schema".into());
    }
    let operation = text(request, "operation")?;
    match operation {
        "raw-transfer" => {
            exact_keys(
                request,
                &[
                    "schema_version",
                    "operation",
                    "transport",
                    "bucket",
                    "wrangler",
                    "scratch_root",
                    "source_path",
                    "object_key",
                    "expected_byte_size",
                    "expected_sha256",
                    "media_type",
                ],
                &["account_id", "cwd", "timeout_seconds", "max_upload_bytes"],
            )?;
            let expected_sha256 = Digest256::from_hex(text(request, "expected_sha256")?)
                .map_err(|_| "expected_sha256 must be lowercase SHA-256")?;
            let mut transport = build_transport(request)?;
            let receipt = raw_transfer_v1(
                &mut transport,
                text(request, "object_key")?,
                &absolute_path(request, "source_path")?,
                unsigned(request, "expected_byte_size")?,
                expected_sha256,
                text(request, "media_type")?,
                &absolute_path(request, "scratch_root")?,
            )?;
            Ok(json!({
                "schema_version":"tos_corpus_r2_result_v1",
                "operation":"raw-transfer",
                "remote_status":receipt.remote_status,
                "upload_attempted":receipt.upload_attempted,
                "byte_size":receipt.byte_size,
                "sha256":receipt.sha256.to_hex(),
                "readback_verified":receipt.readback_verified,
            }))
        }
        "raw-read" => {
            exact_keys(
                request,
                &[
                    "schema_version",
                    "operation",
                    "transport",
                    "bucket",
                    "wrangler",
                    "scratch_root",
                    "output_path",
                    "object_key",
                    "expected_byte_size",
                    "expected_sha256",
                ],
                &["account_id", "cwd", "timeout_seconds", "max_upload_bytes"],
            )?;
            let expected_sha256 = Digest256::from_hex(text(request, "expected_sha256")?)
                .map_err(|_| "expected_sha256 must be lowercase SHA-256")?;
            let mut transport = build_transport(request)?;
            raw_read_verified_v1(
                &mut transport,
                text(request, "object_key")?,
                &absolute_path(request, "output_path")?,
                unsigned(request, "expected_byte_size")?,
                expected_sha256,
                &absolute_path(request, "scratch_root")?,
            )?;
            Ok(json!({
                "schema_version":"tos_corpus_r2_result_v1",
                "operation":"raw-read",
                "byte_size":unsigned(request, "expected_byte_size")?,
                "sha256":expected_sha256.to_hex(),
                "readback_verified":true,
            }))
        }
        "chunk-upload" => {
            exact_keys(
                request,
                &[
                    "schema_version",
                    "operation",
                    "transport",
                    "bucket",
                    "wrangler",
                    "scratch_root",
                    "source_path",
                    "prefix",
                    "expected_byte_size",
                    "expected_sha256",
                    "chunk_bytes",
                    "limits",
                ],
                &["account_id", "cwd", "timeout_seconds", "max_upload_bytes"],
            )?;
            let expected_sha256 = Digest256::from_hex(text(request, "expected_sha256")?)
                .map_err(|_| "expected_sha256 must be lowercase SHA-256")?;
            let limits = parse_limits(request.get("limits").ok_or("limits is required")?)?;
            let mut transport = build_transport(request)?;
            let receipt = upload_chunked_file_v1(
                &mut transport,
                &absolute_path(request, "source_path")?,
                text(request, "prefix")?,
                expected_sha256,
                unsigned(request, "expected_byte_size")?,
                &absolute_path(request, "scratch_root")?,
                unsigned(request, "chunk_bytes")?,
                limits,
            )
            .map_err(|error| error.to_string())?;
            Ok(json!({
                "schema_version":"tos_corpus_r2_result_v1",
                "operation":"chunk-upload",
                "manifest_key":receipt.manifest_key,
                "manifest_sha256":receipt.manifest_sha256.to_hex(),
                "file_sha256":receipt.file_sha256.to_hex(),
                "file_size_bytes":receipt.file_size_bytes,
                "chunk_count":receipt.chunk_count,
                "readback_verified":receipt.readback_verified,
            }))
        }
        "chunk-restore" => {
            exact_keys(
                request,
                &[
                    "schema_version",
                    "operation",
                    "transport",
                    "bucket",
                    "wrangler",
                    "scratch_root",
                    "output_path",
                    "manifest_key",
                    "expected_manifest_sha256",
                    "limits",
                ],
                &["account_id", "cwd", "timeout_seconds", "max_upload_bytes"],
            )?;
            let expected_manifest_sha256 =
                Digest256::from_hex(text(request, "expected_manifest_sha256")?)
                    .map_err(|_| "expected_manifest_sha256 must be lowercase SHA-256")?;
            let limits = parse_limits(request.get("limits").ok_or("limits is required")?)?;
            let mut transport = build_transport(request)?;
            let receipt = restore_chunked_file_v1(
                &mut transport,
                text(request, "manifest_key")?,
                expected_manifest_sha256,
                &absolute_path(request, "output_path")?,
                &absolute_path(request, "scratch_root")?,
                limits,
            )
            .map_err(|error| error.to_string())?;
            Ok(json!({
                "schema_version":"tos_corpus_r2_result_v1",
                "operation":"chunk-restore",
                "manifest_key":receipt.manifest_key,
                "manifest_sha256":receipt.manifest_sha256.to_hex(),
                "file_sha256":receipt.file_sha256.to_hex(),
                "file_size_bytes":receipt.file_size_bytes,
                "chunk_count":receipt.chunk_count,
                "readback_verified":receipt.readback_verified,
            }))
        }
        _ => Err("operation must be raw-transfer, raw-read, chunk-upload, or chunk-restore".into()),
    }
}

fn parse_request(raw: &[u8]) -> Result<Value, String> {
    let limits = JsonLimits {
        max_bytes: REQUEST_BYTES,
        ..JsonLimits::default()
    };
    let document = parse_json(raw, JsonMode::PublishedStrict, limits)
        .map_err(|_| "request is not strict UTF-8 JSON within the request bound")?;
    let canonical = canonical_bytes_v1(
        document.root(),
        CanonicalProfile::SourceCommandInputV1,
        limits,
    )
    .map_err(|_| "request cannot be canonically encoded")?;
    serde_json::from_slice(&canonical).map_err(|_| "request JSON could not be decoded")
}

pub fn run() -> i32 {
    let result = (|| {
        let mut raw = Vec::new();
        io::stdin()
            .lock()
            .take(REQUEST_BYTES as u64 + 1)
            .read_to_end(&mut raw)
            .map_err(|_| "cannot read corpus-r2 request")?;
        if raw.len() > REQUEST_BYTES {
            return Err("corpus-r2 request exceeds byte limit".to_owned());
        }
        let request = parse_request(&raw)?;
        invoke(&request)
    })();
    let mut output = io::stdout().lock();
    match result {
        Ok(value) => {
            let mut raw = match serde_json::to_vec(&value) {
                Ok(raw) => raw,
                Err(_) => {
                    eprintln!("native corpus-r2 refused: response encoding failed");
                    return 2;
                }
            };
            raw.push(b'\n');
            if raw.len() > RESPONSE_BYTES || output.write_all(&raw).is_err() {
                eprintln!("native corpus-r2 refused: output bound or write failure");
                return 2;
            }
            0
        }
        Err(reason) => {
            let _ = writeln!(io::stderr().lock(), "native corpus-r2 refused: {reason}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::os::unix::fs::PermissionsExt;

    struct MemoryTransport {
        objects: BTreeMap<String, Vec<u8>>,
        puts: usize,
    }

    impl ChunkedFileTransportV1 for MemoryTransport {
        fn fetch(&mut self, key: &str, destination: &Path, max_bytes: u64) -> io::Result<bool> {
            let Some(body) = self.objects.get(key) else {
                return Ok(false);
            };
            if body.len() as u64 > max_bytes {
                return Err(io::Error::other("limit"));
            }
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination)?
                .write_all(body)?;
            Ok(true)
        }

        fn put(
            &mut self,
            key: &str,
            source: &Path,
            byte_size: u64,
            media_type: &str,
            storage_class: &str,
        ) -> io::Result<()> {
            assert_eq!(regular_file_size(source).unwrap(), byte_size);
            assert_eq!(media_type, "application/octet-stream");
            assert_eq!(storage_class, "Standard");
            self.objects.insert(key.to_owned(), fs::read(source)?);
            self.puts += 1;
            Ok(())
        }
    }

    #[test]
    fn raw_transfer_refuses_conflicting_remote_and_verifies_new_readback() {
        let directory = std::env::temp_dir().join(format!("tos-r2-raw-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let scratch = directory.join("scratch");
        fs::create_dir_all(&scratch).unwrap();
        let source = directory.join("source.bin");
        fs::write(&source, b"raw bytes").unwrap();
        let digest = Digest256::of_bytes(b"raw bytes");
        let mut transport = MemoryTransport {
            objects: BTreeMap::new(),
            puts: 0,
        };
        let result = raw_transfer_v1(
            &mut transport,
            "blobs/sha256/aa",
            &source,
            9,
            digest,
            "application/octet-stream",
            &scratch,
        )
        .unwrap();
        assert_eq!(result.remote_status, "uploaded");
        assert!(result.upload_attempted && result.readback_verified);
        assert_eq!(transport.puts, 1);
        let matched = raw_transfer_v1(
            &mut transport,
            "blobs/sha256/aa",
            &source,
            9,
            digest,
            "application/octet-stream",
            &scratch,
        )
        .unwrap();
        assert_eq!(matched.remote_status, "already-matched");
        assert!(!matched.upload_attempted);
        assert_eq!(transport.puts, 1);
        transport
            .objects
            .insert("blobs/sha256/aa".into(), b"different".to_vec());
        assert!(
            raw_transfer_v1(
                &mut transport,
                "blobs/sha256/aa",
                &source,
                9,
                digest,
                "application/octet-stream",
                &scratch
            )
            .is_err()
        );
        assert_eq!(transport.puts, 1);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn raw_read_verifies_before_no_clobber_publication() {
        let directory = std::env::temp_dir().join(format!("tos-r2-read-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let scratch = directory.join("scratch");
        fs::create_dir_all(&scratch).unwrap();
        let output = directory.join("output.bin");
        let mut transport = MemoryTransport {
            objects: BTreeMap::from([("objects/a".into(), b"expected".to_vec())]),
            puts: 0,
        };
        raw_read_verified_v1(
            &mut transport,
            "objects/a",
            &output,
            8,
            Digest256::of_bytes(b"expected"),
            &scratch,
        )
        .unwrap();
        assert_eq!(fs::read(&output).unwrap(), b"expected");
        assert!(
            raw_read_verified_v1(
                &mut transport,
                "objects/a",
                &output,
                8,
                Digest256::of_bytes(b"expected"),
                &scratch
            )
            .is_err()
        );
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn cli_request_contract_refuses_unknown_fields_and_missing_rest_identity() {
        let unknown = json!({"schema_version":"tos_corpus_r2_request_v1","operation":"chunk-restore","transport":"wrangler","bucket":"valid-bucket","wrangler":"/usr/bin/wrangler","scratch_root":"/tmp/scratch","output_path":"/tmp/out","manifest_key":"files/sha256/a/manifest.json","expected_manifest_sha256":"a".repeat(64),"limits":{"max_file_bytes":1,"max_chunk_count":1,"max_manifest_bytes":512,"max_chunk_bytes":1},"credential":"secret"});
        assert!(invoke(&unknown).is_err());
        let rest = json!({"schema_version":"tos_corpus_r2_request_v1","operation":"chunk-restore","transport":"rest","bucket":"valid-bucket","wrangler":"/usr/bin/wrangler","scratch_root":"/tmp/scratch","output_path":"/tmp/out","manifest_key":"files/sha256/a/manifest.json","expected_manifest_sha256":"a".repeat(64),"limits":{"max_file_bytes":1,"max_chunk_count":1,"max_manifest_bytes":512,"max_chunk_bytes":1}});
        assert!(invoke(&rest).unwrap_err().contains("account_id"));
    }

    #[test]
    fn transport_selection_validates_bucket_account_and_executable() {
        assert!(
            R2TransportV1::new(
                R2TransportKindV1::Wrangler,
                "Bad Bucket",
                None,
                Path::new("/usr/bin/wrangler"),
                None,
                120
            )
            .is_err()
        );
        assert!(
            R2TransportV1::new(
                R2TransportKindV1::Rest,
                "valid-bucket",
                Some("short"),
                Path::new("/usr/bin/wrangler"),
                None,
                120
            )
            .is_err()
        );
        assert!(
            R2TransportV1::new(
                R2TransportKindV1::Wrangler,
                "valid-bucket",
                None,
                Path::new("wrangler"),
                None,
                120
            )
            .is_err()
        );
        assert!(
            R2TransportV1::new(
                R2TransportKindV1::Wrangler,
                "valid-bucket",
                None,
                Path::new("/usr/bin/wrangler"),
                None,
                120,
            )
            .unwrap()
            .with_max_upload_bytes(MAX_OBJECT_BYTES + 1)
            .is_err()
        );
    }

    #[test]
    fn selected_upload_limit_refuses_before_source_or_transport_io() {
        let mut transport = R2TransportV1::new(
            R2TransportKindV1::Wrangler,
            "valid-bucket",
            None,
            Path::new("/usr/bin/wrangler"),
            None,
            120,
        )
        .unwrap()
        .with_max_upload_bytes(2)
        .unwrap();
        assert_eq!(
            transport
                .put_inner(
                    "objects/a",
                    Path::new("/does/not/exist"),
                    3,
                    "application/octet-stream",
                    "Standard",
                )
                .unwrap_err(),
            "R2 upload exceeds selected per-object byte limit"
        );
    }

    #[test]
    fn auth_failure_does_not_expose_wrangler_output() {
        let sequence = SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let directory =
            std::env::temp_dir().join(format!("tos-r2-auth-{}-{sequence}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let executable = directory.join("wrangler");
        fs::write(
            &executable,
            b"#!/bin/sh\nprintf '%s' 'private-token-output'\nprintf '%s' 'private-token-stderr' >&2\nexit 1\n",
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let account_id = "a".repeat(32);
        let mut transport = R2TransportV1::new(
            R2TransportKindV1::Rest,
            "valid-bucket",
            Some(&account_id),
            &executable,
            None,
            1,
        )
        .unwrap();
        let error = transport.auth_token().unwrap_err();
        assert_eq!(error, "Wrangler auth token command failed");
        assert!(!error.contains("private-token-output"));
        assert!(!error.contains("private-token-stderr"));
        let _ = fs::remove_dir_all(directory);
    }
}
