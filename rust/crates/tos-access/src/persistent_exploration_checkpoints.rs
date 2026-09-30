//! Private disposable PublishedD1 checkpoints, never selected source authority.
//! One BEGIN IMMEDIATE spans load, QRY preparation and final disclosure fence.
//! SQLite/JSON work is bounded; the caller still owns the whole-operation wall.
use crate::exploration_checkpoints::CheckpointLimits;
use rusqlite::{Connection, OpenFlags, OptionalExtension, limits::Limit, params};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tos_foundation::{Digest256, JsonMode, JsonValue, parse_json};
use tos_query::{
    knowledge_exploration::{
        ExplorationBudget, ExplorationCheckpoint, ExplorationCheckpoints, ExplorationState,
        PUBLISHED_EXPLORATION_CACHE_VERSION, PreparedExplorationCheckpoint,
        decode_published_exploration_state,
    },
    search_v2::{SearchV2Error, SearchV2ErrorCode},
};
const SCHEMA: &str = "tos_rust_private_exploration_checkpoints_v1";
const TABLE_META: &str = "CREATE TABLE checkpoint_meta(singleton INTEGER PRIMARY KEY CHECK(singleton=1),config TEXT NOT NULL,last_time INTEGER NOT NULL CHECK(last_time>=0))";
const TABLE_ROWS: &str = "CREATE TABLE checkpoints(token TEXT PRIMARY KEY,revision TEXT NOT NULL,expires INTEGER NOT NULL,sequence INTEGER NOT NULL,kind INTEGER NOT NULL CHECK(kind IN(0,1)),digest TEXT NOT NULL,raw BLOB NOT NULL)";
const INDEX: &str = "CREATE INDEX checkpoints_sequence ON checkpoints(sequence)";
fn err(code: SearchV2ErrorCode, message: &'static str) -> SearchV2Error {
    SearchV2Error { code, message }
}
fn unavailable() -> SearchV2Error {
    err(
        SearchV2ErrorCode::Unavailable,
        "persistent checkpoint unavailable, busy or incompatible; no automatic reset",
    )
}
fn corrupt() -> SearchV2Error {
    err(
        SearchV2ErrorCode::CorruptSelectedCarrier,
        "persistent checkpoint framing or custody changed",
    )
}
fn capacity() -> SearchV2Error {
    err(
        SearchV2ErrorCode::BudgetExceeded,
        "replay and successor exceed protected checkpoint capacity",
    )
}
fn expired() -> SearchV2Error {
    err(
        SearchV2ErrorCode::CursorExpired,
        "exploration checkpoint expired or evicted; restart",
    )
}
fn sql<T>(v: rusqlite::Result<T>) -> Result<T, SearchV2Error> {
    v.map_err(|_| unavailable())
}
fn now() -> Result<i64, SearchV2Error> {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| unavailable())?
        .as_millis();
    i64::try_from(ms).map_err(|_| unavailable())
}
fn bare(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn path_checked(p: &Path) -> Result<PathBuf, SearchV2Error> {
    if !p.is_absolute() || p.file_name().is_none() {
        return Err(corrupt());
    }
    let parent = p.parent().ok_or_else(corrupt)?;
    if fs::canonicalize(parent).map_err(|_| unavailable())? != parent {
        return Err(corrupt());
    }
    Ok(p.to_owned())
}
fn suffixed(p: &Path, suffix: &str) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}
struct Config {
    path: PathBuf,
    source: PathBuf,
    anchor: File,
    identity: (u64, u64),
    limits: CheckpointLimits,
    budget: ExplorationBudget,
    bytes: u64,
    text: String,
}
/// Cloning selects the same store but starts an independent request transaction.
/// A loaded transaction is never shared or silently copied between requests.
pub struct PersistentExplorationCheckpoints {
    config: Arc<Config>,
    pending: Mutex<Option<Held>>,
    probe: Option<Arc<dyn tos_query::AbortProbe>>,
}
impl Clone for PersistentExplorationCheckpoints {
    fn clone(&self) -> Self {
        Self {
            config: Arc::clone(&self.config),
            pending: Mutex::new(None),
            probe: None,
        }
    }
}
struct Held {
    db: Connection,
    clock: i64,
    expires: i64,
    loaded: Option<(String, String, i64)>,
    probe: Option<Arc<dyn tos_query::AbortProbe>>,
}
fn check(probe: Option<&dyn tos_query::AbortProbe>) -> Result<(), SearchV2Error> {
    match probe.and_then(tos_query::AbortProbe::reason) {
        Some(tos_query::AbortReason::Cancelled) => {
            Err(err(SearchV2ErrorCode::Cancelled, "checkpoint cancelled"))
        }
        Some(tos_query::AbortReason::DeadlineExceeded) => Err(err(
            SearchV2ErrorCode::DeadlineExceeded,
            "checkpoint deadline exceeded",
        )),
        None => Ok(()),
    }
}
impl Drop for Held {
    fn drop(&mut self) {
        let _ = self.db.execute_batch("ROLLBACK");
    }
}
impl Config {
    fn verify(&self) -> Result<(), SearchV2Error> {
        let m = fs::symlink_metadata(&self.path).map_err(|_| unavailable())?;
        let a = self.anchor.metadata().map_err(|_| unavailable())?;
        if !m.is_file()
            || m.uid() != unsafe { libc::getuid() }
            || m.mode() & 0o7777 != 0o600
            || m.nlink() != 1
            || (m.dev(), m.ino()) != self.identity
            || (a.dev(), a.ino()) != self.identity
            || m.len() > self.bytes
            || path_checked(&self.path)? != self.path
        {
            return Err(corrupt());
        }
        let source_paths = [
            self.source.clone(),
            suffixed(&self.source, "-journal"),
            suffixed(&self.source, "-wal"),
            suffixed(&self.source, "-shm"),
        ];
        let checkpoint_paths = [
            self.path.clone(),
            suffixed(&self.path, "-journal"),
            suffixed(&self.path, "-wal"),
            suffixed(&self.path, "-shm"),
        ];
        let mut identities = BTreeSet::new();
        for p in &checkpoint_paths {
            if source_paths.contains(p) {
                return Err(corrupt());
            }
            match fs::symlink_metadata(p) {
                Ok(v) => {
                    if !v.is_file()
                        || v.uid() != unsafe { libc::getuid() }
                        || v.mode() & 0o7777 != 0o600
                        || v.nlink() != 1
                        || v.len() > self.bytes + 65536
                        || !identities.insert((v.dev(), v.ino()))
                    {
                        return Err(corrupt());
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound && p != &self.path => {}
                Err(_) => return Err(unavailable()),
            }
        }
        for p in &source_paths {
            match fs::symlink_metadata(p) {
                Ok(v) if !v.is_file() || identities.contains(&(v.dev(), v.ino())) => {
                    return Err(corrupt());
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound && p != &self.source => {}
                Err(_) => return Err(unavailable()),
            }
        }
        Ok(())
    }
    fn connect(
        &self,
        probe: Option<Arc<dyn tos_query::AbortProbe>>,
    ) -> Result<Connection, SearchV2Error> {
        check(probe.as_deref())?;
        self.verify()?;
        // SQLite's own O_NOFOLLOW accompanies retained-FD and pathname checks.
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::from_bits_retain(0x01000000);
        let db = sql(Connection::open_with_flags(&self.path, flags))?;
        self.verify()?;
        let steps = Arc::new(AtomicU64::new(0));
        let started = Instant::now();
        db.progress_handler(
            1000,
            Some(move || {
                steps.fetch_add(1000, Ordering::Relaxed) >= 2_000_000
                    || probe
                        .as_deref()
                        .and_then(tos_query::AbortProbe::reason)
                        .is_some()
                    || started.elapsed() > Duration::from_secs(5)
            }),
        );
        sql(db.busy_timeout(Duration::from_millis(100)))?;
        sql(db.set_limit(
            Limit::SQLITE_LIMIT_LENGTH,
            (self.limits.max_encoded_bytes + 65536) as i32,
        ))?;
        sql(db.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, 16384))?;
        sql(db.set_limit(Limit::SQLITE_LIMIT_COLUMN, 32))?;
        sql(db.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA synchronous=FULL; PRAGMA cache_size=-512; PRAGMA temp_store=MEMORY; PRAGMA mmap_size=0"))?;
        let mode: String = sql(db.query_row("PRAGMA journal_mode", [], |r| r.get(0)))?;
        let page: u64 = sql(db.query_row("PRAGMA page_size", [], |r| r.get(0)))?;
        if mode != "delete" || page != 4096 {
            return Err(corrupt());
        }
        let actual: u64 = sql(db.query_row(
            &format!("PRAGMA max_page_count={}", self.bytes / 4096),
            [],
            |r| r.get(0),
        ))?;
        if actual > self.bytes / 4096 {
            return Err(capacity());
        }
        Ok(db)
    }
    fn validate(&self, db: &Connection) -> Result<i64, SearchV2Error> {
        // Exact Rust schema, including no triggers/views/foreign schema objects.
        let mut stmt=sql(db.prepare("SELECT CASE WHEN length(name)<=128 THEN name ELSE NULL END,CASE WHEN length(sql)<=8192 THEN sql ELSE NULL END FROM sqlite_master WHERE name NOT LIKE 'sqlite_autoindex_%' ORDER BY name LIMIT 5"))?;
        let objects =
            sql(stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))))?
                .collect::<rusqlite::Result<Vec<_>>>();
        let objects = sql(objects)?;
        if objects
            != vec![
                ("checkpoint_meta".into(), TABLE_META.into()),
                ("checkpoints".into(), TABLE_ROWS.into()),
                ("checkpoints_sequence".into(), INDEX.into()),
            ]
        {
            return Err(corrupt());
        }
        let (config, last): (String, i64) = sql(db.query_row(
            "SELECT CASE WHEN length(config)<=8192 THEN config ELSE NULL END,last_time FROM checkpoint_meta WHERE singleton=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ))?;
        let mut meta = sql(db.prepare("SELECT singleton FROM checkpoint_meta LIMIT 2"))?;
        let keys =
            sql(meta.query_map([], |r| r.get::<_, i64>(0)))?.collect::<rusqlite::Result<Vec<_>>>();
        if config != self.text || last < 0 || sql(keys)? != vec![1] {
            return Err(corrupt());
        }
        rows(self, db)?;
        Ok(last)
    }
    fn begin(&self, probe: Option<Arc<dyn tos_query::AbortProbe>>) -> Result<Held, SearchV2Error> {
        check(probe.as_deref())?;
        let db = self.connect(probe.clone())?;
        sql(db.execute_batch("BEGIN IMMEDIATE"))?;
        let held = (|| {
            let clock = now()?;
            if clock < self.validate(&db)? {
                return Err(err(
                    SearchV2ErrorCode::Unavailable,
                    "checkpoint wall clock moved backwards; state retained",
                ));
            }
            sql(db.execute("DELETE FROM checkpoints WHERE expires<=?1", [clock]))?;
            Ok(clock)
        })();
        match held {
            Ok(clock) => Ok(Held {
                db,
                clock,
                expires: clock
                    .checked_add(
                        i64::try_from(self.limits.ttl.as_millis()).map_err(|_| capacity())?,
                    )
                    .ok_or_else(capacity)?,
                loaded: None,
                probe,
            }),
            Err(e) => {
                let _ = db.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }
}
impl PersistentExplorationCheckpoints {
    pub fn set_abort_probe(&mut self, probe: Arc<dyn tos_query::AbortProbe>) {
        self.probe = Some(probe);
    }
    pub fn limits(&self) -> CheckpointLimits {
        self.config.limits
    }
    /// Main database ceiling; one DELETE journal may add this plus headers.
    pub fn database_byte_cap(&self) -> u64 {
        self.config.bytes
    }
    pub fn open(
        path: &Path,
        source_path: &Path,
        limits: CheckpointLimits,
        budget: ExplorationBudget,
    ) -> Result<Self, SearchV2Error> {
        if limits.ttl.as_millis() == 0
            || limits.ttl > Duration::from_secs(86400)
            || limits.max_entries == 0
            || limits.max_entries > 128
            || limits.max_encoded_bytes == 0
            || limits.max_encoded_bytes > 32 * 1024 * 1024
            || budget.max_checkpoint_bytes == 0
            || budget.max_checkpoint_bytes > 32 * 1024 * 1024
            || budget.max_state_bytes == 0
            || budget.max_state_bytes > budget.max_checkpoint_bytes
            || budget.max_checkpoints == 0
            || budget.max_checkpoints > 128
            || limits.max_entries > budget.max_checkpoints
            || limits.max_encoded_bytes > budget.max_checkpoint_bytes
        {
            return Err(err(
                SearchV2ErrorCode::InvalidRequest,
                "invalid persistent checkpoint limits",
            ));
        }
        let path = path_checked(path)?;
        let source = path_checked(source_path)?;
        if [
            source.clone(),
            suffixed(&source, "-journal"),
            suffixed(&source, "-wal"),
            suffixed(&source, "-shm"),
        ]
        .contains(&path)
        {
            return Err(corrupt());
        }
        let bytes = ((limits.max_encoded_bytes as u64 * 3 + 1048576 + 4095) / 4096) * 4096;
        let created;
        let anchor = match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
            .open(&path)
        {
            Ok(f) => {
                created = true;
                f
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                created = false;
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
                    .open(&path)
                    .map_err(|_| unavailable())?
            }
            Err(_) => return Err(unavailable()),
        };
        let m = anchor.metadata().map_err(|_| unavailable())?;
        let text = format!(
            "{SCHEMA}|{PUBLISHED_EXPLORATION_CACHE_VERSION}|ttl-ms:{}|entries:{}|bytes:{}|budget:{budget:?}",
            limits.ttl.as_millis(),
            limits.max_entries,
            limits.max_encoded_bytes
        );
        let config = Arc::new(Config {
            path,
            source,
            anchor,
            identity: (m.dev(), m.ino()),
            limits,
            budget,
            bytes,
            text,
        });
        let db = config.connect(None)?;
        sql(db.execute_batch("BEGIN IMMEDIATE"))?;
        let result = (|| {
            if created {
                sql(db.execute_batch(&format!("{TABLE_META};{TABLE_ROWS};{INDEX}")))?;
                sql(db.execute("INSERT INTO checkpoint_meta VALUES(1,?1,0)", [&config.text]))?;
            }
            if now()? < config.validate(&db)? {
                return Err(unavailable());
            }
            config.verify()?;
            sql(db.execute_batch("COMMIT"))?;
            Ok(())
        })();
        if result.is_err() {
            let _ = db.execute_batch("ROLLBACK");
        }
        result?;
        Ok(Self {
            config,
            pending: Mutex::new(None),
            probe: None,
        })
    }
    fn take(&mut self) -> Result<Option<Held>, SearchV2Error> {
        self.pending
            .get_mut()
            .map_err(|_| unavailable())
            .map(Option::take)
    }
}
fn random_token() -> Result<String, SearchV2Error> {
    let mut bytes = [0u8; 32];
    File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|_| unavailable())?;
    Ok(Digest256::of_bytes(&bytes).to_hex())
}
// Read no more than the declared maximum plus one poisoned-row sentinel.
fn rows(config: &Config, db: &Connection) -> Result<Vec<(String, usize, i64)>, SearchV2Error> {
    let mut stmt = sql(db.prepare("SELECT CASE WHEN length(token)=64 THEN token ELSE NULL END,length(raw),sequence,expires,kind FROM checkpoints LIMIT 129"))?;
    let values = sql(stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, i64>(4)?,
        ))
    }))?
    .collect::<rusqlite::Result<Vec<_>>>();
    let values = sql(values)?;
    if values.len() > config.limits.max_entries {
        return Err(corrupt());
    }
    let mut total = 0usize;
    let mut out = Vec::with_capacity(values.len());
    for (token, size, sequence, expires, kind) in values {
        if !bare(&token) || size < 0 || sequence <= 0 || expires < 0 || !(0..=1).contains(&kind) {
            return Err(corrupt());
        }
        total = total.checked_add(size as usize).ok_or_else(corrupt)?;
        if total > config.limits.max_encoded_bytes {
            return Err(corrupt());
        }
        out.push((token, size as usize, sequence));
    }
    Ok(out)
}
fn put(
    config: &Config,
    held: &Held,
    token: &str,
    revision: &str,
    kind: i64,
    raw: &[u8],
    protected: &BTreeSet<String>,
) -> Result<(), SearchV2Error> {
    if raw.len() > config.limits.max_encoded_bytes || !bare(token) {
        return Err(capacity());
    }
    sql(held
        .db
        .execute("DELETE FROM checkpoints WHERE token=?1", [token]))?;
    let mut existing = rows(config, &held.db)?;
    while existing.len() >= config.limits.max_entries
        || existing.iter().map(|r| r.1).sum::<usize>() + raw.len() > config.limits.max_encoded_bytes
    {
        let oldest = existing
            .iter()
            .filter(|r| !protected.contains(&r.0))
            .min_by_key(|r| (r.2, &r.0))
            .map(|r| r.0.clone())
            .ok_or_else(capacity)?;
        sql(held
            .db
            .execute("DELETE FROM checkpoints WHERE token=?1", [&oldest]))?;
        existing.retain(|r| r.0 != oldest);
    }
    let sequence = existing.iter().map(|r| r.2).max().unwrap_or(0);
    let sequence = sequence.checked_add(1).ok_or_else(capacity)?;
    sql(held.db.execute(
        "INSERT INTO checkpoints VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![
            token,
            revision,
            held.expires,
            sequence,
            kind,
            Digest256::of_bytes(raw).to_hex(),
            raw
        ],
    ))?;
    Ok(())
}
impl ExplorationCheckpoints for PersistentExplorationCheckpoints {
    fn load(
        &mut self,
        cursor: &str,
        revision: &str,
    ) -> Result<ExplorationCheckpoint, SearchV2Error> {
        let operation_probe = self.probe.clone();
        let result: Result<ExplorationCheckpoint, SearchV2Error> = (|| {
            check(self.probe.as_deref())?;
            if !bare(cursor)
                || revision.is_empty()
                || self.pending.get_mut().map_err(|_| unavailable())?.is_some()
            {
                return Err(corrupt());
            }
            let mut held = self.config.begin(self.probe.clone())?;
            let row: Option<(String, i64, String, i64, i64)> = sql(held
            .db
            .query_row(
                "SELECT CASE WHEN length(revision)<=4096 THEN revision ELSE NULL END,kind,CASE WHEN length(digest)=64 THEN digest ELSE NULL END,length(raw),expires FROM checkpoints WHERE token=?1",
                [cursor],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional())?;
            let (stored, kind, digest, size, expires) = row.ok_or_else(expired)?;
            held.expires = expires;
            if stored != revision {
                return Err(err(
                    SearchV2ErrorCode::StaleContinuation,
                    "checkpoint source snapshot or execution changed",
                ));
            }
            if size < 0 || size as usize > self.config.limits.max_encoded_bytes || !bare(&digest) {
                return Err(corrupt());
            }
            let raw: Vec<u8> = sql(held.db.query_row(
                "SELECT raw FROM checkpoints WHERE token=?1",
                [cursor],
                |r| r.get(0),
            ))?;
            if Digest256::of_bytes(&raw).to_hex() != digest {
                return Err(corrupt());
            }
            check(self.probe.as_deref())?;
            let value = match kind {
                0 => ExplorationCheckpoint::State(decode_published_exploration_state(
                    &raw,
                    revision,
                    self.config.budget,
                )?),
                1 => {
                    let mut limits = self.config.budget.read.json;
                    limits.max_bytes = self.config.budget.read.max_response_bytes;
                    let doc = parse_json(&raw, JsonMode::PublishedStrict, limits)
                        .map_err(|_| corrupt())?;
                    ExplorationCheckpoint::Replay {
                        packet: doc.root().clone(),
                        packet_sha256: Digest256::of_bytes(&raw),
                    }
                }
                _ => return Err(corrupt()),
            };
            check(self.probe.as_deref())?;
            held.loaded = Some((cursor.to_owned(), revision.to_owned(), kind));
            *self.pending.get_mut().map_err(|_| unavailable())? = Some(held);
            Ok(value)
        })();
        if result.is_err() {
            check(operation_probe.as_deref())?;
        }
        result
    }
    fn prepare(
        &mut self,
        input: Option<&str>,
        revision: &str,
        successor: Option<&ExplorationState>,
        _packet: &JsonValue,
        budget: ExplorationBudget,
    ) -> Result<Box<dyn PreparedExplorationCheckpoint>, SearchV2Error> {
        let operation_probe = self.probe.clone();
        let result = (|| -> Result<Box<dyn PreparedExplorationCheckpoint>, SearchV2Error> {
            check(self.probe.as_deref())?;
            if format!("{budget:?}") != format!("{:?}", self.config.budget)
                || revision.is_empty()
                || successor.is_some_and(|s| s.snapshot_revision() != revision)
            {
                return Err(corrupt());
            }
            let held = match self.take()? {
                Some(h) => h,
                None if input.is_none() => self.config.begin(self.probe.clone())?,
                _ => return Err(expired()),
            };
            if let Some(input) = input {
                if !bare(input)
                    || held.loaded.as_ref() != Some(&(input.to_owned(), revision.to_owned(), 0))
                {
                    return Err(corrupt());
                }
            } else if held.loaded.is_some() {
                return Err(corrupt());
            }
            let mut protected = BTreeSet::new();
            if let Some(input) = input {
                protected.insert(input.to_owned());
            }
            let mut state_raw = None;
            let next = if let Some(state) = successor {
                let mut limits = budget.read.json;
                limits.max_bytes = limits.max_bytes.min(budget.max_state_bytes);
                state.encoded_state_count(limits)?;
                let raw = state.encoded_state(limits)?;
                let mut chosen = None;
                for _ in 0..4 {
                    let token = random_token()?;
                    let exists: i64 = sql(held.db.query_row(
                        "SELECT count(*) FROM checkpoints WHERE token=?1",
                        [&token],
                        |r| r.get(0),
                    ))?;
                    if exists == 0 {
                        chosen = Some(token);
                        break;
                    }
                }
                let next = chosen.ok_or_else(unavailable)?;
                protected.insert(next.clone());
                state_raw = Some(raw);
                Some(next)
            } else {
                None
            };
            check(self.probe.as_deref())?;
            Ok(Box::new(Staged {
                config: Arc::clone(&self.config),
                held: Some(held),
                input: input.map(str::to_owned),
                revision: revision.to_owned(),
                next,
                protected,
                staged: false,
                replay_only: false,
                state_raw,
            }))
        })();
        if result.is_err() {
            check(operation_probe.as_deref())?;
        }
        result
    }
    fn prepare_replay(
        &mut self,
    ) -> Result<Option<Box<dyn PreparedExplorationCheckpoint>>, SearchV2Error> {
        let operation_probe = self.probe.clone();
        let result =
            (|| -> Result<Option<Box<dyn PreparedExplorationCheckpoint>>, SearchV2Error> {
                check(self.probe.as_deref())?;
                let held = self.take()?.ok_or_else(expired)?;
                if !held.loaded.as_ref().is_some_and(|(_, _, kind)| *kind == 1) {
                    return Err(corrupt());
                }
                Ok(Some(Box::new(Staged {
                    config: Arc::clone(&self.config),
                    held: Some(held),
                    input: None,
                    revision: String::new(),
                    next: None,
                    protected: BTreeSet::new(),
                    staged: false,
                    replay_only: true,
                    state_raw: None,
                })))
            })();
        if result.is_err() {
            check(operation_probe.as_deref())?;
        }
        result
    }
}
struct Staged {
    config: Arc<Config>,
    held: Option<Held>,
    input: Option<String>,
    revision: String,
    next: Option<String>,
    protected: BTreeSet<String>,
    staged: bool,
    replay_only: bool,
    state_raw: Option<Vec<u8>>,
}
impl PreparedExplorationCheckpoint for Staged {
    fn next_cursor(&self) -> Option<&str> {
        self.next.as_deref()
    }
    fn stage_response(&mut self, body: &[u8]) -> Result<(), SearchV2Error> {
        let operation_probe = self.held.as_ref().and_then(|held| held.probe.clone());
        let result: Result<(), SearchV2Error> = (|| {
            check(self.held.as_ref().ok_or_else(corrupt)?.probe.as_deref())?;
            if self.staged
                || self.held.is_none()
                || body.len() > self.config.budget.read.max_response_bytes
            {
                return Err(capacity());
            }
            if let Some(input) = &self.input {
                put(
                    &self.config,
                    self.held.as_ref().ok_or_else(corrupt)?,
                    input,
                    &self.revision,
                    1,
                    body,
                    &self.protected,
                )?;
            }
            if let (Some(next), Some(raw)) = (&self.next, &self.state_raw) {
                put(
                    &self.config,
                    self.held.as_ref().ok_or_else(corrupt)?,
                    next,
                    &self.revision,
                    0,
                    raw,
                    &self.protected,
                )?;
            }
            check(self.held.as_ref().ok_or_else(corrupt)?.probe.as_deref())?;
            self.state_raw.take();
            self.staged = true;
            Ok(())
        })();
        if result.is_err() {
            check(operation_probe.as_deref())?;
        }
        result
    }
    fn commit(&mut self) -> Result<(), SearchV2Error> {
        let operation_probe = self.held.as_ref().and_then(|held| held.probe.clone());
        let result: Result<(), SearchV2Error> = (|| {
            if !self.staged && !self.replay_only {
                return Err(corrupt());
            }
            let held = self.held.as_ref().ok_or_else(corrupt)?;
            check(held.probe.as_deref())?;
            self.config.verify()?;
            let committed_clock = now()?;
            if committed_clock < held.clock {
                return Err(err(
                    SearchV2ErrorCode::Unavailable,
                    "checkpoint wall clock moved backwards; state retained",
                ));
            }
            self.config.validate(&held.db)?;
            sql(held.db.execute(
                "UPDATE checkpoint_meta SET last_time=?1 WHERE singleton=1",
                [committed_clock],
            ))?;
            check(held.probe.as_deref())?;
            sql(held.db.execute_batch("COMMIT"))?;
            self.held.take();
            Ok(())
        })();
        if result.is_err() {
            check(operation_probe.as_deref())?;
        }
        result
    }
}
