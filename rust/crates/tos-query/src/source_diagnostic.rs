//! Read-only maintained source diagnostics. No publication or query authority.
use crate::{
    philosophy_read::{compute_source_philosophy_view_diagnostic, PhilosophyReadBudget},
    AbortProbe,
};
use serde_json::{Map, Value};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Instant,
};
use tos_foundation::{parse_json, Digest256, Digest256Hasher, JsonLimits, JsonMode};

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_input_bytes: u64,
    pub max_json_bytes: usize,
    pub max_rows: u64,
    pub max_work_steps: u64,
    pub max_sql_vm_steps: u64,
    pub sqlite_cache_kib: u32,
}
#[derive(Debug)]
pub struct DiagnosticError(pub String);
impl std::fmt::Display for DiagnosticError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for DiagnosticError {}
pub type Result<T> = std::result::Result<T, DiagnosticError>;
fn err(s: impl ToString) -> DiagnosticError {
    DiagnosticError(s.to_string())
}
const INPUTS: [&str; 5] = [
    "ToS/derived-exports/tos_corpus_index.min.json",
    "ToS/derived-exports/philosophy_graph_projection.min.json",
    "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/doctrine/semantic-interchange/relation-types.v1.json",
];
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stamp(u64, u64, u64, i64, i64, i64, i64);
fn stamp(m: &fs::Metadata) -> Stamp {
    Stamp(
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
struct Held {
    path: PathBuf,
    file: File,
    stamp: Stamp,
}
impl Held {
    fn open(path: &Path, cap: u64) -> Result<Self> {
        Self::open_inner(path, cap, false)
    }
    fn open_owned(path: &Path, cap: u64) -> Result<Self> {
        Self::open_inner(path, cap, true)
    }
    fn open_inner(path: &Path, cap: u64, bounded: bool) -> Result<Self> {
        let file = tos_fd_open::open_absolute_regular(path, cap).map_err(|error| {
            if bounded {
                owned_err("legacy exact regular descriptor open failed")
            } else {
                err(error)
            }
        })?;
        let m = file.metadata().map_err(|error| {
            if bounded {
                owned_err("legacy held metadata failed")
            } else {
                err(error)
            }
        })?;
        let h = Self {
            path: path.into(),
            file,
            stamp: stamp(&m),
        };
        h.verify_inner(bounded)?;
        Ok(h)
    }
    fn verify(&self) -> Result<()> {
        self.verify_inner(false)
    }
    fn verify_owned(&self) -> Result<()> {
        self.verify_inner(true)
    }
    fn verify_inner(&self, bounded: bool) -> Result<()> {
        let failure = |error| {
            if bounded {
                owned_err("legacy held currentness metadata failed")
            } else {
                err(error)
            }
        };
        let m = fs::symlink_metadata(&self.path).map_err(failure)?;
        if !m.is_file()
            || m.file_type().is_symlink()
            || stamp(&m) != self.stamp
            || stamp(&self.file.metadata().map_err(failure)?) != self.stamp
        {
            return Err(if bounded {
                owned_err("source diagnostic input changed")
            } else {
                err("source diagnostic input changed")
            });
        }
        Ok(())
    }
}

struct Meter<'a> {
    limits: Limits,
    deadline: Instant,
    abort: &'a dyn AbortProbe,
    bytes: u64,
    rows: u64,
    work: u64,
    held: Vec<Held>,
}
impl Meter<'_> {
    fn check(&self) -> Result<()> {
        if Instant::now() >= self.deadline {
            return Err(err("source diagnostic deadline"));
        }
        if self.abort.reason().is_some() {
            return Err(err("source diagnostic aborted"));
        }
        Ok(())
    }
    fn charge(&mut self, n: u64) -> Result<()> {
        self.check()?;
        self.bytes = self
            .bytes
            .checked_add(n)
            .filter(|v| *v <= self.limits.max_input_bytes)
            .ok_or_else(|| err("source diagnostic input budget"))?;
        Ok(())
    }
    fn step(&mut self, n: u64) -> Result<()> {
        self.check()?;
        self.work = self
            .work
            .checked_add(n)
            .filter(|v| *v <= self.limits.max_work_steps)
            .ok_or_else(|| err("source diagnostic work budget"))?;
        Ok(())
    }
    fn read(&mut self, path: &Path, cap: usize) -> Result<Vec<u8>> {
        self.check()?;
        let mut h = Held::open(
            path,
            (cap as u64).min(self.limits.max_input_bytes.saturating_sub(self.bytes)),
        )?;
        self.charge(h.stamp.2)?;
        let mut raw = Vec::with_capacity(h.stamp.2 as usize);
        let mut chunk = [0u8; 65536];
        loop {
            self.check()?;
            let n = h.file.read(&mut chunk).map_err(err)?;
            if n == 0 {
                break;
            }
            if raw.len().saturating_add(n) > cap || raw.len().saturating_add(n) > h.stamp.2 as usize
            {
                return Err(err("source diagnostic changed size"));
            }
            raw.extend_from_slice(&chunk[..n]);
        }
        h.verify()?;
        if raw.len() as u64 != h.stamp.2 {
            return Err(err("source diagnostic changed size"));
        }
        self.held.push(h);
        Ok(raw)
    }
    fn parse(&mut self, raw: &[u8]) -> Result<Value> {
        self.step(raw.len() as u64)?;
        let limits = JsonLimits::new(
            self.limits.max_json_bytes,
            96,
            self.limits.max_work_steps.min(usize::MAX as u64) as usize,
            4300,
        )
        .map_err(err)?;
        parse_json(raw, JsonMode::PublishedStrict, limits).map_err(err)?;
        self.check()?;
        serde_json::from_slice(raw).map_err(err)
    }
    fn verify(&self) -> Result<()> {
        self.check()?;
        for h in &self.held {
            h.verify()?;
        }
        Ok(())
    }
}
fn meter(limits: Limits, deadline: Instant, abort: &dyn AbortProbe) -> Result<Meter<'_>> {
    if limits.max_input_bytes == 0
        || limits.max_json_bytes == 0
        || limits.max_json_bytes > i64::MAX as usize
        || limits.max_rows == 0
        || limits.max_work_steps == 0
        || limits.max_sql_vm_steps == 0
        || limits.sqlite_cache_kib == 0
    {
        return Err(err("source diagnostic limits required"));
    }
    Ok(Meter {
        limits,
        deadline,
        abort,
        bytes: 0,
        rows: 0,
        work: 0,
        held: vec![],
    })
}
fn object(v: &Value) -> Result<&Map<String, Value>> {
    v.as_object()
        .ok_or_else(|| err("source diagnostic object required"))
}
fn exact(v: &Value, keys: &[&str]) -> Result<()> {
    let o = object(v)?;
    if o.len() != keys.len() || keys.iter().any(|k| !o.contains_key(*k)) {
        return Err(err("source diagnostic envelope"));
    }
    Ok(())
}
fn string<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v[k].as_str().ok_or_else(|| err("source diagnostic string"))
}
fn count(v: &Value, k: &str) -> Result<u64> {
    v[k].as_u64().ok_or_else(|| err("source diagnostic count"))
}
fn hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn descriptor(root: &Path, v: &Value, prefix: &str) -> Result<PathBuf> {
    exact(
        v,
        &[
            "kind",
            "prefix",
            "path",
            "sha256",
            "size_bytes",
            "decoded_bytes",
            "decoded_sha256",
            "count",
        ],
    )?;
    let kind = string(v, "kind")?;
    let sha = string(v, "sha256")?;
    if !matches!(kind, "data" | "index")
        || string(v, "prefix")? != prefix
        || prefix.len() > 64
        || !prefix
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || !hex(sha)
        || !hex(string(v, "decoded_sha256")?)
    {
        return Err(err("source diagnostic part identity"));
    }
    let bound = if kind == "data" {
        8 * 1024 * 1024
    } else {
        128 * 1024
    };
    if count(v, "decoded_bytes")? > bound || count(v, "size_bytes")? > bound + 65536 {
        return Err(err("source diagnostic part bounds"));
    }
    count(v, "count")?;
    let stem = root
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| err("source diagnostic root filename"))?;
    let expected = format!(
        "{stem}.parts/{}/{}{}",
        &sha[..2],
        sha,
        if kind == "data" {
            ".jsonl.gz"
        } else {
            ".index.json"
        }
    );
    if string(v, "path")? != expected {
        return Err(err("source diagnostic part namespace"));
    }
    Ok(root
        .parent()
        .ok_or_else(|| err("source diagnostic parent"))?
        .join(expected))
}
fn partition_header(path: &Path, v: &Value) -> Result<Value> {
    exact(
        v,
        &[
            "schema_version",
            "logical_schema",
            "header",
            "limits",
            "collections",
        ],
    )?;
    if v["schema_version"] != "tos_partitioned_projection_v1"
        || string(v, "logical_schema")?.is_empty()
        || v["logical_schema"] != v["header"]["schema_version"]
    {
        return Err(err("source diagnostic partition schema"));
    }
    if v["limits"]
        != serde_json::json!({"root_bytes":262144,"index_bytes":131072,"part_bytes":8388608,"key_bytes":4096})
    {
        return Err(err("source diagnostic partition limits"));
    }
    let mut header = v["header"].clone();
    object(&header)?;
    let collections = object(&v["collections"])?;
    if collections.is_empty() {
        return Err(err("source diagnostic empty collections"));
    }
    for (name, spec) in collections {
        if !name.split('/').all(|p| {
            !p.is_empty()
                && p.as_bytes()[0].is_ascii_lowercase()
                && p.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        }) {
            return Err(err("source diagnostic collection name"));
        }
        exact(spec, &["key_field", "order_fields", "root"])?;
        let key = &spec["key_field"];
        if !(key.is_null()
            || key.as_str().is_some_and(|s| !s.is_empty())
            || key
                .as_array()
                .is_some_and(|a| a.iter().all(|v| v.as_str().is_some_and(|s| !s.is_empty()))))
        {
            return Err(err("source diagnostic key field"));
        }
        let order = spec["order_fields"]
            .as_array()
            .ok_or_else(|| err("source diagnostic ordering"))?;
        if order.iter().any(|v| v.as_str().is_none_or(str::is_empty))
            || key.as_array().is_some_and(Vec::is_empty) && !order.is_empty()
        {
            return Err(err("source diagnostic ordering"));
        }
        let mut current = &mut header;
        let parts = name.split('/').collect::<Vec<_>>();
        for p in &parts[..parts.len() - 1] {
            let o = current
                .as_object_mut()
                .ok_or_else(|| err("source diagnostic overlapping collection"))?;
            current = o
                .entry((*p).to_owned())
                .or_insert_with(|| Value::Object(Map::new()));
        }
        let o = current
            .as_object_mut()
            .ok_or_else(|| err("source diagnostic overlapping collection"))?;
        if o.insert(parts.last().unwrap().to_string(), Value::Null)
            .is_some()
        {
            return Err(err("source diagnostic overlapping collection"));
        }
        descriptor(path, &spec["root"], "")?;
    }
    Ok(v["header"].clone())
}
/// Validates only root envelope and descriptors; opens no partition parts.
pub fn projection_header(
    path: &Path,
    limits: Limits,
    deadline: Instant,
    abort: &dyn AbortProbe,
) -> Result<Value> {
    let mut m = meter(limits, deadline, abort)?;
    let raw = m.read(path, limits.max_json_bytes)?;
    let v = m.parse(&raw)?;
    let h = if v["schema_version"] == "tos_partitioned_projection_v1"
        || v["schema"] == "tos_partitioned_projection_v1"
    {
        if raw.len() > 262144 {
            return Err(err("partition root byte bound"));
        }
        partition_header(path, &v)?
    } else {
        object(&v)?;
        v
    };
    m.verify()?;
    Ok(h)
}

/// Genuine maintained legacy snapshot readiness, not a native prepared model.
pub struct LegacyStore {
    pub revision: String,
    pub corpus_header: Value,
    pub graph_header: Value,
    pub catalog: Value,
    db: tos_source_store::PinnedSqliteConnection,
    held: Vec<Held>,
    limits: Limits,
    deadline: Instant,
    abort: Arc<dyn AbortProbe>,
    bytes: u64,
    rows: u64,
    work: u64,
    vm: Arc<AtomicU64>,
    graph_path: PathBuf,
    database_path: PathBuf,
    view_attempted: bool,
    // Only the owned route binds these original counters. Old callers keep None.
    owned: Option<OriginalStoreCounters>,
}
#[path = "source_diagnostic_legacy.rs"]
mod legacy;

impl LegacyStore {
    pub fn open(
        path: &Path,
        inputs: &[(String, PathBuf)],
        limits: Limits,
        deadline: Instant,
        abort: Arc<dyn AbortProbe>,
    ) -> Result<Self> {
        Self::open_bounded(path, inputs, limits, u64::MAX, deadline, abort)
    }
    /// Same authenticated store owner, with a caller-selected held database cap.
    pub fn open_bounded(
        path: &Path,
        inputs: &[(String, PathBuf)],
        limits: Limits,
        max_database_bytes: u64,
        deadline: Instant,
        abort: Arc<dyn AbortProbe>,
    ) -> Result<Self> {
        if max_database_bytes == 0 {
            return Err(err("source diagnostic database budget"));
        }
        let mut m = meter(limits, deadline, abort.as_ref())?;
        let keys = inputs
            .iter()
            .map(|(k, _)| k.as_str())
            .collect::<BTreeSet<_>>();
        if inputs.len() != 5 || keys != INPUTS.into_iter().collect() {
            return Err(err("source diagnostic exact five inputs required"));
        }
        let mut bindings = Map::new();
        for (name, p) in inputs {
            let mut h = Held::open(p, limits.max_input_bytes.saturating_sub(m.bytes))?;
            m.charge(h.stamp.2)?;
            let mut hash = Digest256Hasher::new();
            let mut chunk = [0u8; 65536];
            let mut len = 0u64;
            loop {
                m.check()?;
                let n = h.file.read(&mut chunk).map_err(err)?;
                if n == 0 {
                    break;
                }
                len = len
                    .checked_add(n as u64)
                    .filter(|n| *n <= h.stamp.2)
                    .ok_or_else(|| err("source diagnostic changed size"))?;
                hash.update(&chunk[..n]);
            }
            if len != h.stamp.2 {
                return Err(err("source diagnostic changed size"));
            }
            h.verify()?;
            bindings.insert(name.clone(), Value::String(hash.finalize().to_hex()));
            m.held.push(h);
        }
        no_journal(path)?;
        let held = Held::open(path, max_database_bytes)?;
        m.check()?;
        // The pager reads the exact retained inode through the shared FD VFS.
        // Source authentication and cumulative diagnostic budgets stay here.
        let db = tos_source_store::PinnedSqliteConnection::open_readonly_immutable(&held.file)
            .map_err(err)?;
        m.check()?;
        let vm = Arc::new(AtomicU64::new(0));
        let vm_c = vm.clone();
        let probe = abort.clone();
        db.progress_handler(
            1,
            Some(move || {
                vm_c.fetch_add(1, Ordering::Relaxed) >= limits.max_sql_vm_steps
                    || Instant::now() >= deadline
                    || probe.reason().is_some()
            }),
        );
        db.execute_batch(&format!(
            "PRAGMA query_only=ON; PRAGMA cache_size=-{}; PRAGMA temp_store=MEMORY;",
            limits.sqlite_cache_kib
        ))
        .map_err(err)?;

        let mut metadata = Map::new();
        {
            let mut stmt=db.prepare("SELECT CASE WHEN typeof(key)='text' AND length(CAST(key AS BLOB))<=4096 THEN key ELSE NULL END, CASE WHEN typeof(value)='text' AND length(CAST(value AS BLOB))<=?1 THEN value ELSE NULL END FROM metadata").map_err(err)?;
            let mut rows = stmt.query([limits.max_json_bytes as i64]).map_err(err)?;
            while let Some(row) = rows.next().map_err(err)? {
                m.rows = m
                    .rows
                    .checked_add(1)
                    .ok_or_else(|| err("source diagnostic metadata row overflow"))?;
                if m.rows > limits.max_rows {
                    return Err(err("source diagnostic metadata row budget"));
                }
                let key: String = row.get(0).map_err(err)?;
                let raw: String = row.get(1).map_err(err)?;
                m.charge(raw.len() as u64)?;
                let value = m.parse(raw.as_bytes())?;
                if metadata.insert(key, value).is_some() {
                    return Err(err("source diagnostic duplicate metadata"));
                }
            }
        }
        if metadata.get("schema") != Some(&Value::String("tos_query_store_v1".into()))
            || metadata.get("compiler_version")
                != Some(&Value::String("tos_offline_knowledge_v2".into()))
            || metadata.get("complete") != Some(&Value::Bool(true))
            || metadata.get("snapshot_bindings") != Some(&Value::Object(bindings))
        {
            return Err(err("query store unsupported/incomplete/stale snapshot"));
        }
        let graph_header = metadata
            .remove("graph_header")
            .ok_or_else(|| err("missing graph_header"))?;
        object(&graph_header)?;
        let catalog = metadata
            .remove("catalog")
            .ok_or_else(|| err("missing catalog"))?;
        object(&catalog)?;
        let corpus_header = metadata
            .remove("corpus_header")
            .ok_or_else(|| err("missing corpus_header"))?;
        object(&corpus_header)?;
        let revision = metadata
            .get("exploration_revision")
            .and_then(Value::as_str)
            .ok_or_else(|| err("missing exploration_revision"))?
            .to_owned();
        held.verify()?;
        no_journal(path)?;
        m.verify()?;
        m.held.push(held);
        let graph_path = inputs
            .iter()
            .find(|(k, _)| k == INPUTS[1])
            .unwrap()
            .1
            .clone();
        let bytes = m.bytes;
        let rows = m.rows;
        let work = m.work;
        let held = std::mem::take(&mut m.held);
        drop(m);
        Ok(Self {
            revision,
            corpus_header,
            graph_header,
            catalog,
            db,
            held,
            limits,
            deadline,
            abort,
            bytes,
            rows,
            work,
            vm,
            graph_path,
            database_path: path.to_owned(),
            view_attempted: false,
            owned: None,
        })
    }
    pub fn verify_currentness(&self) -> Result<()> {
        if Instant::now() >= self.deadline || self.abort.reason().is_some() {
            return Err(err("source diagnostic deadline/abort"));
        }
        for h in &self.held {
            h.verify()?;
        }
        no_journal(&self.database_path)
    }
    /// Enumerate every normalized carrier of this authenticated immutable cut.
    /// The callback owns disclosure; no second corpus is accumulated here.
    pub fn visit_knowledge_carriers(
        &mut self,
        mut observe: impl FnMut(&Value) -> Result<()>,
    ) -> Result<(u64, u64)> {
        let nodes = self.visit_knowledge_table(false, |value| observe(value))?;
        let relations = self.visit_knowledge_table(true, |value| observe(value))?;
        Ok((nodes, relations))
    }
    /// Explicit full export uses the same authenticated store and cumulative
    /// work/row/VM limits; no source reconstruction or selected-model admission.
    pub fn visit_knowledge_table(
        &mut self,
        relations: bool,
        mut observe: impl FnMut(&Value) -> Result<()>,
    ) -> Result<u64> {
        self.visit_knowledge_where(relations, "1", &[], self.limits.max_json_bytes, 0, observe)
    }
    fn visit_knowledge_where(
        &mut self,
        relations: bool,
        predicate: &str,
        parameters: &[String],
        max_payload_bytes: usize,
        extra_work_per_byte: u64,
        mut observe: impl FnMut(&Value) -> Result<()>,
    ) -> Result<u64> {
        if self.owned.is_some() {
            return Err(err(
                "legacy owned store requires original-budget operation API",
            ));
        }

        self.verify_currentness()?;
        let table = if relations {
            "knowledge_relations"
        } else {
            "knowledge_nodes"
        };
        let mut count = 0u64;
        let mut statement = self.db.prepare(&format!(
            "SELECT CASE WHEN typeof(payload)='text' AND length(CAST(payload AS BLOB))<=?1 THEN payload ELSE NULL END FROM {table} WHERE {predicate} ORDER BY id"
        )).map_err(err)?;
        let parameters = std::iter::once(rusqlite::types::Value::Integer(
            max_payload_bytes.min(self.limits.max_json_bytes) as i64,
        ))
        .chain(parameters.iter().cloned().map(rusqlite::types::Value::Text));
        let mut rows = statement
            .query(rusqlite::params_from_iter(parameters))
            .map_err(err)?;
        while let Some(row) = rows.next().map_err(err)? {
            self.verify_currentness()?;
            self.rows = self
                .rows
                .checked_add(1)
                .filter(|n| *n <= self.limits.max_rows)
                .ok_or_else(|| err("source diagnostic carrier row budget"))?;
            let raw: String = row.get(0).map_err(err)?;
            let mut m = meter(self.limits, self.deadline, self.abort.as_ref())?;
            m.bytes = self.bytes;
            m.work = self.work;
            m.charge(raw.len() as u64)?;
            // Generic reads reserve normalization/navigation work before the
            // callback. Plain full export retains its existing zero surcharge.
            m.step(
                (raw.len() as u64)
                    .checked_mul(extra_work_per_byte)
                    .ok_or_else(|| err("legacy query work overflow"))?,
            )?;
            let value = m.parse(raw.as_bytes())?;
            object(&value)?;
            self.bytes = m.bytes;
            self.work = m.work;
            observe(&value)?;
            count = count
                .checked_add(1)
                .ok_or_else(|| err("source diagnostic count overflow"))?;
        }
        self.verify_currentness()?;
        Ok(count)
    }
    /// Preserve Reference corpus_header's ordered graph_views selection.
    pub fn corpus_header_with_graph_views(&mut self) -> Result<Value> {
        self.corpus_header_with_graph_views_bounded(usize::MAX)
    }
    /// Same ordered collector with the caller's original live-state ceiling.
    pub fn corpus_header_with_graph_views_bounded(
        &mut self,
        max_state_bytes: usize,
    ) -> Result<Value> {
        if self.owned.is_some() {
            return Err(err(
                "legacy owned store requires original-budget operation API",
            ));
        }

        self.verify_currentness()?;
        let mut state = 0usize;
        legacy::retained(&self.corpus_header, &mut state, max_state_bytes)?;
        let mut payload = self.corpus_header.clone();
        let mut views = Vec::new();
        let mut statement = self.db.prepare(
            "SELECT length(CAST(payload AS BLOB)), CASE WHEN typeof(payload)='text' AND length(CAST(payload AS BLOB))<=?1 THEN payload ELSE NULL END FROM raw_records WHERE collection='corpus/graph_views' ORDER BY position"
        ).map_err(err)?;
        let mut rows = statement
            .query([self.limits.max_json_bytes as i64])
            .map_err(err)?;
        while let Some(row) = rows.next().map_err(err)? {
            self.verify_currentness()?;
            self.rows = self
                .rows
                .checked_add(1)
                .filter(|n| *n <= self.limits.max_rows)
                .ok_or_else(|| err("source diagnostic corpus header row budget"))?;
            let raw_bytes: usize = row.get(0).map_err(err)?;
            // Admit the cumulative raw/DOM/vector forecast before SQLite
            // copies the payload or the strict parser allocates its document.
            legacy::reserve_raw(raw_bytes, &mut state, max_state_bytes)?;
            let raw: String = row.get(1).map_err(err)?;
            let mut m = meter(self.limits, self.deadline, self.abort.as_ref())?;
            m.bytes = self.bytes;
            m.work = self.work;
            m.charge(raw.len() as u64)?;
            let value = m.parse(raw.as_bytes())?;
            object(&value)?;
            self.bytes = m.bytes;
            self.work = m.work;
            views.push(value);
        }
        payload
            .as_object_mut()
            .ok_or_else(|| err("source diagnostic corpus header object"))?
            .insert("graph_views".into(), Value::Array(views));
        self.verify_currentness()?;
        Ok(payload)
    }
    pub fn first_view_packet(&mut self, budget: PhilosophyReadBudget) -> Result<Vec<u8>> {
        if self.owned.is_some() {
            return Err(err(
                "legacy owned store requires original-budget operation API",
            ));
        }

        self.verify_currentness()?;
        if self.view_attempted {
            return Err(err("source diagnostic first view already attempted"));
        }
        self.view_attempted = true;
        let mut m = meter(self.limits, self.deadline, self.abort.as_ref())?;
        m.bytes = self.bytes;
        m.rows = self.rows;
        m.work = self.work;
        let raw = m.read(&self.graph_path, self.limits.max_json_bytes)?;
        let mut v = m.parse(&raw)?;
        if v["schema_version"] == "tos_partitioned_projection_v1"
            || v["schema"] == "tos_partitioned_projection_v1"
        {
            if raw.len() > 262144 {
                return Err(err("partition root byte bound"));
            }
            let mut header = partition_header(&self.graph_path, &v)?;
            for name in [
                "views",
                "nodes",
                "edges",
                "clusters",
                "review_packets",
                "graph_layers",
            ] {
                if let Some(spec) = v["collections"].get(name) {
                    let mut rows = vec![];
                    let selection = if matches!(name, "nodes" | "edges")
                        && spec["key_field"]
                            == if name == "nodes" {
                                "node_id"
                            } else {
                                "edge_id"
                            } {
                        let view = header["views"]
                            .as_array()
                            .and_then(|views| {
                                views
                                    .iter()
                                    .find(|v| v["view_id"].as_str().is_some_and(|s| !s.is_empty()))
                            })
                            .ok_or_else(|| err("projection has no graph views"))?;
                        let inline = ["nodes", "edges"].iter().any(|k| {
                            view[*k]
                                .as_array()
                                .is_some_and(|a| a.iter().any(Value::is_object))
                        });
                        Some(if inline {
                            BTreeSet::new()
                        } else {
                            view[if name == "nodes" {
                                "node_ids"
                            } else {
                                "edge_ids"
                            }]
                            .as_array()
                            .map(|ids| {
                                ids.iter()
                                    .filter_map(Value::as_str)
                                    .map(str::to_owned)
                                    .collect()
                            })
                            .unwrap_or_default()
                        })
                    } else {
                        None
                    };
                    visit(
                        &mut m,
                        &self.graph_path,
                        spec,
                        &spec["root"],
                        "",
                        &mut rows,
                        selection.as_ref(),
                    )?;
                    let sort_work = rows
                        .iter()
                        .try_fold(0u64, |sum, (key, _)| {
                            sum.checked_add(key.len() as u64 + 1)
                                .ok_or_else(|| err("source diagnostic ordering budget"))
                        })?
                        .checked_mul((rows.len().max(1).ilog2() + 1) as u64)
                        .ok_or_else(|| err("source diagnostic ordering budget"))?;
                    m.step(sort_work)?;
                    header
                        .as_object_mut()
                        .unwrap()
                        .insert(name.into(), ordered_rows(&mut m, spec, rows)?);
                }
            }
            v = header;
        }
        let view_id = v["views"]
            .as_array()
            .and_then(|vs| {
                vs.iter()
                    .find_map(|v| v["view_id"].as_str().filter(|s| !s.is_empty()))
            })
            .ok_or_else(|| err("projection has no graph views"))?
            .to_owned();
        let mut sink = CappedJson {
            bytes: Vec::new(),
            cap: self.limits.max_json_bytes.min(
                usize::try_from(self.limits.max_input_bytes.saturating_sub(m.bytes))
                    .unwrap_or(usize::MAX),
            ),
            deadline: self.deadline,
            abort: self.abort.as_ref(),
        };
        serde_json::to_writer(&mut sink, &v).map_err(err)?;
        let raw = sink.bytes;
        if raw.len() > self.limits.max_json_bytes {
            return Err(err("source diagnostic graph aggregate budget"));
        }
        let mut bounded = budget;
        bounded.max_work_steps = bounded
            .max_work_steps
            .min(self.limits.max_work_steps.saturating_sub(m.work));
        bounded.inspect.max_rows = bounded
            .inspect
            .max_rows
            .min(self.limits.max_rows.saturating_sub(m.rows));
        bounded.inspect.max_decoded_bytes = bounded
            .inspect
            .max_decoded_bytes
            .min(self.limits.max_input_bytes.saturating_sub(m.bytes));
        // The existing kernel independently counts these same logical rows.
        // Reserve its actual decoded input and logical row charge before entry.
        let mut kernel_rows = 0u64;
        let mut charge_rows = |value: &Value| -> Result<()> {
            kernel_rows = kernel_rows
                .checked_add(value.as_array().map_or(0, Vec::len) as u64)
                .ok_or_else(|| err("source diagnostic row overflow"))?;
            Ok(())
        };
        charge_rows(&v["nodes"])?;
        charge_rows(&v["edges"])?;
        if let Some(views) = v["views"].as_array() {
            for view in views.iter().filter(|v| v.is_object()) {
                charge_rows(&view["nodes"])?;
                charge_rows(&view["edges"])?;
            }
        }
        m.rows = m
            .rows
            .checked_add(kernel_rows)
            .filter(|n| *n <= self.limits.max_rows)
            .ok_or_else(|| err("source diagnostic aggregate row budget"))?;
        m.charge(raw.len() as u64)?;
        let probe = DeadlineProbe {
            deadline: self.deadline,
            inner: self.abort.as_ref(),
        };
        let body = compute_source_philosophy_view_diagnostic(&raw, &view_id, bounded, &probe)
            .map_err(err)?;
        m.verify()?;
        self.bytes = m.bytes;
        self.rows = m.rows;
        self.work = self.limits.max_work_steps; // A completed kernel consumes this one-shot allowance conservatively.
        self.held.append(&mut m.held);
        self.verify_currentness()?;
        Ok(body)
    }
    /// Actual cumulative VM work (including readiness SQL).
    pub fn sql_vm_steps(&self) -> u64 {
        let _ = &self.db;
        self.vm.load(Ordering::Relaxed)
    }
    /// Cumulative charged input bytes and rows of this owned diagnostic operation; not RSS.
    pub fn resource_usage(&self) -> (u64, u64) {
        (self.bytes, self.rows)
    }
}
fn no_journal(path: &Path) -> Result<()> {
    for suffix in ["-wal", "-journal", "-shm"] {
        let mut p = path.as_os_str().to_owned();
        p.push(suffix);
        match fs::symlink_metadata(Path::new(&p)) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err(err("query store must be an immutable completed snapshot")),
        }
    }
    Ok(())
}
struct DeadlineProbe<'a> {
    deadline: Instant,
    inner: &'a dyn AbortProbe,
}
impl AbortProbe for DeadlineProbe<'_> {
    fn reason(&self) -> Option<crate::AbortReason> {
        if Instant::now() >= self.deadline {
            Some(crate::AbortReason::DeadlineExceeded)
        } else {
            self.inner.reason()
        }
    }
}
fn visit(
    m: &mut Meter<'_>,
    root: &Path,
    spec: &Value,
    d: &Value,
    prefix: &str,
    out: &mut Vec<(String, Value)>,
    selected: Option<&BTreeSet<String>>,
) -> Result<()> {
    m.step(1)?;
    let path = descriptor(root, d, prefix)?;
    if selected.is_some_and(|ids| {
        !ids.iter().any(|id| {
            Digest256::of_bytes(id.as_bytes())
                .to_hex()
                .starts_with(prefix)
        })
    }) {
        return Ok(());
    }
    let n = count(d, "count")?;
    if (selected.is_none() || d["kind"] == "data") && n > m.limits.max_rows.saturating_sub(m.rows) {
        return Err(err("source diagnostic partition row budget"));
    }
    let decoded = count(d, "decoded_bytes")?;
    if decoded > m.limits.max_input_bytes.saturating_sub(m.bytes)
        || decoded > m.limits.max_json_bytes as u64
    {
        return Err(err("source diagnostic decoded part budget"));
    }
    let stored = m.read(&path, count(d, "size_bytes")? as usize)?;
    m.charge(decoded)?;
    let raw = tos_compiler::decode_partition_part(
        &stored,
        string(d, "kind")?,
        stored.len(),
        decoded as usize,
        string(d, "sha256")?,
        string(d, "decoded_sha256")?,
    )
    .map_err(err)?;
    m.check()?;
    if d["kind"] == "index" {
        let index = m.parse(&raw)?;
        exact(&index, &["schema_version", "prefix", "count", "children"])?;
        if index["schema_version"] != "tos_projection_partition_index_v1"
            || index["prefix"] != prefix
            || index["count"] != d["count"]
            || prefix.len() >= 64
        {
            return Err(err("source diagnostic partition directory"));
        }
        let children = object(&index["children"])?;
        if children.is_empty() {
            return Err(err("source diagnostic empty partition directory"));
        }
        let mut total = 0u64;
        for (digit, child) in children {
            if digit.len() != 1
                || !digit
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(err("source diagnostic partition branch"));
            }
            descriptor(root, child, &format!("{prefix}{digit}"))?;
            total = total
                .checked_add(count(child, "count")?)
                .ok_or_else(|| err("source diagnostic count overflow"))?;
        }
        if total != n {
            return Err(err("source diagnostic partition count mismatch"));
        }
        for (digit, child) in children {
            visit(
                m,
                root,
                spec,
                child,
                &format!("{prefix}{digit}"),
                out,
                selected,
            )?;
        }
        return Ok(());
    }
    let mut previous = String::new();
    let mut seen = 0u64;
    for line in raw.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
        m.rows = m
            .rows
            .checked_add(1)
            .filter(|r| *r <= m.limits.max_rows)
            .ok_or_else(|| err("source diagnostic row budget"))?;
        let record = m.parse(line)?;
        exact(&record, &["key", "value"])?;
        let key = string(&record, "key")?;
        if key.is_empty()
            || key.len() > 4096
            || key <= previous.as_str()
            || !Digest256::of_bytes(key.as_bytes())
                .to_hex()
                .starts_with(prefix)
        {
            return Err(err("source diagnostic misplaced/unsorted key"));
        }
        let field = &spec["key_field"];
        if field.as_array().is_some_and(Vec::is_empty) {
            if key.len() != 20
                || !key.bytes().all(|b| b.is_ascii_digit())
                || key.parse::<u64>().map_err(err)? >= count(&spec["root"], "count")?
            {
                return Err(err("source diagnostic sequence position"));
            }
        } else if !field.is_null() {
            let value = &record["value"];
            object(value)?;
            let expected = if let Some(f) = field.as_str() {
                value[f]
                    .as_str()
                    .ok_or_else(|| err("source diagnostic key differs from identity"))?
                    .to_owned()
            } else {
                let fields = field.as_array().unwrap();
                let mut values = vec![];
                for f in fields {
                    let s = value[f.as_str().unwrap()]
                        .as_str()
                        .filter(|s| !s.is_empty() && s.len() <= 4096)
                        .ok_or_else(|| err("source diagnostic compound key"))?;
                    values.push(s);
                } // Python json.dumps default separators intentionally include spaces.
                let encoded = values
                    .iter()
                    .map(|s| serde_json::to_string(s).map_err(err))
                    .collect::<Result<Vec<_>>>()?;
                format!("[{}]", encoded.join(", "))
            };
            if expected != key {
                return Err(err("source diagnostic key differs from identity"));
            }
        }
        previous = key.to_owned();
        if selected.is_none_or(|ids| ids.contains(key)) {
            out.push((previous.clone(), record["value"].clone()));
        }
        seen += 1;
    }
    if seen != n {
        return Err(err("source diagnostic partition row count mismatch"));
    }
    Ok(())
}
fn ordered_rows(m: &mut Meter<'_>, spec: &Value, mut rows: Vec<(String, Value)>) -> Result<Value> {
    if spec["key_field"].is_null() {
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        return Ok(Value::Object(rows.into_iter().collect()));
    }
    if spec["key_field"].as_array().is_some_and(Vec::is_empty) {
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        return Ok(Value::Array(rows.into_iter().map(|(_, v)| v).collect()));
    }
    let order = spec["order_fields"].as_array().unwrap();
    let fields = if !order.is_empty() {
        order.clone()
    } else if spec["key_field"].is_array() {
        spec["key_field"].as_array().unwrap().clone()
    } else {
        vec![spec["key_field"].clone()]
    };
    let mut keyed = vec![];
    for (_, v) in rows {
        m.step(1)?;
        let mut key = vec![];
        for f in &fields {
            m.step(1)?;
            let value = &v[f.as_str().unwrap()];
            let s = if let Some(s) = value.as_str() {
                format!("1:{s}")
            } else if let Some(n) = value.as_u64() {
                let s = n.to_string();
                format!("0:{:020}:{s}", s.len())
            } else if let Some(n) = value.as_i64() {
                format!("1:{n}")
            } else if value.is_null() {
                if v.get(f.as_str().unwrap()).is_some() {
                    "1:None".into()
                } else {
                    "1:".into()
                }
            } else if let Some(b) = value.as_bool() {
                format!("1:{}", if b { "True" } else { "False" })
            } else {
                return Err(err("source diagnostic unsupported ordering scalar"));
            };
            key.push(s);
        }
        keyed.push((key, v));
    }
    let key_bytes = keyed.iter().try_fold(0u64, |n, (key, _)| {
        key.iter().try_fold(n, |n, value| {
            n.checked_add(value.len() as u64)
                .ok_or_else(|| err("source diagnostic ordering budget"))
        })
    })?;
    m.step(
        key_bytes
            .checked_mul((keyed.len().max(1).ilog2() + 1) as u64)
            .ok_or_else(|| err("source diagnostic ordering budget"))?,
    )?;
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    m.check()?;
    Ok(Value::Array(keyed.into_iter().map(|(_, v)| v).collect()))
}
struct CappedJson<'a> {
    bytes: Vec<u8>,
    cap: usize,
    deadline: Instant,
    abort: &'a dyn AbortProbe,
}
impl std::io::Write for CappedJson<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if Instant::now() >= self.deadline || self.abort.reason().is_some() {
            return Err(std::io::Error::other("source diagnostic deadline/abort"));
        }
        if bytes.len() > self.cap.saturating_sub(self.bytes.len()) {
            return Err(std::io::Error::other(
                "source diagnostic graph aggregate budget",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Original Driver counters, borrowed remaining-state authority and absolute
/// ceilings. This is not an admission issuer. The callback remains on the
/// original holding thread and is supplied anew to each owned operation.
pub struct OriginalStoreBudget<'a> {
    /// Already established and admitted ONCE by the dedicated native session.
    /// Its complete pool remains with the Driver, never a per-store cache grant.
    pub original_sqlite_heap: Arc<tos_compiler::DedicatedSessionSqliteHeap>,
    pub remaining_after_retained: &'a dyn Fn(usize) -> Result<usize>,
    pub byte_work: Arc<AtomicU64>,
    pub max_byte_work: u64,
    pub sql_vm_steps: Arc<AtomicU64>,
    pub max_sql_vm_steps: u64,
    pub store_sql_vm_steps: Arc<AtomicU64>,
    pub store_steps: Arc<AtomicU64>,
    pub max_store_steps: u64,
    pub json_visits: Arc<std::sync::atomic::AtomicUsize>,
    pub max_json_visits: usize,
    pub max_rows_remaining: u64,
    pub max_input_bytes_remaining: u64,
}

/// Monotonic attempted usage is retained on success AND failure. A failed
/// operation is terminal; neither this report nor any Arc is a refund token.
#[derive(Default, Debug)]
pub struct StoreUsage {
    pub input_bytes: u64,
    pub rows: u64,
    pub json_visits: usize,
}
#[derive(Clone)]
struct OriginalStoreCounters {
    original_sqlite_heap: Arc<tos_compiler::DedicatedSessionSqliteHeap>,
    byte_work: Arc<AtomicU64>,
    max_byte_work: u64,
    sql_vm_steps: Arc<AtomicU64>,
    max_sql_vm_steps: u64,
    store_sql_vm_steps: Arc<AtomicU64>,
    store_steps: Arc<AtomicU64>,
    max_store_steps: u64,
    json_visits: Arc<std::sync::atomic::AtomicUsize>,
    max_json_visits: usize,
    metadata_storage: usize,
    progress_storage: usize,
    poisoned: bool,
}
fn state_add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| owned_err("legacy owned state overflow"))
}
fn state_slots<T>(n: usize) -> Result<usize> {
    n.checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| owned_err("legacy owned container state overflow"))
}
fn atomic_charge(counter: &AtomicU64, cap: u64, amount: u64) -> Result<()> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
            old.checked_add(amount).filter(|value| *value <= cap)
        })
        .map(|_| ())
        .map_err(|_| owned_err("legacy original cumulative allowance"))
}
impl OriginalStoreBudget<'_> {
    fn check(&self, deadline: Instant, abort: &dyn AbortProbe) -> Result<()> {
        if Instant::now() >= deadline || abort.reason().is_some() {
            return Err(owned_err("legacy original deadline/abort"));
        }
        Ok(())
    }
    fn counters(&self, metadata_storage: usize, progress_storage: usize) -> OriginalStoreCounters {
        OriginalStoreCounters {
            original_sqlite_heap: self.original_sqlite_heap.clone(),
            byte_work: self.byte_work.clone(),
            max_byte_work: self.max_byte_work,
            sql_vm_steps: self.sql_vm_steps.clone(),
            max_sql_vm_steps: self.max_sql_vm_steps,
            store_sql_vm_steps: self.store_sql_vm_steps.clone(),
            store_steps: self.store_steps.clone(),
            max_store_steps: self.max_store_steps,
            json_visits: self.json_visits.clone(),
            max_json_visits: self.max_json_visits,
            metadata_storage,
            progress_storage,
            poisoned: false,
        }
    }
    fn bind(&self, counters: &OriginalStoreCounters) -> Result<()> {
        self.original_sqlite_heap
            .verify_current()
            .map_err(owned_err)?;
        if counters.poisoned
            || !Arc::ptr_eq(&self.original_sqlite_heap, &counters.original_sqlite_heap)
            || !Arc::ptr_eq(&self.byte_work, &counters.byte_work)
            || self.max_byte_work != counters.max_byte_work
            || !Arc::ptr_eq(&self.sql_vm_steps, &counters.sql_vm_steps)
            || self.max_sql_vm_steps != counters.max_sql_vm_steps
            || !Arc::ptr_eq(&self.store_sql_vm_steps, &counters.store_sql_vm_steps)
            || !Arc::ptr_eq(&self.store_steps, &counters.store_steps)
            || self.max_store_steps != counters.max_store_steps
            || !Arc::ptr_eq(&self.json_visits, &counters.json_visits)
            || self.max_json_visits != counters.max_json_visits
        {
            return Err(owned_err("legacy original counter association changed"));
        }
        Ok(())
    }
    fn visits(&self, visits: usize, usage: &mut StoreUsage) -> Result<()> {
        // Record attempted visits even if the absolute original ceiling refuses.
        usage.json_visits = usage
            .json_visits
            .checked_add(visits)
            .ok_or_else(|| owned_err("legacy JSON usage overflow"))?;
        self.json_visits
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
                old.checked_add(visits)
                    .filter(|value| *value <= self.max_json_visits)
            })
            .map(|_| ())
            .map_err(|_| owned_err("legacy original JSON visit allowance"))
    }
}

// Rust1.98.1 alloc/collections/btree/node.rs: B=6, eleven key/value
// slots per leaf, twelve child pointers per internal node. Account unused
// slots and field/tail padding, independent of Rust field reordering.
fn serde_map_nodes_upper_bound(entries: usize) -> Result<usize> {
    if entries == 0 {
        return Ok(0);
    }
    let fields = [
        (
            std::mem::size_of::<Option<std::ptr::NonNull<()>>>(),
            std::mem::align_of::<Option<std::ptr::NonNull<()>>>(),
        ),
        (std::mem::size_of::<u16>(), std::mem::align_of::<u16>()),
        (std::mem::size_of::<u16>(), std::mem::align_of::<u16>()),
        (
            std::mem::size_of::<[std::mem::MaybeUninit<String>; 11]>(),
            std::mem::align_of::<String>(),
        ),
        (
            std::mem::size_of::<[std::mem::MaybeUninit<Value>; 11]>(),
            std::mem::align_of::<Value>(),
        ),
        (
            std::mem::size_of::<[std::mem::MaybeUninit<std::ptr::NonNull<()>>; 12]>(),
            std::mem::align_of::<std::ptr::NonNull<()>>(),
        ),
    ];
    let mut node = 0usize;
    let mut alignment = 1usize;
    for (bytes, align) in fields {
        node = state_add(node, state_add(bytes, align - 1)?)?;
        alignment = alignment.max(align);
    }
    node = state_add(node, alignment - 1)?;
    // Every populated node owns at least one key: existing nodes <= entries.
    // Insertion can split at most one node per level, height <= entries, plus
    // one new root. This bounds existing + simultaneous split/root nodes.
    let nodes = entries
        .checked_mul(2)
        .and_then(|n| n.checked_add(1))
        .ok_or_else(|| owned_err("legacy map node count overflow"))?;
    state_slots_bytes(nodes, node)
}
fn state_slots_bytes(count: usize, bytes: usize) -> Result<usize> {
    count
        .checked_mul(bytes)
        .ok_or_else(|| owned_err("legacy typed storage overflow"))
}
// Foundation owns bounded parse storage; each converted/clone map also needs
// actual Rust node allocation geometry, independently of its string heaps.
fn converted_storage(value: &tos_foundation::JsonValue) -> Result<usize> {
    converted_storage_checked(value, &|| Ok(()))
}
fn converted_storage_checked(
    value: &tos_foundation::JsonValue,
    check: &dyn Fn() -> Result<()>,
) -> Result<usize> {
    check()?;
    use tos_foundation::JsonValue as J;
    match value {
        J::Null | J::Bool(_) => Ok(0),
        J::Number(value) => state_add(value.lexeme.capacity(), state_add(value.lexeme.len(), 1)?),
        J::String(value) => value
            .as_str()
            .map(str::len)
            .ok_or_else(|| owned_err("legacy JSON lone surrogate")),
        J::Array(values) => {
            let mut cost = state_slots::<Value>(values.len())?;
            for value in values {
                cost = state_add(cost, converted_storage_checked(value, check)?)?;
            }
            Ok(cost)
        }
        J::Object(values) => {
            let mut cost = serde_map_nodes_upper_bound(values.len())?;
            for (key, value) in values {
                cost = state_add(
                    cost,
                    key.as_str()
                        .ok_or_else(|| owned_err("legacy JSON lone surrogate"))?
                        .len(),
                )?;
                cost = state_add(cost, converted_storage_checked(value, check)?)?;
            }
            Ok(cost)
        }
    }
}
fn exact_string(value: &str) -> Result<String> {
    let mut string = String::new();
    string.try_reserve_exact(value.len()).map_err(owned_err)?;
    string.push_str(value);
    // Reject allocator overcapacity before it can become a retained owner.
    if string.capacity() != value.len() {
        return Err(owned_err("legacy exact string capacity"));
    }
    Ok(string)
}
fn convert_owned(
    value: tos_foundation::JsonValue,
    check: &dyn Fn() -> Result<()>,
) -> Result<Value> {
    convert_borrowed(&value, check)
}
fn convert_borrowed(
    value: &tos_foundation::JsonValue,
    check: &dyn Fn() -> Result<()>,
) -> Result<Value> {
    use tos_foundation::JsonValue as J;
    check()?;
    Ok(match value {
        J::Null => Value::Null,
        J::Bool(value) => Value::Bool(*value),
        // lexeme was fully validated by the genuine PublishedStrict parser;
        // The borrowed and owned DTO routes use the same numeric normalizer;
        // the exact lexeme copy is admitted beside the original tree.
        J::Number(value) => Value::Number(normalized_number(exact_string(&value.lexeme)?)?),
        J::String(value) => Value::String(exact_string(
            value
                .as_str()
                .ok_or_else(|| owned_err("legacy JSON lone surrogate"))?,
        )?),
        J::Array(values) => {
            let mut converted = Vec::new();
            converted
                .try_reserve_exact(values.len())
                .map_err(owned_err)?;
            if converted.capacity() != values.len() {
                return Err(owned_err("legacy exact array capacity"));
            }
            for value in values {
                converted.push(convert_borrowed(value, check)?);
            }
            Value::Array(converted)
        }
        J::Object(values) => {
            let mut converted = Map::new();
            for (key, value) in values {
                check()?;
                let key = exact_string(
                    key.as_str()
                        .ok_or_else(|| owned_err("legacy JSON lone surrogate"))?,
                )?;
                let value = convert_borrowed(value, check)?;
                if converted.insert(key, value).is_some() {
                    return Err(owned_err("legacy duplicate metadata member"));
                }
            }
            Value::Object(converted)
        }
    })
}
fn owned_parse(
    raw: &[u8],
    limits: Limits,
    deadline: Instant,
    abort: &dyn AbortProbe,
    budget: &OriginalStoreBudget<'_>,
    retained: usize,
    usage: &mut StoreUsage,
) -> Result<(Value, usize)> {
    budget.check(deadline, abort)?;
    atomic_charge(&budget.byte_work, budget.max_byte_work, raw.len() as u64)?;
    atomic_charge(
        &budget.store_steps,
        budget.max_store_steps,
        raw.len() as u64,
    )?;
    let remaining_visits = budget
        .max_json_visits
        .checked_sub(budget.json_visits.load(Ordering::Relaxed))
        .ok_or_else(|| owned_err("legacy original JSON visits exhausted"))?;
    let json_limits =
        JsonLimits::new(limits.max_json_bytes, 96, remaining_visits, 4300).map_err(owned_err)?;
    let available = (budget.remaining_after_retained)(retained)?;
    let mut check = || {
        budget.check(deadline, abort).map_err(|_| {
            tos_foundation::FoundationError::new(
                tos_foundation::FoundationErrorCode::BudgetExceeded,
                "legacy original deadline/abort",
            )
        })
    };
    let parsed = tos_foundation::parse_json_with_state_budget_and_check(
        raw,
        JsonMode::PublishedStrict,
        json_limits,
        available,
        &mut check,
    );
    let document = match parsed {
        Ok(document) => document,
        Err(error) => {
            // Parser failure exposes no partial visits: terminal operation
            // consumes its remaining original visit allowance conservatively.
            budget.visits(remaining_visits, usage)?;
            return Err(owned_err(error));
        }
    };
    budget.visits(document.visits(), usage)?;
    // Four genuine owner walks: retained-storage census, conversion state
    // census, conversion work census, and conversion. Reserve their known value visits BEFORE the first walk.
    let walks = (document.visits() as u64)
        .checked_mul(4)
        .ok_or_else(|| owned_err("legacy conversion work overflow"))?;
    atomic_charge(&budget.byte_work, budget.max_byte_work, walks)?;
    atomic_charge(&budget.store_steps, budget.max_store_steps, walks)?;
    let document_state = state_add(
        std::mem::size_of_val(&document),
        document
            .root()
            .retained_storage_bytes()
            .map_err(owned_err)?,
    )?;
    let converted = converted_storage(document.root())?;
    let conversion_work = conversion_work(document.root())?;
    atomic_charge(&budget.byte_work, budget.max_byte_work, conversion_work)?;
    atomic_charge(&budget.store_steps, budget.max_store_steps, conversion_work)?;
    (budget.remaining_after_retained)(state_add(
        state_add(retained, document_state)?,
        state_add(converted, std::mem::size_of::<Value>())?,
    )?)?;
    let value = convert_owned(document.into_root(), &|| budget.check(deadline, abort))?;
    Ok((value, converted))
}

impl LegacyStore {
    /// Open the exact existing five-input cut under the original Driver owners.
    /// This route does not authorize the old generic-query construction kernels.
    pub fn open_bounded_with_owned_budget(
        path: &Path,
        inputs: &[(String, PathBuf)],
        limits: Limits,
        max_database_bytes: u64,
        owner_deadline: Instant,
        owner_abort: Arc<dyn AbortProbe>,
        creation_deadline: Instant,
        operation: Arc<dyn AbortProbe>,
        budget: &OriginalStoreBudget<'_>,
        usage: &mut StoreUsage,
    ) -> Result<Self> {
        if usage.input_bytes != 0 || usage.rows != 0 || usage.json_visits != 0 {
            return Err(owned_err("legacy opening usage must be empty"));
        }
        if creation_deadline > owner_deadline {
            return Err(owned_err("legacy opening cannot renew original deadline"));
        }
        let deadline = creation_deadline.min(owner_deadline);
        let probe = OwnedBorrowedProbe {
            original: owner_abort.as_ref(),
            operation: operation.as_ref(),
        };
        // Existing limit validation is shared; it allocates no collection.
        drop(meter(limits, deadline, &probe)?);
        budget.check(deadline, &probe)?;
        budget
            .original_sqlite_heap
            .verify_current()
            .map_err(owned_err)?;
        if max_database_bytes == 0
            || budget.max_store_steps > limits.max_work_steps
            || budget.max_store_steps == 0
            || budget.max_byte_work == 0
            || budget.max_sql_vm_steps == 0
            || budget.max_json_visits == 0
        {
            return Err(owned_err("legacy original owned limits required"));
        }
        let original_rows = limits.max_rows.min(budget.max_rows_remaining);
        let original_input = limits.max_input_bytes.min(budget.max_input_bytes_remaining);
        // Exact membership without allocating two BTreeSets.
        if inputs.len() != INPUTS.len()
            || INPUTS
                .iter()
                .any(|name| inputs.iter().filter(|(key, _)| key == name).count() != 1)
        {
            return Err(owned_err("query store requires exact five captured inputs"));
        }
        let mut retained = std::mem::size_of::<Self>();
        // Native SQLite allocations/cache are admitted by the original whole
        // DedicatedSessionSqliteHeap pool in Driver, not duplicated here.
        retained = state_add(retained, state_slots::<Held>(INPUTS.len() + 1)?)?;
        for (_, input) in inputs {
            retained = state_add(retained, input.as_os_str().len())?;
        }
        // Database path custody and the original graph_path are distinct clones.
        retained = state_add(retained, path.as_os_str().len())?;
        retained = state_add(retained, path.as_os_str().len())?;
        retained = state_add(
            retained,
            inputs
                .iter()
                .find(|(key, _)| key == INPUTS[1])
                .unwrap()
                .1
                .as_os_str()
                .len(),
        )?;
        let mut fixed = state_add(
            std::mem::size_of::<OriginalStoreBudget<'_>>(),
            std::mem::size_of::<StoreUsage>(),
        )?;
        fixed = state_add(fixed, std::mem::size_of::<OwnedBorrowedProbe<'_>>())?;
        fixed = state_add(fixed, std::mem::size_of::<Instant>())?;
        fixed = state_add(fixed, std::mem::size_of::<Arc<dyn AbortProbe>>())?;
        fixed = state_add(fixed, state_slots::<u8>(65536)?)?;
        fixed = state_add(fixed, state_slots::<Option<Digest256>>(INPUTS.len())?)?;
        fixed = state_add(fixed, std::mem::size_of::<Digest256Hasher>())?;
        fixed = state_add(fixed, owned_metadata_controller_state_upper_bound()?)?;
        // no_journal temporarily owns one exact selected path plus suffix.
        fixed = state_add(fixed, state_add(path.as_os_str().len(), "-journal".len())?)?;
        (budget.remaining_after_retained)(state_add(retained, fixed)?)?;
        let mut held = Vec::new();
        held.try_reserve_exact(INPUTS.len() + 1)
            .map_err(owned_err)?;
        if held.capacity() != INPUTS.len() + 1 {
            return Err(owned_err("legacy exact held capacity"));
        }
        let mut bindings: [Option<Digest256>; 5] = [None; 5];
        for (name, input) in inputs {
            budget.check(deadline, &probe)?;
            let mut selected =
                Held::open_owned(input, original_input.saturating_sub(usage.input_bytes))?;
            usage.input_bytes = usage
                .input_bytes
                .checked_add(selected.stamp.2)
                .filter(|n| *n <= original_input)
                .ok_or_else(|| owned_err("legacy input allowance"))?;
            let mut hasher = Digest256Hasher::new();
            let mut bytes = 0u64;
            let mut chunk = [0u8; 65536];
            loop {
                budget.check(deadline, &probe)?;
                let requested = usize::try_from(selected.stamp.2.saturating_sub(bytes))
                    .unwrap_or(usize::MAX)
                    .min(chunk.len())
                    .max(1);
                // Retain attempted admission on short read/failure; EOF itself
                // is one bounded byte request under the same original ledger.
                atomic_charge(&budget.byte_work, budget.max_byte_work, requested as u64)?;
                let read = selected
                    .file
                    .read(&mut chunk[..requested])
                    .map_err(owned_err)?;
                if read == 0 {
                    break;
                }
                bytes = bytes
                    .checked_add(read as u64)
                    .filter(|n| *n <= selected.stamp.2)
                    .ok_or_else(|| owned_err("legacy input changed during hash"))?;
                hasher.update(&chunk[..read]);
            }
            if bytes != selected.stamp.2 {
                return Err(owned_err("legacy input shortened during hash"));
            }
            selected.verify_owned()?;
            bindings[INPUTS.iter().position(|key| key == name).unwrap()] = Some(hasher.finalize());
            held.push(selected);
        }
        no_journal_owned(path)?;
        let selected_database = Held::open_owned(path, max_database_bytes)?;
        atomic_charge(&budget.sql_vm_steps, budget.max_sql_vm_steps, 1)?;
        atomic_charge(&budget.store_sql_vm_steps, limits.max_sql_vm_steps, 1)?;
        let open_remaining = |additional| {
            let total = state_add(retained, fixed)
                .and_then(|base| state_add(base, additional))
                .map_err(|_| {
                    tos_source_store::StoreError::new(
                        tos_source_store::StoreErrorCode::DescriptorMismatch,
                        "legacy pinned state arithmetic",
                    )
                })?;
            (budget.remaining_after_retained)(total).map_err(|_| {
                tos_source_store::StoreError::new(
                    tos_source_store::StoreErrorCode::DescriptorMismatch,
                    "legacy pinned original remaining state",
                )
            })
        };
        let db = tos_source_store::PinnedSqliteConnection::open_readonly_immutable_with_state(
            &selected_database.file,
            &open_remaining,
        )
        .map_err(owned_err)?;
        retained = state_add(
            retained,
            db.retained_rust_state_upper_bound().map_err(owned_err)?,
        )?;
        let window = OwnedSqlWindow {
            original: owner_abort.clone(),
            operation: Some(operation.clone()),
            deadline,
            sql: budget.sql_vm_steps.clone(),
            original_cap: budget.max_sql_vm_steps,
            store_sql: budget.store_sql_vm_steps.clone(),
            store_cap: limits.max_sql_vm_steps,
        };
        let progress = move || window.next();
        let progress_storage = std::mem::size_of_val(&progress);
        (budget.remaining_after_retained)(state_add(
            state_add(retained, fixed)?,
            progress_storage,
        )?)?;
        db.progress_handler(1, Some(progress));
        // Fixed SQL and typed pragma argument avoid format! String growth.
        db.execute_static_bounded(c"PRAGMA query_only=ON;")
            .map_err(owned_err)?;
        db.execute_static_bounded(c"PRAGMA temp_store=MEMORY;")
            .map_err(owned_err)?;
        db.set_cache_kib_bounded(limits.sqlite_cache_kib)
            .map_err(owned_err)?;
        let mut metadata = Map::new();
        let mut metadata_state = 0usize;
        {
            let mut statement = db.prepare_static_bounded(c"SELECT CASE WHEN typeof(key)='text' AND length(CAST(key AS BLOB))<=4096 THEN key ELSE NULL END, CASE WHEN typeof(value)='text' AND length(CAST(value AS BLOB))<=?1 THEN value ELSE NULL END FROM metadata ORDER BY key").map_err(owned_err)?;
            statement
                .bind_i64(1, limits.max_json_bytes as i64)
                .map_err(owned_err)?;
            while statement.step().map_err(owned_err)? {
                budget.check(deadline, &probe)?;
                usage.rows = usage.rows.saturating_add(1);
                if usage.rows > original_rows {
                    return Err(owned_err("legacy metadata row allowance"));
                }
                // Borrow actual SQLite text, never row.get::<String>() first.
                let key = statement.text(0).map_err(owned_err)?;
                let raw = statement.text(1).map_err(owned_err)?;
                usage.input_bytes = usage.input_bytes.saturating_add(raw.len() as u64);
                if usage.input_bytes > original_input {
                    return Err(owned_err("legacy metadata input allowance"));
                }
                let node_delta = serde_map_nodes_upper_bound(metadata.len() + 1)?
                    .checked_sub(serde_map_nodes_upper_bound(metadata.len())?)
                    .ok_or_else(|| owned_err("legacy metadata node delta"))?;
                let entry = state_add(node_delta, key.len())?;
                let before = state_add(
                    state_add(state_add(retained, fixed)?, progress_storage)?,
                    state_add(metadata_state, entry)?,
                )?;
                (budget.remaining_after_retained)(state_add(before, raw.len())?)?;
                let (value, value_state) = owned_parse(
                    raw.as_bytes(),
                    limits,
                    deadline,
                    &probe,
                    budget,
                    state_add(before, raw.len())?,
                    usage,
                )?;
                let key = exact_string(key)?;
                if metadata.insert(key, value).is_some() {
                    return Err(owned_err("source diagnostic duplicate metadata"));
                }
                metadata_state = state_add(metadata_state, state_add(entry, value_state)?)?;
            }
        }
        let string = |name: &str| metadata.get(name).and_then(Value::as_str);
        if string("schema") != Some("tos_query_store_v1")
            || string("compiler_version") != Some("tos_offline_knowledge_v2")
            || metadata.get("complete").and_then(Value::as_bool) != Some(true)
        {
            return Err(owned_err(
                "query store unsupported/incomplete/stale snapshot",
            ));
        }
        let captured_bindings = metadata
            .get("snapshot_bindings")
            .and_then(Value::as_object)
            .ok_or_else(|| owned_err("query store snapshot bindings object"))?;
        if captured_bindings.len() != INPUTS.len() {
            return Err(owned_err("query store stale snapshot bindings"));
        }
        // Hash formatting uses one fixed 64-byte String at a time, pre-admitted.
        (budget.remaining_after_retained)(state_add(
            state_add(state_add(retained, fixed)?, progress_storage)?,
            state_add(metadata_state, 64)?,
        )?)?;
        for (index, name) in INPUTS.iter().enumerate() {
            if captured_bindings.get(*name).and_then(Value::as_str)
                != Some(bindings[index].as_ref().unwrap().to_hex().as_str())
            {
                return Err(owned_err("query store stale snapshot bindings"));
            }
        }
        // Move the authentic metadata trees; no header/catalog clone at open.
        let graph_header = metadata
            .remove("graph_header")
            .ok_or_else(|| owned_err("missing graph_header"))?;
        object(&graph_header)?;
        let catalog = metadata
            .remove("catalog")
            .ok_or_else(|| owned_err("missing catalog"))?;
        object(&catalog)?;
        let corpus_header = metadata
            .remove("corpus_header")
            .ok_or_else(|| owned_err("missing corpus_header"))?;
        object(&corpus_header)?;
        let revision = string_revision(&metadata)?;
        (budget.remaining_after_retained)(state_add(
            state_add(state_add(retained, fixed)?, progress_storage)?,
            state_add(metadata_state, revision.len())?,
        )?)?;
        let revision = exact_string(revision)?;
        for item in &held {
            item.verify_owned()?;
        }
        selected_database.verify_owned()?;
        no_journal_owned(path)?;
        held.push(selected_database);
        let graph_path = inputs
            .iter()
            .find(|(name, _)| name == INPUTS[1])
            .unwrap()
            .1
            .clone();
        let counters = budget.counters(
            state_add(metadata_state, revision.capacity())?,
            progress_storage,
        );
        // Metadata_state conservatively retains the entire original map census,
        // including removed/dropped members. It is never refunded per call.
        let mut owner = Self {
            revision,
            corpus_header,
            graph_header,
            catalog,
            db,
            held,
            limits,
            deadline: owner_deadline,
            abort: owner_abort.clone(),
            bytes: usage.input_bytes,
            rows: usage.rows,
            work: budget.store_steps.load(Ordering::Relaxed),
            vm: budget.store_sql_vm_steps.clone(),
            graph_path,
            database_path: path.to_owned(),
            view_attempted: false,
            owned: Some(counters),
        };
        // The construction window ends here; retain the original owner lifetime.
        // install_owned_progress admits simultaneous old/new callback storage.
        owner.install_owned_progress(budget, owner_deadline, None, fixed)?;
        budget.check(deadline, &probe)?;
        Ok(owner)
    }
    /// Actual distinct buffers plus conservative original metadata census.
    /// Shared Arc payloads belong to the Driver; only handles live here.
    pub fn retained_state_upper_bound(&self) -> Result<usize> {
        let owned = self
            .owned
            .as_ref()
            .ok_or_else(|| owned_err("legacy store has no original owned census"))?;
        let mut state = state_add(std::mem::size_of::<Self>(), owned.metadata_storage)?;
        state = state_add(
            state,
            self.db
                .retained_rust_state_upper_bound()
                .map_err(owned_err)?,
        )?;
        state = state_add(state, owned.progress_storage)?;
        state = state_add(state, state_slots::<Held>(self.held.capacity())?)?;
        for item in &self.held {
            state = state_add(state, item.path.capacity())?;
        }
        state = state_add(state, self.graph_path.capacity())?;
        state = state_add(state, self.database_path.capacity())?;
        Ok(state)
    }
    pub fn verify_currentness_with_owned_budget(
        &self,
        budget: &OriginalStoreBudget<'_>,
    ) -> Result<()> {
        budget.bind(
            self.owned
                .as_ref()
                .ok_or_else(|| owned_err("legacy original counter owner absent"))?,
        )?;
        budget.check(self.deadline, self.abort.as_ref())?;
        let scratch = state_add(self.database_path.as_os_str().len(), "-journal".len())?;
        let scratch = state_add(scratch, std::mem::size_of::<std::ffi::OsString>())?;
        (budget.remaining_after_retained)(state_add(self.retained_state_upper_bound()?, scratch)?)?;
        for item in &self.held {
            item.verify_owned()?;
        }
        no_journal_owned(&self.database_path)
    }
}
fn string_revision(metadata: &Map<String, Value>) -> Result<&str> {
    metadata
        .get("exploration_revision")
        .and_then(Value::as_str)
        .ok_or_else(|| owned_err("missing exploration_revision"))
}

// Preserve serde_json 1.0.151 arbitrary_precision decode semantics. Its
// maintained scan_exponent lowercases E and supplies an explicit positive sign;
// integral i64/u64 values normalize through the same Number constructors.
fn normalized_number(lexeme: String) -> Result<serde_json::Number> {
    if !lexeme
        .as_bytes()
        .iter()
        .any(|b| matches!(b, b'.' | b'e' | b'E'))
    {
        if let Ok(value) = lexeme.parse::<u64>() {
            return Ok(value.into());
        }
        if let Ok(value) = lexeme.parse::<i64>() {
            return Ok(value.into());
        }
        return Ok(serde_json::Number::from_string_unchecked(lexeme));
    }
    let Some(exponent) = lexeme.find(['e', 'E']) else {
        return Ok(serde_json::Number::from_string_unchecked(lexeme));
    };
    let positive = !matches!(lexeme.as_bytes()[exponent + 1], b'+' | b'-');
    let length = state_add(lexeme.len(), usize::from(positive))?;
    let mut normalized = String::new();
    normalized.try_reserve_exact(length).map_err(owned_err)?;
    if normalized.capacity() != length {
        return Err(owned_err("legacy exact number capacity"));
    }
    normalized.push_str(&lexeme[..exponent]);
    normalized.push('e');
    if positive {
        normalized.push('+');
    }
    normalized.push_str(&lexeme[exponent + 1..]);
    Ok(serde_json::Number::from_string_unchecked(normalized))
}

// These typed controllers belong to THIS operation, not the retired opening
// statement scope. The parser admits depth64; corpus_header adds array/root2.
// Each recursive owner walk is serial, so reserve one largest walk, not one
// independent whole-state grant for every parser/clone/serializer layer.
fn owned_metadata_controller_state_upper_bound() -> Result<usize> {
    let sql =
        tos_source_store::PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound()
            + std::mem::size_of::<[i64; 1]>()
            + std::mem::size_of::<&str>()
            + std::mem::size_of::<Vec<Value>>();
    let walk = owned_tree_controller_frame_bytes();
    let sql = state_add(sql, std::mem::size_of::<[(usize, usize); 6]>())?;
    state_add(sql, state_slots_bytes(66, walk)?)
}

fn owned_tree_controller_frame_bytes() -> usize {
    let walk = std::mem::size_of::<Value>()
        + std::mem::size_of::<Map<String, Value>>()
        + std::mem::size_of::<Vec<Value>>()
        + std::mem::size_of::<String>()
        + std::mem::size_of::<std::collections::btree_map::Iter<'_, String, Value>>()
        + std::mem::size_of::<std::slice::Iter<'_, Value>>()
        + std::mem::size_of::<&Value>()
        + std::mem::size_of::<&dyn Fn() -> Result<()>>();
    walk
}

fn serde_clone_storage(value: &Value, check: &dyn Fn() -> Result<()>) -> Result<usize> {
    check()?;
    match value {
        Value::Null | Value::Bool(_) => Ok(0),
        Value::Number(number) => Ok(number.as_str().len()),
        Value::String(string) => Ok(string.len()),
        Value::Array(values) => {
            let mut state = state_slots::<Value>(values.len())?;
            for value in values {
                state = state_add(state, serde_clone_storage(value, check)?)?;
            }
            Ok(state)
        }
        Value::Object(values) => {
            let mut state = serde_map_nodes_upper_bound(values.len())?;
            for (key, value) in values {
                state = state_add(state, key.len())?;
                state = state_add(state, serde_clone_storage(value, check)?)?;
            }
            Ok(state)
        }
    }
}
fn clone_serde_owned(value: &Value, check: &dyn Fn() -> Result<()>) -> Result<Value> {
    check()?;
    Ok(match value {
        Value::Null => Value::Null,
        Value::Bool(value) => Value::Bool(*value),
        Value::Number(value) => Value::Number(serde_json::Number::from_string_unchecked(
            exact_string(value.as_str())?,
        )),
        Value::String(value) => Value::String(exact_string(value)?),
        Value::Array(values) => {
            let mut output = Vec::new();
            output.try_reserve_exact(values.len()).map_err(owned_err)?;
            if output.capacity() != values.len() {
                return Err(owned_err("legacy exact clone capacity"));
            }
            for value in values {
                output.push(clone_serde_owned(value, check)?);
            }
            Value::Array(output)
        }
        Value::Object(values) => {
            let mut output = Map::new();
            for (key, value) in values {
                output.insert(exact_string(key)?, clone_serde_owned(value, check)?);
            }
            Value::Object(output)
        }
    })
}
impl LegacyStore {
    /// Maintained metadata operations, including the authentic ordered
    /// corpus/graph_views rows. The result remains inside this same held cut.
    fn metadata_result_with_owned_budget(
        &mut self,
        tool: &str,
        budget: &OriginalStoreBudget<'_>,
        usage: &mut StoreUsage,
        call_deadline: Instant,
        operation: Arc<dyn AbortProbe>,
    ) -> Result<(Value, usize)> {
        let outcome = (|| {
            self.verify_currentness_with_owned_budget(budget)?;
            if call_deadline > self.deadline {
                return Err(owned_err("legacy call cannot renew original deadline"));
            }
            // Both hook targets coexist during replacement, accounted before Box.
            self.install_owned_progress(budget, call_deadline, Some(operation.clone()), 0)?;
            let result =
                self.metadata_owned_inner(tool, budget, usage, call_deadline, operation.as_ref());
            let result_state = result.as_ref().map_or(0, |(_, state)| *state);
            let restored = self.install_owned_progress(budget, self.deadline, None, result_state);
            self.work = budget.store_steps.load(Ordering::Relaxed);
            if result.is_err() || restored.is_err() {
                if let Some(owned) = &mut self.owned {
                    owned.poisoned = true;
                }
            }
            match result {
                Err(error) => Err(error),
                Ok((value, state)) => {
                    restored?;
                    Ok((value, state))
                }
            }
        })();
        if outcome.is_err() {
            if let Some(owned) = &mut self.owned {
                owned.poisoned = true;
            }
        }
        outcome
    }
    fn install_owned_progress(
        &mut self,
        budget: &OriginalStoreBudget<'_>,
        deadline: Instant,
        operation: Option<Arc<dyn AbortProbe>>,
        additional: usize,
    ) -> Result<()> {
        budget.bind(
            self.owned
                .as_ref()
                .ok_or_else(|| owned_err("legacy original counter owner absent"))?,
        )?;
        let window = OwnedSqlWindow {
            original: self.abort.clone(),
            operation,
            deadline,
            sql: budget.sql_vm_steps.clone(),
            original_cap: budget.max_sql_vm_steps,
            store_sql: budget.store_sql_vm_steps.clone(),
            store_cap: self.limits.max_sql_vm_steps,
        };
        let progress = move || window.next();
        let state = std::mem::size_of_val(&progress);
        (budget.remaining_after_retained)(state_add(
            self.retained_state_upper_bound()?,
            state_add(additional, state)?,
        )?)?;
        self.db.progress_handler(1, Some(progress));
        self.owned.as_mut().unwrap().progress_storage = state;
        Ok(())
    }
    fn metadata_owned_inner(
        &mut self,
        tool: &str,
        budget: &OriginalStoreBudget<'_>,
        usage: &mut StoreUsage,
        call_deadline: Instant,
        operation: &dyn AbortProbe,
    ) -> Result<(Value, usize)> {
        self.verify_currentness_with_owned_budget(budget)?;
        let probe = OwnedBorrowedProbe {
            original: self.abort.as_ref(),
            operation,
        };
        budget.check(call_deadline, &probe)?;
        if usage.input_bytes != 0 || usage.rows != 0 || usage.json_visits != 0 {
            return Err(owned_err("legacy metadata usage must be empty"));
        }
        let source = match tool {
            "tos_knowledge_catalog" => &self.catalog,
            "tos_knowledge_header" => &self.graph_header,
            "tos_corpus_header" => &self.corpus_header,
            _ => return Err(owned_err("legacy metadata operation required")),
        };
        let retained = state_add(
            self.retained_state_upper_bound()?,
            state_add(
                std::mem::size_of::<OwnedSqlWindow>(),
                state_add(
                    std::mem::size_of::<OwnedBorrowedProbe<'_>>(),
                    owned_metadata_controller_state_upper_bound()?,
                )?,
            )?,
        )?;
        (budget.remaining_after_retained)(retained)?;
        let check = || budget.check(call_deadline, &probe);
        let census = state_add(
            self.owned.as_ref().unwrap().metadata_storage,
            std::mem::size_of::<Value>(),
        )?;
        let planning_work = census
            .checked_mul(2)
            .ok_or_else(|| owned_err("legacy clone planning overflow"))?;
        atomic_charge(
            &budget.byte_work,
            budget.max_byte_work,
            planning_work as u64,
        )?;
        atomic_charge(
            &budget.store_steps,
            budget.max_store_steps,
            planning_work as u64,
        )?;
        let clone_work = serde_clone_work(source, &check)?;
        atomic_charge(&budget.byte_work, budget.max_byte_work, clone_work)?;
        atomic_charge(&budget.store_steps, budget.max_store_steps, clone_work)?;
        let mut output_state = state_add(
            std::mem::size_of::<Value>(),
            serde_clone_storage(source, &check)?,
        )?;
        (budget.remaining_after_retained)(state_add(retained, output_state)?)?;
        // Price both actual owner walks before the clone walk. The first census
        // uses no allocating parser, serializer, String or Vec scratch.
        atomic_charge(&budget.byte_work, budget.max_byte_work, output_state as u64)?;
        atomic_charge(
            &budget.store_steps,
            budget.max_store_steps,
            output_state as u64,
        )?;
        let mut output = clone_serde_owned(source, &|| budget.check(call_deadline, &probe))?;
        if tool == "tos_corpus_header" {
            let mut values = Vec::new();
            let mut statement = self.db.prepare_static_bounded(c"SELECT CASE WHEN typeof(payload)='text' AND length(CAST(payload AS BLOB))<=?1 THEN payload ELSE NULL END FROM raw_records WHERE collection='corpus/graph_views' ORDER BY position").map_err(owned_err)?;
            statement
                .bind_i64(1, self.limits.max_json_bytes as i64)
                .map_err(owned_err)?;
            while statement.step().map_err(owned_err)? {
                self.verify_currentness_with_owned_budget(budget)?;
                budget.check(call_deadline, &probe)?;
                usage.rows = usage
                    .rows
                    .checked_add(1)
                    .ok_or_else(|| owned_err("legacy row usage overflow"))?;
                if usage.rows > budget.max_rows_remaining {
                    return Err(owned_err("legacy original session remaining rows"));
                }
                self.rows = self
                    .rows
                    .checked_add(1)
                    .filter(|n| *n <= self.limits.max_rows)
                    .ok_or_else(|| owned_err("legacy corpus header cumulative rows"))?;
                let raw = statement.text(0).map_err(owned_err)?;
                usage.input_bytes = usage
                    .input_bytes
                    .checked_add(raw.len() as u64)
                    .ok_or_else(|| owned_err("legacy input usage overflow"))?;
                if usage.input_bytes > budget.max_input_bytes_remaining {
                    return Err(owned_err("legacy original session remaining input"));
                }
                self.bytes = self
                    .bytes
                    .checked_add(raw.len() as u64)
                    .filter(|n| *n <= self.limits.max_input_bytes)
                    .ok_or_else(|| owned_err("legacy corpus header cumulative input"))?;
                // Exact vector growth pre-admission includes old and requested
                // new slots while reallocating; no implicit Vec::push growth.
                let slots = state_slots::<Value>(values.len() + 1)?;
                let workspace = state_add(state_add(output_state, raw.len())?, slots)?;
                (budget.remaining_after_retained)(state_add(retained, workspace)?)?;
                let (value, heap) = owned_parse(
                    raw.as_bytes(),
                    self.limits,
                    call_deadline,
                    &probe,
                    budget,
                    state_add(retained, workspace)?,
                    usage,
                )?;
                object(&value)?;
                values.try_reserve_exact(1).map_err(owned_err)?;
                if values.capacity() != values.len() + 1 {
                    return Err(owned_err("legacy exact graph_views capacity"));
                }
                values.push(value);
                output_state =
                    state_add(output_state, state_add(std::mem::size_of::<Value>(), heap)?)?;
            }
            let members = output
                .as_object()
                .ok_or_else(|| owned_err("legacy corpus header object"))?
                .len();
            let node_delta = serde_map_nodes_upper_bound(members + 1)?
                .checked_sub(serde_map_nodes_upper_bound(members)?)
                .ok_or_else(|| owned_err("legacy header map node delta"))?;
            let key_state = state_add(node_delta, "graph_views".len())?;
            (budget.remaining_after_retained)(state_add(
                retained,
                state_add(output_state, key_state)?,
            )?)?;
            output_state = state_add(output_state, key_state)?;
            output
                .as_object_mut()
                .ok_or_else(|| owned_err("legacy corpus header object"))?
                .insert(exact_string("graph_views")?, Value::Array(values));
        }
        self.verify_currentness_with_owned_budget(budget)?;
        Ok((output, output_state))
    }
}

fn conversion_work(value: &tos_foundation::JsonValue) -> Result<u64> {
    conversion_work_checked(value, &|| Ok(()))
}
fn conversion_work_checked(
    value: &tos_foundation::JsonValue,
    check: &dyn Fn() -> Result<()>,
) -> Result<u64> {
    check()?;
    use tos_foundation::JsonValue as J;
    let checked = |a: u64, b: u64| {
        a.checked_add(b)
            .ok_or_else(|| owned_err("legacy conversion work overflow"))
    };
    match value {
        J::Null | J::Bool(_) => Ok(0),
        // normalized_number has an any scan, two integral parse attempts OR
        // exponent find, plus bounded formatting/copy. Five token lengths
        // bound those actual passes; this is work, not an AST state multiplier.
        J::Number(value) => (value.lexeme.len() as u64)
            .checked_add(1)
            .and_then(|n| n.checked_mul(5))
            .ok_or_else(|| owned_err("legacy numeric work overflow")),
        J::String(value) => Ok(value
            .as_str()
            .ok_or_else(|| owned_err("legacy JSON lone surrogate"))?
            .len() as u64),
        J::Array(values) => values.iter().try_fold(0, |sum, value| {
            checked(sum, conversion_work_checked(value, check)?)
        }),
        J::Object(values) => {
            let mut work = 0u64;
            for (key, value) in values {
                let bytes = key
                    .as_str()
                    .ok_or_else(|| owned_err("legacy JSON lone surrogate"))?
                    .len() as u64;
                // Even an unbalanced tree compares no more than every existing
                // key per insertion; no private BTree balancing assumption.
                let key_work = bytes
                    .checked_mul(values.len() as u64 + 1)
                    .ok_or_else(|| owned_err("legacy map comparison work overflow"))?;
                work = checked(
                    work,
                    checked(key_work, conversion_work_checked(value, check)?)?,
                )?;
            }
            Ok(work)
        }
    }
}

struct OwnedBorrowedProbe<'a> {
    original: &'a dyn AbortProbe,
    operation: &'a dyn AbortProbe,
}
impl AbortProbe for OwnedBorrowedProbe<'_> {
    fn reason(&self) -> Option<crate::AbortReason> {
        self.original.reason().or_else(|| self.operation.reason())
    }
}
struct OwnedSqlWindow {
    original: Arc<dyn AbortProbe>,
    operation: Option<Arc<dyn AbortProbe>>,
    deadline: Instant,
    sql: Arc<AtomicU64>,
    original_cap: u64,
    store_sql: Arc<AtomicU64>,
    store_cap: u64,
}
impl OwnedSqlWindow {
    fn next(&self) -> bool {
        // A prepaid first instruction is carried across callback replacements.
        // Both ledgers observe the continuation attempt even if either refuses.
        let original = self
            .sql
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
                old.checked_add(1).filter(|next| *next <= self.original_cap)
            });
        let store = self
            .store_sql
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
                old.checked_add(1).filter(|next| *next <= self.store_cap)
            });
        original.is_err()
            || store.is_err()
            || Instant::now() >= self.deadline
            || self.original.reason().is_some()
            || self
                .operation
                .as_ref()
                .is_some_and(|probe| probe.reason().is_some())
    }
}

impl LegacyStore {
    pub fn metadata_with_owned_budget(
        &mut self,
        tool: &str,
        budget: &OriginalStoreBudget<'_>,
        usage: &mut StoreUsage,
        call_deadline: Instant,
        operation: Arc<dyn AbortProbe>,
    ) -> Result<Value> {
        self.metadata_result_with_owned_budget(tool, budget, usage, call_deadline, operation)
            .map(|(value, _)| value)
    }
    /// Encode the complete authentic metadata result under the same owner. The
    /// caller holds this store and packet through final Reply/send fences.
    pub fn metadata_packet_with_owned_budget(
        &mut self,
        tool: &str,
        budget: &OriginalStoreBudget<'_>,
        usage: &mut StoreUsage,
        call_deadline: Instant,
        operation: Arc<dyn AbortProbe>,
        max_output_bytes: usize,
    ) -> Result<Vec<u8>> {
        let result = (|| {
            if max_output_bytes == 0 || max_output_bytes > self.limits.max_json_bytes {
                return Err(owned_err("legacy metadata original output cap"));
            }
            let (value, value_state) = self.metadata_result_with_owned_budget(
                tool,
                budget,
                usage,
                call_deadline,
                operation.clone(),
            )?;
            self.packet_from_owned_value(
                value,
                value_state,
                budget,
                usage,
                call_deadline,
                operation.as_ref(),
                max_output_bytes,
            )
        })();
        if result.is_err() {
            if let Some(owned) = &mut self.owned {
                owned.poisoned = true;
            }
        }
        result
    }
    fn packet_from_owned_value(
        &self,
        value: Value,
        value_state: usize,
        budget: &OriginalStoreBudget<'_>,
        usage: &mut StoreUsage,
        call_deadline: Instant,
        operation: &dyn AbortProbe,
        max_output_bytes: usize,
    ) -> Result<Vec<u8>> {
        self.packet_from_owned_value_with_controller(
            value,
            value_state,
            budget,
            usage,
            call_deadline,
            operation,
            max_output_bytes,
            owned_metadata_controller_state_upper_bound()?,
        )
    }
    fn packet_from_owned_value_with_controller(
        &self,
        value: Value,
        value_state: usize,
        budget: &OriginalStoreBudget<'_>,
        usage: &mut StoreUsage,
        call_deadline: Instant,
        operation: &dyn AbortProbe,
        max_output_bytes: usize,
        controller_state: usize,
    ) -> Result<Vec<u8>> {
        let probe = OwnedBorrowedProbe {
            original: self.abort.as_ref(),
            operation,
        };
        budget.check(call_deadline, &probe)?;
        let planning_state = state_add(
            self.retained_state_upper_bound()?,
            state_add(value_state, controller_state)?,
        )?;
        (budget.remaining_after_retained)(planning_state)?;
        // Planning is one nonallocating typed tree walk, pre-admitted from
        // its actual retained slots/strings before touching that tree.
        atomic_charge(&budget.byte_work, budget.max_byte_work, value_state as u64)?;
        atomic_charge(
            &budget.store_steps,
            budget.max_store_steps,
            value_state as u64,
        )?;
        let (encoded_bound, nodes) =
            encoded_geometry(&value, &|| budget.check(call_deadline, &probe))?;
        let work = encoded_bound
            .checked_mul(2)
            .and_then(|n| n.checked_add(nodes.checked_mul(2)?))
            .ok_or_else(|| owned_err("legacy metadata output work overflow"))?;
        atomic_charge(&budget.byte_work, budget.max_byte_work, work as u64)?;
        atomic_charge(&budget.store_steps, budget.max_store_steps, work as u64)?;
        budget.visits(
            nodes
                .checked_mul(2)
                .ok_or_else(|| owned_err("legacy output visits overflow"))?,
            usage,
        )?;
        let check = || budget.check(call_deadline, &probe);
        let fixed = state_add(
            std::mem::size_of::<OwnedCount<'_>>(),
            std::mem::size_of::<serde_json::Serializer<OwnedCount<'_>>>(),
        )?;
        let fixed = state_add(fixed, std::mem::size_of::<OwnedBytes<'_>>())?;
        let retained = state_add(
            self.retained_state_upper_bound()?,
            state_add(value_state, state_add(fixed, controller_state)?)?,
        )?;
        (budget.remaining_after_retained)(retained)?;
        let mut count = OwnedCount {
            bytes: 0,
            cap: max_output_bytes,
            check: &check,
        };
        serde_json::to_writer(&mut count, &value).map_err(owned_err)?;
        (budget.remaining_after_retained)(state_add(retained, count.bytes)?)?;
        let mut output = Vec::new();
        output.try_reserve_exact(count.bytes).map_err(owned_err)?;
        if output.capacity() != count.bytes {
            return Err(owned_err("legacy exact output capacity"));
        }
        let mut writer = OwnedBytes {
            bytes: output,
            cap: count.bytes,
            check: &check,
        };
        serde_json::to_writer(&mut writer, &value).map_err(owned_err)?;
        if writer.bytes.len() != count.bytes {
            return Err(owned_err("legacy metadata count/emit mismatch"));
        }
        self.verify_currentness_with_owned_budget(budget)?;
        budget.check(call_deadline, &probe)?;
        Ok(writer.bytes)
    }
}
fn encoded_geometry(value: &Value, check: &dyn Fn() -> Result<()>) -> Result<(usize, usize)> {
    check()?;
    let string = |value: &str| {
        value
            .len()
            .checked_mul(6)
            .and_then(|n| n.checked_add(2))
            .ok_or_else(|| owned_err("legacy JSON escaped size overflow"))
    };
    match value {
        Value::Null => Ok((4, 1)),
        Value::Bool(value) => Ok((if *value { 4 } else { 5 }, 1)),
        Value::Number(value) => Ok((value.as_str().len(), 1)),
        Value::String(value) => Ok((string(value)?, 1)),
        Value::Array(values) => {
            let mut bytes = state_add(2, values.len().saturating_sub(1))?;
            let mut visits = 1;
            for value in values {
                let (n, v) = encoded_geometry(value, check)?;
                bytes = state_add(bytes, n)?;
                visits = state_add(visits, v)?;
            }
            Ok((bytes, visits))
        }
        Value::Object(values) => {
            let mut bytes = state_add(2, values.len().saturating_sub(1))?;
            let mut visits = 1;
            for (key, value) in values {
                let (n, v) = encoded_geometry(value, check)?;
                bytes = state_add(bytes, state_add(state_add(string(key)?, 1)?, n)?)?;
                visits = state_add(visits, state_add(v, 1)?)?;
            }
            Ok((bytes, visits))
        }
    }
}
struct OwnedCount<'a> {
    bytes: usize,
    cap: usize,
    check: &'a dyn Fn() -> Result<()>,
}
impl std::io::Write for OwnedCount<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        (self.check)().map_err(|_| std::io::Error::from(std::io::ErrorKind::Interrupted))?;
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .filter(|n| *n <= self.cap)
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidData))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        (self.check)().map_err(|_| std::io::Error::from(std::io::ErrorKind::Interrupted))
    }
}
struct OwnedBytes<'a> {
    bytes: Vec<u8>,
    cap: usize,
    check: &'a dyn Fn() -> Result<()>,
}
impl std::io::Write for OwnedBytes<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        (self.check)().map_err(|_| std::io::Error::from(std::io::ErrorKind::Interrupted))?;
        let length = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .filter(|n| *n <= self.cap)
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidData))?;
        if length > self.bytes.capacity() {
            return Err(std::io::Error::from(std::io::ErrorKind::OutOfMemory));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        (self.check)().map_err(|_| std::io::Error::from(std::io::ErrorKind::Interrupted))
    }
}

fn serde_clone_work(value: &Value, check: &dyn Fn() -> Result<()>) -> Result<u64> {
    check()?;
    let add = |a: u64, b: u64| {
        a.checked_add(b)
            .ok_or_else(|| owned_err("legacy clone work overflow"))
    };
    match value {
        Value::Null | Value::Bool(_) => Ok(1),
        Value::Number(number) => add(number.as_str().len() as u64, 1),
        Value::String(string) => add(string.len() as u64, 1),
        Value::Array(values) => values
            .iter()
            .try_fold(1, |sum, value| add(sum, serde_clone_work(value, check)?)),
        Value::Object(values) => {
            let mut work = 1;
            for (key, value) in values {
                let comparisons = (key.len() as u64)
                    .checked_mul(values.len() as u64 + 1)
                    .ok_or_else(|| owned_err("legacy clone comparison work overflow"))?;
                work = add(work, add(comparisons, serde_clone_work(value, check)?)?)?;
            }
            Ok(work)
        }
    }
}

fn no_journal_owned(path: &Path) -> Result<()> {
    let capacity = state_add(path.as_os_str().len(), "-journal".len())?;
    let mut named = std::ffi::OsString::new();
    named.try_reserve_exact(capacity).map_err(owned_err)?;
    if named.capacity() != capacity {
        return Err(owned_err("legacy exact sidecar path capacity"));
    }
    for suffix in ["-wal", "-journal", "-shm"] {
        named.clear();
        named.push(path.as_os_str());
        named.push(suffix);
        match fs::symlink_metadata(Path::new(&named)) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => {
                return Err(owned_err(
                    "query store must be an immutable completed snapshot",
                ));
            }
        }
    }
    Ok(())
}

/// Caller admits this finite diagnostic workspace ONCE before opening or
/// invoking an owned operation, including when the ordinary remainder is zero.
/// Three distinct owners can coexist: fixed formatter storage, returned text,
/// and retained cause text in the controlled writer's I/O/serde error chain.
/// The formatter array is included by its actual type, not counted twice.
pub fn owned_store_diagnostic_workspace_bytes() -> usize {
    2 * OWNED_DIAGNOSTIC_BYTES
        + 2 * std::mem::size_of::<DiagnosticError>()
        + std::mem::size_of::<std::io::Error>()
        + std::mem::size_of::<serde_json::Error>()
        + owned_serde_error_impl_upper_bound()
        + std::mem::size_of::<OwnedDiagnosticText>()
}
// serde_json1.0.151 error.rs: ErrorImpl owns ErrorCode + line/column.
// ErrorCode's only payloads are Box<str> and io::Error; the remaining variants
// have no payload. Pointer-sized discriminant and all field/tail padding bound
// even a layout without niche packing. Owned writers use simple ErrorKind,
// so no Custom I/O cause Box or dynamically owned diagnostic message appears.
fn owned_serde_error_impl_upper_bound() -> usize {
    let payload = std::mem::size_of::<Box<str>>().max(std::mem::size_of::<std::io::Error>());
    let align = std::mem::align_of::<Box<str>>()
        .max(std::mem::align_of::<std::io::Error>())
        .max(std::mem::align_of::<usize>());
    payload + std::mem::size_of::<usize>() + align - 1
        + 2 * (std::mem::size_of::<usize>() + std::mem::align_of::<usize>() - 1)
        + align
        - 1
}
const OWNED_DIAGNOSTIC_BYTES: usize = 1024;
struct OwnedDiagnosticText {
    bytes: [u8; OWNED_DIAGNOSTIC_BYTES],
    length: usize,
}
impl std::fmt::Write for OwnedDiagnosticText {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        let mut length = text.len().min(self.bytes.len() - self.length);
        while !text.is_char_boundary(length) {
            length -= 1;
        }
        self.bytes[self.length..self.length + length].copy_from_slice(&text.as_bytes()[..length]);
        self.length += length;
        Ok(())
    }
}
fn owned_err(value: impl std::fmt::Display) -> DiagnosticError {
    let mut text = OwnedDiagnosticText {
        bytes: [0; OWNED_DIAGNOSTIC_BYTES],
        length: 0,
    };
    let _ = std::fmt::write(&mut text, format_args!("{value}"));
    // Formatter copied only complete UTF-8 prefixes; no second decoder tree.
    let text = std::str::from_utf8(&text.bytes[..text.length])
        .unwrap_or("legacy diagnostic UTF-8 refusal");
    DiagnosticError(text.to_owned())
}
