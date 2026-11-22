//! `tos bridge` (TOS-D-0083): the person's own bridge on a loopback address.
//!
//! It serves the built public-research site at the paths the Worker serves, so
//! the stands' pages are of the bridge's origin; it keeps the person's act
//! journal in a store directory the person names, every record checked and
//! folded by the same Rust rules as the browser (`tos-web-rules`); the page
//! follows the journal through an event stream, and agents reach it through
//! MCP with the permission tiers of `access/contracts/act-commands.v1.json`.
//! It writes nothing into the Tree's sources or a stand, admits nothing and
//! claims no review or canon: its posture is `local_only`.
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    net::{IpAddr, SocketAddr, TcpListener, TcpStream},
    os::unix::fs::FileExt,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tos_foundation::{Digest256Hasher, deployment_profile::BridgeProfile};
use tos_web_rules::{
    AGENT_TIERS, ActJournal, CheckedRecord, PERSON_ONLY_KINDS, PROPOSING_KINDS, check_stored_record,
};

const REGISTRY: &[u8] = include_bytes!("../../../../access/contracts/act-kinds.v1.json");
const ACT_COMMANDS: &str = include_str!("../../../../access/contracts/act-commands.v1.json");
const PAGE_COMMANDS: &str = include_str!("../../../../access/contracts/page-commands.v1.json");
const FIELD_SHAPES: &str = include_str!("../../../../access/contracts/field-shapes.v1.json");
const MEDIA_HOSTS: &str = include_str!("../../../../bonsai/lib/media-hosts.mjs");
const STORE_FILE: &str = "acts.jsonl";
const SNAPSHOT_FILE: &str = "acts.snapshot";
const SNAPSHOT_MAGIC: &[u8; 8] = b"TOSBSN1\0";
const SNAPSHOT_HEADER: usize = 56;
const CONTROL_FILE: &str = "control.json";
const MAX_HEADER_BYTES: usize = 16_384;
const MAX_AGENT_BATCH: usize = 100;
const RESTORE_CHUNK: usize = 8_192;
const EVENT_GAP: Duration = Duration::from_millis(200);
const MCP_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26"];
const PERSON_ONLY: &[&str] = PERSON_ONLY_KINDS;
const PROPOSING: &[&str] = PROPOSING_KINDS;
const COOKIE: &str = "tos_bridge";

// ---------------------------------------------------------------- store

/// The person's act records, one stored record a line, in sequence order. A
/// line is acknowledged only after it is written whole; a torn last line is
/// cut at restore.
pub struct Store {
    file: File,
    lines: Vec<(u64, u32)>,
    ids: HashMap<Box<str>, u64>,
    end: u64,
    hasher: Digest256Hasher,
}

/// How a start found the journal: records taken from a snapshot of the folds,
/// records folded from the store's lines, and why a snapshot was not used.
pub struct Restored {
    pub from_snapshot: u64,
    pub folded: u64,
    pub note: Option<String>,
}

impl Store {
    /// Open the store in `dir` and its journal: from the snapshot of the folds
    /// where it matches the store's first bytes, then every later record checked
    /// and folded in order.
    pub fn open(dir: &Path) -> Result<(Self, ActJournal, Restored), String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("bridge store: {e}"))?;
        let path = dir.join(STORE_FILE);
        let file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(&path)
            .map_err(|e| format!("bridge store: {e}"))?;
        let mut store = Self {
            file,
            lines: Vec::new(),
            ids: HashMap::new(),
            end: 0,
            hasher: Digest256Hasher::new(),
        };
        let mut note = None;
        let journal = match std::fs::read(dir.join(SNAPSHOT_FILE)) {
            Ok(bytes) => match store.adopt(&path, &bytes) {
                Ok(journal) => Some(journal),
                Err(reason) => {
                    store.lines.clear();
                    store.ids.clear();
                    store.end = 0;
                    store.hasher = Digest256Hasher::new();
                    note = Some(reason);
                    None
                }
            },
            Err(_) => None,
        };
        let from_snapshot = store.len();
        let mut journal = match journal {
            Some(journal) => journal,
            None => ActJournal::new(REGISTRY)?,
        };
        store.fold_tail(&path, &mut journal)?;
        let folded = store.len() - from_snapshot;
        Ok((
            store,
            journal,
            Restored {
                from_snapshot,
                folded,
                note,
            },
        ))
    }

    /// Take the journal from a snapshot when the store's first bytes are the ones it was made from.
    fn adopt(&mut self, path: &Path, snapshot: &[u8]) -> Result<ActJournal, String> {
        let header = snapshot
            .get(..SNAPSHOT_HEADER)
            .filter(|header| header.starts_with(SNAPSHOT_MAGIC))
            .ok_or("snapshot_format")?;
        let number =
            |at: usize| u64::from_le_bytes(header[at..at + 8].try_into().unwrap_or_default());
        let (length, records) = (number(8), number(16));
        let stated = &header[24..56];
        let mut file = File::open(path).map_err(|e| format!("bridge store: {e}"))?;
        if file
            .metadata()
            .map_err(|e| format!("bridge store: {e}"))?
            .len()
            < length
        {
            return Err("snapshot_store_shorter".to_owned());
        }
        let mut buffer = vec![0u8; 1 << 20];
        let (mut offset, mut start) = (0u64, 0u64);
        while offset < length {
            let want = buffer.len().min((length - offset) as usize);
            file.read_exact(&mut buffer[..want])
                .map_err(|e| format!("bridge store: {e}"))?;
            self.hasher.update(&buffer[..want]);
            for (at, byte) in buffer[..want].iter().enumerate() {
                if *byte == b'\n' {
                    let end = offset + at as u64;
                    self.lines.push((start, (end - start) as u32));
                    start = end + 1;
                }
            }
            offset += want as u64;
        }
        if start != length || self.len() != records {
            return Err("snapshot_store_lines".to_owned());
        }
        if self.hasher.clone().finalize().as_bytes()[..] != stated[..] {
            return Err("snapshot_store_changed".to_owned());
        }
        let journal = ActJournal::from_snapshot(REGISTRY, &snapshot[SNAPSHOT_HEADER..])?;
        if journal.len() as u64 != records {
            return Err("snapshot_records".to_owned());
        }
        for (index, id) in journal.act_ids().enumerate() {
            self.ids.insert(id.into(), index as u64 + 1);
        }
        self.end = length;
        Ok(journal)
    }

    /// Check and fold every record after `self.end`, cutting a torn last line.
    fn fold_tail(&mut self, path: &Path, journal: &mut ActJournal) -> Result<(), String> {
        let mut file = File::open(path).map_err(|e| format!("bridge store: {e}"))?;
        file.seek(SeekFrom::Start(self.end))
            .map_err(|e| format!("bridge store: {e}"))?;
        let mut reader = BufReader::with_capacity(1 << 20, file);
        // Parsing and checking each record's digest needs nothing from the journal, so it runs on
        // every core a chunk at a time; folding then takes the records in order.
        let threads = std::thread::available_parallelism()
            .map_or(1, |n| n.get())
            .clamp(1, 8);
        let mut offset = self.end;
        let mut torn = false;
        while !torn {
            let mut chunk: Vec<Vec<u8>> = Vec::with_capacity(RESTORE_CHUNK);
            while chunk.len() < RESTORE_CHUNK {
                let mut line = Vec::new();
                let read = reader
                    .read_until(b'\n', &mut line)
                    .map_err(|e| format!("bridge store: {e}"))?;
                if read == 0 {
                    break;
                }
                if line.last() != Some(&b'\n') {
                    // A record cut by a crash was never acknowledged.
                    torn = true;
                    break;
                }
                line.pop();
                chunk.push(line);
            }
            if chunk.is_empty() {
                break;
            }
            let size = chunk.len().div_ceil(threads);
            let checked: Vec<Result<CheckedRecord, String>> = std::thread::scope(|scope| {
                let parts: Vec<_> = chunk
                    .chunks(size)
                    .map(|part| {
                        scope.spawn(move || {
                            part.iter()
                                .map(|line| check_stored_record(line))
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect();
                parts
                    .into_iter()
                    .flat_map(|part| part.join().unwrap_or_default())
                    .collect()
            });
            for (line, record) in chunk.iter().zip(checked) {
                let seq = self.lines.len() as u64 + 1;
                let id = record
                    .and_then(|record| journal.restore_checked(record))
                    .map_err(|error| format!("bridge store: record {seq}: {error}"))?;
                self.ids.insert(id.into(), seq);
                self.lines.push((offset, line.len() as u32));
                self.hasher.update(line);
                self.hasher.update(b"\n");
                offset += line.len() as u64 + 1;
            }
            if chunk.len() < RESTORE_CHUNK {
                break;
            }
        }
        if torn {
            self.file
                .set_len(offset)
                .map_err(|e| format!("bridge store: {e}"))?;
        }
        self.end = offset;
        Ok(())
    }

    /// The snapshot file for the journal as it stands: the store's length, its
    /// records and the digest of its bytes, then the folds.
    fn snapshot(&self, journal: &ActJournal) -> Vec<u8> {
        let folds = journal.snapshot();
        let mut out = Vec::with_capacity(SNAPSHOT_HEADER + folds.len());
        out.extend_from_slice(SNAPSHOT_MAGIC);
        out.extend_from_slice(&self.end.to_le_bytes());
        out.extend_from_slice(&self.len().to_le_bytes());
        out.extend_from_slice(self.hasher.clone().finalize().as_bytes());
        out.extend_from_slice(&folds);
        out
    }

    pub fn len(&self) -> u64 {
        self.lines.len() as u64
    }
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    fn write(&mut self, id: &str, record: &[u8]) -> std::io::Result<u64> {
        let mut line = Vec::with_capacity(record.len() + 1);
        line.extend_from_slice(record);
        line.push(b'\n');
        self.file.write_all(&line)?;
        self.hasher.update(&line);
        self.lines.push((self.end, record.len() as u32));
        self.end += line.len() as u64;
        let seq = self.lines.len() as u64;
        self.ids.insert(id.into(), seq);
        Ok(seq)
    }

    fn row(&self, seq: u64) -> Option<Value> {
        let (offset, len) = *self.lines.get(seq.checked_sub(1)? as usize)?;
        let mut bytes = vec![0; len as usize];
        self.file.read_exact_at(&mut bytes, offset).ok()?;
        let record: Value = serde_json::from_slice(&bytes).ok()?;
        Some(json!({"seq": seq, "id": record["id"], "record": record}))
    }

    fn rows_from(&self, seq: u64, limit: usize) -> Vec<Value> {
        (seq.max(1)..=self.len())
            .take(limit)
            .filter_map(|seq| self.row(seq))
            .collect()
    }

    fn rows_before(&self, seq: u64, limit: usize) -> Vec<Value> {
        (1..seq.min(self.len() + 1))
            .rev()
            .take(limit)
            .filter_map(|seq| self.row(seq))
            .collect()
    }

    fn by_id(&self, id: &str) -> Option<Value> {
        self.row(*self.ids.get(id)?)
    }
}

// ---------------------------------------------------------------- state

struct State {
    journal: ActJournal,
    store: Store,
    paused: bool,
    agent_tier: String,
    last_proposing: HashMap<String, Instant>,
    streams: Vec<mpsc::SyncSender<u64>>,
    ids: u64,
    since_snapshot: u64,
}

impl State {
    /// The open review queue, which the journal keeps in step with each append.
    fn queue(&self) -> usize {
        self.journal.open_proposals()
    }

    fn fresh_id(&mut self, kind: &str) -> String {
        self.ids = self.ids.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut mixed = self.ids;
        mixed ^= mixed >> 30;
        mixed = mixed.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        mixed ^= mixed >> 27;
        let prefix: String = kind
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || "._:-".contains(c) {
                    c
                } else {
                    '-'
                }
            })
            .take(48)
            .collect();
        let prefix = prefix.trim_start_matches(|c: char| !c.is_ascii_alphanumeric());
        format!(
            "{}-{}-{}",
            if prefix.is_empty() { "act" } else { prefix },
            radix36(now_ms() as u64),
            &radix36(mixed)[..8.min(radix36(mixed).len())]
        )
    }

    /// Append drafts in order; the first refusal stops the batch. One sync of the
    /// store covers the batch, then pages following the journal hear of it.
    fn append(&mut self, drafts: &[Vec<u8>]) -> Value {
        let mut appended = Vec::new();
        let mut failure = Value::Null;
        for (index, draft) in drafts.iter().enumerate() {
            match self.journal.append_record(draft) {
                Ok(out) => {
                    if let Err(error) = self.store.write(&out.id, &out.record) {
                        // The journal folded a record the store could not keep: stop here.
                        failure =
                            json!({"index": index, "error": format!("bridge_store: {error}")});
                        break;
                    }
                    let record: Value = serde_json::from_slice(&out.record).unwrap_or(Value::Null);
                    appended.push(
                        json!({"seq": out.seq, "record": record, "collisions": out.collisions}),
                    );
                }
                Err(error) => {
                    failure = json!({"index": index, "error": error});
                    break;
                }
            }
        }
        if !appended.is_empty() {
            self.since_snapshot += appended.len() as u64;
            let seq = self.store.len();
            self.streams.retain(|stream| {
                !matches!(
                    stream.try_send(seq),
                    Err(mpsc::TrySendError::Disconnected(_))
                )
            });
        }
        json!({"appended": appended, "failure": failure})
    }
}

/// Group commit: an append is acknowledged once a sync of the store covers it.
/// The first waiter syncs everything written so far; the others wait for it, so
/// one sync answers many agents at once and the journal's lock is never held
/// across a sync.
struct Syncer {
    file: File,
    written: std::sync::atomic::AtomicU64,
    state: Mutex<(u64, bool)>,
    done: std::sync::Condvar,
}

impl Syncer {
    fn durable(&self, seq: u64) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        while state.0 < seq {
            if state.1 {
                state = self.done.wait(state).unwrap_or_else(|p| p.into_inner());
                continue;
            }
            state.1 = true;
            let target = self.written.load(Ordering::SeqCst);
            drop(state);
            let _ = self.file.sync_data();
            state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state.0 = state.0.max(target);
            state.1 = false;
            self.done.notify_all();
        }
    }
}

fn radix36(mut value: u64) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = Vec::new();
    loop {
        out.push(DIGITS[(value % 36) as usize]);
        value /= 36;
        if value == 0 {
            break;
        }
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn random_token() -> std::io::Result<String> {
    let mut bytes = [0u8; 24];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// The bridge: its site, its store's state and the limits it was started with.
pub struct Bridge {
    state: Mutex<State>,
    syncer: Syncer,
    restored: Restored,
    snapshotting: Arc<std::sync::atomic::AtomicBool>,
    site: PathBuf,
    store_dir: PathBuf,
    token: String,
    hosts: Vec<String>,
    origins: Vec<String>,
    profile: BridgeProfile,
    csp: String,
    connections: AtomicUsize,
    streams: AtomicUsize,
}

/// What the person chose for the bridge, kept beside the store.
fn read_control(dir: &Path) -> (bool, String) {
    let value: Value = std::fs::read(dir.join(CONTROL_FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null);
    let tier = value["agent_tier"]
        .as_str()
        .filter(|tier| AGENT_TIERS.contains(tier))
        .unwrap_or("propose");
    (value["paused"].as_bool().unwrap_or(false), tier.to_owned())
}

/// The Content-Security-Policy of the Worker's reader pages without its
/// analytics hosts: the page and the bridge are one origin.
fn content_security_policy() -> String {
    let hosts: Vec<String> = MEDIA_HOSTS
        .split("MEDIA_HOSTS=Object.freeze([")
        .nth(1)
        .and_then(|rest| rest.split("])").next())
        .map(|list| {
            list.split('\'')
                .skip(1)
                .step_by(2)
                .map(|host| format!("https://{host}"))
                .collect()
        })
        .unwrap_or_default();
    let media = hosts.join(" ");
    format!(
        "default-src 'self'; base-uri 'none'; connect-src 'self' {media}; font-src 'self'; form-action 'self'; frame-ancestors 'none'; img-src 'self' data: {media}; media-src 'self'; object-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; worker-src 'self'"
    )
}

impl Bridge {
    /// Open the store and fold its records. `address` is the loopback address
    /// the listener is bound to.
    pub fn open(
        site: &Path,
        store_dir: &Path,
        address: SocketAddr,
        profile: BridgeProfile,
    ) -> Result<Self, String> {
        let (store, journal, restored) = Store::open(store_dir)?;
        let syncer = Syncer {
            file: store
                .file
                .try_clone()
                .map_err(|e| format!("bridge store: {e}"))?,
            written: std::sync::atomic::AtomicU64::new(store.len()),
            state: Mutex::new((store.len(), false)),
            done: std::sync::Condvar::new(),
        };
        let (paused, agent_tier) = read_control(store_dir);
        let port = address.port();
        let hosts = vec![
            format!("127.0.0.1:{port}"),
            format!("localhost:{port}"),
            format!("[::1]:{port}"),
        ];
        Ok(Self {
            syncer,
            state: Mutex::new(State {
                journal,
                store,
                paused,
                agent_tier,
                last_proposing: HashMap::new(),
                streams: Vec::new(),
                ids: now_ms() as u64,
                since_snapshot: restored.folded,
            }),
            restored,
            snapshotting: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            site: site.to_owned(),
            store_dir: store_dir.to_owned(),
            token: random_token().map_err(|e| format!("bridge token: {e}"))?,
            origins: hosts.iter().map(|host| format!("http://{host}")).collect(),
            hosts,
            profile,
            csp: content_security_policy(),
            connections: AtomicUsize::new(0),
            streams: AtomicUsize::new(0),
        })
    }

    /// Append under the journal's lock, then wait outside it until the store holds the records on disk.
    fn append_durable(&self, drafts: &[Vec<u8>]) -> Value {
        let result = {
            let mut state = self.lock();
            let result = state.append(drafts);
            self.syncer
                .written
                .store(state.store.len(), Ordering::SeqCst);
            self.maybe_snapshot(&mut state, false);
            result
        };
        self.settle(&result);
        result
    }

    fn settle(&self, result: &Value) {
        if let Some(seq) = result["appended"]
            .as_array()
            .and_then(|rows| rows.last())
            .and_then(|row| row["seq"].as_u64())
        {
            self.syncer.durable(seq);
        }
    }

    /// How the start found the journal (from a snapshot, folded from the store, and why not).
    pub fn restored(&self) -> &Restored {
        &self.restored
    }

    /// Write a snapshot of the folds once enough records were appended since the
    /// last one (or now, when `force`). The folds are copied under the journal's
    /// lock; the file is written beside the store by another thread and
    /// replaces the earlier snapshot whole.
    fn maybe_snapshot(&self, state: &mut State, force: bool) {
        if !force && state.since_snapshot < self.profile.snapshot_every_records {
            return;
        }
        if state.store.is_empty() || self.snapshotting.swap(true, Ordering::SeqCst) {
            return;
        }
        state.since_snapshot = 0;
        let bytes = state.store.snapshot(&state.journal);
        let (dir, busy) = (self.store_dir.clone(), Arc::clone(&self.snapshotting));
        std::thread::spawn(move || {
            let temporary = dir.join(format!("{SNAPSHOT_FILE}.tmp"));
            if std::fs::write(&temporary, &bytes).is_ok() {
                let _ = std::fs::rename(&temporary, dir.join(SNAPSHOT_FILE));
            }
            busy.store(false, Ordering::SeqCst);
        });
    }

    /// Write a snapshot now when the start folded records from the store's lines.
    pub fn snapshot_after_start(&self) {
        if self.restored.folded > 0 {
            let mut state = self.lock();
            self.maybe_snapshot(&mut state, true);
        }
    }

    /// Wait until no snapshot is being written (for a clean stop and for tests).
    pub fn settle_snapshot(&self) {
        while self.snapshotting.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    pub fn acts(&self) -> u64 {
        self.lock().store.len()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn status(&self, state: &mut State) -> Value {
        json!({
            "schema": "tos.bridge.v1",
            "version": env!("CARGO_PKG_VERSION"),
            "posture": {"local_only": true, "source": false, "reviewed_by_tree": false, "canon": false},
            "acts": state.store.len(),
            "paused": state.paused,
            "agent_tier": state.agent_tier,
            "queue": {"open": state.queue(), "soft": self.profile.review_queue_soft, "max": self.profile.review_queue_max},
            "pace": {"step_ms": self.profile.pace_step_ms, "max_ms": self.profile.pace_max_ms},
            "streams": self.streams.load(Ordering::Relaxed),
        })
    }

    fn save_control(&self, state: &State) {
        let value = json!({"paused": state.paused, "agent_tier": state.agent_tier});
        let path = self.store_dir.join(CONTROL_FILE);
        let temporary = self.store_dir.join(format!("{CONTROL_FILE}.tmp"));
        if std::fs::write(&temporary, value.to_string()).is_ok() {
            let _ = std::fs::rename(temporary, path);
        }
    }

    /// The pause an agent keeps before its next ticket or proposal, from the open review queue.
    fn pace(&self, open: usize) -> Result<u64, (String, u64)> {
        let (soft, max) = (
            self.profile.review_queue_soft as usize,
            self.profile.review_queue_max as usize,
        );
        if open >= max {
            return Err(("bridge_queue_full".to_owned(), self.profile.pace_max_ms));
        }
        Ok(if open <= soft {
            0
        } else {
            ((open - soft) as u64)
                .saturating_mul(self.profile.pace_step_ms)
                .min(self.profile.pace_max_ms)
        })
    }

    /// One agent's acts: pause, pace and tier first, then the journal's own rules.
    /// The acts before the first refusal are appended; the refusal is reported
    /// with its index (and, for the pace, how long to wait).
    fn agent_append(&self, items: &[Value]) -> Result<Value, (String, Option<u64>)> {
        let mut state = self.lock();
        if state.paused {
            return Err(("bridge_paused".to_owned(), None));
        }
        let tier = state.agent_tier.clone();
        let now = now_ms();
        let mut drafts = Vec::with_capacity(items.len());
        let mut proposing: Vec<(usize, String)> = Vec::new();
        let mut refused = Value::Null;
        for (index, item) in items.iter().enumerate() {
            let kind = item["kind"].as_str().unwrap_or_default().to_owned();
            if PROPOSING.contains(&kind.as_str()) {
                let author = item["author"]["id"].as_str().unwrap_or_default().to_owned();
                let open = state.queue() + proposing.len();
                let wait = match self.pace(open) {
                    Err((code, wait)) => Some((code, wait)),
                    Ok(0) => None,
                    Ok(gap) => {
                        let waited = state
                            .last_proposing
                            .get(&author)
                            .map_or(u64::MAX, |last| last.elapsed().as_millis() as u64);
                        let again = proposing.iter().any(|(_, other)| *other == author);
                        (again || waited < gap).then(|| {
                            (
                                "bridge_pace".to_owned(),
                                gap.saturating_sub(waited.min(gap)).max(1),
                            )
                        })
                    }
                };
                if let Some((code, wait)) = wait {
                    refused = json!({"index": index, "error": code, "retry_after_ms": wait});
                    break;
                }
                proposing.push((index, author));
            }
            let fresh = state.fresh_id(&kind);
            let input = serde_json::to_vec(item).unwrap_or_default();
            match state.journal.agent_draft(&input, &tier, now, &fresh) {
                Ok(draft) => drafts.push(draft),
                Err(error) => {
                    refused = json!({"index": index, "error": error});
                    break;
                }
            }
        }
        if drafts.is_empty() && !refused.is_null() {
            let wait = refused["retry_after_ms"].as_u64();
            return Err((
                refused["error"].as_str().unwrap_or_default().to_owned(),
                wait,
            ));
        }
        let mut result = state.append(&drafts);
        self.syncer
            .written
            .store(state.store.len(), Ordering::SeqCst);
        self.maybe_snapshot(&mut state, false);
        let appended = result["appended"].as_array().map_or(0, Vec::len);
        for (index, author) in &proposing {
            if *index < appended {
                state.last_proposing.insert(author.clone(), Instant::now());
            }
        }
        drop(state);
        self.settle(&result);
        if result["failure"].is_null() {
            result["failure"] = refused;
        }
        Ok(result)
    }
}

// ---------------------------------------------------------------- MCP

fn manifest(state: &State) -> Value {
    let kinds: Value = state
        .journal
        .query(br#"{"view":"kinds"}"#)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null);
    let commands: Value = serde_json::from_str(ACT_COMMANDS).unwrap_or(Value::Null);
    let pages: Value = serde_json::from_str(PAGE_COMMANDS).unwrap_or(Value::Null);
    let shapes: Value = serde_json::from_str(FIELD_SHAPES).unwrap_or(Value::Null);
    let kind_rows: Vec<Value> = kinds["kinds"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|kind| {
            let name = kind["name"].as_str().unwrap_or_default();
            let mut row = kind.clone();
            row["agent"] = json!(if PERSON_ONLY.contains(&name) {
                "person-only"
            } else if PROPOSING.contains(&name) {
                "propose-tier"
            } else {
                "draft-tier"
            });
            row
        })
        .collect();
    json!({
        "schema": "tos.act.manifest.v1",
        "posture": commands["authority_limit"],
        "tier": state.agent_tier,
        "invariants": kinds["invariants"],
        "kinds": kind_rows,
        "categories": kinds["categories"],
        "decisions": kinds["decisions"],
        "limits": {"targets": 64, "grounds": 64, "parents": 32, "intent_chars": 280, "record_bytes": 65536, "batch": MAX_AGENT_BATCH},
        "addressed_commands": commands["commands"],
        "pointing_commands": pages["commands"].as_array().map(|items| items.iter().map(|c| json!({"command_id": c["command_id"], "availability": c["availability"], "effect": c["effect"]})).collect::<Vec<_>>()),
        "permission_tiers": commands["permission_tiers"],
        "field_shapes": shapes,
        "bridge": {"paused": state.paused, "pacing": "tickets and proposals wait when the person's review queue grows; a refusal names retry_after_ms"},
    })
}

fn tools() -> Value {
    let text = json!({"type": "string"});
    let list = json!({"type": "array", "items": {"type": "string"}});
    let act = json!({"type": "object", "properties": {"id": text, "kind": text, "author": {"type": "object"}, "targets": list, "grounds": list, "intent": text, "body": {"type": "object"}, "parents": list, "replaces": text}, "required": ["kind", "author", "targets", "intent"], "additionalProperties": false});
    let read = json!({"readOnlyHint": true, "untrustedContentHint": true});
    let write = json!({"readOnlyHint": false, "untrustedContentHint": true});
    json!([
        {"name": "tos.act.manifest", "description": "Read first: the kinds of act you may append, what each requires, the categories of tickets and proposals, the decisions, the limits, every addressed and pointing command, your permission tier and whether the bridge is paused. Acts are the person's local work; none of them writes the Tree.", "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false}, "annotations": read},
        {"name": "tos.act.query", "description": "Read one fold of the act journal: summary, act (id), target (address), board, review, decided, leases (now_ms), presence (now_ms, window_ms), squads (now_ms, window_ms), history (id) or kinds.", "inputSchema": {"type": "object", "properties": {"view": text, "id": text, "address": text, "now_ms": {"type": "integer"}, "window_ms": {"type": "integer"}, "limit": {"type": "integer", "minimum": 1, "maximum": 1000}}, "required": ["view"], "additionalProperties": false}, "annotations": read},
        {"name": "tos.act.record", "description": "Read the whole stored act record by id.", "inputSchema": {"type": "object", "properties": {"id": text}, "required": ["id"], "additionalProperties": false}, "annotations": read},
        {"name": "tos.act.append", "description": "Append one act as an agent: author {type:\"agent\",id,model,role?,squad?}, at least one target address, grounds (exact places) unless the kind makes them optional, one line of intent and the kind's body. Take a lease before long work on a place; collisions report another author's live lease. Decisions, assignments and squads belong to the person. When the person's review queue grows, tickets and proposals are paced: a refusal names retry_after_ms.", "inputSchema": act, "annotations": write},
        {"name": "tos.act.append-batch", "description": format!("Append up to {MAX_AGENT_BATCH} acts in order, each as for tos.act.append. Stops at the first refusal and reports it with the acts already appended."), "inputSchema": {"type": "object", "properties": {"acts": {"type": "array", "maxItems": MAX_AGENT_BATCH, "items": {"type": "object"}}}, "required": ["acts"], "additionalProperties": false}, "annotations": write},
    ])
}

fn tool_text(value: Value, error: bool) -> Value {
    json!({"content": [{"type": "text", "text": value.to_string()}], "isError": error})
}

fn summary(row: &Value) -> Value {
    json!({"seq": row["seq"], "id": row["record"]["id"], "digest": row["record"]["digest"], "collisions": row["collisions"]})
}

fn refusal(code: String, retry: Option<u64>) -> Value {
    let mut out = json!({"error": code});
    if let Some(wait) = retry {
        out["retry_after_ms"] = json!(wait);
    }
    out
}

impl Bridge {
    fn call_tool(&self, name: &str, input: &Value) -> Value {
        match name {
            "tos.act.manifest" => tool_text(manifest(&self.lock()), false),
            "tos.act.query" => {
                let state = self.lock();
                match state.journal.query(input.to_string().as_bytes()) {
                    Ok(bytes) => {
                        tool_text(serde_json::from_slice(&bytes).unwrap_or(Value::Null), false)
                    }
                    Err(error) => tool_text(json!({"error": error}), true),
                }
            }
            "tos.act.record" => {
                let id = input["id"].as_str().unwrap_or_default();
                let row = self.lock().store.by_id(id);
                tool_text(
                    row.map_or_else(|| json!({"missing": id}), |row| row["record"].clone()),
                    false,
                )
            }
            "tos.act.append" => match self.agent_append(std::slice::from_ref(input)) {
                Ok(result) => match result["appended"].get(0) {
                    Some(row) => tool_text(summary(row), false),
                    None => tool_text(json!({"error": result["failure"]["error"]}), true),
                },
                Err((code, retry)) => tool_text(refusal(code, retry), true),
            },
            "tos.act.append-batch" => {
                let items = input["acts"].as_array().cloned().unwrap_or_default();
                if items.len() > MAX_AGENT_BATCH {
                    return tool_text(json!({"error": "act_batch"}), true);
                }
                match self.agent_append(&items) {
                    Ok(result) => tool_text(
                        json!({"appended": result["appended"].as_array().map(|rows| rows.iter().map(summary).collect::<Vec<_>>()), "failure": result["failure"]}),
                        false,
                    ),
                    Err((code, retry)) => tool_text(
                        json!({"appended": [], "failure": refusal(code, retry)}),
                        true,
                    ),
                }
            }
            _ => tool_text(json!({"error": "unknown_tool"}), true),
        }
    }

    /// One JSON-RPC message of MCP; notifications have no answer.
    fn rpc(&self, message: &Value) -> Option<Value> {
        let id = message.get("id").cloned()?;
        let method = message["method"].as_str().unwrap_or_default();
        let result = match method {
            "initialize" => {
                let asked = message["params"]["protocolVersion"]
                    .as_str()
                    .unwrap_or_default();
                let version = MCP_VERSIONS
                    .iter()
                    .find(|v| **v == asked)
                    .copied()
                    .unwrap_or(MCP_VERSIONS[0]);
                json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {"name": "tos-bridge", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": "The person's own act journal at the Tree of Sophia bridge. Read tos.act.manifest first. Acts are local personal work: they never write the Tree's sources, review or canon.",
                })
            }
            "ping" => json!({}),
            "tools/list" => json!({"tools": tools()}),
            "tools/call" => {
                let name = message["params"]["name"].as_str().unwrap_or_default();
                let input = message["params"]
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                self.call_tool(name, &input)
            }
            _ => {
                return Some(
                    json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": "Method not found"}}),
                );
            }
        };
        Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
    }
}

// ---------------------------------------------------------------- HTTP

struct Request {
    method: String,
    path: String,
    query: HashMap<String, String>,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }
    fn cookie(&self, name: &str) -> Option<&str> {
        self.header("cookie")?
            .split(';')
            .filter_map(|part| part.trim().split_once('='))
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value)
    }
}

fn read_request(stream: &mut TcpStream, max_body: usize) -> Result<Request, u16> {
    let mut buffer = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(at) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            break at;
        }
        if buffer.len() > MAX_HEADER_BYTES {
            return Err(431);
        }
        let read = stream.read(&mut chunk).map_err(|_| 400u16)?;
        if read == 0 {
            return Err(400);
        }
        buffer.extend_from_slice(&chunk[..read]);
    };
    let head = std::str::from_utf8(&buffer[..head_end]).map_err(|_| 400u16)?;
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or_default().split(' ');
    let method = first.next().unwrap_or_default().to_owned();
    let target = first.next().unwrap_or_default();
    let mut headers = HashMap::new();
    for line in lines {
        let (name, value) = line.split_once(':').ok_or(400u16)?;
        headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
    }
    if headers.contains_key("transfer-encoding") {
        return Err(411);
    }
    let length: usize = match headers.get("content-length") {
        Some(value) => value.parse().map_err(|_| 400u16)?,
        None => 0,
    };
    if length > max_body {
        return Err(413);
    }
    let mut body = buffer[head_end + 4..].to_vec();
    while body.len() < length {
        let read = stream.read(&mut chunk).map_err(|_| 400u16)?;
        if read == 0 {
            return Err(400);
        }
        body.extend_from_slice(&chunk[..read]);
    }
    body.truncate(length);
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let query = query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    Ok(Request {
        method,
        path: path.to_owned(),
        query,
        headers,
        body,
    })
}

struct Response {
    status: u16,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
}

impl Response {
    fn json(status: u16, value: &Value) -> Self {
        Self {
            status,
            headers: vec![
                ("Content-Type", "application/json; charset=utf-8".to_owned()),
                ("Cache-Control", "no-store".to_owned()),
            ],
            body: value.to_string().into_bytes(),
        }
    }
    fn bytes(status: u16, content_type: &str, body: Vec<u8>) -> Self {
        Self {
            status,
            headers: vec![
                ("Content-Type", content_type.to_owned()),
                ("Cache-Control", "no-store".to_owned()),
            ],
            body,
        }
    }
    fn text(status: u16, message: &str) -> Self {
        Self::bytes(
            status,
            "text/plain; charset=utf-8",
            message.as_bytes().to_vec(),
        )
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        411 => "Length Required",
        413 => "Content Too Large",
        421 => "Misdirected Request",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        503 => "Service Unavailable",
        _ => "Error",
    }
}

impl Bridge {
    /// The Worker's headers, except that the person's own pages may use the orientation
    /// sensor (the window into the sky) and the microphone (a word spoken into the
    /// collection), which the published site keeps closed (gate Q5).
    fn security_headers(&self) -> Vec<(&'static str, String)> {
        vec![
            ("Content-Security-Policy", self.csp.clone()),
            ("Permissions-Policy", "tools=(self), accelerometer=(self), camera=(), geolocation=(), gyroscope=(self), microphone=(self), payment=(), usb=()".to_owned()),
            ("Cross-Origin-Opener-Policy", "same-origin".to_owned()),
            ("Cross-Origin-Embedder-Policy", "require-corp".to_owned()),
            ("Cross-Origin-Resource-Policy", "same-origin".to_owned()),
            ("Origin-Agent-Cluster", "?1".to_owned()),
            ("Referrer-Policy", "no-referrer".to_owned()),
            ("X-Content-Type-Options", "nosniff".to_owned()),
            ("X-Frame-Options", "DENY".to_owned()),
        ]
    }

    fn write(&self, stream: &mut TcpStream, response: Response, head_only: bool) {
        let mut out = format!(
            "HTTP/1.1 {} {}\r\n",
            response.status,
            reason(response.status)
        );
        for (name, value) in self
            .security_headers()
            .iter()
            .chain(response.headers.iter())
        {
            out.push_str(&format!("{name}: {value}\r\n"));
        }
        out.push_str(&format!(
            "Content-Length: {}\r\nConnection: close\r\n\r\n",
            response.body.len()
        ));
        let _ = stream.write_all(out.as_bytes());
        if !head_only {
            let _ = stream.write_all(&response.body);
        }
        let _ = stream.flush();
    }

    /// Serve one connection: a request, then close (an event stream stays open).
    pub fn handle(self: &Arc<Self>, mut stream: TcpStream) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
        let request = match read_request(&mut stream, self.profile.max_body_bytes as usize) {
            Ok(request) => request,
            Err(status) => {
                return self.write(&mut stream, Response::text(status, reason(status)), false);
            }
        };
        // A page of another site, or a name that only resolves here, never reaches the store.
        if !request
            .header("host")
            .is_some_and(|host| self.hosts.iter().any(|h| h == host))
        {
            return self.write(
                &mut stream,
                Response::text(421, "misdirected request"),
                false,
            );
        }
        if request
            .header("origin")
            .is_some_and(|origin| !self.origins.iter().any(|o| o == origin))
        {
            return self.write(&mut stream, Response::text(403, "foreign origin"), false);
        }
        let head = request.method == "HEAD";
        if request.path == "/__bridge/v1/events" && request.method == "GET" {
            return self.events(stream, &request);
        }
        let response = self.route(&request);
        self.write(&mut stream, response, head);
    }

    fn person(&self, request: &Request) -> bool {
        request.cookie(COOKIE) == Some(self.token.as_str())
    }

    fn route(&self, request: &Request) -> Response {
        let path = request.path.as_str();
        if path == "/mcp" {
            return match request.method.as_str() {
                "POST" => self.mcp(&request.body),
                _ => Response::text(405, "the bridge answers MCP messages by POST"),
            };
        }
        if let Some(api) = path.strip_prefix("/__bridge/v1/") {
            return self.api(api, request);
        }
        if request.method != "GET" && request.method != "HEAD" {
            return Response::text(405, "method not allowed");
        }
        self.static_file(path)
    }

    fn mcp(&self, body: &[u8]) -> Response {
        let Ok(message) = serde_json::from_slice::<Value>(body) else {
            return Response::json(
                400,
                &json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "Parse error"}}),
            );
        };
        let answer = match &message {
            Value::Array(items) => {
                let answers: Vec<Value> = items.iter().filter_map(|item| self.rpc(item)).collect();
                if answers.is_empty() {
                    None
                } else {
                    Some(Value::Array(answers))
                }
            }
            _ => self.rpc(&message),
        };
        match answer {
            Some(value) => Response::json(200, &value),
            None => Response {
                status: 202,
                headers: Vec::new(),
                body: Vec::new(),
            },
        }
    }

    fn api(&self, api: &str, request: &Request) -> Response {
        if api == "hello" {
            let mut state = self.lock();
            let mut status = self.status(&mut state);
            status["person"] = json!(self.person(request));
            return Response::json(200, &status);
        }
        // The rest is the person's page only: it carries the cookie the bridge set on it.
        if !self.person(request) {
            return Response::json(403, &json!({"error": "bridge_person_only"}));
        }
        let number = |name: &str, fallback: u64| {
            request
                .query
                .get(name)
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(fallback)
        };
        let window =
            (number("limit", 100) as usize).clamp(1, self.profile.max_window_records as usize);
        match (request.method.as_str(), api) {
            ("GET", "acts") => {
                let state = self.lock();
                if let Some(id) = request.query.get("id") {
                    return Response::json(200, &json!({"row": state.store.by_id(id)}));
                }
                let rows = if request.query.contains_key("before") {
                    state.store.rows_before(number("before", u64::MAX), window)
                } else {
                    state.store.rows_from(number("from", 1), window)
                };
                Response::json(200, &json!({"last": state.store.len(), "rows": rows}))
            }
            ("POST", "acts") => {
                let Ok(Value::Array(items)) = serde_json::from_slice::<Value>(&request.body) else {
                    return Response::json(400, &json!({"error": "act_invalid:batch"}));
                };
                let drafts: Vec<Vec<u8>> = items
                    .iter()
                    .map(|item| item.to_string().into_bytes())
                    .collect();
                Response::json(200, &self.append_durable(&drafts))
            }
            ("POST", "records") => {
                let ids: Vec<String> = serde_json::from_slice::<Value>(&request.body)
                    .ok()
                    .and_then(|value| value["ids"].as_array().cloned())
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|id| id.as_str().map(str::to_owned))
                    .take(self.profile.max_window_records as usize)
                    .collect();
                let state = self.lock();
                let rows: Vec<Value> = ids.iter().filter_map(|id| state.store.by_id(id)).collect();
                Response::json(200, &json!({"rows": rows}))
            }
            ("POST", "query") => match self.lock().journal.query(&request.body) {
                Ok(bytes) => Response::bytes(200, "application/json; charset=utf-8", bytes),
                Err(error) => Response::json(400, &json!({"error": error})),
            },
            ("POST", "control") => {
                let Ok(change) = serde_json::from_slice::<Value>(&request.body) else {
                    return Response::json(400, &json!({"error": "bridge_control"}));
                };
                let mut state = self.lock();
                if let Some(paused) = change["paused"].as_bool() {
                    state.paused = paused;
                }
                if let Some(tier) = change["agent_tier"].as_str() {
                    if !AGENT_TIERS.contains(&tier) {
                        return Response::json(400, &json!({"error": "act_tier"}));
                    }
                    state.agent_tier = tier.to_owned();
                }
                self.save_control(&state);
                let status = self.status(&mut state);
                Response::json(200, &status)
            }
            _ => Response::json(404, &json!({"error": "bridge_unknown"})),
        }
    }

    /// Follow the journal: an event with the last sequence number after each append.
    fn events(&self, mut stream: TcpStream, request: &Request) {
        if !self.person(request) {
            return self.write(
                &mut stream,
                Response::json(403, &json!({"error": "bridge_person_only"})),
                false,
            );
        }
        if self.streams.fetch_add(1, Ordering::SeqCst) >= self.profile.max_event_streams as usize {
            self.streams.fetch_sub(1, Ordering::SeqCst);
            return self.write(
                &mut stream,
                Response::text(503, "too many event streams"),
                false,
            );
        }
        let (sender, receiver) = mpsc::sync_channel::<u64>(64);
        let last = {
            let mut state = self.lock();
            state.streams.push(sender);
            state.store.len()
        };
        let mut head = "HTTP/1.1 200 OK\r\n".to_owned();
        for (name, value) in self.security_headers() {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        head.push_str("Content-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: keep-alive\r\n\r\n");
        let mut open = stream.write_all(head.as_bytes()).is_ok()
            && stream
                .write_all(format!("event: acts\ndata: {{\"last\":{last}}}\n\n").as_bytes())
                .is_ok();
        let mut sent = Instant::now();
        while open {
            open = match receiver.recv_timeout(Duration::from_secs(15)) {
                Ok(mut seq) => {
                    // One summary event at most every EVENT_GAP: an army's acts reach the page as a stream of
                    // last sequence numbers, not one event an act.
                    let due = sent + EVENT_GAP;
                    while let Some(left) = due.checked_duration_since(Instant::now()) {
                        match receiver.recv_timeout(left) {
                            Ok(next) => seq = next,
                            Err(_) => break,
                        }
                    }
                    while let Ok(next) = receiver.try_recv() {
                        seq = next;
                    }
                    sent = Instant::now();
                    stream
                        .write_all(format!("event: acts\ndata: {{\"last\":{seq}}}\n\n").as_bytes())
                        .is_ok()
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    stream.write_all(b": keep-alive\n\n").is_ok()
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => false,
            };
        }
        self.streams.fetch_sub(1, Ordering::SeqCst);
    }

    /// The built site at the Worker's paths: `/` the home page, `/<stand>/` a stand,
    /// `/__research/…` the build's files, and the root files beside them.
    fn static_file(&self, path: &str) -> Response {
        let relative = if path == "/" {
            Some("home/index.html".to_owned())
        } else if let Some(rest) = path.strip_prefix("/__research/") {
            Some(if rest.is_empty() || rest.ends_with('/') {
                format!("{rest}index.html")
            } else {
                rest.to_owned()
            })
        } else {
            let name = path.trim_matches('/');
            if !name.is_empty()
                && !name.contains('/')
                && name.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
                && self.site.join(name).join("index.html").is_file()
            {
                Some(format!("{name}/index.html"))
            } else if [
                "/favicon.svg",
                "/favicon.ico",
                "/apple-touch-icon.png",
                "/robots.txt",
                "/sitemap.xml",
                "/theme.js",
                "/sw.js",
            ]
            .contains(&path)
                || (path.starts_with("/og/") && path.ends_with(".jpg"))
            {
                Some(path[1..].to_owned())
            } else {
                None
            }
        };
        let file = relative
            .filter(|relative| {
                relative
                    .split('/')
                    .all(|part| !part.is_empty() && part != "." && part != "..")
                    && relative
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-/~".contains(&b))
            })
            .map(|relative| self.site.join(relative))
            .filter(|file| file.is_file());
        let Some(file) = file else {
            let page = std::fs::read(self.site.join("not-found/index.html"))
                .unwrap_or_else(|_| b"not found".to_vec());
            return Response::bytes(404, "text/html; charset=utf-8", page);
        };
        let Ok(body) = std::fs::read(&file) else {
            return Response::text(404, "not found");
        };
        let extension = file
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();
        let mut response = Response::bytes(200, content_type(extension), body);
        response.headers[1].1 = if path.starts_with("/__research/assets/") {
            "public, max-age=31536000, immutable".to_owned()
        } else {
            "no-cache".to_owned()
        };
        if extension == "html" {
            // The person's page receives the key to the store; another origin never sees it.
            response.headers.push((
                "Set-Cookie",
                format!("{COOKIE}={}; Path=/; HttpOnly; SameSite=Strict", self.token),
            ));
        }
        response
    }
}

fn content_type(extension: &str) -> &'static str {
    match extension {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "webmanifest" => "application/manifest+json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "wasm" => "application/wasm",
        "woff2" => "font/woff2",
        "txt" => "text/plain; charset=utf-8",
        "xml" => "application/xml",
        "mp3" => "audio/mpeg",
        "ogg" | "oga" => "audio/ogg",
        "m4a" => "audio/mp4",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

/// Accept connections until the listener fails, one thread a connection, within the profile's limit.
pub fn serve(listener: TcpListener, bridge: Arc<Bridge>) -> std::io::Result<()> {
    for stream in listener.incoming() {
        let mut stream = stream?;
        if bridge.connections.fetch_add(1, Ordering::SeqCst)
            >= bridge.profile.max_connections as usize
        {
            bridge.connections.fetch_sub(1, Ordering::SeqCst);
            bridge.write(
                &mut stream,
                Response::text(503, "too many connections"),
                false,
            );
            continue;
        }
        let bridge = Arc::clone(&bridge);
        std::thread::spawn(move || {
            bridge.handle(stream);
            bridge.connections.fetch_sub(1, Ordering::SeqCst);
        });
    }
    Ok(())
}

// ---------------------------------------------------------------- command

const USAGE: &str = "usage: tos bridge [LOOPBACK:PORT] --site BUILT_SITE_DIR --store STORE_DIR\n       tos bridge mcp --connect LOOPBACK:PORT   MCP over stdio, relayed to a running bridge\n";

fn loopback(text: &str) -> Option<SocketAddr> {
    let address: SocketAddr = text.parse().ok()?;
    match address.ip() {
        IpAddr::V4(ip) if ip.is_loopback() => Some(address),
        IpAddr::V6(ip) if ip.is_loopback() => Some(address),
        _ => None,
    }
}

/// Relay MCP messages from stdin to a running bridge and its answers to stdout, one JSON line each.
fn relay(address: SocketAddr, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { return 1 };
        if line.trim().is_empty() {
            continue;
        }
        let answer = (|| -> std::io::Result<Vec<u8>> {
            let mut stream = TcpStream::connect(address)?;
            write!(
                stream,
                "POST /mcp HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{line}",
                line.len()
            )?;
            let mut out = Vec::new();
            stream.read_to_end(&mut out)?;
            Ok(out)
        })();
        match answer {
            Ok(bytes) => {
                let body = bytes
                    .windows(4)
                    .position(|w| w == b"\r\n\r\n")
                    .map_or(&bytes[..0], |at| &bytes[at + 4..]);
                if !body.is_empty() {
                    let _ = stdout.write_all(body);
                    let _ = stdout.write_all(b"\n");
                    let _ = stdout.flush();
                }
            }
            Err(error) => {
                let _ = writeln!(stderr, "tos bridge mcp: {error}");
                return 1;
            }
        }
    }
    0
}

/// `tos bridge …`; `None` when the arguments name another command.
pub fn run_if_requested(
    args: &[String],
    profile: &BridgeProfile,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Option<i32> {
    if args.first().map(String::as_str) != Some("bridge") {
        return None;
    }
    let mut rest = args[1..].iter();
    let (mut address, mut site, mut store, mut connect, mut relay_mode) =
        (None, None, None, None, false);
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                let _ = stdout.write_all(USAGE.as_bytes());
                return Some(0);
            }
            "--site" => site = rest.next().map(PathBuf::from),
            "--store" => store = rest.next().map(PathBuf::from),
            "--connect" => connect = rest.next().cloned(),
            "mcp" if !relay_mode && address.is_none() => relay_mode = true,
            other if address.is_none() && !other.starts_with("--") => {
                address = Some(other.to_owned())
            }
            _ => {
                let _ = stderr.write_all(USAGE.as_bytes());
                return Some(2);
            }
        }
    }
    if relay_mode {
        let Some(address) = connect.as_deref().and_then(loopback) else {
            let _ = writeln!(
                stderr,
                "tos bridge mcp: --connect needs a loopback address and port"
            );
            return Some(2);
        };
        return Some(relay(address, stdout, stderr));
    }
    let (Some(site), Some(store)) = (site, store) else {
        let _ = stderr.write_all(USAGE.as_bytes());
        return Some(2);
    };
    let Some(address) = loopback(address.as_deref().unwrap_or("127.0.0.1:8742")) else {
        let _ = writeln!(
            stderr,
            "tos bridge: the bridge listens on a loopback address only"
        );
        return Some(2);
    };
    if !site.join("home/index.html").is_file() {
        let _ = writeln!(
            stderr,
            "tos bridge: {} is not a built public-research site",
            site.display()
        );
        return Some(2);
    }
    if !site.join("layers").is_dir() {
        let _ = writeln!(
            stderr,
            "tos bridge: {} holds no layers: the stands' word layers and media copies will be missing (build it with `npm run build:public-research --prefix access/web`)",
            site.display()
        );
    }
    let listener = match TcpListener::bind(address) {
        Ok(listener) => listener,
        Err(error) => {
            let _ = writeln!(stderr, "tos bridge: {address}: {error}");
            return Some(1);
        }
    };
    let bound = listener.local_addr().unwrap_or(address);
    let started = Instant::now();
    let bridge = match Bridge::open(&site, &store, bound, profile.clone()) {
        Ok(bridge) => Arc::new(bridge),
        Err(error) => {
            let _ = writeln!(stderr, "tos bridge: {error}");
            return Some(1);
        }
    };
    let _ = writeln!(stdout, "http://{bound}/");
    let _ = stdout.flush();
    let restored = bridge.restored();
    let _ = writeln!(
        stderr,
        "tos bridge: {} acts restored in {} ms from {} ({} from the snapshot of the folds, {} checked and folded{}); MCP at http://{bound}/mcp; local only",
        bridge.acts(),
        started.elapsed().as_millis(),
        store.display(),
        restored.from_snapshot,
        restored.folded,
        restored
            .note
            .as_deref()
            .map(|note| format!("; snapshot not used: {note}"))
            .unwrap_or_default()
    );
    bridge.snapshot_after_start();
    match serve(listener, bridge) {
        Ok(()) => Some(0),
        Err(error) => {
            let _ = writeln!(stderr, "tos bridge: {error}");
            Some(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        dir: PathBuf,
        address: SocketAddr,
    }

    fn fixture(name: &str, profile: Option<BridgeProfile>) -> Fixture {
        let dir = std::env::temp_dir().join(format!("tos-bridge-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("site/home")).unwrap();
        std::fs::create_dir_all(dir.join("site/zarathustra")).unwrap();
        std::fs::create_dir_all(dir.join("site/assets")).unwrap();
        std::fs::write(
            dir.join("site/home/index.html"),
            "<!doctype html><title>home</title>",
        )
        .unwrap();
        std::fs::write(
            dir.join("site/zarathustra/index.html"),
            "<!doctype html><title>Z</title>",
        )
        .unwrap();
        std::fs::write(dir.join("site/assets/app-1.js"), "export{}").unwrap();
        std::fs::write(dir.join("secret.txt"), "private").unwrap();
        let address = start(&dir, profile);
        Fixture { dir, address }
    }

    fn default_profile() -> BridgeProfile {
        tos_foundation::deployment_profile::parse_deployment_profile(
            tos_foundation::deployment_profile::DEFAULT_DEPLOYMENT_PROFILE,
        )
        .unwrap()
        .bridge
    }

    fn start(dir: &Path, profile: Option<BridgeProfile>) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let bridge = Arc::new(
            Bridge::open(
                &dir.join("site"),
                &dir.join("store"),
                address,
                profile.unwrap_or_else(default_profile),
            )
            .unwrap(),
        );
        std::thread::spawn(move || serve(listener, bridge));
        address
    }

    fn request(address: SocketAddr, raw: &str) -> (u16, String, String) {
        let mut stream = TcpStream::connect(address).unwrap();
        stream.write_all(raw.as_bytes()).unwrap();
        let mut out = String::new();
        stream.read_to_string(&mut out).unwrap();
        let (head, body) = out.split_once("\r\n\r\n").unwrap();
        let status = head.split(' ').nth(1).unwrap().parse().unwrap();
        (status, head.to_owned(), body.to_owned())
    }

    fn get(address: SocketAddr, path: &str, extra: &str) -> (u16, String, String) {
        request(
            address,
            &format!("GET {path} HTTP/1.1\r\nHost: {address}\r\n{extra}\r\n"),
        )
    }

    fn post(address: SocketAddr, path: &str, extra: &str, body: &str) -> (u16, String, String) {
        request(
            address,
            &format!(
                "POST {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{extra}\r\n{body}",
                body.len()
            ),
        )
    }

    fn cookie(address: SocketAddr) -> String {
        let (_, head, _) = get(address, "/zarathustra/", "");
        let line = head
            .lines()
            .find(|line| line.starts_with("Set-Cookie:"))
            .unwrap();
        format!(
            "Cookie: {}\r\n",
            line["Set-Cookie: ".len()..].split(';').next().unwrap()
        )
    }

    fn mcp(address: SocketAddr, method: &str, params: Value) -> Value {
        let body =
            json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).to_string();
        let (status, _, body) = post(address, "/mcp", "", &body);
        assert_eq!(status, 200, "{body}");
        serde_json::from_str(&body).unwrap()
    }

    fn call(address: SocketAddr, name: &str, arguments: Value) -> (bool, Value) {
        let answer = mcp(
            address,
            "tools/call",
            json!({"name": name, "arguments": arguments}),
        );
        let result = &answer["result"];
        (
            result["isError"].as_bool().unwrap(),
            serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap(),
        )
    }

    fn note(agent: &str, target: &str) -> Value {
        json!({"kind": "note", "author": {"type": "agent", "id": agent, "model": "m"}, "targets": [target], "intent": "reading", "body": {"text": "seen"}})
    }

    fn proposal(agent: &str, n: usize) -> Value {
        json!({"kind": "proposal", "author": {"type": "agent", "id": agent, "model": "m"}, "targets": [format!("link:{n}")], "grounds": [format!("place:{n}")], "intent": "word", "body": {"category": "translation", "change": {"from": "дитя", "to": "ребёнок"}}})
    }

    #[test]
    fn the_site_is_served_at_the_workers_paths_and_nothing_else() {
        let f = fixture("site", None);
        let (status, head, body) = get(f.address, "/", "");
        assert_eq!((status, body.contains("home")), (200, true));
        assert!(head.contains("Content-Security-Policy: default-src 'self'"));
        assert!(head.contains("'wasm-unsafe-eval'"));
        assert!(head.contains("Set-Cookie: tos_bridge="));
        assert!(get(f.address, "/zarathustra", "").2.contains("<title>Z"));
        let (status, head, _) = get(f.address, "/__research/assets/app-1.js", "");
        assert_eq!(status, 200);
        assert!(head.contains("immutable") && head.contains("text/javascript"));
        assert_eq!(get(f.address, "/__research/../secret.txt", "").0, 404);
        assert_eq!(get(f.address, "/__research/%2e%2e/secret.txt", "").0, 404);
        assert_eq!(get(f.address, "/nowhere/", "").0, 404);
        // A name that resolves here from another site is refused before anything else.
        let (status, _, _) = request(f.address, "GET / HTTP/1.1\r\nHost: evil.example\r\n\r\n");
        assert_eq!(status, 421);
        let (status, _, _) = post(f.address, "/mcp", "Origin: https://evil.example\r\n", "{}");
        assert_eq!(status, 403);
        let _ = std::fs::remove_dir_all(&f.dir);
    }

    #[test]
    fn the_persons_page_appends_and_the_store_restores_every_record() {
        let f = fixture("store", None);
        let key = cookie(f.address);
        let draft = json!([{"schema": "tos.act.v1", "id": "t1", "kind": "ticket", "author": {"type": "human", "id": "local-person"}, "targets": ["link:1"], "grounds": [], "intent": "word", "body": {"category": "translation", "observation": "дитя"}, "parents": [], "at": "2026-11-20T10:00:00.000Z", "posture": {"local_only": true, "source": false, "reviewed_by_tree": false, "canon": false}}]).to_string();
        assert_eq!(post(f.address, "/__bridge/v1/acts", "", &draft).0, 403);
        let (status, _, body) = post(f.address, "/__bridge/v1/acts", &key, &draft);
        assert_eq!(status, 200);
        let out: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(out["appended"][0]["seq"], 1);
        assert!(out["failure"].is_null());
        let (_, _, body) = get(f.address, "/__bridge/v1/acts?from=1&limit=10", &key);
        let rows: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(rows["rows"][0]["record"]["id"], "t1");
        // A second bridge over the same store folds the same journal.
        let again = start(&f.dir, None);
        let key = cookie(again);
        let (_, _, body) = post(again, "/__bridge/v1/query", &key, r#"{"view":"board"}"#);
        let board: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(board["counts"]["open"], 1);
        let (_, _, body) = post(
            again,
            "/__bridge/v1/records",
            &key,
            r#"{"ids":["t1","nope"]}"#,
        );
        assert_eq!(
            serde_json::from_str::<Value>(&body).unwrap()["rows"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let (_, _, body) = get(again, "/__bridge/v1/acts?id=t1", &key);
        assert_eq!(
            serde_json::from_str::<Value>(&body).unwrap()["row"]["seq"],
            1
        );
        let _ = std::fs::remove_dir_all(&f.dir);
    }

    #[test]
    fn a_torn_last_line_is_cut_and_a_changed_record_stops_the_start() {
        let dir = std::env::temp_dir().join(format!("tos-bridge-torn-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (mut store, mut journal, _) = Store::open(&dir).unwrap();
        let record = journal
            .append_record(br#"{"schema":"tos.act.v1","id":"n1","kind":"note","author":{"type":"human","id":"p"},"targets":["x"],"grounds":[],"intent":"a","body":{"text":"t"},"parents":[],"at":"2026-11-20T10:00:00.000Z","posture":{"local_only":true,"source":false,"reviewed_by_tree":false,"canon":false}}"#)
            .unwrap();
        store.write(&record.id, &record.record).unwrap();
        drop(store);
        let path = dir.join(STORE_FILE);
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"{\"schema\":\"tos.act").unwrap();
        assert_eq!(Store::open(&dir).unwrap().0.len(), 1);
        assert!(std::fs::read_to_string(&path).unwrap().ends_with("}\n"));
        let changed = std::fs::read_to_string(&path)
            .unwrap()
            .replace("\"a\"", "\"b\"");
        std::fs::write(&path, changed).unwrap();
        assert!(
            Store::open(&dir)
                .err()
                .unwrap()
                .contains("record 1: act_digest_mismatch")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn agents_append_through_mcp_within_their_tier() {
        let f = fixture("mcp", None);
        let init = mcp(
            f.address,
            "initialize",
            json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}),
        );
        assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
        let (status, _, _) = post(
            f.address,
            "/mcp",
            "",
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        );
        assert_eq!(status, 202);
        let names: Vec<String> = mcp(f.address, "tools/list", json!({}))["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_owned())
            .collect();
        assert!(names.contains(&"tos.act.append-batch".to_owned()));
        let (error, manifest) = call(f.address, "tos.act.manifest", json!({}));
        assert!(!error);
        assert_eq!(manifest["tier"], "propose");
        assert!(
            manifest["kinds"]
                .as_array()
                .unwrap()
                .iter()
                .any(|k| k["name"] == "review" && k["agent"] == "person-only")
        );
        let (error, out) = call(f.address, "tos.act.append", note("a1", "III.02"));
        assert!(!error, "{out}");
        assert_eq!(out["seq"], 1);
        let mut human = note("a1", "x");
        human["author"]["type"] = json!("human");
        assert_eq!(
            call(f.address, "tos.act.append", human).1["error"],
            "act_author"
        );
        let review = json!({"kind": "review", "author": {"type": "agent", "id": "a1"}, "targets": ["act:x"], "intent": "x", "body": {"decision": "accept", "reason": "r"}});
        assert_eq!(
            call(f.address, "tos.act.append", review).1["error"],
            "act_person_only"
        );
        let (error, out) = call(
            f.address,
            "tos.act.append-batch",
            json!({"acts": [note("a2", "a"), note("a2", "b"), {"kind": "note"}]}),
        );
        assert!(!error);
        assert_eq!(out["appended"].as_array().unwrap().len(), 2);
        assert_eq!(out["failure"]["index"], 2);
        let (_, record) = call(
            f.address,
            "tos.act.record",
            json!({"id": out["appended"][0]["id"]}),
        );
        assert_eq!(record["author"]["id"], "a2");
        let (_, summary) = call(f.address, "tos.act.query", json!({"view": "summary"}));
        assert_eq!(summary["acts"], 3);
        let _ = std::fs::remove_dir_all(&f.dir);
    }

    #[test]
    fn the_person_pauses_the_army_and_the_queue_paces_proposals() {
        let mut profile = default_profile();
        profile.review_queue_soft = 2;
        profile.review_queue_max = 4;
        profile.pace_step_ms = 60_000;
        let f = fixture("pace", Some(profile));
        let key = cookie(f.address);
        for n in 0..3 {
            let (error, out) = call(f.address, "tos.act.append", proposal(&format!("a{n}"), n));
            assert!(!error, "{out}");
        }
        // Three open proposals, one above the soft queue: the agent that just proposed waits.
        let (error, out) = call(f.address, "tos.act.append", proposal("a2", 9));
        assert!(error);
        assert_eq!(out["error"], "bridge_pace");
        assert!(out["retry_after_ms"].as_u64().unwrap() > 0);
        assert!(!call(f.address, "tos.act.append", proposal("a9", 10)).0);
        assert_eq!(
            call(f.address, "tos.act.append", proposal("a8", 11)).1["error"],
            "bridge_queue_full"
        );
        // Notes go on; a pause stops every agent act until the person lifts it.
        assert!(!call(f.address, "tos.act.append", note("a8", "x")).0);
        let (status, _, body) = post(
            f.address,
            "/__bridge/v1/control",
            &key,
            r#"{"paused":true}"#,
        );
        assert_eq!(status, 200);
        assert_eq!(
            serde_json::from_str::<Value>(&body).unwrap()["paused"],
            true
        );
        assert_eq!(
            call(f.address, "tos.act.append", note("a8", "y")).1["error"],
            "bridge_paused"
        );
        let restarted = start(&f.dir, None);
        let (_, _, body) = get(restarted, "/__bridge/v1/hello", "");
        assert_eq!(
            serde_json::from_str::<Value>(&body).unwrap()["paused"],
            true
        );
        post(
            f.address,
            "/__bridge/v1/control",
            &key,
            r#"{"paused":false,"agent_tier":"read"}"#,
        );
        assert_eq!(
            call(f.address, "tos.act.append", note("a8", "y")).1["error"],
            "act_tier"
        );
        let _ = std::fs::remove_dir_all(&f.dir);
    }

    #[test]
    fn the_page_hears_of_each_append() {
        let f = fixture("events", None);
        let key = cookie(f.address);
        let mut stream = TcpStream::connect(f.address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        write!(
            stream,
            "GET /__bridge/v1/events HTTP/1.1\r\nHost: {}\r\n{key}\r\n",
            f.address
        )
        .unwrap();
        let mut seen = String::new();
        let mut buffer = [0u8; 1024];
        while !seen.contains("\"last\":0") {
            let read = stream.read(&mut buffer).unwrap();
            seen.push_str(std::str::from_utf8(&buffer[..read]).unwrap());
        }
        call(f.address, "tos.act.append", note("a1", "x"));
        while !seen.contains("\"last\":1") {
            let read = stream.read(&mut buffer).unwrap();
            seen.push_str(std::str::from_utf8(&buffer[..read]).unwrap());
        }
        assert!(seen.contains("text/event-stream"));
        let _ = std::fs::remove_dir_all(&f.dir);
    }

    #[test]
    fn a_start_reads_the_snapshot_of_the_folds_and_only_the_records_after_it() {
        let f = fixture("snapshot", None);
        let key = cookie(f.address);
        let note = |id: &str| json!({"schema": "tos.act.v1", "id": id, "kind": "note", "author": {"type": "human", "id": "p"}, "targets": ["x"], "grounds": [], "intent": "a", "body": {"text": "t"}, "parents": [], "at": "2026-11-20T10:00:00.000Z", "posture": {"local_only": true, "source": false, "reviewed_by_tree": false, "canon": false}});
        let batch = json!((0..40).map(|n| note(&format!("n{n}"))).collect::<Vec<_>>()).to_string();
        assert_eq!(post(f.address, "/__bridge/v1/acts", &key, &batch).0, 200);
        let open = || {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            Bridge::open(
                &f.dir.join("site"),
                &f.dir.join("store"),
                listener.local_addr().unwrap(),
                default_profile(),
            )
            .unwrap()
        };
        let first = open();
        assert_eq!(
            (first.restored().from_snapshot, first.restored().folded),
            (0, 40)
        );
        first.snapshot_after_start();
        first.settle_snapshot();
        assert!(f.dir.join("store").join(SNAPSHOT_FILE).is_file());
        let second = open();
        assert_eq!(
            (second.restored().from_snapshot, second.restored().folded),
            (40, 0)
        );
        assert_eq!(second.lock().store.by_id("n39").unwrap()["seq"], 40);
        // Records after the snapshot are checked and folded; the journal goes on from both.
        assert_eq!(
            second.append_durable(&[note("n40").to_string().into_bytes()])["appended"][0]["seq"],
            41
        );
        let third = open();
        assert_eq!(
            (third.restored().from_snapshot, third.restored().folded),
            (40, 1)
        );
        // A store whose first bytes changed is folded whole; the snapshot is not used.
        let path = f.dir.join("store").join(STORE_FILE);
        let text =
            std::fs::read_to_string(&path)
                .unwrap()
                .replacen("{\"schema\"", "{ \"schema\"", 1);
        std::fs::write(&path, text).unwrap();
        let fourth = open();
        assert_eq!(
            (fourth.restored().from_snapshot, fourth.restored().folded),
            (0, 41)
        );
        assert!(fourth.restored().note.is_some());
        let _ = std::fs::remove_dir_all(&f.dir);
    }

    #[test]
    fn only_loopback_addresses_are_taken() {
        assert!(loopback("127.0.0.1:8742").is_some());
        assert!(loopback("[::1]:8742").is_some());
        assert!(loopback("0.0.0.0:8742").is_none());
        assert!(loopback("192.168.1.2:8742").is_none());
    }
}
