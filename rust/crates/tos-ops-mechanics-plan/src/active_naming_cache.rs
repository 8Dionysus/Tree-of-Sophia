//! Optional local performance hints for active_naming; never a release input.
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};
use tos_foundation::Digest256;

const MAX_CACHE_BYTES: u64 = 64 * 1024 * 1024;

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
fn sql(error: rusqlite::Error) -> io::Error {
    io::Error::other(error.to_string())
}

pub(super) fn validate_selection(root: &Path, path: &Path) -> io::Result<()> {
    if !path.is_absolute()
        || path.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        || path.file_name().is_none()
    {
        return Err(invalid("feedback cache needs an absolute file path without dot components"));
    }
    if path.starts_with(root) {
        return Err(invalid("feedback cache must be outside the repository"));
    }
    // Check the nearest existing ancestor before creating any missing directory.
    // In particular an external symlink into the repository cannot authorize a write.
    let mut ancestor = path;
    while !ancestor.exists() {
        ancestor = ancestor.parent().ok_or_else(|| invalid("cache has no existing ancestor"))?;
    }
    let resolved = fs::canonicalize(ancestor)?.join(path.strip_prefix(ancestor).map_err(|e| invalid(e.to_string()))?);
    if resolved.starts_with(root) {
        return Err(invalid("feedback cache must be outside the repository"));
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct FeedbackStats {
    pub path: PathBuf,
    pub hits: u64,
    pub misses: u64,
}

pub(super) struct FeedbackCache {
    connection: Connection,
    policy: String,
    stats: FeedbackStats,
}

fn policy() -> String {
    // Binding source and resolved dependencies also fences Unicode/regex/runtime
    // updates. Python cache policies deliberately do not match this native one.
    Digest256::of_bytes(&[
        include_bytes!("active_naming.rs").as_slice(),
        include_bytes!("active_naming_cache.rs").as_slice(),
        include_bytes!("../../../../Cargo.lock").as_slice(),
        b"tos-native-active-naming-feedback-v1",
    ].concat()).to_hex()
}

#[cfg(target_os = "linux")]
fn prepare_cache(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let parent = path.parent().ok_or_else(|| invalid("cache parent missing"))?;
    fs::create_dir_all(parent)?;
    let held = tos_fd_open::open_absolute_directory(parent).map_err(io::Error::other)?;
    let metadata = held.metadata()?;
    if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o022 != 0 {
        return Err(invalid("feedback cache parent must be owned and not writable by other users"));
    }
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    let file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            options.create_new(true).mode(0o600).open(path)?
        }
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > MAX_CACHE_BYTES || metadata.nlink() != 1
        || metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o022 != 0
    {
        return Err(invalid("feedback cache must be an owned single-link regular file under 64 MiB"));
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn prepare_cache(_path: &Path) -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "feedback cache is supported on Linux"))
}

impl FeedbackCache {
    pub(super) fn open(path: &Path, deadline: Instant) -> io::Result<Self> {
        prepare_cache(path)?;
        // SQLite NOFOLLOW rejects symlinks anywhere in its canonical pathname.
        // A /proc/self/fd alias would itself be a symlink, so use the selected path.
        let connection = Connection::open_with_flags(path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_NOFOLLOW).map_err(sql)?;
        connection.busy_timeout(Duration::ZERO).map_err(sql)?;
        connection.progress_handler(1000, Some(move || Instant::now() >= deadline));
        connection.set_limit(rusqlite::limits::Limit::SQLITE_LIMIT_LENGTH, 16 * 1024).map_err(sql)?;
        connection.pragma_update(None, "trusted_schema", false).map_err(sql)?;
        connection.pragma_update(None, "journal_mode", "DELETE").map_err(sql)?;
        connection.pragma_update(None, "cache_size", -1024).map_err(sql)?;
        let page_size: u64 = connection.pragma_query_value(None, "page_size", |r| r.get(0)).map_err(sql)?;
        if !(512..=65536).contains(&page_size) || !page_size.is_power_of_two() {
            return Err(invalid("unsupported feedback cache page size"));
        }
        connection.pragma_update(None, "max_page_count", MAX_CACHE_BYTES / page_size).map_err(sql)?;
        connection.execute_batch("CREATE TABLE IF NOT EXISTS content_results (
            policy TEXT NOT NULL, content_digest TEXT NOT NULL, result_json TEXT NOT NULL,
            PRIMARY KEY(policy, content_digest));").map_err(sql)?;
        let policy = policy();
        connection.execute("DELETE FROM content_results WHERE policy != ?1", [&policy]).map_err(sql)?;
        connection.execute_batch("BEGIN").map_err(sql)?;
        Ok(Self { connection, policy,
            stats: FeedbackStats { path: path.into(), hits: 0, misses: 0 } })
    }

    pub(super) fn lookup(&mut self, text: &str) -> io::Result<Option<Option<String>>> {
        let digest = Digest256::of_bytes(text.as_bytes()).to_hex();
        let row: Option<rusqlite::types::Value> = self.connection.query_row(
            "SELECT result_json FROM content_results WHERE policy=?1 AND content_digest=?2",
            params![self.policy, digest], |r| r.get(0)).optional().map_err(sql)?;
        if let Some(raw) = row {
            if let rusqlite::types::Value::Text(raw) = raw {
                if let Ok(result) = serde_json::from_str::<Option<String>>(&raw) {
                if result.as_ref().is_none_or(|s| s.len() <= super::MAX_ISSUE_BYTES) {
                    self.stats.hits += 1;
                    return Ok(Some(result));
                }
            }
            }
            self.connection.execute("DELETE FROM content_results WHERE policy=?1 AND content_digest=?2",
                params![self.policy, digest]).map_err(sql)?;
        }
        self.stats.misses += 1;
        Ok(None)
    }

    pub(super) fn store(&self, text: &str, result: Option<&str>) -> io::Result<()> {
        let encoded = serde_json::to_string(&result).map_err(io::Error::other)?;
        if encoded.len() > 16 * 1024 {
            return Err(invalid("feedback cache result exceeds row budget"));
        }
        self.connection.execute("INSERT OR REPLACE INTO content_results(policy, content_digest, result_json) VALUES(?1,?2,?3)",
            params![self.policy, Digest256::of_bytes(text.as_bytes()).to_hex(), encoded]).map_err(sql)?;
        Ok(())
    }

    pub(super) fn stats(&self) -> FeedbackStats { self.stats.clone() }
    pub(super) fn finish(self) -> io::Result<()> {
        self.connection.execute_batch("COMMIT").map_err(sql)
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::active_naming::validate_with_feedback;
    use std::os::unix::fs::symlink;

    struct Fixture { directory: PathBuf, root: PathBuf, cache: PathBuf }
    impl Fixture {
        fn new() -> Self {
            let directory = std::env::temp_dir().join(format!("tos-naming-cache-{}-{}", std::process::id(),
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
            let root = directory.join("source");
            fs::create_dir_all(root.join("mechanics/experience/v0.7")).unwrap();
            fs::write(root.join("clean.txt"), "plain-text").unwrap();
            fs::write(root.join("bad.txt"), "seed-route").unwrap();
            let cache = directory.join("feedback.sqlite");
            Self { directory, root, cache }
        }
    }
    impl Drop for Fixture { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.directory); } }

    #[test]
    fn cache_hits_keep_content_edits_and_paths_live() {
        let fixture = Fixture::new();
        let first = validate_with_feedback(&fixture.root, Some(&fixture.cache)).unwrap();
        assert!(first.warnings.is_empty(), "{:?}", first.warnings);
        assert_eq!((first.feedback.as_ref().unwrap().hits, first.feedback.as_ref().unwrap().misses), (0, 2));
        let second = validate_with_feedback(&fixture.root, Some(&fixture.cache)).unwrap();
        assert_eq!(first.issues, second.issues);
        assert_eq!((second.feedback.as_ref().unwrap().hits, second.feedback.as_ref().unwrap().misses), (2, 0));
        fs::write(fixture.root.join("bad.txt"), "fixed-text").unwrap();
        let repaired = validate_with_feedback(&fixture.root, Some(&fixture.cache)).unwrap();
        assert!(!repaired.issues.iter().any(|s| s.contains("bad.txt")));
        assert!(repaired.issues.iter().any(|s| s.contains("v0.7")));
        assert_eq!((repaired.feedback.as_ref().unwrap().hits, repaired.feedback.as_ref().unwrap().misses), (1, 1));
        fs::rename(fixture.root.join("bad.txt"), fixture.root.join("seed-route.txt")).unwrap();
        let renamed = validate_with_feedback(&fixture.root, Some(&fixture.cache)).unwrap();
        assert!(renamed.issues.iter().any(|s| s.contains("seed-route.txt")));
        assert_eq!(renamed.feedback.as_ref().unwrap().hits, 2);
        assert_eq!(renamed.issues, crate::active_naming::validate(&fixture.root).unwrap());
    }

    #[test]
    fn corrupt_stale_locked_and_invalid_cache_never_skip_live_validation() {
        let fixture = Fixture::new();
        let first = validate_with_feedback(&fixture.root, Some(&fixture.cache)).unwrap();
        assert!(first.warnings.is_empty());
        let connection = Connection::open(&fixture.cache).unwrap();
        for invalid in ["not-json", "false", "{}"] {
            connection.execute("UPDATE content_results SET result_json=?1", [invalid]).unwrap();
            let report = validate_with_feedback(&fixture.root, Some(&fixture.cache)).unwrap();
            assert_eq!(first.issues, report.issues);
            assert_eq!(report.feedback.unwrap().misses, 2);
        }
        connection.execute("UPDATE content_results SET result_json=x'ff'", []).unwrap();
        let binary_row = validate_with_feedback(&fixture.root, Some(&fixture.cache)).unwrap();
        assert_eq!(first.issues, binary_row.issues);
        assert_eq!(binary_row.feedback.unwrap().misses, 2);
        connection.execute("INSERT INTO content_results VALUES('old-policy','old-content','null')", []).unwrap();
        validate_with_feedback(&fixture.root, Some(&fixture.cache)).unwrap();
        let stale: u64 = connection.query_row("SELECT count(*) FROM content_results WHERE policy='old-policy'", [], |r| r.get(0)).unwrap();
        assert_eq!(stale, 0);
        connection.execute_batch("BEGIN EXCLUSIVE").unwrap();
        let start = Instant::now();
        let locked = validate_with_feedback(&fixture.root, Some(&fixture.cache)).unwrap();
        assert!(start.elapsed() < Duration::from_secs(2));
        assert_eq!(first.issues, locked.issues);
        assert!(!locked.warnings.is_empty());
        connection.execute_batch("ROLLBACK; DROP TABLE content_results; CREATE TABLE content_results(wrong TEXT)").unwrap();
        let malformed = validate_with_feedback(&fixture.root, Some(&fixture.cache)).unwrap();
        assert_eq!(first.issues, malformed.issues);
        assert!(!malformed.warnings.is_empty());
        connection.execute_batch("BEGIN EXCLUSIVE; ROLLBACK").unwrap();
        assert!(validate_with_feedback(&fixture.root, Some(&fixture.root.join("internal.sqlite"))).is_err());
        let alias = fixture.directory.join("inside");
        symlink(&fixture.root, &alias).unwrap();
        assert!(validate_with_feedback(&fixture.root, Some(&alias.join("internal.sqlite"))).is_err());
        assert!(!fixture.root.join("internal.sqlite").exists());
    }
}
