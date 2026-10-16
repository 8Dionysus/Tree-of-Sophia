//! Bounded, read-back verified chunk transport for large files.
//!
//! The owner supplies the remote byte transport. This layer owns only exact
//! chunk manifests, local file fixity, and read-back verification; it grants no
//! rights, admission, or publication authority.

use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonNumber, JsonNumberKind,
    JsonString, JsonValue, canonical_bytes_v1, parse_json,
};

use crate::error::{Result, StoreError, StoreErrorCode as Code};

pub const CHUNKED_FILE_SCHEMA_V1: &str = "tos_chunked_file_v1";
pub const CHUNKED_FILE_MAX_CHUNK_BYTES_V1: u64 = 64 * 1024 * 1024;
const READ_BLOCK_BYTES: usize = 1024 * 1024;
const PART_INDEX_WIDTH: usize = 8;
const HASH_PREFIX: &str = "files/sha256/";
static SCRATCH_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Explicit finite working limits for one chunked transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChunkedFileLimitsV1 {
    pub max_file_bytes: u64,
    pub max_chunk_count: usize,
    pub max_manifest_bytes: usize,
    pub max_chunk_bytes: u64,
}

impl ChunkedFileLimitsV1 {
    pub fn validate(self) -> Result<Self> {
        if self.max_file_bytes == 0
            || self.max_file_bytes == u64::MAX
            || self.max_chunk_count == 0
            || self.max_chunk_count == usize::MAX
            || self.max_manifest_bytes < 512
            || self.max_manifest_bytes == usize::MAX
            || self.max_chunk_bytes == 0
            || self.max_chunk_bytes > CHUNKED_FILE_MAX_CHUNK_BYTES_V1
        {
            return Err(StoreError::new(
                Code::BudgetExceeded,
                "invalid chunked-file limits",
            ));
        }
        Ok(self)
    }
}

/// Minimal remote byte transport required by the integrity protocol.
///
/// Implementations must write `fetch` results only to a fresh destination.
/// `fetch` must refuse an object larger than the requested byte ceiling before
/// writing beyond that ceiling.
/// Errors are intentionally collapsed to safe owner-local refusals by this
/// module so credential-bearing transport diagnostics cannot escape.
pub trait ChunkedFileTransportV1 {
    fn fetch(&mut self, key: &str, destination: &Path, max_bytes: u64) -> io::Result<bool>;
    fn put(
        &mut self,
        key: &str,
        source: &Path,
        byte_size: u64,
        media_type: &str,
        storage_class: &str,
    ) -> io::Result<()>;
}

/// Result of a completed upload or restore. `readback_verified` is true only
/// after every reused or newly written remote object was fetched and hashed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChunkedFileReceiptV1 {
    pub manifest_key: String,
    pub manifest_sha256: Digest256,
    pub file_sha256: Digest256,
    pub file_size_bytes: u64,
    pub chunk_count: usize,
    pub readback_verified: bool,
    pub output: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Fingerprint {
    device: u64,
    inode: u64,
    file_type: u32,
    links: u64,
    uid: u32,
    gid: u32,
    size: u64,
    mtime: (i64, i64),
    ctime: (i64, i64),
}

fn error(code: Code, detail: &'static str) -> StoreError {
    StoreError::new(code, detail)
}

fn io_error(detail: &'static str, source: io::Error) -> StoreError {
    StoreError::io(detail, source)
}

fn fingerprint(metadata: &Metadata) -> Fingerprint {
    Fingerprint {
        device: metadata.dev(),
        inode: metadata.ino(),
        file_type: metadata.mode() & 0o170000,
        links: metadata.nlink(),
        uid: metadata.uid(),
        gid: metadata.gid(),
        size: metadata.len(),
        mtime: (metadata.mtime(), metadata.mtime_nsec()),
        ctime: (metadata.ctime(), metadata.ctime_nsec()),
    }
}

fn regular_path(path: &Path, detail: &'static str) -> Result<(Metadata, Fingerprint)> {
    let metadata = fs::symlink_metadata(path).map_err(|source| io_error(detail, source))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(error(
            Code::UnsafePath,
            "selected file must be regular and not a symlink",
        ));
    }
    let stamp = fingerprint(&metadata);
    Ok((metadata, stamp))
}

fn open_regular(path: &Path, expected: Fingerprint, detail: &'static str) -> Result<File> {
    let file = File::open(path).map_err(|source| io_error(detail, source))?;
    let metadata = file.metadata().map_err(|source| io_error(detail, source))?;
    if !metadata.file_type().is_file() || fingerprint(&metadata) != expected {
        return Err(error(
            Code::DescriptorMismatch,
            "selected file changed before read",
        ));
    }
    Ok(file)
}

fn ensure_unchanged(path: &Path, expected: Fingerprint) -> Result<()> {
    let (_, current) = regular_path(path, "selected file disappeared during transfer")?;
    if current != expected {
        return Err(error(
            Code::DescriptorMismatch,
            "selected file changed during transfer",
        ));
    }
    Ok(())
}

fn digest_regular_file(path: &Path, expected_size: u64, expected_digest: Digest256) -> Result<()> {
    let (_, before) = regular_path(path, "transport read-back is unavailable")?;
    if before.size != expected_size {
        return Err(error(
            Code::CorruptSelectedObject,
            "transport read-back size mismatch",
        ));
    }
    let mut file = open_regular(path, before, "cannot open transport read-back")?;
    let mut digest = Digest256Hasher::new();
    let mut total = 0u64;
    let mut buffer = vec![0u8; READ_BLOCK_BYTES];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|source| io_error("cannot read transport read-back", source))?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or_else(|| error(Code::BudgetExceeded, "transport read-back size overflow"))?;
        if total > expected_size {
            return Err(error(
                Code::CorruptSelectedObject,
                "transport read-back grew during hash",
            ));
        }
        digest.update(&buffer[..count]);
    }
    let opened_after = file
        .metadata()
        .map_err(|source| io_error("cannot restat transport read-back", source))?;
    let (_, path_after) = regular_path(path, "transport read-back disappeared")?;
    if fingerprint(&opened_after) != before || path_after != before {
        return Err(error(
            Code::DescriptorMismatch,
            "transport read-back changed while verified",
        ));
    }
    if total != expected_size || digest.finalize() != expected_digest {
        return Err(error(
            Code::CorruptSelectedObject,
            "transport read-back digest mismatch",
        ));
    }
    Ok(())
}

fn validate_prefix(prefix: &str) -> Result<()> {
    if prefix.is_empty() {
        return Ok(());
    }
    if prefix.starts_with('/')
        || prefix.contains('\\')
        || prefix.contains('\0')
        || prefix.chars().any(|ch| ch <= '\u{1f}' || ch == '\u{7f}')
        || prefix
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || (prefix
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_alphabetic())
            && prefix.as_bytes().get(1) == Some(&b':'))
    {
        return Err(error(Code::UnsafePath, "unsafe relative object-key prefix"));
    }
    Ok(())
}

fn validate_key(key: &str) -> Result<()> {
    if key.is_empty()
        || key.starts_with('/')
        || key.contains('\\')
        || key.contains('\0')
        || key.chars().any(|ch| ch <= '\u{1f}' || ch == '\u{7f}')
        || key
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(error(Code::UnsafePath, "unsafe object key"));
    }
    Ok(())
}

fn prefix_part(prefix: &str) -> String {
    if prefix.is_empty() {
        String::new()
    } else {
        format!("{prefix}/")
    }
}

fn part_key(prefix: &str, file_sha256: &str, index: usize, part_sha256: &str) -> String {
    format!(
        "{}{}/parts/{:0width$}-{part_sha256}",
        prefix_part(prefix),
        manifest_file_prefix(file_sha256),
        index,
        width = PART_INDEX_WIDTH,
    )
}

fn manifest_key(prefix: &str, file_sha256: &str) -> String {
    format!(
        "{}{}/manifest.json",
        prefix_part(prefix),
        manifest_file_prefix(file_sha256)
    )
}

fn manifest_file_prefix(file_sha256: &str) -> String {
    format!("files/sha256/{file_sha256}")
}

fn number(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: value.to_string(),
    })
}

fn string(value: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(value))
}

fn object(values: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        values
            .into_iter()
            .map(|(key, value)| (JsonString::from_utf8(key), value))
            .collect(),
    )
}

fn array(values: Vec<JsonValue>) -> JsonValue {
    JsonValue::Array(values)
}

fn canonical(value: &JsonValue, max_bytes: usize) -> Result<Vec<u8>> {
    canonical_bytes_v1(
        value,
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits::new(max_bytes, 16, max_bytes.saturating_mul(4).max(64), 20)
            .map_err(|_| error(Code::BudgetExceeded, "invalid chunked-file JSON limits"))?,
    )
    .map_err(|_| {
        error(
            Code::InvalidCanonicalSnapshot,
            "chunked-file manifest is not canonical",
        )
    })
}

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn create(root: &Path) -> Result<Self> {
        fs::create_dir_all(root)
            .map_err(|source| io_error("cannot prepare transfer scratch root", source))?;
        let metadata = fs::symlink_metadata(root)
            .map_err(|source| io_error("cannot inspect transfer scratch root", source))?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
            return Err(error(
                Code::UnsafePath,
                "transfer scratch root must be a real directory",
            ));
        }
        for _ in 0..128 {
            let sequence = SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = root.join(format!(".tos-chunked-{}-{sequence}", std::process::id()));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(source) if source.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(source) => {
                    return Err(io_error("cannot create transfer scratch directory", source));
                }
            }
        }
        Err(error(Code::Io, "cannot reserve transfer scratch directory"))
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn create_new(path: &Path) -> Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| io_error("cannot create transfer file exclusively", source))
}

fn fetch_new<T: ChunkedFileTransportV1>(
    transport: &mut T,
    key: &str,
    path: &Path,
    max_bytes: u64,
) -> Result<bool> {
    validate_key(key)?;
    if fs::symlink_metadata(path).is_ok() {
        return Err(error(
            Code::UnsafePath,
            "transport destination must be fresh",
        ));
    }
    let exists = transport
        .fetch(key, path, max_bytes)
        .map_err(|_| error(Code::Io, "remote object fetch failed"))?;
    if !exists && fs::symlink_metadata(path).is_ok() {
        return Err(error(
            Code::DescriptorMismatch,
            "transport created a path for absent object",
        ));
    }
    Ok(exists)
}

fn put_checked<T: ChunkedFileTransportV1>(
    transport: &mut T,
    key: &str,
    path: &Path,
    byte_size: u64,
    media_type: &str,
) -> Result<()> {
    validate_key(key)?;
    let (metadata, _) = regular_path(path, "local upload part is unavailable")?;
    if metadata.len() != byte_size {
        return Err(error(
            Code::DescriptorMismatch,
            "local upload part size changed",
        ));
    }
    transport
        .put(key, path, byte_size, media_type, "Standard")
        .map_err(|_| error(Code::Io, "remote object put failed"))
}

fn ensure_remote_object<T: ChunkedFileTransportV1>(
    transport: &mut T,
    key: &str,
    source: &Path,
    byte_size: u64,
    digest: Digest256,
    media_type: &str,
    scratch: &Path,
    label: &str,
) -> Result<()> {
    let existing = scratch.join(format!("{label}.existing"));
    if fetch_new(transport, key, &existing, byte_size)? {
        let result = digest_regular_file(&existing, byte_size, digest);
        let _ = fs::remove_file(&existing);
        return result;
    }
    put_checked(transport, key, source, byte_size, media_type)?;
    let readback = scratch.join(format!("{label}.readback"));
    if !fetch_new(transport, key, &readback, byte_size)? {
        return Err(error(
            Code::MissingMember,
            "remote object absent immediately after put",
        ));
    }
    let result = digest_regular_file(&readback, byte_size, digest);
    let _ = fs::remove_file(&readback);
    result
}

fn expected_chunks(file_size: u64, chunk_bytes: u64) -> Result<usize> {
    let count = file_size / chunk_bytes + (file_size % chunk_bytes != 0) as u64;
    usize::try_from(count)
        .map_err(|_| error(Code::BudgetExceeded, "chunk count exceeds address space"))
}

/// Upload a regular file in content-addressed chunks and publish its manifest last.
pub fn upload_chunked_file_v1<T: ChunkedFileTransportV1>(
    transport: &mut T,
    source: &Path,
    prefix: &str,
    expected_sha256: Digest256,
    expected_size: u64,
    scratch_root: &Path,
    chunk_bytes: u64,
    limits: ChunkedFileLimitsV1,
) -> Result<ChunkedFileReceiptV1> {
    let limits = limits.validate()?;
    validate_prefix(prefix)?;
    if chunk_bytes == 0 || chunk_bytes > limits.max_chunk_bytes {
        return Err(error(
            Code::BudgetExceeded,
            "chunk_bytes exceeds the selected finite limit",
        ));
    }
    if expected_size > limits.max_file_bytes {
        return Err(error(
            Code::BudgetExceeded,
            "selected file exceeds byte budget",
        ));
    }
    let count = expected_chunks(expected_size, chunk_bytes)?;
    if count > limits.max_chunk_count {
        return Err(error(
            Code::BudgetExceeded,
            "selected file exceeds chunk-count budget",
        ));
    }
    let (_, baseline) = regular_path(source, "source file is unavailable")?;
    if baseline.size != expected_size {
        return Err(error(
            Code::DescriptorMismatch,
            "source size differs from expected size",
        ));
    }
    let mut first = open_regular(source, baseline, "cannot open selected source")?;
    let mut first_digest = Digest256Hasher::new();
    let mut first_total = 0u64;
    let mut buffer = vec![0u8; READ_BLOCK_BYTES];
    loop {
        let read = first
            .read(&mut buffer)
            .map_err(|source| io_error("cannot hash selected source", source))?;
        if read == 0 {
            break;
        }
        first_total = first_total
            .checked_add(read as u64)
            .ok_or_else(|| error(Code::BudgetExceeded, "source size overflow"))?;
        if first_total > expected_size {
            return Err(error(
                Code::DescriptorMismatch,
                "source grew during initial hash",
            ));
        }
        first_digest.update(&buffer[..read]);
    }
    if first_total != expected_size || first_digest.finalize() != expected_sha256 {
        return Err(error(
            Code::CorruptSelectedObject,
            "source does not match expected digest and size",
        ));
    }
    ensure_unchanged(source, baseline)?;

    let scratch = ScratchDirectory::create(scratch_root)?;
    let manifest_key = manifest_key(prefix, &expected_sha256.to_hex());
    let mut chunks = Vec::with_capacity(count);
    let mut full_digest = Digest256Hasher::new();
    let mut second_total = 0u64;
    let mut second = open_regular(source, baseline, "cannot reopen selected source")?;
    for index in 0..count {
        let part_path = scratch.0.join(format!(
            "part-{:0width$}.bin",
            index,
            width = PART_INDEX_WIDTH
        ));
        let mut part = create_new(&part_path)?;
        let expected_part_size = chunk_bytes.min(expected_size - second_total);
        let mut part_digest = Digest256Hasher::new();
        let mut part_total = 0u64;
        while part_total < expected_part_size {
            let remaining = expected_part_size - part_total;
            let read = second
                .read(&mut buffer[..(remaining.min(READ_BLOCK_BYTES as u64) as usize)])
                .map_err(|source| io_error("cannot read selected source chunk", source))?;
            if read == 0 {
                break;
            }
            part.write_all(&buffer[..read])
                .map_err(|source| io_error("cannot write local source chunk", source))?;
            part_digest.update(&buffer[..read]);
            full_digest.update(&buffer[..read]);
            part_total += read as u64;
            second_total += read as u64;
        }
        part.flush()
            .map_err(|source| io_error("cannot flush local source chunk", source))?;
        part.sync_all()
            .map_err(|source| io_error("cannot sync local source chunk", source))?;
        drop(part);
        if part_total != expected_part_size {
            return Err(error(
                Code::DescriptorMismatch,
                "source shortened while chunks were prepared",
            ));
        }
        let part_sha256 = part_digest.finalize();
        let key = part_key(
            prefix,
            &expected_sha256.to_hex(),
            index,
            &part_sha256.to_hex(),
        );
        ensure_remote_object(
            transport,
            &key,
            &part_path,
            part_total,
            part_sha256,
            "application/octet-stream",
            &scratch.0,
            &format!("part-{:0width$}", index, width = PART_INDEX_WIDTH),
        )?;
        fs::remove_file(&part_path)
            .map_err(|source| io_error("cannot remove local source chunk", source))?;
        chunks.push(object(vec![
            ("index", number(index as u64)),
            ("offset", number(second_total - part_total)),
            ("size_bytes", number(part_total)),
            ("sha256", string(&part_sha256.to_hex())),
            ("key", string(&key)),
        ]));
    }
    if second_total != expected_size || full_digest.finalize() != expected_sha256 {
        return Err(error(
            Code::DescriptorMismatch,
            "source changed while chunks were prepared",
        ));
    }
    ensure_unchanged(source, baseline)?;
    let manifest = object(vec![
        ("schema_version", string(CHUNKED_FILE_SCHEMA_V1)),
        ("file_sha256", string(&expected_sha256.to_hex())),
        ("file_size_bytes", number(expected_size)),
        ("chunk_bytes", number(chunk_bytes)),
        ("chunks", array(chunks)),
    ]);
    let manifest_bytes = canonical(&manifest, limits.max_manifest_bytes)?;
    let manifest_sha256 = Digest256::of_bytes(&manifest_bytes);
    let manifest_path = scratch.0.join("manifest.json");
    let mut file = create_new(&manifest_path)?;
    file.write_all(&manifest_bytes)
        .map_err(|source| io_error("cannot write local chunk manifest", source))?;
    file.flush()
        .map_err(|source| io_error("cannot flush local chunk manifest", source))?;
    file.sync_all()
        .map_err(|source| io_error("cannot sync local chunk manifest", source))?;
    drop(file);
    ensure_unchanged(source, baseline)?;
    ensure_remote_object(
        transport,
        &manifest_key,
        &manifest_path,
        manifest_bytes.len() as u64,
        manifest_sha256,
        "application/json",
        &scratch.0,
        "manifest",
    )?;
    ensure_unchanged(source, baseline)?;
    Ok(ChunkedFileReceiptV1 {
        manifest_key,
        manifest_sha256,
        file_sha256: expected_sha256,
        file_size_bytes: expected_size,
        chunk_count: count,
        readback_verified: true,
        output: None,
    })
}

fn manifest_location(key: &str) -> Result<(String, String)> {
    validate_key(key)?;
    let (prefix, remainder) = if let Some(remainder) = key.strip_prefix(HASH_PREFIX) {
        (String::new(), remainder)
    } else {
        let marker = key
            .rfind(&format!("/{HASH_PREFIX}"))
            .filter(|marker| *marker > 0)
            .ok_or_else(|| {
                error(
                    Code::InvalidMemberIndex,
                    "manifest key lacks files/sha256 path",
                )
            })?;
        let prefix = key[..marker].to_owned();
        validate_prefix(&prefix)?;
        (prefix, &key[marker + 1 + HASH_PREFIX.len()..])
    };
    let Some((digest, suffix)) = remainder.split_once("/manifest.json") else {
        return Err(error(
            Code::InvalidMemberIndex,
            "manifest key has invalid digest suffix",
        ));
    };
    if !suffix.is_empty()
        || digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(error(
            Code::InvalidMemberIndex,
            "manifest key has invalid digest suffix",
        ));
    }
    let expected = manifest_key(&prefix, digest);
    if expected != key {
        return Err(error(
            Code::InvalidMemberIndex,
            "manifest key is not canonical",
        ));
    }
    Ok((prefix, digest.to_owned()))
}

fn field<'a>(value: &'a JsonValue, name: &str) -> Result<&'a JsonValue> {
    value.object_get(name).ok_or_else(|| {
        error(
            Code::InvalidCanonicalSnapshot,
            "chunk manifest field is missing",
        )
    })
}

fn text_field<'a>(value: &'a JsonValue, name: &str) -> Result<&'a str> {
    field(value, name)?.as_str().ok_or_else(|| {
        error(
            Code::InvalidCanonicalSnapshot,
            "chunk manifest text field is invalid",
        )
    })
}

fn uint_field(value: &JsonValue, name: &str) -> Result<u64> {
    field(value, name)?.as_u64().ok_or_else(|| {
        error(
            Code::InvalidCanonicalSnapshot,
            "chunk manifest integer field is invalid",
        )
    })
}

fn exact_fields(value: &JsonValue, names: &[&str]) -> bool {
    value.as_object().is_some_and(|entries| {
        entries.len() == names.len() && names.iter().all(|name| value.object_get(name).is_some())
    })
}

fn parse_manifest(
    raw: &[u8],
    expected_digest: Digest256,
    limits: ChunkedFileLimitsV1,
) -> Result<JsonValue> {
    if raw.len() > limits.max_manifest_bytes {
        return Err(error(
            Code::BudgetExceeded,
            "remote chunk manifest exceeds byte limit",
        ));
    }
    if Digest256::of_bytes(raw) != expected_digest {
        return Err(error(
            Code::CorruptSelectedObject,
            "remote manifest digest mismatch",
        ));
    }
    let json_limits = JsonLimits::new(
        limits.max_manifest_bytes,
        16,
        limits.max_chunk_count.saturating_mul(16).max(128),
        20,
    )
    .map_err(|_| error(Code::BudgetExceeded, "invalid chunk manifest parser limits"))?;
    let document = parse_json(raw, JsonMode::PublishedStrict, json_limits).map_err(|_| {
        error(
            Code::InvalidCanonicalSnapshot,
            "manifest is not strict UTF-8 JSON",
        )
    })?;
    let value = document.root().clone();
    if !matches!(&value, JsonValue::Object(_))
        || canonical(&value, limits.max_manifest_bytes)? != raw
    {
        return Err(error(
            Code::InvalidCanonicalSnapshot,
            "manifest is not canonical JSON object",
        ));
    }
    Ok(value)
}

fn validate_manifest(
    value: &JsonValue,
    prefix: &str,
    path_file_sha256: &str,
    limits: ChunkedFileLimitsV1,
) -> Result<(Digest256, u64, Vec<(u64, Digest256, String)>)> {
    if !exact_fields(
        value,
        &[
            "schema_version",
            "file_sha256",
            "file_size_bytes",
            "chunk_bytes",
            "chunks",
        ],
    ) || text_field(value, "schema_version")? != CHUNKED_FILE_SCHEMA_V1
    {
        return Err(error(
            Code::UnsupportedFormat,
            "chunk manifest schema or fields differ",
        ));
    }
    let file_sha = text_field(value, "file_sha256")?;
    let file_sha256 = Digest256::from_hex(file_sha)
        .map_err(|_| error(Code::InvalidMemberIndex, "invalid manifest file digest"))?;
    if file_sha != path_file_sha256 {
        return Err(error(
            Code::DescriptorMismatch,
            "manifest digest is not bound to its key",
        ));
    }
    let file_size = uint_field(value, "file_size_bytes")?;
    if file_size > limits.max_file_bytes {
        return Err(error(
            Code::BudgetExceeded,
            "manifest file exceeds byte limit",
        ));
    }
    let chunk_bytes = uint_field(value, "chunk_bytes")?;
    if chunk_bytes == 0 || chunk_bytes > limits.max_chunk_bytes {
        return Err(error(
            Code::BudgetExceeded,
            "manifest chunk_bytes exceeds selected limit",
        ));
    }
    let count = expected_chunks(file_size, chunk_bytes)?;
    if count > limits.max_chunk_count {
        return Err(error(
            Code::BudgetExceeded,
            "manifest exceeds chunk-count limit",
        ));
    }
    let rows = field(value, "chunks")?
        .as_array()
        .ok_or_else(|| error(Code::InvalidMemberIndex, "manifest chunks must be an array"))?;
    if rows.len() != count {
        return Err(error(
            Code::InvalidMemberIndex,
            "manifest chunk count differs from file size",
        ));
    }
    let mut offset = 0u64;
    let mut chunks = Vec::with_capacity(count);
    for (index, row) in rows.iter().enumerate() {
        if !exact_fields(row, &["index", "offset", "size_bytes", "sha256", "key"]) {
            return Err(error(
                Code::InvalidMemberIndex,
                "manifest chunk fields differ",
            ));
        }
        if uint_field(row, "index")? != index as u64 || uint_field(row, "offset")? != offset {
            return Err(error(
                Code::InvalidMemberIndex,
                "manifest chunk ordering or offset differs",
            ));
        }
        let size = uint_field(row, "size_bytes")?;
        let expected_size = chunk_bytes.min(file_size - offset);
        if size == 0 || size != expected_size {
            return Err(error(
                Code::InvalidMemberIndex,
                "manifest chunk size differs",
            ));
        }
        let digest_text = text_field(row, "sha256")?;
        let digest = Digest256::from_hex(digest_text)
            .map_err(|_| error(Code::InvalidMemberIndex, "invalid part digest"))?;
        let key = text_field(row, "key")?;
        if key != part_key(prefix, file_sha, index, digest_text) {
            return Err(error(
                Code::DescriptorMismatch,
                "manifest part key is not bound to its digest",
            ));
        }
        chunks.push((size, digest, key.to_owned()));
        offset = offset
            .checked_add(size)
            .ok_or_else(|| error(Code::BudgetExceeded, "manifest chunk offsets overflow"))?;
    }
    if offset != file_size {
        return Err(error(
            Code::InvalidMemberIndex,
            "manifest chunk sizes do not sum to file size",
        ));
    }
    Ok((file_sha256, file_size, chunks))
}

/// Restore a chunked file only after verifying the manifest, every part, and
/// the complete assembled digest. The output is published by an exclusive
/// hardlink and an existing file or symlink is never replaced.
pub fn restore_chunked_file_v1<T: ChunkedFileTransportV1>(
    transport: &mut T,
    manifest_key: &str,
    expected_manifest_sha256: Digest256,
    output: &Path,
    scratch_root: &Path,
    limits: ChunkedFileLimitsV1,
) -> Result<ChunkedFileReceiptV1> {
    let limits = limits.validate()?;
    let (prefix, path_file_sha256) = manifest_location(manifest_key)?;
    let scratch = ScratchDirectory::create(scratch_root)?;
    let manifest_path = scratch.0.join("manifest.remote");
    if output.as_os_str().is_empty() || fs::symlink_metadata(output).is_ok() {
        return Err(error(Code::UnsafePath, "restore output already exists"));
    }
    if !fetch_new(
        transport,
        manifest_key,
        &manifest_path,
        limits.max_manifest_bytes as u64,
    )? {
        return Err(error(Code::MissingMember, "remote manifest is absent"));
    }
    let (manifest_metadata, _) = regular_path(&manifest_path, "remote manifest is unavailable")?;
    if manifest_metadata.len() > limits.max_manifest_bytes as u64 {
        return Err(error(
            Code::BudgetExceeded,
            "remote chunk manifest exceeds byte limit",
        ));
    }
    let manifest_size = manifest_metadata.len();
    let mut raw = Vec::with_capacity(manifest_size as usize);
    let mut file = File::open(&manifest_path)
        .map_err(|source| io_error("cannot read remote manifest", source))?;
    file.take(limits.max_manifest_bytes as u64 + 1)
        .read_to_end(&mut raw)
        .map_err(|source| io_error("cannot read remote manifest", source))?;
    if raw.len() as u64 != manifest_size {
        return Err(error(
            Code::DescriptorMismatch,
            "remote manifest changed while read",
        ));
    }
    let manifest = parse_manifest(&raw, expected_manifest_sha256, limits)?;
    let (file_sha256, file_size, chunks) =
        validate_manifest(&manifest, &prefix, &path_file_sha256, limits)?;

    let assembled_path = scratch.0.join("assembled.bin");
    let mut assembled = create_new(&assembled_path)?;
    let mut total = 0u64;
    let mut full_digest = Digest256Hasher::new();
    let mut buffer = vec![0u8; READ_BLOCK_BYTES];
    for (index, (size, digest, key)) in chunks.iter().enumerate() {
        let part_path = scratch.0.join(format!(
            "part-{:0width$}.remote",
            index,
            width = PART_INDEX_WIDTH
        ));
        if !fetch_new(transport, key, &part_path, *size)? {
            return Err(error(Code::MissingMember, "manifest part object is absent"));
        }
        digest_regular_file(&part_path, *size, *digest)?;
        let mut part =
            File::open(&part_path).map_err(|source| io_error("cannot open remote part", source))?;
        loop {
            let read = part
                .read(&mut buffer)
                .map_err(|source| io_error("cannot read remote part", source))?;
            if read == 0 {
                break;
            }
            assembled
                .write_all(&buffer[..read])
                .map_err(|source| io_error("cannot assemble restored file", source))?;
            full_digest.update(&buffer[..read]);
            total = total
                .checked_add(read as u64)
                .ok_or_else(|| error(Code::BudgetExceeded, "restored file size overflow"))?;
        }
        fs::remove_file(&part_path)
            .map_err(|source| io_error("cannot remove restored part", source))?;
    }
    assembled
        .flush()
        .map_err(|source| io_error("cannot flush restored file", source))?;
    assembled
        .sync_all()
        .map_err(|source| io_error("cannot sync restored file", source))?;
    drop(assembled);
    if total != file_size || full_digest.finalize() != file_sha256 {
        return Err(error(
            Code::CorruptSelectedObject,
            "assembled file digest or size mismatch",
        ));
    }
    let parent = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|source| io_error("cannot prepare restore output parent", source))?;
    if fs::symlink_metadata(output).is_ok() {
        return Err(error(Code::UnsafePath, "restore output already exists"));
    }
    fs::hard_link(&assembled_path, output).map_err(|source| {
        if source.kind() == io::ErrorKind::AlreadyExists {
            error(Code::UnsafePath, "restore output appeared during transfer")
        } else {
            io_error("cannot publish restored file exclusively", source)
        }
    })?;
    Ok(ChunkedFileReceiptV1 {
        manifest_key: manifest_key.to_owned(),
        manifest_sha256: expected_manifest_sha256,
        file_sha256,
        file_size_bytes: file_size,
        chunk_count: chunks.len(),
        readback_verified: true,
        output: Some(output.to_owned()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};
    use std::os::unix::fs::symlink;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            for _ in 0..128 {
                let sequence = SCRATCH_SEQUENCE.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "tos-chunked-file-test-{}-{sequence}",
                    std::process::id()
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(source) if source.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(source) => panic!("cannot create test directory: {source}"),
                }
            }
            panic!("cannot reserve test directory")
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[derive(Default)]
    struct MemoryTransport {
        objects: BTreeMap<String, Vec<u8>>,
        fetches: Vec<String>,
        puts: Vec<String>,
        hidden_after_put: BTreeSet<String>,
        mutate_source_on_put: Option<PathBuf>,
    }

    impl ChunkedFileTransportV1 for MemoryTransport {
        fn fetch(&mut self, key: &str, destination: &Path, max_bytes: u64) -> io::Result<bool> {
            self.fetches.push(key.to_owned());
            if self.hidden_after_put.contains(key) && self.puts.iter().any(|put| put == key) {
                return Ok(false);
            }
            let Some(raw) = self.objects.get(key) else {
                return Ok(false);
            };
            if raw.len() as u64 > max_bytes {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "object exceeds requested byte limit",
                ));
            }
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination)?;
            file.write_all(raw)?;
            Ok(true)
        }

        fn put(
            &mut self,
            key: &str,
            source: &Path,
            byte_size: u64,
            _media_type: &str,
            storage_class: &str,
        ) -> io::Result<()> {
            if storage_class != "Standard" {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "storage class"));
            }
            let raw = fs::read(source)?;
            if raw.len() as u64 != byte_size {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "size"));
            }
            self.objects.insert(key.to_owned(), raw);
            self.puts.push(key.to_owned());
            if let Some(path) = self.mutate_source_on_put.take() {
                OpenOptions::new()
                    .append(true)
                    .open(path)?
                    .write_all(b"x")?;
            }
            Ok(())
        }
    }

    fn limits() -> ChunkedFileLimitsV1 {
        ChunkedFileLimitsV1 {
            max_file_bytes: 1_048_576,
            max_chunk_count: 64,
            max_manifest_bytes: 32_768,
            max_chunk_bytes: 64,
        }
    }

    fn write_source(root: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let path = root.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }

    fn upload(
        root: &Path,
        transport: &mut MemoryTransport,
        bytes: &[u8],
        prefix: &str,
        chunk_bytes: u64,
    ) -> ChunkedFileReceiptV1 {
        let source = write_source(root, "source.bin", bytes);
        upload_chunked_file_v1(
            transport,
            &source,
            prefix,
            Digest256::of_bytes(bytes),
            bytes.len() as u64,
            &root.join("scratch"),
            chunk_bytes,
            limits(),
        )
        .unwrap()
    }

    fn restore(
        root: &Path,
        transport: &mut MemoryTransport,
        receipt: &ChunkedFileReceiptV1,
        name: &str,
    ) -> Result<ChunkedFileReceiptV1> {
        restore_chunked_file_v1(
            transport,
            &receipt.manifest_key,
            receipt.manifest_sha256,
            &root.join(name),
            &root.join("scratch"),
            limits(),
        )
    }

    #[test]
    fn multi_chunk_round_trip_uses_the_exact_canonical_manifest() {
        let root = TestDirectory::new();
        let mut remote = MemoryTransport::default();
        let receipt = upload(&root.0, &mut remote, b"abcdef", "", 4);
        assert_eq!(receipt.chunk_count, 2);
        assert_eq!(
            receipt.manifest_key,
            "files/sha256/bef57ec7f53a6d40beb640a780a639c83bc29ac8a9816f1fc6c5c6dcd93c4721/manifest.json"
        );
        let expected = concat!(
            "{\"chunk_bytes\":4,\"chunks\":[{\"index\":0,\"key\":\"files/sha256/bef57ec7f53a6d40beb640a780a639c83bc29ac8a9816f1fc6c5c6dcd93c4721/parts/00000000-88d4266fd4e6338d13b845fcf289579d209c897823b9217da3e161936f031589\",\"offset\":0,\"sha256\":\"88d4266fd4e6338d13b845fcf289579d209c897823b9217da3e161936f031589\",\"size_bytes\":4},{\"index\":1,\"key\":\"files/sha256/bef57ec7f53a6d40beb640a780a639c83bc29ac8a9816f1fc6c5c6dcd93c4721/parts/00000001-4ca669ac3713d1f4aea07dae8dcc0d1c9867d27ea82a3ba4e6158a42206f959b\",\"offset\":4,\"sha256\":\"4ca669ac3713d1f4aea07dae8dcc0d1c9867d27ea82a3ba4e6158a42206f959b\",\"size_bytes\":2}],\"file_sha256\":\"bef57ec7f53a6d40beb640a780a639c83bc29ac8a9816f1fc6c5c6dcd93c4721\",\"file_size_bytes\":6,\"schema_version\":\"tos_chunked_file_v1\"}\n"
        );
        assert_eq!(
            remote.objects.get(&receipt.manifest_key).unwrap(),
            expected.as_bytes()
        );
        let restored = restore(&root.0, &mut remote, &receipt, "restored.bin").unwrap();
        assert!(restored.readback_verified);
        assert_eq!(fs::read(root.0.join("restored.bin")).unwrap(), b"abcdef");
    }

    #[test]
    fn empty_file_round_trip_has_no_parts_and_preserves_empty_digest() {
        let root = TestDirectory::new();
        let mut remote = MemoryTransport::default();
        let receipt = upload(&root.0, &mut remote, b"", "empty", 4);
        assert_eq!(receipt.chunk_count, 0);
        assert_eq!(receipt.file_sha256, Digest256::of_bytes(b""));
        let restored = restore(&root.0, &mut remote, &receipt, "empty-restored.bin").unwrap();
        assert_eq!(restored.file_size_bytes, 0);
        assert_eq!(fs::read(root.0.join("empty-restored.bin")).unwrap(), b"");
    }

    #[test]
    fn existing_corrupt_part_is_refused_without_put_or_manifest() {
        let root = TestDirectory::new();
        let file_hash = Digest256::of_bytes(b"abcdef").to_hex();
        let key = part_key("", &file_hash, 0, &Digest256::of_bytes(b"abcd").to_hex());
        let mut remote = MemoryTransport::default();
        remote.objects.insert(key, b"wrong".to_vec());
        let source = write_source(&root.0, "source.bin", b"abcdef");
        let result = upload_chunked_file_v1(
            &mut remote,
            &source,
            "",
            Digest256::of_bytes(b"abcdef"),
            6,
            &root.0.join("scratch"),
            4,
            limits(),
        );
        assert!(result.is_err());
        assert!(remote.puts.is_empty());
        assert!(!remote.objects.contains_key(&manifest_key("", &file_hash)));
    }

    #[test]
    fn failed_part_readback_does_not_publish_manifest() {
        let root = TestDirectory::new();
        let mut remote = MemoryTransport::default();
        let hash = Digest256::of_bytes(b"abcdef").to_hex();
        let first = part_key("", &hash, 0, &Digest256::of_bytes(b"abcd").to_hex());
        remote.hidden_after_put.insert(first);
        let source = write_source(&root.0, "source.bin", b"abcdef");
        assert!(
            upload_chunked_file_v1(
                &mut remote,
                &source,
                "",
                Digest256::of_bytes(b"abcdef"),
                6,
                &root.0.join("scratch"),
                4,
                limits(),
            )
            .is_err()
        );
        assert!(!remote.objects.contains_key(&manifest_key("", &hash)));
    }

    #[test]
    fn source_mutation_after_chunk_put_prevents_manifest_publication() {
        let root = TestDirectory::new();
        let source = write_source(&root.0, "source.bin", b"abcdef");
        let mut remote = MemoryTransport {
            mutate_source_on_put: Some(source.clone()),
            ..MemoryTransport::default()
        };
        assert!(
            upload_chunked_file_v1(
                &mut remote,
                &source,
                "",
                Digest256::of_bytes(b"abcdef"),
                6,
                &root.0.join("scratch"),
                4,
                limits(),
            )
            .is_err()
        );
        assert!(
            !remote
                .objects
                .contains_key(&manifest_key("", &Digest256::of_bytes(b"abcdef").to_hex()))
        );
    }

    #[test]
    fn retry_reuses_exact_remote_objects_after_verified_readback() {
        let root = TestDirectory::new();
        let mut remote = MemoryTransport::default();
        let first = upload(&root.0, &mut remote, b"abcdef", "resume", 4);
        let first_puts = remote.puts.len();
        let second = upload(&root.0, &mut remote, b"abcdef", "resume", 4);
        assert_eq!(first, second);
        assert_eq!(remote.puts.len(), first_puts);
        assert!(second.readback_verified);
    }

    #[test]
    fn existing_output_and_broken_symlink_are_never_overwritten() {
        let root = TestDirectory::new();
        let mut remote = MemoryTransport::default();
        let receipt = upload(&root.0, &mut remote, b"safe", "", 4);
        let existing = root.0.join("existing.bin");
        fs::write(&existing, b"keep").unwrap();
        let fetch_count = remote.fetches.len();
        assert!(restore(&root.0, &mut remote, &receipt, "existing.bin").is_err());
        assert_eq!(fs::read(&existing).unwrap(), b"keep");
        assert_eq!(remote.fetches.len(), fetch_count);
        let broken = root.0.join("broken-link.bin");
        symlink(root.0.join("missing-target"), &broken).unwrap();
        assert!(restore(&root.0, &mut remote, &receipt, "broken-link.bin").is_err());
        assert_eq!(remote.fetches.len(), fetch_count);
        assert!(
            fs::symlink_metadata(&broken)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn tampered_manifest_digest_order_offset_and_key_are_rejected() {
        let root = TestDirectory::new();
        let mut remote = MemoryTransport::default();
        let receipt = upload(&root.0, &mut remote, b"abcdef", "", 4);
        assert!(
            restore_chunked_file_v1(
                &mut remote,
                &receipt.manifest_key,
                Digest256::of_bytes(b"wrong"),
                &root.0.join("wrong-hash.bin"),
                &root.0.join("scratch"),
                limits(),
            )
            .is_err()
        );
        let original = remote.objects.get(&receipt.manifest_key).unwrap().clone();
        let mut value = parse_json(&original, JsonMode::PublishedStrict, JsonLimits::default())
            .unwrap()
            .root()
            .clone();
        if let JsonValue::Object(fields) = &mut value {
            for (name, chunks) in fields {
                if name.as_str() == Some("chunks") {
                    if let JsonValue::Array(rows) = chunks {
                        rows.swap(0, 1);
                    }
                }
            }
        }
        let swapped = canonical(&value, limits().max_manifest_bytes).unwrap();
        remote
            .objects
            .insert(receipt.manifest_key.clone(), swapped.clone());
        assert!(
            restore_chunked_file_v1(
                &mut remote,
                &receipt.manifest_key,
                Digest256::of_bytes(&swapped),
                &root.0.join("reordered.bin"),
                &root.0.join("scratch"),
                limits(),
            )
            .is_err()
        );
        remote
            .objects
            .insert(receipt.manifest_key.clone(), original.clone());
        let mut changed = String::from_utf8(original).unwrap();
        changed = changed.replace("\"offset\":4", "\"offset\":5");
        let changed = changed.into_bytes();
        remote
            .objects
            .insert(receipt.manifest_key.clone(), changed.clone());
        assert!(
            restore_chunked_file_v1(
                &mut remote,
                &receipt.manifest_key,
                Digest256::of_bytes(&changed),
                &root.0.join("offset.bin"),
                &root.0.join("scratch"),
                limits(),
            )
            .is_err()
        );
        let mut changed =
            String::from_utf8(remote.objects.get(&receipt.manifest_key).unwrap().clone()).unwrap();
        changed = changed.replace("parts/00000000-", "parts/00000000-0");
        let changed = changed.into_bytes();
        remote
            .objects
            .insert(receipt.manifest_key.clone(), changed.clone());
        assert!(
            restore_chunked_file_v1(
                &mut remote,
                &receipt.manifest_key,
                Digest256::of_bytes(&changed),
                &root.0.join("key.bin"),
                &root.0.join("scratch"),
                limits(),
            )
            .is_err()
        );
    }

    #[test]
    fn missing_or_corrupt_part_leaves_no_output() {
        let root = TestDirectory::new();
        let mut remote = MemoryTransport::default();
        let receipt = upload(&root.0, &mut remote, b"abcdef", "", 4);
        let first = part_key(
            "",
            &receipt.file_sha256.to_hex(),
            0,
            &Digest256::of_bytes(b"abcd").to_hex(),
        );
        remote.objects.remove(&first);
        assert!(restore(&root.0, &mut remote, &receipt, "missing.bin").is_err());
        assert!(!root.0.join("missing.bin").exists());
        remote.objects.insert(first, b"bad!".to_vec());
        assert!(restore(&root.0, &mut remote, &receipt, "corrupt.bin").is_err());
        assert!(!root.0.join("corrupt.bin").exists());
    }

    #[test]
    fn invalid_source_digest_prefix_and_symlink_are_rejected_before_put() {
        let root = TestDirectory::new();
        let source = write_source(&root.0, "source.bin", b"data");
        let mut remote = MemoryTransport::default();
        assert!(
            upload_chunked_file_v1(
                &mut remote,
                &source,
                "../escape",
                Digest256::of_bytes(b"data"),
                4,
                &root.0.join("scratch"),
                4,
                limits(),
            )
            .is_err()
        );
        assert!(
            upload_chunked_file_v1(
                &mut remote,
                &source,
                "",
                Digest256::of_bytes(b"wrong"),
                4,
                &root.0.join("scratch"),
                4,
                limits(),
            )
            .is_err()
        );
        let link = root.0.join("source-link");
        symlink(&source, &link).unwrap();
        assert!(
            upload_chunked_file_v1(
                &mut remote,
                &link,
                "",
                Digest256::of_bytes(b"data"),
                4,
                &root.0.join("scratch"),
                4,
                limits(),
            )
            .is_err()
        );
        assert!(remote.puts.is_empty());
    }
}
