//! Bounded native file and publication primitives shared by ToS exports.
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::{Duration, Instant};
use tos_foundation::{CanonicalProfile, Digest256Hasher, JsonLimits, canonical_raw_bytes_v1};

// Individual export formats apply their narrower limits on top of these bounds.
pub const MAX_MANIFEST_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_MEMBERS: usize = 100_000;
pub const MAX_ARTIFACT_BYTES: u64 = 1024 * 1024 * 1024;
static CANCELLED: AtomicI32 = AtomicI32::new(0);
thread_local! { static DEADLINE: std::cell::Cell<Option<Instant>> = const { std::cell::Cell::new(None) }; }
pub fn cancel(signal: i32) {
    CANCELLED.store(signal, Ordering::Relaxed);
}
pub(crate) fn budget_check() -> io::Result<()> {
    if CANCELLED.load(Ordering::Relaxed) != 0
        || DEADLINE.with(|d| d.get().is_some_and(|end| Instant::now() >= end))
    {
        return Err(invalid("ToS export operation cancelled or expired"));
    }
    Ok(())
}
pub(crate) struct WholeBudget {
    owns_deadline: bool,
}
impl WholeBudget {
    /// The original current operation's deadline, never a renewed allowance.
    pub(crate) fn deadline(&self) -> io::Result<Instant> {
        budget_check()?;
        DEADLINE
            .with(|d| d.get())
            .ok_or_else(|| invalid("ToS export operation has no active deadline"))
    }
    pub(crate) fn begin() -> io::Result<Self> {
        budget_check()?;
        let owns_deadline = DEADLINE.with(|d| {
            if d.get().is_some() {
                false
            } else {
                d.set(Some(Instant::now() + Duration::from_secs(600)));
                true
            }
        });
        Ok(Self { owns_deadline })
    }
}
impl Drop for WholeBudget {
    fn drop(&mut self) {
        if self.owns_deadline {
            DEADLINE.with(|d| d.set(None));
        }
    }
}

pub(crate) fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
pub fn canonical(value: &Value) -> io::Result<Vec<u8>> {
    let raw = serde_json::to_vec(value).map_err(|e| invalid(e.to_string()))?;
    canonical_raw_bytes_v1(
        &raw,
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits {
            max_bytes: MAX_MANIFEST_BYTES,
            ..JsonLimits::default()
        },
    )
    .map_err(|e| invalid(e.to_string()))
}
pub(crate) fn strict_json(raw: &[u8]) -> io::Result<Value> {
    // Canonicalization first rejects decoded duplicate fields and nonfinite JSON.
    canonical_raw_bytes_v1(
        raw,
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits {
            max_bytes: MAX_MANIFEST_BYTES,
            ..JsonLimits::default()
        },
    )
    .map_err(|e| invalid(e.to_string()))?;
    serde_json::from_slice(raw).map_err(|e| invalid(e.to_string()))
}
pub(crate) fn keys(value: &Value, expected: &[&str]) -> io::Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("expected JSON object"))?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid("unexpected JSON field set"));
    }
    Ok(())
}
pub(crate) fn hex(value: &Value) -> io::Result<&str> {
    let value = value
        .as_str()
        .ok_or_else(|| invalid("digest must be a string"))?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid("digest must be lowercase SHA-256"));
    }
    Ok(value)
}
pub(crate) fn safe_absolute(path: &Path) -> io::Result<PathBuf> {
    budget_check()?;
    if path
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(invalid("path contains traversal segments"));
    }
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut current = PathBuf::new();
    for component in path.components() {
        budget_check()?;
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(invalid("path contains symlink"));
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => break,
            Err(e) => return Err(e),
        }
    }
    Ok(path)
}
pub(crate) fn relative(value: &Value) -> io::Result<&str> {
    let value = value
        .as_str()
        .ok_or_else(|| invalid("member path must be string"))?;
    if value.is_empty()
        || value.contains('\\')
        || value.chars().any(|c| (c as u32) < 32)
        || value.starts_with('/')
        || value
            .split('/')
            .any(|c| c.is_empty() || matches!(c, "." | ".." | ".git"))
    {
        return Err(invalid("member path is not normalized"));
    }
    Ok(value)
}
pub(crate) fn regular(path: &Path) -> io::Result<fs::Metadata> {
    safe_absolute(path)?;
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(invalid("expected regular file"));
    }
    Ok(meta)
}
pub(crate) fn directory(path: &Path) -> io::Result<()> {
    safe_absolute(path)?;
    if !fs::symlink_metadata(path)?.is_dir() {
        return Err(invalid("expected regular directory"));
    }
    Ok(())
}
pub(crate) fn digest(path: &Path) -> io::Result<String> {
    budget_check()?;
    let meta = regular(path)?;
    if meta.len() > MAX_ARTIFACT_BYTES {
        return Err(invalid("artifact byte limit exceeded"));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid("artifact is not a regular file"));
    }
    let mut hash = Digest256Hasher::new();
    let mut bytes = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        budget_check()?;
        let count = file.read(&mut bytes)?;
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count as u64)
            .ok_or_else(|| invalid("byte count overflow"))?;
        if total > MAX_ARTIFACT_BYTES {
            return Err(invalid("artifact byte limit exceeded"));
        }
        hash.update(&bytes[..count]);
    }
    if total != meta.len() || file.metadata()?.len() != meta.len() {
        return Err(invalid("file changed while hashing"));
    }
    Ok(hash.finalize().to_hex())
}
pub(crate) fn read_json(path: &Path) -> io::Result<Value> {
    if regular(path)?.len() > MAX_MANIFEST_BYTES as u64 {
        return Err(invalid("manifest byte limit exceeded"));
    }
    let mut raw = Vec::new();
    File::open(path)?
        .take(MAX_MANIFEST_BYTES as u64 + 1)
        .read_to_end(&mut raw)?;
    if raw.len() > MAX_MANIFEST_BYTES {
        return Err(invalid("manifest byte limit exceeded"));
    }
    strict_json(&raw)
}
pub(crate) fn tree(root: &Path) -> io::Result<(Vec<(String, PathBuf)>, BTreeSet<String>)> {
    directory(root)?;
    let mut files = Vec::new();
    let mut directories = BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    let mut visited = 0usize;
    while let Some(current) = pending.pop() {
        budget_check()?;
        for entry in fs::read_dir(&current)? {
            budget_check()?;
            let entry = entry?;
            visited += 1;
            if visited > MAX_MEMBERS {
                return Err(invalid("artifact member limit exceeded"));
            }
            let path = entry.path();
            let name = path
                .strip_prefix(root)
                .map_err(|_| invalid("member leaves root"))?
                .to_str()
                .ok_or_else(|| invalid("artifact name is not UTF-8"))?
                .to_owned();
            relative(&Value::String(name.clone()))?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                directories.insert(name);
                pending.push(path);
            } else if kind.is_file() {
                regular(&path)?;
                files.push((name, path));
            } else {
                return Err(invalid("artifact tree contains symlink or special file"));
            }
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok((files, directories))
}
#[cfg(target_os = "linux")]
pub(crate) fn rename_new(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let source = std::ffi::CString::new(source.as_os_str().as_bytes())
        .map_err(|_| invalid("unsafe rename source"))?;
    let destination = std::ffi::CString::new(destination.as_os_str().as_bytes())
        .map_err(|_| invalid("unsafe rename destination"))?;
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
#[cfg(not(target_os = "linux"))]
pub(crate) fn rename_new(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "immutable publication requires Linux no-replace rename",
    ))
}
