//! Read-only maintained source diagnostics. No publication or query authority.
use crate::AbortProbe;
use serde_json::{Map, Value};
use std::{
    fs::{self, File},
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::Instant,
};
use tos_foundation::{JsonLimits, JsonMode, parse_json};

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_input_bytes: u64,
    pub max_json_bytes: usize,
    pub max_rows: u64,
    pub max_work_steps: u64,
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
        || limits.max_work_steps == 0
    {
        return Err(err("source diagnostic limits required"));
    }
    Ok(Meter {
        limits,
        deadline,
        abort,
        bytes: 0,
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
        || !tos_foundation::is_lower_hex(sha, 64)
        || !tos_foundation::is_lower_hex(string(v, "decoded_sha256")?, 64)
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
