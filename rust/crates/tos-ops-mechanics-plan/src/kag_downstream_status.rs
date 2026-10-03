//! Local downstream attempt bookkeeping, ported from scripts/downstream_status.py.
//! This status does not validate artifacts or authorize publication or semantics.
#![cfg(target_os = "linux")]

use serde_json::{Value, json};
use std::ffi::CString;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use tos_foundation::{CanonicalProfile, JsonLimits, canonical_raw_bytes_v1};

pub const SCHEMA_VERSION: &str = "downstream_status_v1";
const MAX_ERROR_CHARS: usize = 4096;
// A schema-valid state contains only small identity fields and one bounded error.
const MAX_STATE_BYTES: usize = 65536;
const SUCCESS: &[&str] = &[
    "attempt_id",
    "source_revision",
    "started_at",
    "completed_at",
    "artifact_revision",
    "artifact_manifest_sha256",
];

fn bad(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
fn keys(value: &Value, expected: &[&str]) -> io::Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| bad("record must be an object"))?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(bad("record has an unexpected field set"));
    }
    Ok(())
}
fn text<'a>(value: &'a Value, key: &str) -> io::Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| bad(format!("{key} must be a string")))
}
fn hex(value: &str, length: usize, label: &str) -> io::Result<()> {
    if value.len() != length
        || !value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(bad(format!(
            "{label} must be a lowercase {}",
            if length == 64 {
                "SHA-256 digest"
            } else {
                "random attempt id"
            }
        )));
    }
    Ok(())
}
fn timestamp(value: &str) -> io::Result<()> {
    let b = value.as_bytes();
    if b.len() != 20
        || [4, 7, 10, 13, 16, 19].iter().any(|&i| {
            b[i] != match i {
                4 | 7 => b'-',
                10 => b'T',
                13 | 16 => b':',
                _ => b'Z',
            }
        })
        || b.iter()
            .enumerate()
            .any(|(i, c)| ![4, 7, 10, 13, 16, 19].contains(&i) && !c.is_ascii_digit())
    {
        return Err(bad("timestamp must be UTC"));
    }
    let number = |a: usize, z: usize| value[a..z].parse::<u32>().unwrap();
    let (y, m, d) = (number(0, 4), number(5, 7), number(8, 10));
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => 0,
    };
    if y == 0
        || d == 0
        || d > days
        || number(11, 13) > 23
        || number(14, 16) > 59
        || number(17, 19) > 59
    {
        return Err(bad("timestamp is not a valid UTC timestamp"));
    }
    Ok(())
}
fn common(value: &Value) -> io::Result<()> {
    hex(text(value, "attempt_id")?, 32, "attempt_id")?;
    hex(text(value, "source_revision")?, 64, "source_revision")?;
    timestamp(text(value, "started_at")?)
}
fn success(value: &Value) -> io::Result<()> {
    keys(value, SUCCESS)?;
    common(value)?;
    timestamp(text(value, "completed_at")?)?;
    hex(text(value, "artifact_revision")?, 64, "artifact_revision")?;
    hex(
        text(value, "artifact_manifest_sha256")?,
        64,
        "artifact_manifest_sha256",
    )?;
    if text(value, "completed_at")? < text(value, "started_at")? {
        return Err(bad("completed_at precedes started_at"));
    }
    Ok(())
}
fn safe_error(error: &str) -> io::Result<()> {
    if error.is_empty() || error.contains(['\0', '\u{7f}']) {
        return Err(bad(
            "error must be non-empty and contain no unsafe control character",
        ));
    }
    Ok(())
}
fn validate(value: &Value, consumer: &str) -> io::Result<()> {
    keys(
        value,
        &[
            "schema_version",
            "consumer",
            "latest",
            "last_success",
            "previous_success",
        ],
    )?;
    if text(value, "schema_version")? != SCHEMA_VERSION {
        return Err(bad("unsupported schema version"));
    }
    if text(value, "consumer")? != consumer {
        return Err(bad("downstream state consumer does not match"));
    }
    let latest = &value["latest"];
    match text(latest, "state")? {
        "running" => {
            keys(
                latest,
                &["attempt_id", "source_revision", "started_at", "state"],
            )?;
            common(latest)?;
        }
        "succeeded" => {
            let mut expected = SUCCESS.to_vec();
            expected.push("state");
            keys(latest, &expected)?;
            let mut record = latest.clone();
            record.as_object_mut().unwrap().remove("state");
            success(&record)?;
            if value["last_success"] != record {
                return Err(bad("succeeded latest attempt must be last_success"));
            }
        }
        "failed" => {
            keys(
                latest,
                &[
                    "attempt_id",
                    "source_revision",
                    "started_at",
                    "completed_at",
                    "state",
                    "error",
                ],
            )?;
            common(latest)?;
            timestamp(text(latest, "completed_at")?)?;
            let error = text(latest, "error")?;
            safe_error(error)?;
            if error.chars().count() > MAX_ERROR_CHARS {
                return Err(bad("latest.error exceeds character bound"));
            }
            // Python's failed-record validator does not compare start/completion order.
        }
        _ => return Err(bad("latest attempt has unsupported state")),
    }
    let last = &value["last_success"];
    let previous = &value["previous_success"];
    if !last.is_null() {
        success(last)?;
    }
    if !previous.is_null() {
        success(previous)?;
        if last.is_null() {
            return Err(bad("previous_success requires last_success"));
        }
        if last["attempt_id"] == previous["attempt_id"] {
            return Err(bad("success attempts must differ"));
        }
        if text(previous, "completed_at")? > text(last, "completed_at")? {
            return Err(bad("previous_success is newer than last_success"));
        }
    }
    Ok(())
}
fn canonical(raw: &[u8]) -> io::Result<Vec<u8>> {
    // Foundation performs decoded duplicate-name rejection, finite UTF-8 parsing,
    // sorted Python JSON escaping and exactly one LF. No serde last-wins input.
    canonical_raw_bytes_v1(
        raw,
        CanonicalProfile::CorpusSnapshotV1,
        JsonLimits {
            max_bytes: MAX_STATE_BYTES,
            max_depth: 16,
            max_visits: 256,
            max_integer_digits: 4300,
        },
    )
    .map_err(|e| bad(format!("invalid downstream JSON: {e}")))
}
fn now() -> io::Result<String> {
    let mut seconds: libc::time_t = 0;
    if unsafe { libc::time(&mut seconds) } == -1 {
        return Err(io::Error::last_os_error());
    }
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::gmtime_r(&seconds, &mut tm) }.is_null() {
        return Err(bad("cannot obtain UTC time"));
    }
    let value = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    );
    timestamp(&value)?;
    Ok(value)
}
fn random_id() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn cstr(name: &std::ffi::OsStr) -> io::Result<CString> {
    CString::new(name.as_bytes()).map_err(|_| bad("path contains NUL"))
}
fn at(directory: &File, name: &std::ffi::OsStr, flags: i32) -> io::Result<File> {
    let name = cstr(name)?;
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
            0o600,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn regular(directory: &File, name: &str, flags: i32) -> io::Result<File> {
    let file = at(directory, std::ffi::OsStr::new(name), flags)?;
    if !file.metadata()?.is_file() {
        return Err(bad("managed status file must be regular"));
    }
    Ok(file)
}
fn root_open_create(root: &Path, create: bool) -> io::Result<File> {
    let mut dir = File::open("/")?;
    for component in root.components() {
        if let Component::Normal(name) = component {
            match at(&dir, name, libc::O_RDONLY | libc::O_DIRECTORY) {
                Ok(next) => dir = next,
                Err(e) if create && e.kind() == io::ErrorKind::NotFound => {
                    let name_c = cstr(name)?;
                    if unsafe { libc::mkdirat(dir.as_raw_fd(), name_c.as_ptr(), 0o777) } != 0 {
                        let e = io::Error::last_os_error();
                        if e.kind() != io::ErrorKind::AlreadyExists {
                            return Err(e);
                        }
                    }
                    dir = at(&dir, name, libc::O_RDONLY | libc::O_DIRECTORY)?;
                }
                Err(e) => return Err(e),
            }
        }
    }
    Ok(dir)
}
fn inspect(root: &Path) -> io::Result<bool> {
    let mut current = PathBuf::new();
    for component in root.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(bad("downstream status path must not contain symlinks"));
            }
            Ok(m) if !m.is_dir() => {
                return Err(bad("downstream status root component must be a directory"));
            }
            Ok(_) => (),
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e),
        }
    }
    Ok(true)
}
fn optional_regular(directory: &File, name: &str) -> io::Result<Option<File>> {
    match regular(directory, name, libc::O_RDONLY) {
        Ok(f) => Ok(Some(f)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
fn lock(file: &File, exclusive: bool) -> io::Result<()> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if std::time::Instant::now() >= deadline {
            return Err(bad("downstream status lock deadline"));
        }
        if unsafe {
            libc::flock(
                file.as_raw_fd(),
                if exclusive {
                    libc::LOCK_EX | libc::LOCK_NB
                } else {
                    libc::LOCK_SH | libc::LOCK_NB
                },
            )
        } == 0
        {
            return Ok(());
        }
        let e = io::Error::last_os_error();
        if e.kind() == io::ErrorKind::WouldBlock {
            std::thread::sleep(std::time::Duration::from_millis(10));
        } else if e.kind() != io::ErrorKind::Interrupted {
            return Err(e);
        }
    }
}

/// Small local state for one independently published downstream consumer.
/// Linux only, matching the source's flock and O_NOFOLLOW host contract.
pub struct Status {
    root: PathBuf,
    consumer: String,
}
pub type DownstreamStatus = Status;
impl Status {
    pub fn new(root: &Path, consumer: &str) -> io::Result<Self> {
        if !["kag", "stats"].contains(&consumer) {
            return Err(bad("consumer must be exactly kag or stats"));
        }
        if root
            .as_os_str()
            .as_bytes()
            .split(|b| *b == b'/')
            .any(|p| p == b"." || p == b"..")
        {
            return Err(bad("downstream status root contains traversal segments"));
        }
        let root = if root.is_absolute() {
            root.to_owned()
        } else {
            std::env::current_dir()?.join(root)
        };
        inspect(&root)?;
        Ok(Self {
            root,
            consumer: consumer.into(),
        })
    }
    fn read(&self, directory: &File) -> io::Result<Option<Value>> {
        let Some(file) = optional_regular(directory, "state.json")? else {
            return Ok(None);
        };
        let mut raw = Vec::new();
        file.take(MAX_STATE_BYTES as u64 + 1)
            .read_to_end(&mut raw)?;
        if raw.len() > MAX_STATE_BYTES {
            return Err(bad("downstream state exceeds schema byte bound"));
        }
        if canonical(&raw)? != raw {
            return Err(bad("downstream state is not canonical JSON"));
        }
        let state: Value = serde_json::from_slice(&raw)
            .map_err(|e| bad(format!("invalid downstream state: {e}")))?;
        validate(&state, &self.consumer)?;
        Ok(Some(state))
    }
    fn writer(&self) -> io::Result<(File, File)> {
        inspect(&self.root)?;
        let directory = root_open_create(&self.root, true)?;
        let file = regular(&directory, ".lock", libc::O_RDWR | libc::O_CREAT)?;
        lock(&file, true)?;
        Ok((directory, file)) // dropping file releases flock even on error paths
    }
    fn write(&self, directory: &File, state: &Value) -> io::Result<()> {
        validate(state, &self.consumer)?;
        optional_regular(directory, "state.json")?;
        let raw = canonical(&serde_json::to_vec(state).map_err(|e| bad(e.to_string()))?)?;
        let name = format!(".state.{}", random_id()?);
        let temp = regular(
            directory,
            &name,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        )?;
        let temporary = cstr(std::ffi::OsStr::new(&name))?;
        let result = (|| {
            let mut file = temp;
            if unsafe { libc::fchmod(file.as_raw_fd(), 0o600) } != 0 {
                return Err(io::Error::last_os_error());
            }
            file.write_all(&raw)?;
            file.sync_all()?;
            let dest = cstr(std::ffi::OsStr::new("state.json"))?;
            if unsafe {
                libc::renameat(
                    directory.as_raw_fd(),
                    temporary.as_ptr(),
                    directory.as_raw_fd(),
                    dest.as_ptr(),
                )
            } != 0
            {
                return Err(io::Error::last_os_error());
            }
            directory.sync_all()
        })();
        // After rename this harmlessly returns ENOENT; before rename it removes only our temp.
        unsafe { libc::unlinkat(directory.as_raw_fd(), temporary.as_ptr(), 0) };
        result
    }
    pub fn begin(&self, source_revision: &str) -> io::Result<String> {
        hex(source_revision, 64, "source_revision")?;
        let attempt = random_id()?;
        let started = now()?;
        let (directory, _guard) = self.writer()?;
        let mut state = self.read(&directory)?.unwrap_or_else(||json!({"schema_version":SCHEMA_VERSION,"consumer":self.consumer,"latest":null,"last_success":null,"previous_success":null}));
        state["latest"] = json!({"attempt_id":attempt,"source_revision":source_revision,"started_at":started,"state":"running"});
        self.write(&directory, &state)?;
        Ok(attempt)
    }
    fn running(&self, directory: &File, attempt: &str) -> io::Result<Value> {
        let state = self
            .read(directory)?
            .ok_or_else(|| bad("cannot complete without a begun attempt"))?;
        if text(&state["latest"], "attempt_id")? != attempt {
            return Err(bad("attempt is stale and cannot advance downstream status"));
        }
        if text(&state["latest"], "state")? != "running" {
            return Err(bad("only a running latest attempt may complete"));
        }
        Ok(state)
    }
    pub fn succeed(
        &self,
        attempt: &str,
        artifact_revision: &str,
        artifact_manifest_sha256: &str,
    ) -> io::Result<()> {
        hex(attempt, 32, "attempt_id")?;
        hex(artifact_revision, 64, "artifact_revision")?;
        hex(artifact_manifest_sha256, 64, "artifact_manifest_sha256")?;
        let (directory, _guard) = self.writer()?;
        let mut state = self.running(&directory, attempt)?;
        let mut record = state["latest"].clone();
        record.as_object_mut().unwrap().remove("state");
        record["completed_at"] = json!(now()?);
        record["artifact_revision"] = json!(artifact_revision);
        record["artifact_manifest_sha256"] = json!(artifact_manifest_sha256);
        state["previous_success"] = state["last_success"].clone();
        state["last_success"] = record.clone();
        record["state"] = json!("succeeded");
        state["latest"] = record;
        self.write(&directory, &state)
    }
    pub fn fail(&self, attempt: &str, error: &str) -> io::Result<()> {
        hex(attempt, 32, "attempt_id")?;
        safe_error(error)?;
        let error: String = error.chars().take(MAX_ERROR_CHARS).collect();
        let (directory, _guard) = self.writer()?;
        let mut state = self.running(&directory, attempt)?;
        state["latest"]["completed_at"] = json!(now()?);
        state["latest"]["state"] = json!("failed");
        state["latest"]["error"] = json!(error);
        self.write(&directory, &state)
    }
    pub fn status(&self, expected_source_revision: &str) -> io::Result<Value> {
        hex(expected_source_revision, 64, "expected_source_revision")?;
        if !inspect(&self.root)? {
            return Ok(json!({"state":null,"freshness":"missing","latest_attempt":null}));
        }
        let directory = root_open_create(&self.root, false)?;
        // Validate both managed paths before taking an optional shared read lock.
        optional_regular(&directory, "state.json")?;
        let guard = optional_regular(&directory, ".lock")?;
        if let Some(file) = &guard {
            lock(file, false)?;
        }
        let Some(state) = self.read(&directory)? else {
            return Ok(json!({"state":null,"freshness":"missing","latest_attempt":null}));
        };
        let freshness = if state["last_success"].is_null() {
            "missing"
        } else if text(&state["last_success"], "source_revision")? == expected_source_revision {
            "current"
        } else {
            "stale"
        };
        Ok(json!({"latest_attempt":state["latest"],"state":state,"freshness":freshness}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("tos-kag-status-{}", random_id().unwrap())))
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn finite_duplicate_free_canonical_state() {
        assert!(canonical(br#"{"x":1,"x":2}"#).is_err());
        assert!(canonical(br#"{"x":NaN}"#).is_err());
        assert!(canonical(br#"{"x":"\ud800"}"#).is_err());
        assert_eq!(
            canonical(r#"{"z":"строка","a":null}"#.as_bytes()).unwrap(),
            "{\"a\":null,\"z\":\"строка\"}\n".as_bytes()
        );
        assert!(timestamp("2026-02-29T00:00:00Z").is_err());
        assert!(timestamp("2024-02-29T00:00:00Z").is_ok());
    }
    #[test]
    fn lifecycle_stale_attempts_and_two_success_history() {
        let scratch = Scratch::new();
        let root = scratch.0.join("state");
        let status = Status::new(&root, "kag").unwrap();
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let artifact = "c".repeat(64);
        let manifest = "d".repeat(64);
        assert_eq!(
            status.status(&a).unwrap(),
            json!({"state":null,"freshness":"missing","latest_attempt":null})
        );
        assert!(!scratch.0.exists());
        let first = status.begin(&a).unwrap();
        status.succeed(&first, &artifact, &manifest).unwrap();
        let old = status.begin(&b).unwrap();
        let second = status.begin(&b).unwrap();
        assert!(
            status
                .fail(&old, "failure")
                .unwrap_err()
                .to_string()
                .contains("stale")
        );
        assert!(status.succeed(&old, &artifact, &manifest).is_err());
        status.succeed(&second, &artifact, &manifest).unwrap();
        let state = status.status(&b).unwrap();
        assert_eq!(state["freshness"], "current");
        assert_eq!(state["state"]["previous_success"]["attempt_id"], first);
        assert_eq!(status.status(&a).unwrap()["freshness"], "stale");
        let failed = status.begin(&a).unwrap();
        status.fail(&failed, &"λ".repeat(5000)).unwrap();
        let state = status.status(&b).unwrap();
        assert_eq!(state["freshness"], "current");
        assert_eq!(state["state"]["last_success"]["attempt_id"], second);
        assert_eq!(
            state["latest_attempt"]["error"]
                .as_str()
                .unwrap()
                .chars()
                .count(),
            4096
        );
        assert!(status.succeed(&failed, &artifact, &manifest).is_err());
    }
    #[test]
    fn symlinks_and_noncanonical_state_refused_without_advancing() {
        use std::os::unix::fs::symlink;
        let scratch = Scratch::new();
        fs::create_dir_all(&scratch.0).unwrap();
        symlink(&scratch.0, scratch.0.join("alias")).unwrap();
        assert!(Status::new(&scratch.0.join("alias/state"), "kag").is_err());
        let status = Status::new(&scratch.0.join("state"), "kag").unwrap();
        let a = "a".repeat(64);
        let attempt = status.begin(&a).unwrap();
        assert!(status.fail(&attempt, "bad\0error").is_err());
        assert_eq!(
            status.status(&a).unwrap()["latest_attempt"]["state"],
            "running"
        );
        let path = scratch.0.join("state/state.json");
        let mut raw = fs::read(&path).unwrap();
        raw.insert(0, b' ');
        fs::write(&path, raw).unwrap();
        assert!(
            status
                .status(&a)
                .unwrap_err()
                .to_string()
                .contains("canonical")
        );
        fs::remove_file(&path).unwrap();
        symlink(scratch.0.join("outside"), &path).unwrap();
        assert!(status.status(&a).is_err());
    }
}
