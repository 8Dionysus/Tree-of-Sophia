//! Deterministic existing capture v1/v2 byte transport from an exact Git commit.
//! No source admission, rights, currentness or executable selection is granted.
use crate::{Result, StoreError, StoreErrorCode as Code};
use flate2::{Compression, GzBuilder};
use sha1::{Digest, Sha1};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonNumber, JsonNumberKind,
    JsonString, JsonValue, canonical_bytes_v1,
};

#[derive(Clone, Copy, Debug)]
pub struct GitCaptureLimits {
    pub max_members: usize,
    pub max_member_bytes: u64,
    pub max_source_bytes: u64,
    pub max_metadata_bytes: usize,
    pub max_tree_bytes: u64,
    pub max_archive_bytes: u64,
}
impl Default for GitCaptureLimits {
    fn default() -> Self {
        Self {
            max_members: 2048,
            max_member_bytes: 8_388_608,
            max_source_bytes: 33_554_432,
            max_metadata_bytes: 8_388_608,
            max_tree_bytes: 16_777_216,
            max_archive_bytes: 67_108_864,
        }
    }
}
pub struct CaptureGitRequest<'a> {
    pub repository: &'a Path,
    pub commit: &'a str,
    pub include_prefixes: &'a [String],
    pub exclude_prefixes: &'a [String],
    pub exclude_path_parts: &'a [String],
    pub output: &'a Path,
}
pub struct CaptureGitResult {
    pub manifest: JsonValue,
    pub manifest_sha256: Digest256,
}
fn fail(detail: &'static str) -> StoreError {
    StoreError::new(Code::CorruptSelectedObject, detail)
}
fn ioe(error: io::Error) -> StoreError {
    StoreError::io("Git capture IO", error)
}
fn active(deadline: Instant, cancelled: &AtomicBool) -> io::Result<()> {
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "Git capture interrupted",
        ))
    } else {
        Ok(())
    }
}
fn invalid(detail: &'static str) -> io::Error {
    io::Error::other(detail)
}
fn safe(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.chars().any(|c| c < ' ')
        && path
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
}
fn normalize(raw: &[String], parts: bool, cap: usize) -> Result<Vec<String>> {
    let mut values = BTreeSet::new();
    let mut bytes = 0usize;
    for value in raw {
        let value = if parts {
            value.as_str()
        } else {
            value.trim_end_matches('/')
        };
        if !safe(value) || parts && (value.contains('/') || value.contains('\u{7f}')) {
            return Err(fail("invalid capture prefix or path part"));
        }
        bytes = bytes
            .checked_add(value.len() + std::mem::size_of::<String>())
            .filter(|n| *n <= cap)
            .ok_or(fail("capture selector metadata budget"))?;
        values.insert(value.to_owned());
    }
    Ok(values.into_iter().collect())
}
fn matches(path: &str, prefixes: &[String]) -> bool {
    prefixes
        .iter()
        .any(|p| path == p || path.strip_prefix(p).is_some_and(|s| s.starts_with('/')))
}
fn hex40(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
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
    JsonValue::Array(values.iter().map(|s| string(s)).collect())
}
fn object(values: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        values
            .into_iter()
            .map(|(k, v)| (JsonString::from_utf8(k), v))
            .collect(),
    )
}
fn canonical(value: &JsonValue, cap: usize) -> Result<Vec<u8>> {
    canonical_bytes_v1(
        value,
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits::new(cap, 32, 100_000, 4096).map_err(|_| fail("capture JSON limits"))?,
    )
    .map_err(|_| fail("capture canonical JSON budget"))
}

// Linux pipe nonblocking mode prevents a stalled Git child from bypassing the
// same whole deadline on reads or writes. tos-fd-open already requires Linux.
unsafe extern "C" {
    fn fcntl(fd: i32, command: i32, ...) -> i32;
}
fn nonblocking(fd: i32) -> io::Result<()> {
    // F_GETFL/F_SETFL and O_NONBLOCK are Linux ABI constants; fd remains owned.
    let flags = unsafe { fcntl(fd, 3) };
    if flags < 0 || unsafe { fcntl(fd, 4, flags | 2048) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
struct Git<'a> {
    child: Child,
    input: Option<ChildStdin>,
    output: ChildStdout,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    reaped: bool,
}
impl<'a> Git<'a> {
    fn spawn(
        root: &File,
        args: &[&str],
        deadline: Instant,
        cancelled: &'a AtomicBool,
    ) -> Result<Self> {
        active(deadline, cancelled).map_err(ioe)?;
        let mut child = Command::new("/usr/bin/git")
            .args(args)
            .current_dir(format!(
                "/proc/{}/fd/{}",
                std::process::id(),
                root.as_raw_fd()
            ))
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LC_ALL", "C")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .env("GIT_NO_LAZY_FETCH", "1")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(ioe)?;
        let output = child.stdout.take().ok_or(fail("Git stdout absent"))?;
        let input = child.stdin.take();
        let mut process = Self {
            child,
            input,
            output,
            deadline,
            cancelled,
            reaped: false,
        };
        nonblocking(process.output.as_raw_fd()).map_err(ioe)?;
        if let Some(input) = &process.input {
            nonblocking(input.as_raw_fd()).map_err(ioe)?;
        }
        if args.first() != Some(&"cat-file") {
            process.input.take();
        }
        Ok(process)
    }
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            active(self.deadline, self.cancelled)?;
            match self.output.read(buffer) {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(2))
                }
                result => return result,
            }
        }
    }
    fn exact(&mut self, mut out: &mut [u8]) -> io::Result<()> {
        while !out.is_empty() {
            let n = self.read(out)?;
            if n == 0 {
                return Err(invalid("truncated Git output"));
            }
            out = &mut out[n..];
        }
        Ok(())
    }
    fn line(&mut self, cap: usize) -> io::Result<Vec<u8>> {
        let mut value = Vec::new();
        let mut byte = [0];
        loop {
            self.exact(&mut byte)?;
            if byte[0] == b'\n' {
                return Ok(value);
            }
            if value.len() >= cap {
                return Err(invalid("Git line budget"));
            }
            value.push(byte[0]);
        }
    }
    fn write(&mut self, mut bytes: &[u8]) -> io::Result<()> {
        while !bytes.is_empty() {
            active(self.deadline, self.cancelled)?;
            match self
                .input
                .as_mut()
                .ok_or(invalid("Git stdin absent"))?
                .write(bytes)
            {
                Ok(0) => return Err(invalid("Git stdin closed")),
                Ok(n) => bytes = &bytes[n..],
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(2))
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
    fn finish(&mut self) -> Result<()> {
        self.input.take();
        loop {
            active(self.deadline, self.cancelled).map_err(ioe)?;
            if let Some(status) = self.child.try_wait().map_err(ioe)? {
                self.reaped = true;
                return if status.success() {
                    Ok(())
                } else {
                    Err(fail("Git child refused"))
                };
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
impl Drop for Git<'_> {
    fn drop(&mut self) {
        if !self.reaped {
            self.input.take();
            let _ = self.child.kill();
            // Bounded cleanup grace; no blocking Read/write/wait on an error.
            let end = Instant::now() + Duration::from_secs(10);
            while Instant::now() < end {
                if self.child.try_wait().ok().flatten().is_some() {
                    self.reaped = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }
}
fn resolve(
    root: &File,
    expression: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<String> {
    let mut child = Git::spawn(
        root,
        &["rev-parse", "--verify", expression],
        deadline,
        cancelled,
    )?;
    let line = child.line(40).map_err(ioe)?;
    let mut extra = [0];
    if child.read(&mut extra).map_err(ioe)? != 0 {
        return Err(fail("Git resolution trailing bytes"));
    }
    child.finish()?;
    let text = String::from_utf8(line).map_err(|_| fail("Git object ID encoding"))?;
    if !hex40(&text) {
        return Err(fail("Git object ID syntax"));
    }
    Ok(text)
}
struct Member {
    path: String,
    oid: String,
    size: u64,
    mode: u32,
}
fn entries(
    root: &File,
    commit: &str,
    include: &[String],
    exclude: &[String],
    parts: &[String],
    limits: GitCaptureLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<Member>> {
    let mut child = Git::spawn(
        root,
        &["ls-tree", "-r", "-l", "-z", commit],
        deadline,
        cancelled,
    )?;
    let mut selected = BTreeMap::new();
    let mut pending = Vec::new();
    let mut buffer = [0; 65536];
    let mut source = 0u64;
    let mut metadata = 0usize;
    let mut seen = 0u64;
    loop {
        let n = child.read(&mut buffer).map_err(ioe)?;
        if n == 0 {
            break;
        }
        seen = seen
            .checked_add(n as u64)
            .filter(|n| *n <= limits.max_tree_bytes)
            .ok_or(fail("Git tree byte budget"))?;
        for byte in &buffer[..n] {
            if *byte != 0 {
                if pending.len() >= limits.max_metadata_bytes / 4 {
                    return Err(fail("Git tree record budget"));
                }
                pending.push(*byte);
                continue;
            }
            if pending.is_empty() {
                continue;
            }
            let tab = pending
                .iter()
                .position(|byte| *byte == b'\t')
                .ok_or(fail("Git tree record"))?;
            let left = std::str::from_utf8(&pending[..tab])
                .map_err(|_| fail("Git tree metadata encoding"))?;
            let path_raw = &pending[tab + 1..];
            let raw_matches = |prefixes: &[String]| {
                prefixes.iter().any(|prefix| {
                    path_raw == prefix.as_bytes()
                        || path_raw
                            .strip_prefix(prefix.as_bytes())
                            .is_some_and(|rest| rest.starts_with(b"/"))
                })
            };
            if !raw_matches(include)
                || raw_matches(exclude)
                || path_raw
                    .split(|b| *b == b'/')
                    .any(|part| parts.iter().any(|selected| part == selected.as_bytes()))
            {
                pending.clear();
                continue;
            }
            let path =
                std::str::from_utf8(path_raw).map_err(|_| fail("selected Git path UTF-8"))?;
            let fields = left.split_ascii_whitespace().collect::<Vec<_>>();
            if fields.len() != 4 {
                return Err(fail("Git tree fields"));
            }
            if matches(path, include)
                && !matches(path, exclude)
                && !path.split('/').any(|p| parts.iter().any(|x| x == p))
            {
                if !safe(path)
                    || !matches!(fields[0], "100644" | "100755")
                    || fields[1] != "blob"
                    || !hex40(fields[2])
                {
                    return Err(fail("selected Git path is not a regular safe blob"));
                }
                let size: u64 = fields[3].parse().map_err(|_| fail("Git blob size"))?;
                if size > limits.max_member_bytes {
                    return Err(fail("Git member byte budget"));
                }
                source = source
                    .checked_add(size)
                    .filter(|n| *n <= limits.max_source_bytes)
                    .ok_or(fail("Git source byte budget"))?;
                metadata = metadata
                    .checked_add(path.len() + 40 + std::mem::size_of::<Member>())
                    .filter(|n| *n <= limits.max_metadata_bytes / 2)
                    .ok_or(fail("capture member metadata budget"))?;
                let entry = Member {
                    path: path.to_owned(),
                    oid: fields[2].to_owned(),
                    size,
                    mode: if fields[0] == "100755" { 0o755 } else { 0o644 },
                };
                if selected.insert(path.to_owned(), entry).is_some()
                    || selected.len() > limits.max_members
                {
                    return Err(fail("capture duplicate/member budget"));
                }
            }
            pending.clear();
        }
    }
    if !pending.is_empty() {
        return Err(fail("Git tree output not NUL terminated"));
    }
    child.finish()?;
    Ok(selected.into_values().collect())
}
struct Blob<'g, 'a> {
    git: &'g mut Git<'a>,
    remaining: u64,
    sha256: Digest256Hasher,
    sha1: Sha1,
}
impl<'g, 'a> Blob<'g, 'a> {
    fn request(git: &'g mut Git<'a>, member: &Member) -> Result<Self> {
        git.write(format!("{}\n", member.oid).as_bytes())
            .map_err(ioe)?;
        let header = git.line(128).map_err(ioe)?;
        let expected = format!("{} blob {}", member.oid, member.size);
        if header != expected.as_bytes() {
            return Err(fail("Git batch object differs from selected tree"));
        }
        let mut sha1 = Sha1::new();
        sha1.update(format!("blob {}\0", member.size).as_bytes());
        Ok(Self {
            git,
            remaining: member.size,
            sha256: Digest256Hasher::new(),
            sha1,
        })
    }
    fn finish(mut self, oid: &str) -> Result<Digest256> {
        if self.remaining != 0 {
            return Err(fail("Git blob was not consumed"));
        }
        let mut lf = [0];
        self.git.exact(&mut lf).map_err(ioe)?;
        if lf != [b'\n'] || format!("{:x}", self.sha1.finalize()) != oid {
            return Err(fail("Git blob object identity mismatch"));
        }
        Ok(self.sha256.finalize())
    }
}
impl Read for Blob<'_, '_> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        active(self.git.deadline, self.git.cancelled)?;
        if self.remaining == 0 || out.is_empty() {
            return Ok(0);
        }
        let cap = out.len().min(65536).min(self.remaining.min(65536) as usize);
        let n = self.git.read(&mut out[..cap])?;
        if n == 0 {
            return Err(invalid("truncated Git blob"));
        }
        self.remaining -= n as u64;
        self.sha256.update(&out[..n]);
        self.sha1.update(&out[..n]);
        Ok(n)
    }
}
struct Sink<'a> {
    file: File,
    sha: Digest256Hasher,
    bytes: u64,
    cap: u64,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
impl Write for Sink<'_> {
    fn write(&mut self, raw: &[u8]) -> io::Result<usize> {
        active(self.deadline, self.cancelled)?;
        if self
            .bytes
            .checked_add(raw.len() as u64)
            .is_none_or(|n| n > self.cap)
        {
            return Err(invalid("capture output byte budget"));
        }
        let n = self.file.write(raw)?;
        self.bytes += n as u64;
        self.sha.update(&raw[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        active(self.deadline, self.cancelled)?;
        self.file.flush()
    }
}
fn fd_path(root: &File, name: &str) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", root.as_raw_fd())).join(name)
}
fn new_file(root: &File, name: &str) -> Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(0x20000)
        .open(fd_path(root, name))
        .map_err(ioe)
}
fn output(path: &Path, deadline: Instant, cancelled: &AtomicBool) -> Result<(File, File)> {
    active(deadline, cancelled).map_err(ioe)?;
    if !path.is_absolute() {
        return Err(fail("capture output must be absolute"));
    }
    let parent = path.parent().ok_or(fail("capture output parent"))?;
    let leaf = path.file_name().ok_or(fail("capture output leaf"))?;
    let mut parent_fd = tos_fd_open::open_absolute_directory(Path::new("/"))
        .map_err(|_| fail("unsafe filesystem root"))?;
    let mut depth = 0usize;
    for component in parent.components() {
        active(deadline, cancelled).map_err(ioe)?;
        let std::path::Component::Normal(name) = component else {
            if component == std::path::Component::RootDir {
                continue;
            }
            return Err(fail("capture output path is not normalized"));
        };
        depth += 1;
        if depth > 64 {
            return Err(fail("capture output parent depth"));
        }
        parent_fd = match tos_fd_open::open_directory_at(&parent_fd, Path::new(name)) {
            Ok(directory) => directory,
            Err(error)
                if error
                    .source
                    .as_ref()
                    .is_some_and(|e| e.kind() == io::ErrorKind::NotFound) =>
            {
                fs::DirBuilder::new()
                    .mode(0o700)
                    .create(fd_path(
                        &parent_fd,
                        name.to_str().ok_or(fail("capture parent UTF-8"))?,
                    ))
                    .map_err(ioe)?;
                parent_fd.sync_all().map_err(ioe)?;
                tos_fd_open::open_directory_at(&parent_fd, Path::new(name))
                    .map_err(|_| fail("unsafe new capture parent"))?
            }
            Err(_) => return Err(fail("unsafe capture parent")),
        };
    }
    let target = fd_path(
        &parent_fd,
        leaf.to_str().ok_or(fail("capture output UTF-8"))?,
    );
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&target)
        .map_err(ioe)?;
    let root = tos_fd_open::open_directory_at(&parent_fd, Path::new(leaf))
        .map_err(|_| fail("unsafe fresh capture directory"))?;
    Ok((parent_fd, root))
}
fn verify_paths(
    repository_path: &Path,
    repository: &File,
    output_path: &Path,
    root: &File,
) -> Result<()> {
    let actual = tos_fd_open::open_absolute_directory(repository_path)
        .map_err(|_| fail("Git repository path changed"))?;
    let (before, after) = (
        repository.metadata().map_err(ioe)?,
        actual.metadata().map_err(ioe)?,
    );
    if before.dev() != after.dev() || before.ino() != after.ino() {
        return Err(fail("Git repository directory changed"));
    }
    let selected_output = tos_fd_open::open_absolute_directory(output_path)
        .map_err(|_| fail("capture output path changed"))?;
    let (held, current) = (
        root.metadata().map_err(ioe)?,
        selected_output.metadata().map_err(ioe)?,
    );
    if held.dev() != current.dev() || held.ino() != current.ino() {
        return Err(fail("capture output directory changed"));
    }
    Ok(())
}
/// Write exactly source.tar.gz, members.jsonl and capture.json into a fresh leaf.
/// An interrupted output remains incomplete and conveys no capture authority.
/// Missing parents are created through held directory descriptors (depth <=64);
/// no existing parent may be a symlink.
pub fn capture_git(
    request: CaptureGitRequest<'_>,
    limits: GitCaptureLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<CaptureGitResult> {
    active(deadline, cancelled).map_err(ioe)?;
    if !hex40(request.commit)
        || limits.max_members == 0
        || limits.max_member_bytes == 0
        || limits.max_source_bytes == 0
        || limits.max_metadata_bytes < 4096
        || limits.max_tree_bytes == 0
        || limits.max_archive_bytes == 0
    {
        return Err(fail("capture exact commit and finite limits required"));
    }
    let include = normalize(
        request.include_prefixes,
        false,
        limits.max_metadata_bytes / 8,
    )?;
    let exclude = normalize(
        request.exclude_prefixes,
        false,
        limits.max_metadata_bytes / 8,
    )?;
    let parts = normalize(
        request.exclude_path_parts,
        true,
        limits.max_metadata_bytes / 8,
    )?;
    let repository = tos_fd_open::open_absolute_directory(request.repository)
        .map_err(|_| fail("unsafe Git repository directory"))?;
    let resolved = resolve(
        &repository,
        &format!("{}^{{commit}}", request.commit),
        deadline,
        cancelled,
    )?;
    if resolved != request.commit {
        return Err(fail("Git resolved commit differs"));
    }
    let tree = resolve(
        &repository,
        &format!("{}^{{tree}}", request.commit),
        deadline,
        cancelled,
    )?;
    let entries = entries(
        &repository,
        request.commit,
        &include,
        &exclude,
        &parts,
        limits,
        deadline,
        cancelled,
    )?;
    let source_bytes = entries
        .iter()
        .try_fold(0u64, |n, e| n.checked_add(e.size))
        .ok_or(fail("capture source size overflow"))?;
    let (parent, root) = output(request.output, deadline, cancelled)?;
    let archive = Sink {
        file: new_file(&root, "source.tar.gz")?,
        sha: Digest256Hasher::new(),
        bytes: 0,
        cap: limits.max_archive_bytes,
        deadline,
        cancelled,
    };
    let mut members = Sink {
        file: new_file(&root, "members.jsonl")?,
        sha: Digest256Hasher::new(),
        bytes: 0,
        cap: limits.max_metadata_bytes as u64,
        deadline,
        cancelled,
    };
    let gzip = GzBuilder::new()
        .mtime(0)
        .operating_system(255)
        .write(archive, Compression::best());
    let mut tar = tar::Builder::new(gzip);
    let mut blobs = Git::spawn(&repository, &["cat-file", "--batch"], deadline, cancelled)?;
    for member in &entries {
        active(deadline, cancelled).map_err(ioe)?;
        let pax = !member.path.is_ascii() || member.path.len() > 100;
        if pax {
            tar.append_pax_extensions([("path", member.path.as_bytes())])
                .map_err(ioe)?;
        }
        let mut header = tar::Header::new_ustar();
        header
            .set_path(if pax { "PaxMember" } else { &member.path })
            .map_err(ioe)?;
        header.set_entry_type(tar::EntryType::Regular);
        header.set_mode(member.mode);
        header.set_size(member.size);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_username("").map_err(ioe)?;
        header.set_groupname("").map_err(ioe)?;
        header.set_cksum();
        let mut blob = Blob::request(&mut blobs, member)?;
        tar.append(&header, &mut blob).map_err(ioe)?;
        let sha = blob.finish(&member.oid)?;
        let row = object(vec![
            ("path", string(&member.path)),
            ("git_blob_oid", string(&member.oid)),
            ("size_bytes", number(member.size)),
            ("sha256", string(&sha.to_hex())),
            ("mode", number(member.mode as u64)),
        ]);
        members
            .write_all(&canonical(&row, limits.max_metadata_bytes / 4)?)
            .map_err(ioe)?;
    }
    blobs.input.take();
    let mut extra = [0];
    if blobs.read(&mut extra).map_err(ioe)? != 0 {
        return Err(fail("Git batch trailing bytes"));
    }
    blobs.finish()?;
    let gzip = tar.into_inner().map_err(ioe)?;
    let mut archive = gzip.finish().map_err(ioe)?;
    archive.flush().map_err(ioe)?;
    members.flush().map_err(ioe)?;
    archive.file.sync_all().map_err(ioe)?;
    members.file.sync_all().map_err(ioe)?;
    let v2 = !exclude.is_empty() || !parts.is_empty();
    let mut manifest = object(vec![
        (
            "schema_version",
            string(if v2 {
                "tos_corpus_capture_v2"
            } else {
                "tos_corpus_capture_v1"
            }),
        ),
        ("source_git_commit", string(request.commit)),
        ("source_git_tree", string(&tree)),
        ("include_prefixes", array(&include)),
        ("member_count", number(entries.len() as u64)),
        ("source_bytes", number(source_bytes)),
        ("members_sha256", string(&members.sha.finalize().to_hex())),
        ("archive_sha256", string(&archive.sha.finalize().to_hex())),
        ("archive_size_bytes", number(archive.bytes)),
    ]);
    if let JsonValue::Object(values) = &mut manifest {
        if v2 {
            values.push((JsonString::from_utf8("exclude_prefixes"), array(&exclude)));
            values.push((JsonString::from_utf8("exclude_path_parts"), array(&parts)));
        }
    }
    verify_paths(request.repository, &repository, request.output, &root)?;
    active(deadline, cancelled).map_err(ioe)?;
    let raw = canonical(&manifest, limits.max_metadata_bytes / 4)?;
    let mut manifest_file = new_file(&root, "capture.json")?;
    manifest_file.write_all(&raw).map_err(ioe)?;
    manifest_file.sync_all().map_err(ioe)?;
    root.sync_all().map_err(ioe)?;
    parent.sync_all().map_err(ioe)?;
    verify_paths(request.repository, &repository, request.output, &root)?;
    active(deadline, cancelled).map_err(ioe)?;
    Ok(CaptureGitResult {
        manifest,
        manifest_sha256: Digest256::of_bytes(&raw),
    })
}

/// Exact historical one-file locator, separate from a complete capture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitMemberDescriptor {
    pub commit: String,
    pub path: String,
    pub git_blob_oid: String,
    pub size_bytes: u64,
    pub mode: u32,
}
pub fn resolve_git_member(
    repository: &Path,
    commit: &str,
    path: &str,
    max_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<GitMemberDescriptor> {
    active(deadline, cancelled).map_err(ioe)?;
    if !hex40(commit) || !safe(path) || tos_foundation::RelativePath::parse(path).is_err() {
        return Err(fail("exact Git commit and safe member required"));
    }
    let root = tos_fd_open::open_absolute_directory(repository)
        .map_err(|_| fail("unsafe Git repository"))?;
    if resolve(&root, &format!("{commit}^{{commit}}"), deadline, cancelled)? != commit {
        return Err(fail("selection is not exact Git commit"));
    }
    let mut child = Git::spawn(
        &root,
        &[
            "--literal-pathspecs",
            "ls-tree",
            "-l",
            "-z",
            commit,
            "--",
            path,
        ],
        deadline,
        cancelled,
    )?;
    let mut raw = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = child.read(&mut buf).map_err(ioe)?;
        if n == 0 {
            break;
        }
        if raw.len() + n > 65536 {
            return Err(fail("Git descriptor budget"));
        }
        raw.extend_from_slice(&buf[..n]);
    }
    child.finish()?;
    if raw.last() != Some(&0) || raw[..raw.len() - 1].contains(&0) {
        return Err(fail("Git member missing or ambiguous"));
    }
    let text =
        std::str::from_utf8(&raw[..raw.len() - 1]).map_err(|_| fail("Git descriptor encoding"))?;
    let (facts, selected) = text.split_once('\t').ok_or(fail("Git descriptor row"))?;
    let facts: Vec<_> = facts.split_whitespace().collect();
    if selected != path || facts.len() != 4 || facts[1] != "blob" || !hex40(facts[2]) {
        return Err(fail("Git descriptor differs"));
    }
    let mode = match facts[0] {
        "100644" => 0o644,
        "100755" => 0o755,
        _ => return Err(fail("Git member not regular")),
    };
    let size = facts[3]
        .parse::<u64>()
        .map_err(|_| fail("Git member size"))?;
    if size > max_bytes {
        return Err(fail("Git member byte budget"));
    }
    Ok(GitMemberDescriptor {
        commit: commit.into(),
        path: path.into(),
        git_blob_oid: facts[2].into(),
        size_bytes: size,
        mode,
    })
}
/// Revalidate descriptor then authenticate Git blob OID while streaming to an unpublished sink.
pub fn read_git_member(
    repository: &Path,
    descriptor: &GitMemberDescriptor,
    max_bytes: u64,
    sink: &mut impl Write,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Digest256> {
    if &resolve_git_member(
        repository,
        &descriptor.commit,
        &descriptor.path,
        max_bytes,
        deadline,
        cancelled,
    )? != descriptor
    {
        return Err(fail("Git descriptor differs from exact tree"));
    }
    let root = tos_fd_open::open_absolute_directory(repository)
        .map_err(|_| fail("unsafe Git repository"))?;
    let member = Member {
        path: descriptor.path.clone(),
        oid: descriptor.git_blob_oid.clone(),
        size: descriptor.size_bytes,
        mode: descriptor.mode,
    };
    let mut child = Git::spawn(&root, &["cat-file", "--batch"], deadline, cancelled)?;
    let mut blob = Blob::request(&mut child, &member)?;
    let mut buffer = [0u8; 65536];
    loop {
        let n = blob.read(&mut buffer).map_err(ioe)?;
        if n == 0 {
            break;
        }
        sink.write_all(&buffer[..n]).map_err(ioe)?;
    }
    let digest = blob.finish(&member.oid)?;
    child.finish()?;
    Ok(digest)
}
