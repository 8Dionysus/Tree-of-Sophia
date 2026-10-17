//! Optional Rust-only cache for deterministic source-processing handlers.
//!
//! This database stores disposable pure outputs and exact execution-DAG
//! metadata. It grants no source, review, rights, canon, publication, or
//! runtime authority. Its `tos_rust_processing_*` namespace deliberately
//! ignores the retired Python scheduler tables in older cache files.

use crate::{Error, Result};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    io::{self, Write},
    path::Path,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use tos_foundation::{Digest256, Digest256Hasher};

pub const DEFAULT_CACHE_BYTES: u64 = 1024 * 1024 * 1024;
pub const DEFAULT_CACHE_ENTRIES: u64 = 500_000;

const CACHE_KEY_DOMAIN: &[u8] = b"tos-rust-processing-cache-key-v1\0";
const MAX_CANONICAL_VALUE_BYTES: usize = 8 * 1024 * 1024;
static RUN_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Explicit memory and work bounds for the disposable Rust processing cache.
#[derive(Clone, Copy, Debug)]
pub struct ProcessingCacheLimits {
    pub max_payload_bytes: u64,
    pub max_entries: u64,
    pub max_identifier_bytes: usize,
    pub max_tasks_per_run: usize,
    pub max_input_bytes: usize,
    pub max_output_bytes: usize,
    /// Aggregate retained JSON state held by a live run's result map.
    pub max_retained_state_bytes: usize,
    /// Aggregate queued execution receipt metadata before its next flush.
    pub max_pending_metadata_bytes: usize,
    pub max_closure_nodes: usize,
    pub max_closure_edges: usize,
    pub max_closure_result_bytes: usize,
}

impl Default for ProcessingCacheLimits {
    fn default() -> Self {
        Self {
            max_payload_bytes: DEFAULT_CACHE_BYTES,
            max_entries: DEFAULT_CACHE_ENTRIES,
            max_identifier_bytes: 4096,
            max_tasks_per_run: 500_000,
            max_input_bytes: 8 * 1024 * 1024,
            max_output_bytes: 8 * 1024 * 1024,
            max_retained_state_bytes: 512 * 1024 * 1024,
            max_pending_metadata_bytes: 64 * 1024 * 1024,
            max_closure_nodes: 4096,
            max_closure_edges: 16_384,
            max_closure_result_bytes: 1024 * 1024,
        }
    }
}

impl ProcessingCacheLimits {
    fn validate(self) -> Result<()> {
        if self.max_payload_bytes == 0
            || self.max_payload_bytes > i64::MAX as u64
            || self.max_entries == 0
            || self.max_identifier_bytes == 0
            || self.max_identifier_bytes > 4096
            || self.max_tasks_per_run == 0
            || self.max_input_bytes == 0
            || self.max_input_bytes > MAX_CANONICAL_VALUE_BYTES
            || self.max_output_bytes == 0
            || self.max_output_bytes > MAX_CANONICAL_VALUE_BYTES
            || self.max_retained_state_bytes == 0
            || self.max_pending_metadata_bytes == 0
            || self.max_closure_nodes == 0
            || self.max_closure_edges == 0
            || self.max_closure_result_bytes == 0
        {
            return Err(Error::Budget("processing cache limits"));
        }
        Ok(())
    }
}

/// A task's completed output, tied to the exact in-process run that produced it.
#[derive(Clone, Debug)]
pub struct ProcessingValue {
    run_id: String,
    id: String,
    kind: String,
    output_digest: String,
    value: Rc<Value>,
}

impl ProcessingValue {
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn kind(&self) -> &str {
        &self.kind
    }
    pub fn output_digest(&self) -> &str {
        &self.output_digest
    }
    pub fn value(&self) -> &Value {
        self.value.as_ref()
    }
}

struct CacheShared {
    db: RefCell<Connection>,
    limits: ProcessingCacheLimits,
    unreported_evictions: Cell<u64>,
}

/// Explicit owner of the disposable processing cache connection.
pub struct ProcessingCache {
    shared: Rc<CacheShared>,
}

impl ProcessingCache {
    /// Open or create an explicitly selected cache database.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_limits(path, ProcessingCacheLimits::default())
    }

    pub fn open_with_limits(path: impl AsRef<Path>, limits: ProcessingCacheLimits) -> Result<Self> {
        let db = Connection::open(path)?;
        Self::from_connection_with_limits(db, limits)
    }

    /// Create a cache over a caller-provided connection, useful for bounded
    /// test fixtures and explicit embedding by the offline producer.
    pub fn from_connection(db: Connection) -> Result<Self> {
        Self::from_connection_with_limits(db, ProcessingCacheLimits::default())
    }

    pub fn from_connection_with_limits(
        db: Connection,
        limits: ProcessingCacheLimits,
    ) -> Result<Self> {
        limits.validate()?;
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        if db.is_autocommit() == false {
            return Err(Error::Invalid(
                "processing cache initialization needs autocommit",
            ));
        }
        let journal_mode: String = db.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
        if journal_mode != "wal" && journal_mode != "memory" {
            return Err(Error::Invalid("processing cache requires WAL journal mode"));
        }
        immediate_transaction(&db, || {
            db.execute_batch(
                "CREATE TABLE IF NOT EXISTS tos_rust_processing_runs (
                id TEXT PRIMARY KEY, status TEXT NOT NULL, error_kind TEXT
             );
             CREATE TABLE IF NOT EXISTS tos_rust_processing_tasks (
                run_id TEXT NOT NULL, id TEXT NOT NULL, kind TEXT NOT NULL,
                cache_key TEXT, output_digest TEXT, status TEXT NOT NULL,
                PRIMARY KEY(run_id,id)
             );
             CREATE TABLE IF NOT EXISTS tos_rust_processing_dependencies (
                run_id TEXT NOT NULL, task_id TEXT NOT NULL, dependency_id TEXT NOT NULL,
                PRIMARY KEY(run_id,task_id,dependency_id)
             );
             CREATE INDEX IF NOT EXISTS tos_rust_processing_dependencies_reverse
                ON tos_rust_processing_dependencies(run_id,dependency_id,task_id);
             CREATE TABLE IF NOT EXISTS tos_rust_processing_outputs (
                cache_key TEXT PRIMARY KEY, payload BLOB NOT NULL,
                payload_digest TEXT NOT NULL, bytes INTEGER NOT NULL, used INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS tos_rust_processing_outputs_age
                ON tos_rust_processing_outputs(used,cache_key);
             CREATE TABLE IF NOT EXISTS tos_rust_processing_output_usage (
                singleton INTEGER PRIMARY KEY CHECK(singleton=1),
                bytes INTEGER NOT NULL CHECK(bytes>=0),
                entries INTEGER NOT NULL CHECK(entries>=0)
             );
             CREATE TABLE IF NOT EXISTS tos_rust_processing_meta (
                key TEXT PRIMARY KEY, value TEXT NOT NULL
             );
             CREATE TRIGGER IF NOT EXISTS tos_rust_processing_output_usage_insert
                AFTER INSERT ON tos_rust_processing_outputs BEGIN
                    UPDATE tos_rust_processing_output_usage
                    SET bytes=bytes+NEW.bytes,entries=entries+1 WHERE singleton=1;
                END;
             CREATE TRIGGER IF NOT EXISTS tos_rust_processing_output_usage_delete
                AFTER DELETE ON tos_rust_processing_outputs BEGIN
                    UPDATE tos_rust_processing_output_usage
                    SET bytes=bytes-OLD.bytes,entries=entries-1 WHERE singleton=1;
                END;
             CREATE TRIGGER IF NOT EXISTS tos_rust_processing_output_usage_update
                AFTER UPDATE OF bytes ON tos_rust_processing_outputs BEGIN
                    UPDATE tos_rust_processing_output_usage
                    SET bytes=bytes-OLD.bytes+NEW.bytes WHERE singleton=1;
                END;
             CREATE TABLE IF NOT EXISTS tos_rust_processing_publication (
                singleton INTEGER PRIMARY KEY CHECK(singleton=1), run_id TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS tos_rust_processing_run_bases (
                run_id TEXT PRIMARY KEY, base_run_id TEXT
             );",
            )?;
            Ok(())
        })?;
        let usage_ready: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM tos_rust_processing_meta WHERE key='output-usage-v1')",
            [],
            |row| row.get(0),
        )?;
        if !usage_ready {
            immediate_transaction(&db, || {
                let became_ready: bool = db.query_row(
                    "SELECT EXISTS(SELECT 1 FROM tos_rust_processing_meta WHERE key='output-usage-v1')",
                    [],
                    |row| row.get(0),
                )?;
                if became_ready {
                    return Ok(());
                }
                let (bytes, entries): (i64, i64) = db.query_row(
                    "SELECT coalesce(sum(bytes),0),count(*) FROM tos_rust_processing_outputs",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                db.execute(
                    "INSERT INTO tos_rust_processing_output_usage(singleton,bytes,entries) VALUES (1,?1,?2)",
                    params![bytes, entries],
                )?;
                db.execute(
                    "INSERT INTO tos_rust_processing_meta(key,value) VALUES ('output-usage-v1','ready')",
                    [],
                )?;
                Ok(())
            })?;
        }
        // Never infer that another process's run is abandoned merely because
        // this connection opened. Incomplete runs remain non-readable until
        // explicit recovery under externally exclusive cache use.
        let evicted_on_open = immediate_transaction(&db, || trim_outputs(&db, limits))?;
        Ok(Self {
            shared: Rc::new(CacheShared {
                db: RefCell::new(db),
                limits,
                unreported_evictions: Cell::new(evicted_on_open),
            }),
        })
    }

    pub fn begin_run(&self) -> Result<ProcessingRun> {
        let db = self.shared.db.borrow();
        let (baseline, run_id) = immediate_transaction(&db, || {
            let baseline: Option<String> = db
                .query_row(
                    "SELECT run_id FROM tos_rust_processing_publication WHERE singleton=1",
                    [],
                    |row| row.get(0),
                )
                .optional()?;
            let run_id = new_run_id();
            db.execute(
                "INSERT INTO tos_rust_processing_runs(id,status,error_kind) VALUES (?1,'running',NULL)",
                [&run_id],
            )?;
            db.execute(
                "INSERT INTO tos_rust_processing_run_bases(run_id,base_run_id) VALUES (?1,?2)",
                params![run_id, baseline],
            )?;
            Ok((baseline, run_id))
        })?;
        drop(db);
        Ok(ProcessingRun {
            handle: ProcessingRunHandle(Rc::new(RefCell::new(ProcessingRunInner {
                shared: Rc::clone(&self.shared),
                run_id,
                baseline,
                definitions: HashMap::new(),
                results: HashMap::new(),
                latest_outputs: HashMap::new(),
                failed: false,
                finished: false,
                executed: 0,
                reused: 0,
                evicted: self.shared.unreported_evictions.replace(0),
                oversized: 0,
                retained_state_bytes: 0,
                pending_metadata_bytes: 0,
                pending: Vec::new(),
                pending_touches: BTreeSet::new(),
                by_kind: BTreeMap::new(),
            }))),
            finalized: false,
        })
    }

    pub fn active_run_id(&self) -> Result<Option<String>> {
        self.shared
            .db
            .borrow()
            .query_row(
                "SELECT run_id FROM tos_rust_processing_publication WHERE singleton=1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(Error::from)
    }
}

/// Mutable run guard. Dropping it unfinished records interruption and cannot
/// advance the active dependency snapshot.
pub struct ProcessingRun {
    handle: ProcessingRunHandle,
    finalized: bool,
}

impl ProcessingRun {
    pub fn handle(&self) -> ProcessingRunHandle {
        self.handle.clone()
    }
    pub fn id(&self) -> String {
        self.handle.0.borrow().run_id.clone()
    }
    pub fn input(&mut self, id: &str, value: &Value) -> Result<ProcessingValue> {
        self.handle.input(id, value)
    }
    pub fn evaluate_task(
        &mut self,
        id: &str,
        kind: &str,
        handler: &str,
        handler_fingerprint: Digest256,
        parameters: &Value,
        dependencies: &[ProcessingValue],
        action: impl FnOnce() -> Result<Value>,
    ) -> Result<ProcessingValue> {
        self.handle.evaluate_task(
            id,
            kind,
            handler,
            handler_fingerprint,
            parameters,
            dependencies,
            action,
        )
    }
    pub fn finish(&mut self) -> Result<()> {
        self.handle.finish()?;
        self.finalized = true;
        Ok(())
    }
    pub fn fail(&mut self, error_kind: &str) -> Result<()> {
        self.handle.fail(error_kind)?;
        self.finalized = true;
        Ok(())
    }
    pub fn report(&self) -> Result<ProcessingRunReport> {
        self.handle.report()
    }
}

impl Drop for ProcessingRun {
    fn drop(&mut self) {
        if !self.finalized {
            let _ = self.handle.fail("dropped-before-finish");
        }
    }
}

/// Cloneable local handle for attaching one explicit run to synchronous Rust
/// normalizers. It does not open connections or escape the owning cache.
#[derive(Clone)]
pub struct ProcessingRunHandle(Rc<RefCell<ProcessingRunInner>>);

impl ProcessingRunHandle {
    pub fn id(&self) -> String {
        self.0.borrow().run_id.clone()
    }

    pub fn input(&self, id: &str, value: &Value) -> Result<ProcessingValue> {
        self.0.borrow_mut().input(id, value)
    }

    pub fn evaluate_task(
        &self,
        id: &str,
        kind: &str,
        handler: &str,
        handler_fingerprint: Digest256,
        parameters: &Value,
        dependencies: &[ProcessingValue],
        action: impl FnOnce() -> Result<Value>,
    ) -> Result<ProcessingValue> {
        self.0.borrow_mut().evaluate_task(
            id,
            kind,
            handler,
            handler_fingerprint,
            parameters,
            dependencies,
            action,
        )
    }

    pub fn report(&self) -> Result<ProcessingRunReport> {
        self.0.borrow_mut().report()
    }

    /// Return a value already evaluated in this run, so downstream owner
    /// adapters can bind an exact producer edge without rebuilding a map.
    pub fn result(&self, id: &str) -> Option<ProcessingValue> {
        self.0.borrow().results.get(id).cloned()
    }

    /// Return the most recent recognized producer output for a node or
    /// relation identity. Arbitrary input/task JSON `id` fields are not
    /// treated as owner identities.
    pub fn latest_output(&self, namespace: &str, stable_id: &str) -> Option<ProcessingValue> {
        let run = self.0.borrow();
        if !matches!(namespace, "node" | "relation")
            || stable_id.is_empty()
            || stable_id.len() > run.shared.limits.max_identifier_bytes
            || stable_id.contains('\0')
        {
            return None;
        }
        let result_id = run
            .latest_outputs
            .get(&(namespace.to_owned(), stable_id.to_owned()))?;
        run.results.get(result_id).cloned()
    }

    fn finish(&self) -> Result<()> {
        self.0.borrow_mut().finish()
    }
    fn fail(&self, error_kind: &str) -> Result<()> {
        self.0.borrow_mut().fail(error_kind)
    }
}

struct ProcessingRunInner {
    shared: Rc<CacheShared>,
    run_id: String,
    baseline: Option<String>,
    definitions: HashMap<String, String>,
    results: HashMap<String, ProcessingValue>,
    latest_outputs: HashMap<(String, String), String>,
    failed: bool,
    finished: bool,
    executed: u64,
    reused: u64,
    evicted: u64,
    oversized: u64,
    retained_state_bytes: usize,
    pending_metadata_bytes: usize,
    pending: Vec<PendingTask>,
    pending_touches: BTreeSet<String>,
    by_kind: BTreeMap<String, (u64, u64)>,
}

struct PendingTask {
    id: String,
    category: &'static str,
    cache_key: Option<String>,
    output_digest: Option<String>,
    status: &'static str,
    dependencies: Vec<String>,
}

impl ProcessingRunInner {
    fn active(&self) -> Result<()> {
        if self.finished {
            return Err(Error::Invalid("processing run already finalized"));
        }
        if self.failed {
            return Err(Error::Invalid("processing run contains failed work"));
        }
        Ok(())
    }

    fn queue_pending(&mut self, task: PendingTask) -> Result<()> {
        let mut bytes = task
            .id
            .capacity()
            .checked_add(task.cache_key.as_ref().map_or(0, String::capacity))
            .and_then(|n| n.checked_add(task.output_digest.as_ref().map_or(0, String::capacity)))
            .and_then(|n| {
                n.checked_add(task.dependencies.capacity() * std::mem::size_of::<String>())
            })
            .ok_or(Error::Budget("processing pending metadata"))?;
        for dependency in &task.dependencies {
            bytes = bytes
                .checked_add(dependency.capacity())
                .ok_or(Error::Budget("processing pending metadata"))?;
        }
        let next = self
            .pending_metadata_bytes
            .checked_add(bytes)
            .ok_or(Error::Budget("processing pending metadata"))?;
        if next > self.shared.limits.max_pending_metadata_bytes {
            return self.poison(
                "pending-metadata-budget",
                Error::Budget("processing pending metadata"),
            );
        }
        self.pending_metadata_bytes = next;
        self.pending.push(task);
        Ok(())
    }

    fn queue_touch(&mut self, cache_key: &str) -> Result<()> {
        if self.pending_touches.contains(cache_key) {
            return Ok(());
        }
        let owned = cache_key.to_owned();
        let bytes = owned
            .capacity()
            .checked_add(std::mem::size_of::<String>())
            .ok_or(Error::Budget("processing pending metadata"))?;
        let next = self
            .pending_metadata_bytes
            .checked_add(bytes)
            .ok_or(Error::Budget("processing pending metadata"))?;
        if next > self.shared.limits.max_pending_metadata_bytes {
            return self.poison(
                "pending-metadata-budget",
                Error::Budget("processing pending metadata"),
            );
        }
        self.pending_metadata_bytes = next;
        self.pending_touches.insert(owned);
        Ok(())
    }

    fn persist_pending(&self, db: &Connection) -> Result<()> {
        {
            let mut tasks = db.prepare(
                "INSERT INTO tos_rust_processing_tasks(run_id,id,kind,cache_key,output_digest,status) VALUES (?1,?2,?3,?4,?5,?6)",
            )?;
            let mut edges = db.prepare(
                "INSERT INTO tos_rust_processing_dependencies(run_id,task_id,dependency_id) VALUES (?1,?2,?3)",
            )?;
            for task in &self.pending {
                tasks.execute(params![
                    self.run_id,
                    task.id,
                    task.category,
                    task.cache_key,
                    task.output_digest,
                    task.status
                ])?;
                for dependency in &task.dependencies {
                    edges.execute(params![self.run_id, task.id, dependency])?;
                }
            }
        }
        for cache_key in &self.pending_touches {
            db.execute(
                "UPDATE tos_rust_processing_outputs SET used=?1 WHERE cache_key=?2",
                params![next_used(), cache_key],
            )?;
        }
        Ok(())
    }

    fn clear_pending(&mut self) {
        self.pending.clear();
        self.pending_touches.clear();
        self.pending_metadata_bytes = 0;
    }

    fn flush_pending(&mut self) -> Result<()> {
        if self.pending.is_empty() && self.pending_touches.is_empty() {
            return Ok(());
        }
        let shared = Rc::clone(&self.shared);
        {
            let db = shared.db.borrow();
            immediate_transaction(&db, || self.persist_pending(&db))?;
        }
        self.clear_pending();
        Ok(())
    }

    fn input(&mut self, id: &str, value: &Value) -> Result<ProcessingValue> {
        self.active()?;
        if let Err(error) = validate_identifier(id, self.shared.limits.max_identifier_bytes) {
            return self.poison("invalid-input-identity", error);
        }
        if self.results.len() >= self.shared.limits.max_tasks_per_run
            && !self.results.contains_key(id)
        {
            self.failed = true;
            self.fail("task-limit")?;
            return Err(Error::Budget("processing tasks per run"));
        }
        let payload = match canonical_value(value, self.shared.limits.max_input_bytes) {
            Ok(payload) => payload,
            Err(error) => return self.poison("input-budget", error),
        };
        let digest = Digest256::of_bytes(&payload).to_hex();
        let definition = format!("input:{digest}");
        if let Some(previous) = self.definitions.get(id) {
            if previous != &definition {
                self.failed = true;
                self.fail("conflicting-input")?;
                return Err(Error::Invalid("conflicting processing input identity"));
            }
            return self
                .results
                .get(id)
                .cloned()
                .ok_or(Error::Invalid("processing input result absent"));
        }
        let result = ProcessingValue {
            run_id: self.run_id.clone(),
            id: id.to_owned(),
            kind: "input".into(),
            output_digest: digest.clone(),
            value: Rc::new(value.clone()),
        };
        self.retain_result(result.clone(), &definition, None)?;
        self.queue_pending(PendingTask {
            id: id.to_owned(),
            category: "input",
            cache_key: None,
            output_digest: Some(digest),
            status: "complete",
            dependencies: Vec::new(),
        })?;
        self.definitions.insert(id.to_owned(), definition);
        Ok(result)
    }

    fn evaluate_task(
        &mut self,
        id: &str,
        kind: &str,
        handler: &str,
        handler_fingerprint: Digest256,
        parameters: &Value,
        dependencies: &[ProcessingValue],
        action: impl FnOnce() -> Result<Value>,
    ) -> Result<ProcessingValue> {
        self.active()?;
        for identity in [id, kind, handler] {
            if let Err(error) =
                validate_identifier(identity, self.shared.limits.max_identifier_bytes)
            {
                return self.poison("invalid-task-identity", error);
            }
        }
        if self.results.len() >= self.shared.limits.max_tasks_per_run
            && !self.results.contains_key(id)
        {
            self.failed = true;
            self.fail("task-limit")?;
            return Err(Error::Budget("processing tasks per run"));
        }
        if dependencies
            .iter()
            .any(|dependency| dependency.run_id != self.run_id)
        {
            return self.poison(
                "foreign-dependency",
                Error::Invalid("processing dependency belongs to another run"),
            );
        }
        if dependencies.len() > self.shared.limits.max_tasks_per_run {
            return self.poison(
                "dependency-limit",
                Error::Budget("processing dependencies per task"),
            );
        }
        let mut dependency_bytes = 0usize;
        for dependency in dependencies {
            dependency_bytes = match dependency_bytes
                .checked_add(dependency.id.len())
                .and_then(|bytes| bytes.checked_add(dependency.output_digest.len() + 8))
            {
                Some(bytes) => bytes,
                None => {
                    return self.poison(
                        "dependency-budget",
                        Error::Budget("processing dependency key"),
                    );
                }
            };
            if dependency_bytes > self.shared.limits.max_input_bytes {
                return self.poison(
                    "dependency-budget",
                    Error::Budget("processing dependency key"),
                );
            }
        }
        let parameters_bytes = match canonical_value(parameters, self.shared.limits.max_input_bytes)
        {
            Ok(bytes) => bytes,
            Err(error) => return self.poison("task-parameter-budget", error),
        };
        let dependency_packet = dependencies
            .iter()
            .map(|dependency| json!([dependency.id, dependency.output_digest]))
            .collect::<Vec<_>>();
        let key_value = json!({
            "domain":"tos-rust-processing-cache-key-v1",
            "id":id,
            "kind":kind,
            "handler":handler,
            "handler_fingerprint":handler_fingerprint.to_hex(),
            "parameters":match decode_value(&parameters_bytes) {
                Ok(value) => value,
                Err(error) => return self.poison("invalid-task-parameters", error),
            },
            "dependencies":dependency_packet,
        });
        let key_bytes = match canonical_value(&key_value, self.shared.limits.max_input_bytes) {
            Ok(bytes) => bytes,
            Err(error) => return self.poison("task-key-budget", error),
        };
        let cache_key = Digest256::of_bytes(&key_bytes).to_hex();
        let definition = format!("{kind}:{cache_key}");
        if let Some(previous) = self.definitions.get(id) {
            if previous != &definition {
                self.failed = true;
                self.fail("conflicting-task")?;
                return Err(Error::Invalid("conflicting processing task identity"));
            }
            return self
                .results
                .get(id)
                .cloned()
                .ok_or(Error::Invalid("processing task result absent"));
        }
        let mut unique_dependencies = BTreeSet::new();
        let dependency_ids = dependencies
            .iter()
            .filter(|dependency| unique_dependencies.insert(dependency.id.as_str()))
            .map(|dependency| dependency.id.clone())
            .collect::<Vec<_>>();
        let cached = {
            let db = self.shared.db.borrow();
            read_output(&db, &cache_key, self.shared.limits.max_output_bytes)
        };
        let cached = match cached {
            Ok(value) => value,
            Err(error) => return self.poison("cache-read", error),
        };
        if let Some((value, output_digest)) = cached {
            self.queue_touch(&cache_key)?;
            self.queue_pending(PendingTask {
                id: id.to_owned(),
                category: "task",
                cache_key: Some(cache_key),
                output_digest: Some(output_digest.clone()),
                status: "complete",
                dependencies: dependency_ids,
            })?;
            self.reused += 1;
            self.by_kind.entry(kind.to_owned()).or_default().1 += 1;
            let result = ProcessingValue {
                run_id: self.run_id.clone(),
                id: id.to_owned(),
                kind: "task".to_owned(),
                output_digest,
                value: Rc::new(value),
            };
            self.retain_result(result.clone(), &definition, Some(kind))?;
            self.definitions.insert(id.to_owned(), definition);
            return Ok(result);
        }

        self.queue_pending(PendingTask {
            id: id.to_owned(),
            category: "task",
            cache_key: Some(cache_key.clone()),
            output_digest: None,
            status: "running",
            dependencies: dependency_ids,
        })?;
        if let Err(error) = self.flush_pending() {
            return self.poison("receipt-flush", error);
        }
        let value = match action() {
            Ok(value) => value,
            Err(error) => return self.poison("handler-error", error),
        };
        let payload = match ordered_value(&value, self.shared.limits.max_output_bytes) {
            Ok(payload) => payload,
            Err(error) => return self.poison("output-budget", error),
        };
        let canonical = match canonical_value(&value, self.shared.limits.max_output_bytes) {
            Ok(canonical) => canonical,
            Err(error) => return self.poison("output-budget", error),
        };
        let digest = Digest256::of_bytes(&canonical).to_hex();
        let payload_digest = Digest256::of_bytes(&payload).to_hex();
        let stored = {
            let db = self.shared.db.borrow();
            immediate_transaction(&db, || {
                let stored = write_output(
                    &db,
                    &cache_key,
                    &payload,
                    &payload_digest,
                    self.shared.limits,
                )?;
                self.evicted += trim_outputs(&db, self.shared.limits)?;
                let updated = db.execute(
                    "UPDATE tos_rust_processing_tasks SET status='complete',output_digest=?1 WHERE run_id=?2 AND id=?3 AND status='running'",
                    params![digest, self.run_id, id],
                )?;
                if updated != 1 {
                    return Err(Error::Invalid("processing task completion receipt absent"));
                }
                Ok(stored)
            })
        };
        let stored = match stored {
            Ok(stored) => stored,
            Err(error) => return self.poison("cache-write", error),
        };
        if !stored {
            self.oversized += 1;
        }
        self.executed += 1;
        self.by_kind.entry(kind.to_owned()).or_default().0 += 1;
        let result = ProcessingValue {
            run_id: self.run_id.clone(),
            id: id.to_owned(),
            kind: "task".to_owned(),
            output_digest: digest,
            value: Rc::new(value),
        };
        self.retain_result(result.clone(), &definition, Some(kind))?;
        self.definitions.insert(id.to_owned(), definition);
        Ok(result)
    }

    fn retain_result(
        &mut self,
        result: ProcessingValue,
        definition: &str,
        producer_kind: Option<&str>,
    ) -> Result<()> {
        let json_bytes =
            match crate::knowledge_normalization::serde_retained_heap_upper(result.value(), 0) {
                Ok(bytes) => bytes,
                Err(error) => return self.poison("retained-state-budget", error),
            };
        let metadata_bytes = std::mem::size_of::<ProcessingValue>()
            .checked_add(result.run_id.capacity())
            .and_then(|size| size.checked_add(result.id.capacity()))
            .and_then(|size| size.checked_add(result.kind.capacity()))
            .and_then(|size| size.checked_add(result.output_digest.capacity()));
        let output_namespace = match producer_kind {
            Some(
                "node"
                | "claim-node"
                | "claim-finalization"
                | "literal-claim-contribution"
                | "final-node"
                | "readable-node",
            ) => Some("node"),
            Some("relation" | "claim-relation" | "final-relation" | "readable-relation") => {
                Some("relation")
            }
            _ => None,
        };
        let output_identity = output_namespace.and_then(|namespace| {
            result
                .value()
                .get("id")
                .and_then(Value::as_str)
                .filter(|identity| {
                    !identity.is_empty()
                        && identity.len() <= self.shared.limits.max_identifier_bytes
                        && !identity.contains('\0')
                })
                .map(|identity| (namespace.to_owned(), identity.to_owned()))
        });
        let output_index_bytes = output_identity.as_ref().map_or(0, |(namespace, identity)| {
            std::mem::size_of::<(String, String, String)>()
                + namespace.capacity()
                + identity.capacity()
                + result.id.capacity()
        });
        let retained = metadata_bytes.and_then(|metadata_bytes| {
            json_bytes
                .checked_add(std::mem::size_of::<Rc<Value>>())
                .and_then(|size| size.checked_add(metadata_bytes.saturating_mul(2)))
                .and_then(|size| size.checked_add(output_index_bytes))
                .and_then(|size| {
                    size.checked_add(
                        std::mem::size_of::<(String, ProcessingValue)>() + result.id.capacity(),
                    )
                })
                .and_then(|size| {
                    size.checked_add(std::mem::size_of::<(String, String)>() + definition.len())
                })
        });
        let retained = match retained {
            Some(bytes) => bytes,
            None => {
                return self.poison(
                    "retained-state-budget",
                    Error::Budget("processing retained state"),
                );
            }
        };
        let next = self.retained_state_bytes.checked_add(retained);
        let next = match next {
            Some(bytes) => bytes,
            None => {
                return self.poison(
                    "retained-state-budget",
                    Error::Budget("processing retained state"),
                );
            }
        };
        if next > self.shared.limits.max_retained_state_bytes {
            return self.poison(
                "retained-state-budget",
                Error::Budget("processing retained state"),
            );
        }
        self.retained_state_bytes = next;
        if let Some(identity) = output_identity {
            self.latest_outputs.insert(identity, result.id.clone());
        }
        self.results.insert(result.id.clone(), result);
        Ok(())
    }

    fn poison<T>(&mut self, error_kind: &str, error: Error) -> Result<T> {
        self.failed = true;
        self.fail(error_kind)?;
        Err(error)
    }

    fn finish(&mut self) -> Result<()> {
        if self.finished {
            return Err(Error::Invalid("processing run already finalized"));
        }
        if self.failed {
            self.fail("failed-run")?;
            self.finished = true;
            return Err(Error::Invalid("processing graph contains failed work"));
        }
        if self.definitions.is_empty() {
            let db = self.shared.db.borrow();
            immediate_transaction(&db, || {
                db.execute(
                    "UPDATE tos_rust_processing_runs SET status='skipped' WHERE id=?1",
                    [&self.run_id],
                )?;
                Ok(())
            })?;
            self.finished = true;
            return Ok(());
        }
        let shared = Rc::clone(&self.shared);
        let superseded = {
            let db = shared.db.borrow();
            immediate_transaction(&db, || {
                self.persist_pending(&db)?;
                let current: Option<String> = db
                    .query_row(
                        "SELECT run_id FROM tos_rust_processing_publication WHERE singleton=1",
                        [],
                        |row| row.get(0),
                    )
                    .optional()?;
                if current != self.baseline {
                    db.execute(
                        "UPDATE tos_rust_processing_runs SET status='superseded',error_kind='baseline-changed' WHERE id=?1",
                        [&self.run_id],
                    )?;
                    return Ok(true);
                }
                db.execute(
                    "UPDATE tos_rust_processing_runs SET status='complete',error_kind=NULL WHERE id=?1",
                    [&self.run_id],
                )?;
                db.execute(
                    "INSERT INTO tos_rust_processing_publication(singleton,run_id) VALUES (1,?1) ON CONFLICT(singleton) DO UPDATE SET run_id=excluded.run_id",
                    [&self.run_id],
                )?;
                Ok(false)
            })?
        };
        self.clear_pending();
        if superseded {
            self.finished = true;
            return Err(Error::Invalid("processing baseline changed; retry run"));
        }
        self.finished = true;
        Ok(())
    }

    fn fail(&mut self, error_kind: &str) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        let safe_kind = if error_kind.is_empty() || error_kind.len() > 128 {
            "processing-error"
        } else {
            error_kind
        };
        let shared = Rc::clone(&self.shared);
        {
            let db = shared.db.borrow();
            immediate_transaction(&db, || {
                self.persist_pending(&db)?;
                db.execute(
                    "UPDATE tos_rust_processing_runs SET status='failed',error_kind=?1 WHERE id=?2 AND status='running'",
                    params![safe_kind, self.run_id],
                )?;
                db.execute(
                    "UPDATE tos_rust_processing_tasks SET status='interrupted' WHERE run_id=?1 AND status='running'",
                    [&self.run_id],
                )?;
                Ok(())
            })?;
        }
        self.clear_pending();
        self.finished = true;
        Ok(())
    }

    fn report(&mut self) -> Result<ProcessingRunReport> {
        self.flush_pending()?;
        let db = self.shared.db.borrow();
        let status: String = db.query_row(
            "SELECT status FROM tos_rust_processing_runs WHERE id=?1",
            [&self.run_id],
            |row| row.get(0),
        )?;
        let (payload_bytes, entries): (u64, u64) = db.query_row(
            "SELECT coalesce(sum(bytes),0),count(*) FROM tos_rust_processing_outputs",
            [],
            |row| Ok((row.get::<_, i64>(0)? as u64, row.get::<_, i64>(1)? as u64)),
        )?;
        let input_changes =
            input_change_counts(&db, &self.run_id, self.baseline.as_deref(), &status)?;
        let removed_task_ids = if status == "complete" {
            let base = self.baseline.as_deref().unwrap_or("");
            let mut current_statement =
                db.prepare("SELECT id FROM tos_rust_processing_tasks WHERE run_id=?1")?;
            let current = current_statement
                .query_map([&self.run_id], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<BTreeSet<_>, _>>()?;
            let mut base_statement =
                db.prepare("SELECT id FROM tos_rust_processing_tasks WHERE run_id=?1")?;
            base_statement
                .query_map([base], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?
                .into_iter()
                .filter(|id| !current.contains(id))
                .collect()
        } else {
            Vec::new()
        };
        let input_coverage = if status == "complete" {
            "complete"
        } else if self.results.is_empty() {
            "not-run"
        } else {
            "partial"
        };
        Ok(ProcessingRunReport {
            run_id: self.run_id.clone(),
            status,
            executed: self.executed,
            reused: self.reused,
            input_changes,
            input_coverage,
            steps_by_kind: self.by_kind.clone(),
            cache_payload_bytes: payload_bytes,
            cache_entries: entries,
            max_payload_bytes: self.shared.limits.max_payload_bytes,
            max_entries: self.shared.limits.max_entries,
            evicted_outputs: self.evicted,
            uncached_oversized_outputs: self.oversized,
            removed_task_ids,
            is_semantic_acceptance: false,
        })
    }
}

/// Summary of one run; input deltas remain separately available in bounded pages.
#[derive(Clone, Debug)]
pub struct ProcessingRunReport {
    pub run_id: String,
    pub status: String,
    pub executed: u64,
    pub reused: u64,
    pub input_changes: BTreeMap<String, u64>,
    pub input_coverage: &'static str,
    pub steps_by_kind: BTreeMap<String, (u64, u64)>,
    pub cache_payload_bytes: u64,
    pub cache_entries: u64,
    pub max_payload_bytes: u64,
    pub max_entries: u64,
    pub evicted_outputs: u64,
    pub uncached_oversized_outputs: u64,
    pub removed_task_ids: Vec<String>,
    pub is_semantic_acceptance: bool,
}

fn input_change_counts(
    db: &Connection,
    run_id: &str,
    base_run_id: Option<&str>,
    status: &str,
) -> Result<BTreeMap<String, u64>> {
    let mut counts = BTreeMap::from([
        ("added".to_owned(), 0),
        ("changed".to_owned(), 0),
        ("removed".to_owned(), 0),
    ]);
    let base = base_run_id.unwrap_or("");
    let mut statement = db.prepare(
        "WITH current AS (SELECT id,output_digest FROM tos_rust_processing_tasks WHERE run_id=?1 AND kind='input'), \
         previous AS (SELECT id,output_digest FROM tos_rust_processing_tasks WHERE run_id=?2 AND kind='input') \
         SELECT CASE WHEN p.id IS NULL THEN 'added' ELSE 'changed' END \
           FROM current c LEFT JOIN previous p USING(id) \
          WHERE p.id IS NULL OR p.output_digest!=c.output_digest \
         UNION ALL SELECT 'removed' FROM previous p LEFT JOIN current c USING(id) \
          WHERE c.id IS NULL AND ?3='complete'",
    )?;
    let rows = statement.query_map(params![run_id, base, status], |row| row.get::<_, String>(0))?;
    for row in rows {
        let change = row?;
        if let Some(count) = counts.get_mut(&change) {
            *count += 1;
        }
    }
    Ok(counts)
}

/// Read-only bounded result for the exact retained completed run.
#[derive(Clone, Copy, Debug)]
pub struct ClosureLimits {
    pub max_nodes: usize,
    pub max_edges: usize,
    pub max_identifier_bytes: usize,
    pub max_result_bytes: usize,
}

impl Default for ClosureLimits {
    fn default() -> Self {
        Self {
            max_nodes: 4096,
            max_edges: 16_384,
            max_identifier_bytes: 4096,
            max_result_bytes: 1024 * 1024,
        }
    }
}

impl ClosureLimits {
    fn validate(self) -> Result<()> {
        if self.max_nodes == 0
            || self.max_edges == 0
            || self.max_identifier_bytes == 0
            || self.max_identifier_bytes > 4096
            || self.max_result_bytes == 0
            || self.max_result_bytes > 16 * 1024 * 1024
            || self.max_nodes > 16_384
            || self.max_edges > 65_536
        {
            return Err(Error::Budget("processing closure limits"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct ProcessingDependencyNode {
    pub id: String,
    pub kind: String,
    pub output_digest: String,
}

#[derive(Clone, Debug)]
pub struct ProcessingDependencyEdge {
    pub dependency_id: String,
    pub task_id: String,
}

#[derive(Clone, Debug)]
pub struct ProcessingDependencyClosure {
    pub run_id: String,
    pub changed_ids: Vec<String>,
    pub nodes: Vec<ProcessingDependencyNode>,
    pub edges: Vec<ProcessingDependencyEdge>,
    pub coverage: &'static str,
    pub is_semantic_acceptance: bool,
    pub is_source_change_completeness: bool,
}

/// Exact immutable SQLite read transaction. Methods only inspect retained
/// metadata and fail closed when the requested run was retired or incomplete.
pub struct ProcessingReadSnapshot<'a> {
    db: &'a Connection,
    limits: ProcessingCacheLimits,
}

impl<'a> ProcessingReadSnapshot<'a> {
    /// Borrow a caller-managed SQLite read transaction. This constructor does
    /// not initialize schema or change transaction state.
    pub fn new(db: &'a Connection, limits: ProcessingCacheLimits) -> Result<Self> {
        limits.validate()?;
        if db.is_autocommit() {
            return Err(Error::Invalid("processing read transaction required"));
        }
        Ok(Self { db, limits })
    }

    pub fn dependency_closure(
        &self,
        run_id: &str,
        changed_ids: &[String],
        limits: ClosureLimits,
    ) -> Result<ProcessingDependencyClosure> {
        limits.validate()?;
        validate_identifier(run_id, limits.max_identifier_bytes)?;
        if changed_ids.is_empty() {
            return Err(Error::Invalid(
                "processing closure needs a changed identity",
            ));
        }
        if changed_ids.len() > limits.max_nodes {
            return Err(Error::Budget("processing closure node budget"));
        }
        let state: Option<String> = self
            .db
            .query_row(
                "SELECT CASE WHEN status='complete' THEN 'complete' END FROM tos_rust_processing_runs WHERE id=?1",
                [run_id],
                |row| row.get(0),
            )
            .optional()?;
        if state.as_deref() != Some("complete") {
            return Err(Error::Invalid("exact completed processing run required"));
        }
        let index_columns = {
            let mut statement = self
                .db
                .prepare("PRAGMA index_info(tos_rust_processing_dependencies_reverse)")?;
            statement
                .query_map([], |row| row.get::<_, String>(2))?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        if index_columns.iter().map(String::as_str).collect::<Vec<_>>()
            != ["run_id", "dependency_id", "task_id"]
        {
            return Err(Error::Invalid("processing reverse index missing"));
        }
        let mut seeds = BTreeSet::new();
        let mut seed_bytes = 0usize;
        for id in changed_ids {
            validate_identifier(id, limits.max_identifier_bytes)?;
            seed_bytes = seed_bytes
                .checked_add(id.len() + 3)
                .ok_or(Error::Budget("processing closure result bytes"))?;
            if seed_bytes > limits.max_result_bytes {
                return Err(Error::Budget("processing closure result bytes"));
            }
            if !seeds.insert(id.clone()) {
                return Err(Error::Invalid("duplicate processing seed"));
            }
        }
        let mut packet = ProcessingDependencyClosure {
            run_id: run_id.to_owned(),
            changed_ids: seeds.iter().cloned().collect(),
            nodes: Vec::new(),
            edges: Vec::new(),
            coverage: "complete-retained-dag",
            is_semantic_acceptance: false,
            is_source_change_completeness: false,
        };
        let mut used = closure_encoded_bytes(&packet)?;
        if used > limits.max_result_bytes {
            return Err(Error::Budget("processing closure result bytes"));
        }
        let mut pending = VecDeque::from(packet.changed_ids.clone());
        let mut seen = seeds;
        let mut children: HashMap<String, Vec<String>> = HashMap::new();
        let mut indegrees: HashMap<String, usize> = HashMap::new();
        while let Some(current) = pending.pop_front() {
            let row: Option<(Option<String>, Option<String>, Option<String>)> = self
                .db
                .query_row(
                    "SELECT CASE WHEN length(CAST(kind AS BLOB))<=16 THEN kind END,
                            CASE WHEN length(CAST(output_digest AS BLOB))=64 THEN output_digest END,
                            CASE WHEN status='complete' THEN 'complete' END
                       FROM tos_rust_processing_tasks WHERE run_id=?1 AND id=?2",
                    params![run_id, current],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            let Some((Some(kind), Some(digest), Some(status))) = row else {
                return Err(Error::Invalid(
                    "processing closure seed or dependency absent",
                ));
            };
            if !matches!(kind.as_str(), "input" | "task")
                || status != "complete"
                || !valid_digest(&digest)
            {
                return Err(Error::Invalid("incomplete processing closure node"));
            }
            append_closure_node(
                &mut packet,
                ProcessingDependencyNode {
                    id: current.clone(),
                    kind: kind.clone(),
                    output_digest: digest,
                },
                &mut used,
                limits,
            )?;
            let remaining = limits.max_edges.saturating_sub(packet.edges.len());
            let mut statement = self.db.prepare(
                "SELECT CASE WHEN length(CAST(task_id AS BLOB))<=?4 THEN task_id END \
                   FROM tos_rust_processing_dependencies INDEXED BY tos_rust_processing_dependencies_reverse \
                  WHERE run_id=?1 AND dependency_id=?2 ORDER BY task_id LIMIT ?3",
            )?;
            let rows = statement.query_map(
                params![
                    run_id,
                    current,
                    remaining as i64 + 1,
                    limits.max_identifier_bytes as i64
                ],
                |row| row.get::<_, Option<String>>(0),
            )?;
            for row in rows {
                if packet.edges.len() >= limits.max_edges {
                    return Err(Error::Budget("processing closure edge budget"));
                }
                let consumer =
                    row?.ok_or(Error::Invalid("malformed processing dependency identity"))?;
                if seen.insert(consumer.clone()) {
                    if seen.len() > limits.max_nodes {
                        return Err(Error::Budget("processing closure node budget"));
                    }
                    pending.push_back(consumer.clone());
                }
                append_closure_edge(
                    &mut packet,
                    ProcessingDependencyEdge {
                        dependency_id: current.clone(),
                        task_id: consumer.clone(),
                    },
                    &mut used,
                    limits,
                )?;
                children
                    .entry(current.clone())
                    .or_default()
                    .push(consumer.clone());
                *indegrees.entry(consumer).or_default() += 1;
            }
        }
        let kinds: HashMap<_, _> = packet
            .nodes
            .iter()
            .map(|node| (node.id.clone(), node.kind.clone()))
            .collect();
        if packet
            .nodes
            .iter()
            .any(|node| node.kind == "input" && indegrees.get(&node.id).copied().unwrap_or(0) > 0)
        {
            return Err(Error::Invalid("processing input cannot depend on a task"));
        }
        let consumers = packet
            .edges
            .iter()
            .map(|edge| edge.task_id.as_str())
            .collect::<BTreeSet<_>>();
        if packet
            .nodes
            .iter()
            .any(|node| node.kind == "input" && consumers.contains(node.id.as_str()))
        {
            return Err(Error::Invalid("processing input cannot depend on a task"));
        }
        // Kahn's algorithm detects damaged cycles instead of returning a partial closure.
        let mut indegree_work = indegrees;
        let mut ready: VecDeque<String> = packet
            .nodes
            .iter()
            .filter(|node| !indegree_work.contains_key(&node.id))
            .map(|node| node.id.clone())
            .collect();
        let mut visited = 0usize;
        while let Some(current) = ready.pop_front() {
            visited += 1;
            for consumer in children.get(&current).into_iter().flatten() {
                if let Some(indegree) = indegree_work.get_mut(consumer) {
                    *indegree -= 1;
                    if *indegree == 0 {
                        indegree_work.remove(consumer);
                        ready.push_back(consumer.clone());
                    }
                }
            }
        }
        if visited != packet.nodes.len() || kinds.len() != packet.nodes.len() {
            return Err(Error::Invalid(
                "cyclic or duplicate processing closure nodes",
            ));
        }
        Ok(packet)
    }

    pub fn input_changes(
        &self,
        run_id: &str,
        after: &str,
        limit: usize,
        source_only: bool,
    ) -> Result<ProcessingInputChanges> {
        validate_identifier(run_id, self.limits.max_identifier_bytes)?;
        if after.len() > self.limits.max_identifier_bytes || limit == 0 || limit > 1000 {
            return Err(Error::Invalid("invalid processing input change page"));
        }
        if !after.is_empty() {
            validate_identifier(after, self.limits.max_identifier_bytes)?;
        }
        let run: Option<(Option<String>, Option<String>, bool, bool)> = self
            .db
            .query_row(
                "SELECT CASE WHEN r.status IN ('running','complete','failed','skipped','superseded','interrupted') THEN r.status END, \
                        CASE WHEN b.base_run_id IS NULL OR length(CAST(b.base_run_id AS BLOB))<=?2 THEN b.base_run_id END, \
                        b.base_run_id IS NULL OR length(CAST(b.base_run_id AS BLOB))<=?2, \
                        b.run_id IS NOT NULL \
                   FROM tos_rust_processing_runs r \
                 LEFT JOIN tos_rust_processing_run_bases b ON b.run_id=r.id WHERE r.id=?1",
                params![run_id, self.limits.max_identifier_bytes as i64],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        let Some((Some(status), base_run_id, true, true)) = run else {
            return Err(Error::Invalid(
                "processing run or baseline metadata retired",
            ));
        };
        if let Some(base) = base_run_id.as_deref() {
            let retained: bool = self.db.query_row(
                "SELECT EXISTS(SELECT 1 FROM tos_rust_processing_runs WHERE id=?1)",
                [base],
                |row| row.get(0),
            )?;
            if !retained {
                return Err(Error::Invalid("processing baseline retired"));
            }
        }
        let base = base_run_id.as_deref().unwrap_or("");
        let source_filter = if source_only {
            " AND (id LIKE 'source-%' OR id LIKE 'entity-type:%' OR id LIKE 'relation-type:%')"
        } else {
            ""
        };
        let sql = format!(
            "WITH current AS (SELECT id,output_digest FROM tos_rust_processing_tasks WHERE run_id=?1 AND kind='input'), \
             previous AS (SELECT id,output_digest FROM tos_rust_processing_tasks WHERE run_id=?2 AND kind='input'), \
             changes AS (SELECT c.id,p.output_digest AS before,c.output_digest AS after, \
                 CASE WHEN p.id IS NULL THEN 'added' ELSE 'changed' END AS change \
                 FROM current c LEFT JOIN previous p USING(id) WHERE p.id IS NULL OR p.output_digest!=c.output_digest \
                 UNION ALL SELECT p.id,p.output_digest,NULL,'removed' FROM previous p LEFT JOIN current c USING(id) \
                 WHERE c.id IS NULL AND ?3='complete') \
             SELECT CASE WHEN length(CAST(id AS BLOB))<=?6 THEN id END, \
                    CASE WHEN before IS NULL OR length(CAST(before AS BLOB))=64 THEN before END, \
                    CASE WHEN after IS NULL OR length(CAST(after AS BLOB))=64 THEN after END,change \
               FROM changes WHERE id>?4{source_filter} ORDER BY id LIMIT ?5"
        );
        let mut statement = self.db.prepare(&sql)?;
        let rows = statement
            .query_map(
                params![
                    run_id,
                    base,
                    status,
                    after,
                    limit as i64 + 1,
                    self.limits.max_identifier_bytes as i64
                ],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let rows = rows
            .into_iter()
            .map(|(id, before, after, change)| {
                let id = id.ok_or(Error::Invalid("malformed processing input identity"))?;
                let change = change.ok_or(Error::Invalid("malformed processing input change"))?;
                let digests_valid = match change.as_str() {
                    "added" => before.is_none() && after.as_deref().is_some_and(valid_digest),
                    "changed" => {
                        before.as_deref().is_some_and(valid_digest)
                            && after.as_deref().is_some_and(valid_digest)
                    }
                    "removed" => before.as_deref().is_some_and(valid_digest) && after.is_none(),
                    _ => false,
                };
                if !digests_valid {
                    return Err(Error::Invalid("malformed processing input digest"));
                }
                Ok(ProcessingInputChange {
                    id,
                    before,
                    after,
                    change,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let has_more = rows.len() > limit;
        let items = rows.into_iter().take(limit).collect::<Vec<_>>();
        let next_after = if has_more {
            items.last().map(|item| item.id.clone())
        } else {
            None
        };
        Ok(ProcessingInputChanges {
            run_id: run_id.to_owned(),
            base_run_id,
            coverage: if status == "complete" {
                "complete"
            } else {
                "partial"
            },
            items,
            next_after,
            is_semantic_acceptance: false,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ProcessingInputChange {
    pub id: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub change: String,
}

#[derive(Clone, Debug)]
pub struct ProcessingInputChanges {
    pub run_id: String,
    pub base_run_id: Option<String>,
    pub coverage: &'static str,
    pub items: Vec<ProcessingInputChange>,
    pub next_after: Option<String>,
    pub is_semantic_acceptance: bool,
}

/// Read the bounded reverse dependency closure through a caller-owned active
/// read transaction. No schema or transaction changes occur here.
pub fn processing_dependency_closure(
    db: &Connection,
    run_id: &str,
    changed_ids: &[String],
    limits: ClosureLimits,
) -> Result<ProcessingDependencyClosure> {
    ProcessingReadSnapshot::new(db, ProcessingCacheLimits::default())?.dependency_closure(
        run_id,
        changed_ids,
        limits,
    )
}

/// Read one bounded input-delta page through a caller-owned active read
/// transaction. No schema or transaction changes occur here.
pub fn processing_input_changes(
    db: &Connection,
    run_id: &str,
    after: &str,
    limit: usize,
    source_only: bool,
) -> Result<ProcessingInputChanges> {
    ProcessingReadSnapshot::new(db, ProcessingCacheLimits::default())?.input_changes(
        run_id,
        after,
        limit,
        source_only,
    )
}

/// Fingerprint a handler from the exact Rust source files that implement its
/// operation and shared mechanics. Callers pass `include_bytes!` for those
/// files; unrelated crate sources do not invalidate the cached output.
pub fn rust_source_fingerprint(handler: &str, sources: &[&[u8]]) -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    hasher.update(b"tos-rust-handler-source-v1\0");
    hasher.update(&(handler.len() as u64).to_be_bytes());
    hasher.update(handler.as_bytes());
    for source in sources {
        hasher.update(&(source.len() as u64).to_be_bytes());
        hasher.update(source);
    }
    hasher.finalize()
}

fn append_closure_node(
    packet: &mut ProcessingDependencyClosure,
    node: ProcessingDependencyNode,
    used: &mut usize,
    limits: ClosureLimits,
) -> Result<()> {
    let size = serde_json::to_vec(
        &json!({"id":node.id,"kind":node.kind,"output_digest":node.output_digest}),
    )
    .map_err(|_| Error::Invalid("processing closure node encoding"))?
    .len();
    *used = used
        .checked_add(size + usize::from(!packet.nodes.is_empty()))
        .ok_or(Error::Budget("processing closure result bytes"))?;
    if *used > limits.max_result_bytes {
        return Err(Error::Budget("processing closure result bytes"));
    }
    packet.nodes.push(node);
    if packet.nodes.len() > limits.max_nodes {
        return Err(Error::Budget("processing closure node budget"));
    }
    Ok(())
}

fn append_closure_edge(
    packet: &mut ProcessingDependencyClosure,
    edge: ProcessingDependencyEdge,
    used: &mut usize,
    limits: ClosureLimits,
) -> Result<()> {
    let size =
        serde_json::to_vec(&json!({"dependency_id":edge.dependency_id,"task_id":edge.task_id}))
            .map_err(|_| Error::Invalid("processing closure edge encoding"))?
            .len();
    *used = used
        .checked_add(size + usize::from(!packet.edges.is_empty()))
        .ok_or(Error::Budget("processing closure result bytes"))?;
    if *used > limits.max_result_bytes {
        return Err(Error::Budget("processing closure result bytes"));
    }
    packet.edges.push(edge);
    Ok(())
}

fn closure_encoded_bytes(packet: &ProcessingDependencyClosure) -> Result<usize> {
    serde_json::to_vec(&json!({
        "schema":"tos_processing_dependency_closure_v1",
        "run_id":packet.run_id,
        "changed_ids":packet.changed_ids,
        "nodes":[],
        "edges":[],
        "coverage":packet.coverage,
        "is_semantic_acceptance":false,
        "is_source_change_completeness":false
    }))
    .map(|bytes| bytes.len())
    .map_err(|_| Error::Invalid("processing closure envelope encoding"))
}

fn canonical_value(value: &Value, max_bytes: usize) -> Result<Vec<u8>> {
    if max_bytes == 0 || max_bytes > MAX_CANONICAL_VALUE_BYTES {
        return Err(Error::Budget("processing JSON bytes"));
    }
    crate::knowledge_seal::canonical(value, max_bytes)
}

struct CappedJsonWriter {
    bytes: Vec<u8>,
    max: usize,
}

impl Write for CappedJsonWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("processing JSON size"))?;
        if next > self.max {
            return Err(io::Error::other("processing JSON cap"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn ordered_value(value: &Value, max_bytes: usize) -> Result<Vec<u8>> {
    if max_bytes == 0 || max_bytes > MAX_CANONICAL_VALUE_BYTES {
        return Err(Error::Budget("processing JSON bytes"));
    }
    let mut writer = CappedJsonWriter {
        bytes: Vec::new(),
        max: max_bytes,
    };
    serde_json::to_writer(&mut writer, value)
        .map_err(|_| Error::Budget("processing JSON bytes"))?;
    Ok(writer.bytes)
}

struct TransactionRollback<'a> {
    db: &'a Connection,
    active: bool,
}

impl Drop for TransactionRollback<'_> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.db.execute_batch("ROLLBACK");
        }
    }
}

fn immediate_transaction<T>(db: &Connection, operation: impl FnOnce() -> Result<T>) -> Result<T> {
    db.execute_batch("BEGIN IMMEDIATE")?;
    let mut rollback = TransactionRollback { db, active: true };
    let value = operation()?;
    db.execute_batch("COMMIT")?;
    rollback.active = false;
    Ok(value)
}

fn decode_value(bytes: &[u8]) -> Result<Value> {
    serde_json::from_slice(bytes).map_err(|_| Error::Invalid("processing cache JSON"))
}

fn decode_strict_value(bytes: &[u8], max_bytes: usize) -> Result<Value> {
    let limits = tos_foundation::JsonLimits::new(max_bytes, 96, 1_000_000, 4096)
        .map_err(|_| Error::Budget("processing cache JSON limits"))?;
    tos_foundation::parse_json(bytes, tos_foundation::JsonMode::PublishedStrict, limits)
        .map_err(|_| Error::Invalid("processing cache JSON"))?;
    decode_value(bytes)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_identifier(value: &str, max_bytes: usize) -> Result<()> {
    if value.is_empty() || value.len() > max_bytes || value.contains('\0') {
        return Err(Error::Invalid("invalid processing identity"));
    }
    Ok(())
}

fn new_run_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let sequence = RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let mut hasher = Digest256Hasher::new();
    hasher.update(b"tos-rust-processing-run-v1\0");
    hasher.update(now.to_string().as_bytes());
    hasher.update(b":");
    hasher.update(std::process::id().to_string().as_bytes());
    hasher.update(b":");
    hasher.update(sequence.to_string().as_bytes());
    hasher.finalize().to_hex()
}

fn next_used() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

fn read_output(
    db: &Connection,
    key: &str,
    max_output_bytes: usize,
) -> Result<Option<(Value, String)>> {
    let row: Option<(Option<Vec<u8>>, Option<String>, i64)> = db
        .query_row(
            "SELECT CASE WHEN bytes>=0 AND bytes<=?2 AND length(payload)<=?2 THEN payload END, \
                    CASE WHEN length(CAST(payload_digest AS BLOB))=64 THEN payload_digest END,bytes \
               FROM tos_rust_processing_outputs WHERE cache_key=?1",
            params![key, max_output_bytes as i64],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((Some(payload), Some(claimed_digest), claimed_bytes)) = row else {
        return Ok(None);
    };
    let actual_digest = Digest256::of_bytes(&payload).to_hex();
    let decoded = if claimed_bytes < 0
        || claimed_bytes as usize != payload.len()
        || payload.len() > max_output_bytes
        || claimed_digest != actual_digest
    {
        None
    } else {
        decode_strict_value(&payload, max_output_bytes)
            .ok()
            .and_then(|value| {
                canonical_value(&value, max_output_bytes)
                    .ok()
                    .map(|canonical| (value, Digest256::of_bytes(&canonical).to_hex()))
            })
    };
    let Some((value, output_digest)) = decoded else {
        return Ok(None);
    };
    Ok(Some((value, output_digest)))
}

fn write_output(
    db: &Connection,
    key: &str,
    payload: &[u8],
    digest: &str,
    limits: ProcessingCacheLimits,
) -> Result<bool> {
    let bytes =
        u64::try_from(payload.len()).map_err(|_| Error::Budget("processing output size"))?;
    if bytes > limits.max_payload_bytes {
        return Ok(false);
    }
    db.execute(
        "INSERT INTO tos_rust_processing_outputs(cache_key,payload,payload_digest,bytes,used) \
         VALUES (?1,?2,?3,?4,?5) ON CONFLICT(cache_key) DO UPDATE SET \
         payload=excluded.payload,payload_digest=excluded.payload_digest,bytes=excluded.bytes,used=excluded.used",
        params![key, payload, digest, bytes as i64, next_used()],
    )?;
    Ok(true)
}

fn trim_outputs(db: &Connection, limits: ProcessingCacheLimits) -> Result<u64> {
    let mut evicted = 0u64;
    loop {
        let (bytes, entries): (u64, u64) = db.query_row(
            "SELECT bytes,entries FROM tos_rust_processing_output_usage WHERE singleton=1",
            [],
            |row| Ok((row.get::<_, i64>(0)? as u64, row.get::<_, i64>(1)? as u64)),
        )?;
        if bytes <= limits.max_payload_bytes && entries <= limits.max_entries {
            return Ok(evicted);
        }
        let deleted = db.execute(
            "DELETE FROM tos_rust_processing_outputs WHERE cache_key IN \
                (SELECT cache_key FROM tos_rust_processing_outputs ORDER BY used,cache_key LIMIT 128)",
            [],
        )?;
        if deleted == 0 {
            return Ok(evicted);
        }
        evicted = evicted
            .checked_add(deleted as u64)
            .ok_or(Error::Budget("processing evicted output count"))?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    static COMMIT_TRACE_COUNT: AtomicUsize = AtomicUsize::new(0);

    fn count_commit() -> bool {
        COMMIT_TRACE_COUNT.fetch_add(1, Ordering::Relaxed);
        false
    }

    fn fingerprint(bytes: &[u8]) -> Digest256 {
        Digest256::of_bytes(bytes)
    }

    #[test]
    fn rust_namespace_reuses_outputs_and_publishes_exact_closure() {
        let cache =
            ProcessingCache::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        let mut first = cache.begin_run().unwrap();
        let source = first
            .input("source-node:work", &json!({"id":"work","title":"One"}))
            .unwrap();
        let node = first
            .evaluate_task(
                "node:work",
                "node",
                "knowledge_base::normalize_node",
                fingerprint(b"knowledge_base.rs:normalize_node-v1"),
                &json!({"source_graph":"philosophy"}),
                &[source.clone()],
                || Ok(json!({"id":"work","display":{"title":"One"}})),
            )
            .unwrap();
        let title = first
            .evaluate_task(
                "endpoint-title:work",
                "endpoint-title",
                "knowledge_global_titles::title",
                fingerprint(b"knowledge_global_titles.rs:title-v1"),
                &json!({"node_id":"work"}),
                &[node.clone()],
                || Ok(json!({"title":"One"})),
            )
            .unwrap();
        let edge = first
            .evaluate_task(
                "relation:edge",
                "relation",
                "knowledge_base::normalize_relation",
                fingerprint(b"knowledge_base.rs:normalize_relation-v1"),
                &json!({"id":"edge"}),
                &[source, title],
                || Ok(json!({"id":"edge"})),
            )
            .unwrap();
        assert_eq!(edge.kind(), "task");
        let first_id = first.id();
        first.finish().unwrap();
        let db = cache.shared.db.borrow();
        db.execute_batch("BEGIN").unwrap();
        let closure = ProcessingReadSnapshot::new(&db, ProcessingCacheLimits::default())
            .unwrap()
            .dependency_closure(
                &first_id,
                &["source-node:work".to_owned()],
                ClosureLimits::default(),
            )
            .unwrap();
        db.execute_batch("ROLLBACK").unwrap();
        assert!(
            closure
                .nodes
                .iter()
                .any(|node| node.id == "endpoint-title:work")
        );
        assert!(closure.nodes.iter().any(|node| node.id == "relation:edge"));
        assert!(!closure.is_semantic_acceptance);
        drop(db);

        let mut second = cache.begin_run().unwrap();
        let source = second
            .input("source-node:work", &json!({"id":"work","title":"One"}))
            .unwrap();
        let node = second
            .evaluate_task(
                "node:work",
                "node",
                "knowledge_base::normalize_node",
                fingerprint(b"knowledge_base.rs:normalize_node-v1"),
                &json!({"source_graph":"philosophy"}),
                &[source.clone()],
                || panic!("cache hit expected"),
            )
            .unwrap();
        assert_eq!(
            node.output_digest(),
            closure
                .nodes
                .iter()
                .find(|node| node.id == "node:work")
                .unwrap()
                .output_digest
        );
        second.finish().unwrap();
    }

    #[test]
    fn dropped_run_and_python_namespace_do_not_publish_or_migrate() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE completed_steps(cache_key TEXT PRIMARY KEY,payload TEXT NOT NULL);",
        )
        .unwrap();
        db.execute("INSERT INTO completed_steps VALUES ('old','{}')", [])
            .unwrap();
        let cache = ProcessingCache::from_connection(db).unwrap();
        let mut run = cache.begin_run().unwrap();
        let input = run.input("source-row:a", &json!({"id":"a"})).unwrap();
        run.evaluate_task(
            "node:a",
            "node",
            "handler",
            fingerprint(b"handler.rs"),
            &json!({}),
            &[input],
            || Ok(json!({"id":"a"})),
        )
        .unwrap();
        drop(run);
        assert!(cache.active_run_id().unwrap().is_none());
        let db = cache.shared.db.borrow();
        db.execute_batch("BEGIN").unwrap();
        let old_tables: i64 = db
            .query_row("SELECT count(*) FROM completed_steps", [], |row| row.get(0))
            .unwrap();
        assert_eq!(old_tables, 1);
        db.execute_batch("ROLLBACK").unwrap();
    }

    #[test]
    fn warm_receipts_batch_and_cold_output_is_durable_before_finish() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("processing.sqlite");
        let cache = ProcessingCache::open(&path).unwrap();
        let tasks = (0..12).map(|index| index.to_string()).collect::<Vec<_>>();
        let mut first = cache.begin_run().unwrap();
        for index in &tasks {
            let source = first
                .input(&format!("source:{index}"), &json!({"index":index}))
                .unwrap();
            first
                .evaluate_task(
                    &format!("task:{index}"),
                    "node",
                    "test-handler",
                    fingerprint(b"test-handler.rs:v1"),
                    &json!({}),
                    &[source],
                    || Ok(json!({"value":index})),
                )
                .unwrap();
        }
        first.finish().unwrap();
        drop(first);

        let mut warm = cache.begin_run().unwrap();
        COMMIT_TRACE_COUNT.store(0, Ordering::Relaxed);
        cache.shared.db.borrow().commit_hook(Some(count_commit));
        for index in &tasks {
            let source = warm
                .input(&format!("source:{index}"), &json!({"index":index}))
                .unwrap();
            warm.evaluate_task(
                &format!("task:{index}"),
                "node",
                "test-handler",
                fingerprint(b"test-handler.rs:v1"),
                &json!({}),
                &[source],
                || panic!("warm cache hit expected"),
            )
            .unwrap();
        }
        assert_eq!(COMMIT_TRACE_COUNT.load(Ordering::Relaxed), 0);
        let source = warm.input("source:new", &json!({"index":"new"})).unwrap();
        warm.evaluate_task(
            "task:new",
            "node",
            "test-handler",
            fingerprint(b"test-handler.rs:v1"),
            &json!({}),
            &[source],
            || Ok(json!({"value":"new"})),
        )
        .unwrap();
        let observer = Connection::open(&path).unwrap();
        let durable: i64 = observer
            .query_row(
                "SELECT count(*) FROM tos_rust_processing_outputs",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(durable, 13);
        cache.shared.db.borrow().commit_hook(None::<fn() -> bool>);
        warm.finish().unwrap();
    }

    #[test]
    fn wal_reader_snapshot_survives_concurrent_history_retirement() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("processing.sqlite");
        let cache = ProcessingCache::open(&path).unwrap();
        let mut run = cache.begin_run().unwrap();
        let input = run.input("source:a", &json!({"label":"Alpha"})).unwrap();
        run.evaluate_task(
            "node:a",
            "node",
            "test-handler",
            fingerprint(b"test-handler.rs:v1"),
            &json!({}),
            &[input],
            || Ok(json!({"label":"Alpha"})),
        )
        .unwrap();
        let run_id = run.id();
        run.finish().unwrap();

        let reader = Connection::open(&path).unwrap();
        reader.execute_batch("PRAGMA query_only=ON; BEGIN").unwrap();
        let expected = ProcessingReadSnapshot::new(&reader, ProcessingCacheLimits::default())
            .unwrap()
            .dependency_closure(&run_id, &["source:a".to_owned()], ClosureLimits::default())
            .unwrap();
        {
            let writer = cache.shared.db.borrow();
            immediate_transaction(&writer, || {
                writer.execute(
                    "DELETE FROM tos_rust_processing_run_bases WHERE run_id=?1",
                    [&run_id],
                )?;
                writer.execute(
                    "DELETE FROM tos_rust_processing_dependencies WHERE run_id=?1",
                    [&run_id],
                )?;
                writer.execute(
                    "DELETE FROM tos_rust_processing_tasks WHERE run_id=?1",
                    [&run_id],
                )?;
                writer.execute(
                    "DELETE FROM tos_rust_processing_runs WHERE id=?1",
                    [&run_id],
                )?;
                Ok(())
            })
            .unwrap();
        }
        let pinned = ProcessingReadSnapshot::new(&reader, ProcessingCacheLimits::default())
            .unwrap()
            .dependency_closure(&run_id, &["source:a".to_owned()], ClosureLimits::default())
            .unwrap();
        assert_eq!(pinned.nodes.len(), expected.nodes.len());
        reader.execute_batch("ROLLBACK; BEGIN").unwrap();
        assert!(
            ProcessingReadSnapshot::new(&reader, ProcessingCacheLimits::default())
                .unwrap()
                .dependency_closure(&run_id, &["source:a".to_owned()], ClosureLimits::default())
                .is_err()
        );
    }

    #[test]
    fn closure_refuses_duplicate_seed_but_deduplicates_shared_paths() {
        let cache =
            ProcessingCache::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        let mut run = cache.begin_run().unwrap();
        let source = run.input("source:a", &json!({"id":"a"})).unwrap();
        let node = run
            .evaluate_task(
                "node:a",
                "node",
                "test-handler",
                fingerprint(b"test-handler.rs:v1"),
                &json!({}),
                &[source.clone()],
                || Ok(json!({"id":"a"})),
            )
            .unwrap();
        run.evaluate_task(
            "final:a",
            "final-node",
            "test-finalizer",
            fingerprint(b"test-finalizer.rs:v1"),
            &json!({}),
            &[source, node],
            || Ok(json!({"id":"a"})),
        )
        .unwrap();
        assert_eq!(
            run.handle().latest_output("node", "a").unwrap().id(),
            "final:a"
        );
        let run_id = run.id();
        run.finish().unwrap();
        let db = cache.shared.db.borrow();
        db.execute_batch("BEGIN").unwrap();
        let snapshot = ProcessingReadSnapshot::new(&db, ProcessingCacheLimits::default()).unwrap();
        assert!(
            snapshot
                .dependency_closure(
                    &run_id,
                    &["source:a".to_owned(), "source:a".to_owned()],
                    ClosureLimits::default(),
                )
                .is_err()
        );
        let closure = snapshot
            .dependency_closure(
                &run_id,
                &["source:a".to_owned(), "node:a".to_owned()],
                ClosureLimits::default(),
            )
            .unwrap();
        assert_eq!(
            closure
                .nodes
                .iter()
                .filter(|node| node.id == "final:a")
                .count(),
            1
        );
        assert_eq!(
            closure
                .edges
                .iter()
                .filter(|edge| edge.task_id == "final:a")
                .count(),
            2
        );
        db.execute_batch("ROLLBACK").unwrap();
    }

    #[test]
    fn publication_cas_failure_recovery_input_invalidation_and_corruption_miss() {
        let cache =
            ProcessingCache::from_connection(Connection::open_in_memory().unwrap()).unwrap();
        let mut winner = cache.begin_run().unwrap();
        let winner_input = winner.input("source:row", &json!({"version":1})).unwrap();
        winner
            .evaluate_task(
                "node:row",
                "node",
                "test-normalizer",
                fingerprint(b"test-normalizer-v1"),
                &json!({"graph":"test"}),
                &[winner_input],
                || Ok(json!({"id":"row","version":1})),
            )
            .unwrap();
        let winner_id = winner.id();
        let mut competing = cache.begin_run().unwrap();
        let competing_input = competing
            .input("source:row", &json!({"version":1}))
            .unwrap();
        competing
            .evaluate_task(
                "node:row",
                "node",
                "test-normalizer",
                fingerprint(b"test-normalizer-v1"),
                &json!({"graph":"test"}),
                &[competing_input],
                || panic!("competing full run should share the durable pure output"),
            )
            .unwrap();
        let competing_id = competing.id();
        winner.finish().unwrap();
        assert!(competing.finish().is_err());
        assert_eq!(
            cache.active_run_id().unwrap().as_deref(),
            Some(winner_id.as_str())
        );

        let mut failed = cache.begin_run().unwrap();
        let changed = failed.input("source:row", &json!({"version":2})).unwrap();
        assert!(
            failed
                .evaluate_task(
                    "node:row",
                    "node",
                    "test-normalizer",
                    fingerprint(b"test-normalizer-v1"),
                    &json!({"graph":"test"}),
                    &[changed],
                    || Err(Error::Invalid("fixture handler failure")),
                )
                .is_err()
        );
        assert_eq!(failed.report().unwrap().status, "failed");
        assert_eq!(
            cache.active_run_id().unwrap().as_deref(),
            Some(winner_id.as_str())
        );

        let mut recovered = cache.begin_run().unwrap();
        let changed = recovered
            .input("source:row", &json!({"version":2}))
            .unwrap();
        let recovered_node = recovered
            .evaluate_task(
                "node:row",
                "node",
                "test-normalizer",
                fingerprint(b"test-normalizer-v1"),
                &json!({"graph":"test"}),
                &[changed],
                || Ok(json!({"id":"row","version":2})),
            )
            .unwrap();
        assert_eq!(recovered_node.value(), &json!({"id":"row","version":2}));
        let recovered_id = recovered.id();
        recovered.finish().unwrap();
        assert_eq!(recovered.report().unwrap().input_changes["changed"], 1);

        let key: String = cache
            .shared
            .db
            .borrow()
            .query_row(
                "SELECT cache_key FROM tos_rust_processing_tasks WHERE run_id=?1 AND id='node:row'",
                [&winner_id],
                |row| row.get(0),
            )
            .unwrap();
        cache
            .shared
            .db
            .borrow()
            .execute(
                "UPDATE tos_rust_processing_outputs SET payload=x'7b7d' WHERE cache_key=?1",
                [&key],
            )
            .unwrap();
        let recomputed = AtomicUsize::new(0);
        let mut after_corruption = cache.begin_run().unwrap();
        let original = after_corruption
            .input("source:row", &json!({"version":1}))
            .unwrap();
        after_corruption
            .evaluate_task(
                "node:row",
                "node",
                "test-normalizer",
                fingerprint(b"test-normalizer-v1"),
                &json!({"graph":"test"}),
                &[original],
                || {
                    recomputed.fetch_add(1, Ordering::Relaxed);
                    Ok(json!({"id":"row","version":1}))
                },
            )
            .unwrap();
        assert_eq!(recomputed.load(Ordering::Relaxed), 1);
        after_corruption.finish().unwrap();
        assert_ne!(recovered_id, competing_id);
    }
}
