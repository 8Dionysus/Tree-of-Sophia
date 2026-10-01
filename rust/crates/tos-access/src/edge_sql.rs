//! Trusted public-producer SQL framing and explicit offline D1 import.
//! No cloud invocation, serving-store selection or owner admission occurs here.
use rusqlite::{Connection, OpenFlags, TransactionBehavior, limits::Limit};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    ffi::CString,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    time::{Duration, Instant},
};

const MAX_STATEMENT_BYTES: usize = 100_000;
const MAX_VALUE_BYTES: i32 = 2_000_000;
type Result<T> = std::result::Result<T, String>;

fn input(path: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("SQL input must be a regular file".into());
    }
    Ok(file)
}

fn identity(file: &std::fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        file.dev(),
        file.ino(),
        file.len(),
        file.mtime(),
        file.mtime_nsec(),
        file.ctime(),
        file.ctime_nsec(),
    )
}

fn unchanged(path: &Path, original: &std::fs::Metadata) -> Result<()> {
    if identity(&std::fs::symlink_metadata(path).map_err(|e| e.to_string())?) != identity(original)
    {
        return Err("producer SQL source changed during operation".into());
    }
    Ok(())
}

struct Statements<R: BufRead> {
    reader: R,
    done: bool,
    deadline: Option<Instant>,
}
impl<R: BufRead> Statements<R> {
    fn next(&mut self) -> Result<Option<Vec<u8>>> {
        if self.done {
            return Ok(None);
        }
        let envelope = MAX_STATEMENT_BYTES + 2;
        let mut pending = Vec::new();
        loop {
            if self
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
            {
                return Err("SQL stream deadline exceeded".into());
            }
            // Cap allocation even when a corrupt record never contains LF.
            let read = self
                .reader
                .by_ref()
                .take((envelope + 1 - pending.len()) as u64)
                .read_until(b'\n', &mut pending)
                .map_err(|e| e.to_string())?;
            if pending.len() > envelope {
                return Err("producer SQL statement exceeds 100000 bytes".into());
            }
            if read == 0 {
                self.done = true;
                if pending.iter().any(|b| !b.is_ascii_whitespace()) {
                    return Err("producer SQL ends with an incomplete statement".into());
                }
                return Ok((!pending.is_empty()).then_some(pending));
            }
            let text = std::str::from_utf8(&pending).map_err(|_| "producer SQL is not UTF-8")?;
            let ctext = CString::new(text).map_err(|_| "NUL in producer SQL")?;
            // SQLite's parser recognizes literal newlines and trigger bodies;
            // semicolons and physical lines alone are never statement borders.
            if unsafe { rusqlite::ffi::sqlite3_complete(ctext.as_ptr()) } != 0 {
                if text.trim_end_matches(['\r', '\n']).len() > MAX_STATEMENT_BYTES {
                    return Err("producer SQL statement exceeds 100000 bytes".into());
                }
                return Ok(Some(pending));
            }
        }
    }
}

fn chunk(source: &Path, output: &Path, offset: u64, maximum: u64) -> Result<Value> {
    chunk_until(source, output, offset, maximum, None)
}

fn chunk_until(
    source: &Path,
    output: &Path,
    offset: u64,
    maximum: u64,
    deadline: Option<Instant>,
) -> Result<Value> {
    if maximum == 0 {
        return Err("SQL chunk size must be positive".into());
    }
    let mut file = input(source)?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if offset > metadata.len() {
        return Err("SQL offset exceeds input".into());
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| e.to_string())?;
    let mut target = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(output)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        let mut statements = Statements {
            reader: BufReader::new(file.take(metadata.len() - offset)),
            done: false,
            deadline,
        };
        let mut written = 0u64;
        let mut count = 0u64;
        let mut eof = true;
        while let Some(statement) = statements.next()? {
            let length = statement.len() as u64;
            if written != 0 && written.checked_add(length).ok_or("SQL byte overflow")? > maximum {
                eof = false;
                break;
            }
            target.write_all(&statement).map_err(|e| e.to_string())?;
            written = written.checked_add(length).ok_or("SQL byte overflow")?;
            count += u64::from(statement.iter().any(|b| !b.is_ascii_whitespace()));
        }
        unchanged(source, &metadata)?;
        target.sync_all().map_err(|e| e.to_string())?;
        Ok(
            json!({"next_offset": offset.checked_add(written).ok_or("SQL offset overflow")?,
            "bytes": written, "statements": count, "eof": eof}),
        )
    })();
    drop(target);
    if result.is_err() {
        let _ = std::fs::remove_file(output);
    }
    result
}

fn revision(db: &Connection) -> Result<Option<String>> {
    let exists: bool = db
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='edge_meta')",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if !exists {
        return Ok(None);
    }
    let mut columns = db
        .prepare("PRAGMA table_info(edge_meta)")
        .map_err(|e| e.to_string())?;
    let names: Vec<String> = columns
        .query_map([], |row| row.get(1))
        .map_err(|e| e.to_string())?
        .collect::<std::result::Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let query = if names.iter().any(|name| name == "json_chunk")
        && names.iter().any(|name| name == "part")
    {
        "SELECT json_chunk FROM edge_meta WHERE key='data_revision' ORDER BY part"
    } else if names.iter().any(|name| name == "json") {
        "SELECT json FROM edge_meta WHERE key='data_revision'"
    } else {
        return Err("local edge_meta schema is unsupported".into());
    };
    let mut query = db.prepare(query).map_err(|e| e.to_string())?;
    let mut rows = query.query([]).map_err(|e| e.to_string())?;
    let mut text = String::new();
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let part: String = row.get(0).map_err(|e| e.to_string())?;
        if text
            .len()
            .checked_add(part.len())
            .is_none_or(|len| len > MAX_VALUE_BYTES as usize)
        {
            return Err("local revision metadata exceeds budget".into());
        }
        text.push_str(&part);
    }
    if text.is_empty() {
        return Ok(None);
    }
    let value: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if !value.is_object() {
        return Err("local revision metadata is not an object".into());
    }
    match value.get("sha256") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(revision)) => Ok(Some(revision.clone())),
        // A malformed present value must never become an empty baseline and
        // admit --base null. The retained oracle rejects this baseline too.
        Some(_) => Err("local revision sha256 is not a string".into()),
    }
}

fn import(database: &Path, source: &Path, base: Option<&str>, target: &str) -> Result<Value> {
    // Existing explicit local store only, never CREATE or a URI-selected store.
    let metadata = std::fs::symlink_metadata(database).map_err(|e| e.to_string())?;
    if !metadata.is_file() {
        return Err("local database must be an existing regular file".into());
    }
    let source_path = source;
    let source = input(source_path)?;
    let source_metadata = source.metadata().map_err(|e| e.to_string())?;
    let mut db = Connection::open_with_flags(
        database,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| e.to_string())?;
    db.set_limit(Limit::SQLITE_LIMIT_LENGTH, MAX_VALUE_BYTES)
        .map_err(|e| e.to_string())?;
    db.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, MAX_STATEMENT_BYTES as i32)
        .map_err(|e| e.to_string())?;
    let transaction = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    if revision(&transaction)?.as_deref() != base {
        return Err("local bootstrap baseline changed".into());
    }
    let mut statements = Statements {
        reader: BufReader::new(source.take(source_metadata.len())),
        done: false,
        deadline: None,
    };
    let mut count = 0u64;
    while let Some(statement) = statements.next()? {
        let text = std::str::from_utf8(&statement).map_err(|_| "producer SQL is not UTF-8")?;
        if text.trim().is_empty() {
            continue;
        }
        let mut statement = transaction
            .prepare(text.trim_end_matches(['\r', '\n']))
            .map_err(|e| e.to_string())?;
        // Like sqlite3.Connection.execute, also allow bounded producer PRAGMA
        // or SELECT records; step once and release the unused result cursor.
        statement.raw_query().next().map_err(|e| e.to_string())?;
        count += 1;
    }
    unchanged(source_path, &source_metadata)?;
    if revision(&transaction)?.as_deref() != Some(target) {
        return Err("local bootstrap target revision mismatch".into());
    }
    transaction.commit().map_err(|e| e.to_string())?;
    Ok(json!({"statements":count,"revision":target,"publication":"local-sqlite-transaction"}))
}

fn options<'a>(args: &'a [String], allowed: &[&str]) -> Result<BTreeMap<&'a str, &'a str>> {
    let mut options = BTreeMap::new();
    let mut remaining = args.iter().skip(1);
    while let Some(key) = remaining.next() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("unknown Edge SQL option: {key}"));
        }
        let value = remaining
            .next()
            .ok_or_else(|| format!("{key} requires a value"))?;
        if options.insert(key.as_str(), value.as_str()).is_some() {
            return Err(format!("duplicate option: {key}"));
        }
    }
    Ok(options)
}

fn run(args: &[String]) -> Result<Value> {
    let allowed: &[&str] = if args[0] == "edge-sql-chunk" {
        &["--source", "--output", "--offset", "--maximum-bytes"]
    } else {
        &["--database", "--sql", "--base", "--target"]
    };
    let options = options(args, allowed)?;
    let value = |key| {
        options
            .get(key)
            .copied()
            .ok_or_else(|| format!("{key} is required"))
    };
    let number = |key| {
        value(key)?
            .parse::<u64>()
            .map_err(|_| format!("invalid {key}"))
    };
    if args[0] == "edge-sql-chunk" {
        chunk(
            Path::new(value("--source")?),
            Path::new(value("--output")?),
            number("--offset")?,
            number("--maximum-bytes")?,
        )
    } else {
        let base = value("--base")?;
        let target = value("--target")?;
        if target.is_empty() {
            return Err("target revision is empty".into());
        }
        import(
            Path::new(value("--database")?),
            Path::new(value("--sql")?),
            (base != "null").then_some(base),
            target,
        )
    }
}

struct OwnedChunk(Option<std::path::PathBuf>, bool);
impl OwnedChunk {
    fn clear(&mut self) -> Result<()> {
        if let Some(path) = self.0.take() {
            std::fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        self.1 = false;
        Ok(())
    }
}
impl Drop for OwnedChunk {
    fn drop(&mut self) {
        // After emitting a chunk, its consumer may still be opening/uploading
        // it. Deadline/EOF never unlinks those bytes beneath that consumer;
        // the platform's owned directory closeout handles abandonment.
        if !self.1 {
            if let Some(path) = self.0.take() {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

fn next_request(deadline: Instant) -> Result<bool> {
    let mut word = Vec::new();
    loop {
        let now = Instant::now();
        if now >= deadline {
            return Err("SQL stream deadline exceeded".into());
        }
        let milliseconds =
            (deadline.duration_since(now).as_millis() + 1).min(i32::MAX as u128) as i32;
        let mut fd = libc::pollfd {
            fd: libc::STDIN_FILENO,
            events: libc::POLLIN,
            revents: 0,
        };
        let ready = unsafe { libc::poll(&mut fd, 1, milliseconds) };
        if ready == 0 {
            continue;
        }
        if ready < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error.to_string());
        }
        let mut byte = 0u8;
        let read = unsafe { libc::read(libc::STDIN_FILENO, (&mut byte as *mut u8).cast(), 1) };
        if read == 0 {
            return if word.is_empty() {
                Ok(false)
            } else {
                Err("incomplete SQL stream request".into())
            };
        }
        if read < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error.to_string());
        }
        word.push(byte);
        if byte == b'\n' {
            return if word == b"next\n" {
                Ok(true)
            } else {
                Err("unknown SQL stream request".into())
            };
        }
        if word.len() >= 5 {
            return Err("SQL stream request exceeds budget".into());
        }
    }
}

fn stream(args: &[String], stdout: &mut dyn Write) -> Result<()> {
    let options = options(
        args,
        &[
            "--source",
            "--directory",
            "--maximum-bytes",
            "--maximum-bytes-type",
            "--max-seconds",
        ],
    )?;
    let value = |key| {
        options
            .get(key)
            .copied()
            .ok_or_else(|| format!("{key} is required"))
    };
    let maximum = value("--maximum-bytes")?
        .parse::<u64>()
        .ok()
        .filter(|n| *n > 0 && *n <= 9_007_199_254_740_991);
    if options
        .get("--maximum-bytes-type")
        .copied()
        .unwrap_or("number")
        != "number"
        || maximum.is_none()
    {
        return Err("SQL import chunk size must be a positive safe integer".into());
    }
    let seconds = value("--max-seconds")?
        .parse::<u64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or("SQL stream requires a positive --max-seconds or TOS_D1_SQL_STREAM_MAX_SECONDS")?;
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(seconds))
        .ok_or("SQL stream deadline overflow")?;
    let source = Path::new(value("--source")?);
    let pin = input(source)?;
    let metadata = pin.metadata().map_err(|e| e.to_string())?;
    let directory = Path::new(value("--directory")?);
    if !std::fs::symlink_metadata(directory)
        .map_err(|e| e.to_string())?
        .is_dir()
        || std::fs::read_dir(directory)
            .map_err(|e| e.to_string())?
            .next()
            .is_some()
    {
        return Err("SQL stream requires an empty regular directory".into());
    }
    let mut owned = OwnedChunk(None, false);
    let mut offset = 0;
    let mut part = 0u64;
    let mut eof = false;
    while next_request(deadline)? {
        owned.clear()?;
        unchanged(source, &metadata)
            .map_err(|error| format!("SQL import source changed between chunks: {error}"))?;
        if eof {
            writeln!(stdout, "{}", json!({"kind":"done"})).map_err(|e| e.to_string())?;
            stdout.flush().map_err(|e| e.to_string())?;
            return Ok(());
        }
        let filename = format!("part-{part}.sql");
        let target = directory.join(&filename);
        let result = chunk_until(source, &target, offset, maximum.unwrap(), Some(deadline))?;
        owned.0 = Some(target);
        if Instant::now() >= deadline {
            return Err("SQL stream deadline exceeded".into());
        }
        unchanged(source, &metadata)?;
        offset = result["next_offset"]
            .as_u64()
            .ok_or("SQL stream progress")?;
        eof = result["eof"].as_bool().ok_or("SQL stream completion")?;
        if result["bytes"] == 0 {
            owned.clear()?;
            writeln!(stdout, "{}", json!({"kind":"done"})).map_err(|e| e.to_string())?;
            stdout.flush().map_err(|e| e.to_string())?;
            return Ok(());
        }
        writeln!(stdout, "{}", json!({"kind":"chunk","file":filename}))
            .map_err(|e| e.to_string())?;
        stdout.flush().map_err(|e| e.to_string())?;
        owned.1 = true;
        part = part.checked_add(1).ok_or("SQL stream part overflow")?;
    }
    Ok(())
}

pub fn run_if_requested(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Option<i32> {
    if args.first().is_none_or(|arg| {
        !matches!(
            arg.as_str(),
            "edge-sql-chunk" | "edge-sql-stream" | "edge-import-local"
        )
    }) {
        return None;
    }
    let result = if args[0] == "edge-sql-stream" {
        stream(args, stdout)
    } else {
        run(args).and_then(|value| writeln!(stdout, "{value}").map_err(|e| e.to_string()))
    };
    Some(match result {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(stderr, "Edge SQL: {error}");
            2
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "tos-edge-sql-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn chunk_preserves_literal_crlf_unicode_and_trigger_semicolons() {
        let fixture = Fixture::new();
        let source = fixture.path("input.sql");
        let sql = "INSERT INTO t VALUES ('София''s 🌳\r\n\n  \rnext;\n-- literal');\nCREATE TRIGGER p AFTER INSERT ON t\nBEGIN\nSELECT 1;\nSELECT 2;\nEND;\nSELECT 3;";
        fs::write(&source, sql).unwrap();
        let mut offset = 0;
        let mut actual = Vec::new();
        let mut parts = 0;
        loop {
            let output = fixture.path(&format!("part-{parts}.sql"));
            let result = chunk(&source, &output, offset, 40).unwrap();
            actual.extend(fs::read(&output).unwrap());
            offset = result["next_offset"].as_u64().unwrap();
            parts += 1;
            if result["eof"] == true {
                break;
            }
        }
        assert_eq!(actual, sql.as_bytes());
        assert_eq!(parts, 3);
    }

    #[test]
    fn chunk_accepts_exact_limit_and_refuses_incomplete_oversize_or_nul() {
        let fixture = Fixture::new();
        let source = fixture.path("input.sql");
        let output = fixture.path("part.sql");
        let sql = format!(
            "SELECT '{}';\r\n",
            "x".repeat(MAX_STATEMENT_BYTES - "SELECT '';".len())
        );
        fs::write(&source, &sql).unwrap();
        assert_eq!(chunk(&source, &output, 0, 1).unwrap()["bytes"], sql.len());
        assert_eq!(fs::read(&output).unwrap(), sql.as_bytes());
        fs::remove_file(&output).unwrap();
        for invalid in [
            "SELECT 1".to_owned(),
            "SELECT '\0';\n".into(),
            format!("SELECT '{}';\n", "🌳".repeat(25_000)),
        ] {
            fs::write(&source, invalid).unwrap();
            assert!(chunk(&source, &output, 0, 100).is_err());
            assert!(!output.exists());
        }
    }

    #[test]
    fn chunk_never_overwrites_or_removes_existing_output() {
        let fixture = Fixture::new();
        let source = fixture.path("input.sql");
        let output = fixture.path("part.sql");
        fs::write(&source, "SELECT 1;\n").unwrap();
        fs::write(&output, "keep").unwrap();
        assert!(chunk(&source, &output, 0, 100).is_err());
        assert_eq!(fs::read_to_string(&output).unwrap(), "keep");
    }

    fn database(fixture: &Fixture) -> PathBuf {
        let path = fixture.path("store.sqlite");
        Connection::open(&path).unwrap().execute_batch("CREATE TABLE edge_meta(key TEXT, part INTEGER, json_chunk TEXT); INSERT INTO edge_meta VALUES('data_revision',0,'{\"sha256\":\"old\"}'); CREATE TABLE t(value TEXT);").unwrap();
        path
    }

    #[test]
    fn local_import_executes_trigger_and_checks_target_in_same_transaction() {
        let fixture = Fixture::new();
        let database = database(&fixture);
        let sql = fixture.path("input.sql");
        fs::write(&sql, "CREATE TRIGGER p AFTER INSERT ON t BEGIN UPDATE edge_meta SET json_chunk='{\"sha256\":\"new\"}'; END;\nINSERT INTO t VALUES ('София\r\n🌳;');\n").unwrap();
        let result = import(&database, &sql, Some("old"), "new").unwrap();
        assert_eq!(result["statements"], 2);
        let db = Connection::open(&database).unwrap();
        assert_eq!(revision(&db).unwrap().as_deref(), Some("new"));
        assert_eq!(
            db.query_row("SELECT value FROM t", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "София\r\n🌳;"
        );
    }

    #[test]
    fn every_refusal_rolls_back_written_rows_and_revision() {
        let fixture = Fixture::new();
        let database = database(&fixture);
        let sql = fixture.path("input.sql");
        let prefix = "INSERT INTO t VALUES ('must roll back');\nUPDATE edge_meta SET json_chunk='{\"sha256\":\"new\"}';\n";
        for (base, target, suffix) in [
            ("foreign", "new", ""),
            ("old", "foreign", ""),
            ("old", "new", "SELECT 'incomplete"),
            ("old", "new", "INSERT INTO absent VALUES (1);\n"),
        ] {
            fs::write(&sql, format!("{prefix}{suffix}")).unwrap();
            assert!(import(&database, &sql, Some(base), target).is_err());
            let db = Connection::open(&database).unwrap();
            assert_eq!(revision(&db).unwrap().as_deref(), Some("old"));
            assert_eq!(
                db.query_row("SELECT COUNT(*) FROM t", [], |r| r.get::<_, u64>(0))
                    .unwrap(),
                0
            );
        }
        let absent = fixture.path("absent.sqlite");
        assert!(import(&absent, &sql, None, "new").is_err());
        assert!(!absent.exists());
    }

    #[test]
    fn legacy_and_chunked_revision_shapes_are_both_read() {
        let db = Connection::open_in_memory().unwrap();
        assert_eq!(revision(&db).unwrap(), None);
        db.execute_batch("CREATE TABLE edge_meta(key TEXT,json TEXT);INSERT INTO edge_meta VALUES('data_revision','{\"sha256\":\"legacy\"}');").unwrap();
        assert_eq!(revision(&db).unwrap().as_deref(), Some("legacy"));
        db.execute_batch("DROP TABLE edge_meta; CREATE TABLE edge_meta(key TEXT,part INTEGER,json_chunk TEXT); INSERT INTO edge_meta VALUES('data_revision',1,'\"chunked\"}'),('data_revision',0,'{\"sha256\":');").unwrap();
        assert_eq!(revision(&db).unwrap().as_deref(), Some("chunked"));
    }

    #[test]
    fn nonstring_present_revision_never_admits_empty_baseline() {
        let fixture = Fixture::new();
        let database = database(&fixture);
        let sql = fixture.path("input.sql");
        fs::write(&sql, "INSERT INTO t VALUES ('must not enter');\nUPDATE edge_meta SET json_chunk='{\"sha256\":\"new\"}';\n").unwrap();
        for malformed in [
            "{\"sha256\":123}",
            "{\"sha256\":true}",
            "{\"sha256\":{}}",
            "{\"sha256\":[]}",
            "123",
            "true",
            "[]",
            "null",
            "\"old\"",
        ] {
            let db = Connection::open(&database).unwrap();
            db.execute("UPDATE edge_meta SET json_chunk=?", [&malformed])
                .unwrap();
            drop(db);
            assert!(import(&database, &sql, None, "new").is_err());
            let db = Connection::open(&database).unwrap();
            assert_eq!(
                db.query_row("SELECT json_chunk FROM edge_meta", [], |r| r
                    .get::<_, String>(0))
                    .unwrap(),
                malformed
            );
            assert_eq!(
                db.query_row("SELECT COUNT(*) FROM t", [], |r| r.get::<_, u64>(0))
                    .unwrap(),
                0
            );
        }
    }

    #[test]
    fn absent_empty_missing_or_null_object_digest_keeps_oracle_empty_baseline() {
        for prior in [None, Some(""), Some("{}"), Some("{\"sha256\":null}")] {
            let fixture = Fixture::new();
            let database = database(&fixture);
            let db = Connection::open(&database).unwrap();
            db.execute("DELETE FROM edge_meta", []).unwrap();
            if let Some(prior) = prior {
                db.execute("INSERT INTO edge_meta VALUES('data_revision',0,?)", [prior])
                    .unwrap();
            }
            assert_eq!(revision(&db).unwrap(), None);
            drop(db);
            let sql = fixture.path("input.sql");
            fs::write(&sql, "DELETE FROM edge_meta;\nINSERT INTO edge_meta VALUES('data_revision',0,'{\"sha256\":\"new\"}');\n").unwrap();
            import(&database, &sql, None, "new").unwrap();
            assert_eq!(
                revision(&Connection::open(&database).unwrap())
                    .unwrap()
                    .as_deref(),
                Some("new")
            );
        }
    }

    #[test]
    fn stream_validates_safe_integer_type_and_deadline_before_input_or_output() {
        for (value, kind) in [
            ("0", "number"),
            ("-1", "number"),
            ("1.5", "number"),
            ("Infinity", "number"),
            ("9007199254740992", "number"),
            ("40", "bigint"),
            ("40", "string"),
        ] {
            let args = [
                "edge-sql-stream",
                "--maximum-bytes",
                value,
                "--maximum-bytes-type",
                kind,
            ]
            .map(str::to_owned);
            let mut output = Vec::new();
            assert!(
                stream(&args, &mut output)
                    .unwrap_err()
                    .contains("positive safe integer")
            );
            assert!(output.is_empty());
        }
        let args = [
            "edge-sql-stream",
            "--maximum-bytes",
            "40",
            "--max-seconds",
            "0",
        ]
        .map(str::to_owned);
        assert!(
            stream(&args, &mut Vec::new())
                .unwrap_err()
                .contains("positive --max-seconds")
        );
        let fixture = Fixture::new();
        let source = fixture.path("input.sql");
        let output = fixture.path("part.sql");
        fs::write(&source, "SELECT 1;\n").unwrap();
        assert!(
            chunk_until(&source, &output, 0, 40, Some(Instant::now()))
                .unwrap_err()
                .contains("deadline")
        );
        assert!(!output.exists());
    }
}
