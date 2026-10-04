//! Read-only maintained source diagnostics. No publication or query authority.
use crate::{
    AbortProbe,
    philosophy_read::{PhilosophyReadBudget, compute_source_philosophy_view_diagnostic},
};
use serde_json::{Map, Value};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};
use tos_foundation::{Digest256, Digest256Hasher, JsonLimits, JsonMode, parse_json};

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
        let file = tos_fd_open::open_absolute_regular(path, cap).map_err(err)?;
        let m = file.metadata().map_err(err)?;
        let h = Self {
            path: path.into(),
            file,
            stamp: stamp(&m),
        };
        h.verify()?;
        Ok(h)
    }
    fn verify(&self) -> Result<()> {
        let m = fs::symlink_metadata(&self.path).map_err(err)?;
        if !m.is_file()
            || m.file_type().is_symlink()
            || stamp(&m) != self.stamp
            || stamp(&self.file.metadata().map_err(err)?) != self.stamp
        {
            return Err(err("source diagnostic input changed"));
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
