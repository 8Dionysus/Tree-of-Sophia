use super::*;
use rusqlite::{Connection, OpenFlags, types::ValueRef};
use serde_json::{Map, Value};
use std::{
    collections::BTreeMap,
    fs::{File, Metadata},
    io::{Read, Seek, SeekFrom},
    os::{fd::AsRawFd, unix::fs::MetadataExt},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tos_foundation::{Digest256Hasher, JsonMode, parse_json};
use tos_validation::{FormatProfile, SchemaBackendProbe, SchemaResource};
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum Root {
    Source,
    Analysis,
    Software,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct ReadingSearchCharge {
    pub opened_files: u64,
    pub file_bytes_read: u64,
    pub fixity_bytes_hashed: u64,
    pub software_bytes_read: u64,
    pub software_fixity_bytes_hashed: u64,
    pub sql_vm_steps: u64,
    pub sql_rows: u64,
    pub sql_decoded_bytes: u64,
    /// Logical serialized bytes admitted for JSON trees/copies, not heap/RSS.
    pub materialized_json_bytes: u64,
    pub work_steps: u64,
    pub schema_validations: u64,
    pub schema_input_bytes: u64,
    pub response_bytes: u64,
    pub path_pin_rechecks: u64,
}
struct Pin {
    path: PathBuf,
    file: File,
    metadata: Metadata,
    digest: String,
}
fn same(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.len() == b.len()
        && a.mode() == b.mode()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
impl Pin {
    fn recheck(&self) -> Result<()> {
        let current = self
            .file
            .metadata()
            .map_err(|_| corrupt("reading inode unavailable"))?;
        let named = std::fs::symlink_metadata(&self.path)
            .map_err(|_| corrupt("reading pathname unavailable"))?;
        if !named.is_file() || !same(&self.metadata, &current) || !same(&self.metadata, &named) {
            return Err(corrupt("reading selected inode or pathname changed"));
        }
        // Reopen with the same shared NO_SYMLINKS traversal to catch replaced
        // parent symlinks, even where the final pathname still reaches our inode.
        let reopened = tos_fd_open::open_absolute_regular(&self.path, self.metadata.len())
            .map_err(|_| corrupt("reading path traversal changed"))?;
        if !same(
            &self.metadata,
            &reopened
                .metadata()
                .map_err(|_| corrupt("reading recheck stat failed"))?,
        ) {
            return Err(corrupt("reading selected path changed"));
        }
        Ok(())
    }
}
struct RootPin {
    path: PathBuf,
    file: File,
    device: u64,
    inode: u64,
}
impl RootPin {
    fn recheck(&self) -> Result<()> {
        let file = tos_fd_open::open_absolute_directory(&self.path)
            .map_err(|_| corrupt("reading root traversal changed"))?;
        for metadata in [self.file.metadata(), file.metadata()] {
            let metadata = metadata.map_err(|_| corrupt("reading root stat failed"))?;
            if metadata.dev() != self.device || metadata.ino() != self.inode {
                return Err(corrupt("reading selected root changed"));
            }
        }
        Ok(())
    }
}
/// Exact body plus retained selected inodes. The API's actual software/data
/// current hold must outlive this value and response flush. Recheck immediately
/// before exposing the body; bytes alone are not a current-authority packet.
pub struct ReadingSearchResult {
    pub body: Vec<u8>,
    pub charge: ReadingSearchCharge,
    pins: Vec<Pin>,
    roots: Vec<RootPin>,
}
impl ReadingSearchResult {
    pub fn recheck(&mut self, probe: &dyn AbortProbe) -> Result<()> {
        for p in &self.roots {
            check(probe)?;
            p.recheck()?;
            self.charge.path_pin_rechecks += 1;
        }
        for p in &self.pins {
            check(probe)?;
            p.recheck()?;
            self.charge.path_pin_rechecks += 1;
        }
        check(probe)
    }
}
pub(super) fn corrupt(message: &'static str) -> ReadingSearchError {
    error(ReadingSearchErrorCode::CorruptSelectedCarrier, message)
}
pub(super) fn budget_error() -> ReadingSearchError {
    error(
        ReadingSearchErrorCode::BudgetExceeded,
        "reading search budget exceeded",
    )
}
pub(super) fn hash(s: &str) -> String {
    tos_foundation::Digest256::of_bytes(s.as_bytes()).to_hex()
}
pub(super) fn string<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v.get(k)
        .and_then(Value::as_str)
        .ok_or_else(|| corrupt("required reading string absent"))
}
pub(super) fn array<'a>(v: &'a Value, k: &str) -> Result<&'a [Value]> {
    v.get(k)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| corrupt("required reading array absent"))
}
pub(super) fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
        Value::Number(n) => n.as_f64().is_none_or(|n| n != 0.0),
    }
}
pub(super) fn list(row: &Value, key: &str, read: &mut Reader<'_>) -> Result<Value> {
    let raw = match row.get(key) {
        None => "[]",
        Some(v) => v
            .as_str()
            .ok_or_else(|| corrupt("reading encoded list is not string"))?,
    };
    let v = read.parse(raw.as_bytes())?;
    if !v.is_array() {
        return Err(corrupt("reading encoded list is not array"));
    }
    Ok(v)
}
pub(super) struct Reader<'a> {
    roots: BTreeMap<Root, PathBuf>,
    pins: BTreeMap<(Root, String), Pin>,
    root_pins: Vec<RootPin>,
    pub(super) budget: ReadingSearchBudget,
    pub(super) probe: &'a dyn AbortProbe,
    sql_probe: Arc<dyn AbortProbe>,
    pub(super) charge: ReadingSearchCharge,
}
impl<'a> Reader<'a> {
    pub(super) fn new(
        roots: &ExplicitReadingRoots,
        _software: &ReadingSoftware,
        budget: ReadingSearchBudget,
        probe: &'a Arc<dyn AbortProbe>,
    ) -> Result<Self> {
        if budget.max_open_files == 0
            || budget.max_file_bytes == 0
            || budget.max_total_file_bytes == 0
            || budget.max_work_steps == 0
            || budget.max_sql_vm_steps == 0
            || budget.max_sql_rows == 0
            || budget.max_sql_decoded_bytes == 0
            || budget.max_materialized_json_bytes == 0
            || budget.max_response_bytes == 0
        {
            return Err(budget_error());
        }
        let roots = BTreeMap::from([
            (Root::Source, roots.source_root.clone()),
            (Root::Analysis, roots.analysis_root.clone()),
        ]);
        if roots.len() >= budget.max_open_files {
            return Err(budget_error());
        }
        let mut root_pins = Vec::new();
        for root in roots.values() {
            let file = tos_fd_open::open_absolute_directory(root).map_err(|e| match e.code {
                tos_fd_open::OpenErrorCode::Io => error(
                    ReadingSearchErrorCode::Unavailable,
                    "explicit reading root unavailable",
                ),
                _ => error(
                    ReadingSearchErrorCode::InvalidRequest,
                    "reading roots must be absolute and contain no symlinks",
                ),
            })?;
            let meta = file
                .metadata()
                .map_err(|_| corrupt("reading root stat failed"))?;
            root_pins.push(RootPin {
                path: root.clone(),
                device: meta.dev(),
                inode: meta.ino(),
                file,
            });
        }
        Ok(Self {
            roots,
            pins: BTreeMap::new(),
            root_pins,
            budget,
            probe: probe.as_ref(),
            sql_probe: probe.clone(),
            charge: ReadingSearchCharge::default(),
        })
    }
    pub(super) fn work(&mut self, n: usize) -> Result<()> {
        check(self.probe)?;
        self.charge.work_steps = self
            .charge
            .work_steps
            .checked_add(n as u64)
            .filter(|n| *n <= self.budget.max_work_steps)
            .ok_or_else(budget_error)?;
        Ok(())
    }
    fn materialize(&mut self, n: usize) -> Result<()> {
        self.charge.materialized_json_bytes = self
            .charge
            .materialized_json_bytes
            .checked_add(n as u64)
            .filter(|n| *n <= self.budget.max_materialized_json_bytes)
            .ok_or_else(budget_error)?;
        self.work(n)
    }
    pub(super) fn admit_value(&mut self, value: &Value) -> Result<()> {
        struct Counter {
            n: usize,
            max: u64,
        }
        impl std::io::Write for Counter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.n = self
                    .n
                    .checked_add(bytes.len())
                    .filter(|n| *n as u64 <= self.max)
                    .ok_or_else(|| std::io::Error::other("reading cohort bytes"))?;
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut counter = Counter {
            n: 0,
            max: self
                .budget
                .max_materialized_json_bytes
                .checked_sub(self.charge.materialized_json_bytes)
                .ok_or_else(budget_error)?,
        };
        serde_json::to_writer(&mut counter, value).map_err(|_| budget_error())?;
        self.materialize(counter.n)
    }
    pub(super) fn clone_value(&mut self, value: &Value) -> Result<Value> {
        self.admit_value(value)?;
        Ok(value.clone())
    }
    fn path(&self, root: Root, reference: &str) -> Result<PathBuf> {
        tos_foundation::RelativePath::parse(reference)
            .map_err(|_| corrupt("reading artifact ref must be root-relative"))?;
        Ok(self.roots[&root].join(reference))
    }
    fn open(&mut self, root: Root, reference: &str, private: bool) -> Result<File> {
        self.work(reference.len())?;
        let key = (root, reference.to_owned());
        if !self.pins.contains_key(&key) {
            if self.pins.len() + self.root_pins.len() >= self.budget.max_open_files {
                return Err(budget_error());
            }
            let path = self.path(root, reference)?;
            let mut file = tos_fd_open::open_absolute_regular(&path, self.budget.max_file_bytes)
                .map_err(|e| match e.code {
                    tos_fd_open::OpenErrorCode::Io => error(
                        ReadingSearchErrorCode::Unavailable,
                        "selected reading artifact not installed",
                    ),
                    tos_fd_open::OpenErrorCode::BudgetExceeded => budget_error(),
                    _ => corrupt("reading artifact path is unsafe"),
                })?;
            let metadata = file
                .metadata()
                .map_err(|_| corrupt("reading artifact stat failed"))?;
            if private && metadata.mode() & 0o7777 != 0o600 {
                return Err(corrupt("private reading artifact must be mode 0600"));
            }
            let mut hasher = Digest256Hasher::new();
            let mut chunk = [0; 65536];
            loop {
                check(self.probe)?;
                let n = file
                    .read(&mut chunk)
                    .map_err(|_| corrupt("reading artifact read failed"))?;
                if n == 0 {
                    break;
                }
                self.charge.fixity_bytes_hashed = self
                    .charge
                    .fixity_bytes_hashed
                    .checked_add(n as u64)
                    .filter(|n| *n <= self.budget.max_total_file_bytes)
                    .ok_or_else(budget_error)?;
                self.work(n)?;
                hasher.update(&chunk[..n]);
            }
            if !same(
                &metadata,
                &file
                    .metadata()
                    .map_err(|_| corrupt("reading artifact stat failed"))?,
            ) {
                return Err(corrupt("reading artifact changed during digest"));
            }
            self.charge.opened_files += 1;
            self.pins.insert(
                key.clone(),
                Pin {
                    path,
                    file,
                    metadata,
                    digest: hasher.finalize().to_hex(),
                },
            );
        }
        let pin = &self.pins[&key];
        if private && pin.metadata.mode() & 0o7777 != 0o600 {
            return Err(corrupt("private reading artifact must be mode 0600"));
        }
        pin.file
            .try_clone()
            .map_err(|_| corrupt("cannot retain reading inode"))
    }
    pub(super) fn open_private_request(&mut self, reference: &str) -> Result<File> {
        self.open(Root::Source, reference, true)
    }
    pub(super) fn file_hash(&mut self, root: Root, reference: &str) -> Result<String> {
        if root == Root::Software {
            let raw = embedded(reference)?;
            self.work(raw.len())?;
            self.charge.software_fixity_bytes_hashed += raw.len() as u64;
            return Ok(tos_foundation::Digest256::of_bytes(raw).to_hex());
        }
        self.open(root, reference, false)?;
        Ok(self.pins[&(root, reference.to_owned())].digest.clone())
    }
    pub(super) fn verified_file(
        &mut self,
        root: Root,
        reference: &str,
        expected: &str,
        private: bool,
    ) -> Result<File> {
        let file = self.open(root, reference, private)?;
        if self.pins[&(root, reference.to_owned())].digest != expected {
            return Err(corrupt("reading artifact fixity mismatch"));
        }
        Ok(file)
    }
    pub(super) fn bytes(&mut self, root: Root, reference: &str, private: bool) -> Result<Vec<u8>> {
        if root == Root::Software {
            let raw = embedded(reference)?;
            self.work(raw.len())?;
            self.charge.software_bytes_read += raw.len() as u64;
            return Ok(raw.to_vec());
        }
        let mut file = self.open(root, reference, private)?;
        let n = file
            .metadata()
            .map_err(|_| corrupt("reading artifact stat failed"))?
            .len();
        self.charge.file_bytes_read = self
            .charge
            .file_bytes_read
            .checked_add(n)
            .filter(|n| *n <= self.budget.max_total_file_bytes)
            .ok_or_else(budget_error)?;
        self.work(usize::try_from(n).map_err(|_| budget_error())?)?;
        file.seek(SeekFrom::Start(0))
            .map_err(|_| corrupt("reading seek failed"))?;
        let mut bytes = Vec::new();
        let mut remaining = n;
        let mut chunk = [0; 65536];
        while remaining > 0 {
            check(self.probe)?;
            let cap =
                usize::try_from(remaining.min(chunk.len() as u64)).map_err(|_| budget_error())?;
            let got = file
                .read(&mut chunk[..cap])
                .map_err(|_| corrupt("reading bytes failed"))?;
            if got == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..got]);
            remaining -= got as u64;
        }
        check(self.probe)?;
        if bytes.len() as u64 != n {
            return Err(corrupt("reading artifact size changed"));
        }
        Ok(bytes)
    }
    pub(super) fn parse(&mut self, raw: &[u8]) -> Result<Value> {
        self.materialize(raw.len())?;
        parse_json(raw, JsonMode::RequestLastWins, self.budget.json).map_err(|e| {
            if e.code == tos_foundation::FoundationErrorCode::BudgetExceeded {
                budget_error()
            } else {
                corrupt("invalid reading JSON")
            }
        })?;
        serde_json::from_slice(raw).map_err(|_| {
            error(
                ReadingSearchErrorCode::Unsupported,
                "reading JSON outside native scalar-string profile",
            )
        })
    }
    pub(super) fn json(&mut self, root: Root, reference: &str, private: bool) -> Result<Value> {
        let raw = self.bytes(root, reference, private)?;
        let v = self.parse(&raw)?;
        if !v.is_object() {
            return Err(corrupt("reading JSON object required"));
        }
        Ok(v)
    }
    pub(super) fn jsonl(&mut self, root: Root, reference: &str) -> Result<Vec<Value>> {
        let raw = self.bytes(root, reference, false)?;
        let text = std::str::from_utf8(&raw).map_err(|_| corrupt("reading JSONL UTF-8 invalid"))?;
        // Python splitlines excludes a final empty row, preserves internal
        // empties and consumes CRLF as one boundary. No silent empty-row skip.
        let mut out = Vec::new();
        let mut start = 0;
        let mut positions = text.char_indices().peekable();
        while let Some((offset, c)) = positions.next() {
            if matches!(
                c,
                '\n' | '\r'
                    | '\u{b}'
                    | '\u{c}'
                    | '\u{1c}'
                    | '\u{1d}'
                    | '\u{1e}'
                    | '\u{85}'
                    | '\u{2028}'
                    | '\u{2029}'
            ) {
                out.push(self.parse(text[start..offset].as_bytes())?);
                start = offset + c.len_utf8();
                if c == '\r' && positions.peek().is_some_and(|(_, c)| *c == '\n') {
                    let (offset, c) = positions.next().unwrap();
                    start = offset + c.len_utf8();
                }
            }
        }
        if start < text.len() {
            out.push(self.parse(text[start..].as_bytes())?);
        }
        Ok(out)
    }
    pub(super) fn database(&mut self, file: &File) -> Result<Connection> {
        self.work(1)?;
        let uri = format!(
            "file:/proc/self/fd/{}?mode=ro&immutable=1",
            file.as_raw_fd()
        );
        let db = Connection::open_with_flags(
            uri,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|_| corrupt("reading SQLite open failed"))?;
        self.sql(&db, "PRAGMA query_only=ON", &[])?;
        self.sql(&db, "PRAGMA trusted_schema=OFF", &[])?;
        self.sql(&db, "PRAGMA mmap_size=0", &[])?;
        Ok(db)
    }
    pub(super) fn sql(&mut self, db: &Connection, sql: &str, args: &[Value]) -> Result<Vec<Value>> {
        self.work(sql.len())?;
        let remaining = self
            .budget
            .max_sql_vm_steps
            .checked_sub(self.charge.sql_vm_steps)
            .filter(|n| *n > 0)
            .ok_or_else(budget_error)?;
        let counter = Arc::new(AtomicU64::new(0));
        let seen = counter.clone();
        let sql_probe = self.sql_probe.clone();
        db.progress_handler(
            1,
            Some(move || {
                sql_probe.reason().is_some() || seen.fetch_add(1, Ordering::Relaxed) >= remaining
            }),
        );
        let result = (|| {
            let mut statement = db
                .prepare(sql)
                .map_err(|_| corrupt("reading SQL prepare failed"))?;
            let names = statement
                .column_names()
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>();
            let params = args
                .iter()
                .map(|v| match v {
                    Value::Null => Ok(rusqlite::types::Value::Null),
                    Value::String(s) => Ok(rusqlite::types::Value::Text(s.clone())),
                    Value::Number(n) => n
                        .as_i64()
                        .map(rusqlite::types::Value::Integer)
                        .ok_or_else(|| corrupt("invalid SQL integer")),
                    _ => Err(corrupt("invalid SQL bind")),
                })
                .collect::<Result<Vec<_>>>()?;
            let mut rows = statement
                .query(rusqlite::params_from_iter(params))
                .map_err(|_| corrupt("reading SQL query failed"))?;
            let mut out = Vec::new();
            while let Some(row) = rows.next().map_err(|_| corrupt("reading SQL row failed"))? {
                self.work(1)?;
                self.charge.sql_rows = self
                    .charge
                    .sql_rows
                    .checked_add(1)
                    .filter(|n| *n <= self.budget.max_sql_rows)
                    .ok_or_else(budget_error)?;
                let mut object = Map::new();
                for (i, name) in names.iter().enumerate() {
                    let raw = row
                        .get_ref(i)
                        .map_err(|_| corrupt("reading SQL column failed"))?;
                    let n = match raw {
                        ValueRef::Null => 0,
                        ValueRef::Integer(_) => 8,
                        ValueRef::Text(s) => s.len(),
                        _ => return Err(corrupt("unexpected reading SQL scalar")),
                    };
                    // Check raw SQLite column payload before any JSON string copy.
                    self.charge.sql_decoded_bytes = self
                        .charge
                        .sql_decoded_bytes
                        .checked_add(n as u64)
                        .filter(|n| *n <= self.budget.max_sql_decoded_bytes)
                        .ok_or_else(budget_error)?;
                    self.materialize(n)?;
                    let value = match raw {
                        ValueRef::Null => Value::Null,
                        ValueRef::Integer(n) => json!(n),
                        ValueRef::Text(s) => json!(
                            std::str::from_utf8(s)
                                .map_err(|_| corrupt("reading SQL text UTF-8 invalid"))?
                        ),
                        _ => return Err(corrupt("unexpected reading SQL scalar")),
                    };
                    object.insert(name.clone(), value);
                }
                out.push(Value::Object(object));
            }
            Ok(out)
        })();
        db.progress_handler(0, None::<fn() -> bool>);
        self.charge.sql_vm_steps = self
            .charge
            .sql_vm_steps
            .checked_add(counter.load(Ordering::Relaxed))
            .ok_or_else(budget_error)?;
        check(self.probe)?;
        if self.charge.sql_vm_steps > self.budget.max_sql_vm_steps {
            return Err(budget_error());
        }
        result
    }
    pub(super) fn emit(&mut self, v: &Value) -> Result<Vec<u8>> {
        struct Capped(Vec<u8>, usize);
        impl std::io::Write for Capped {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                if self.0.len().checked_add(b.len()).is_none_or(|n| n > self.1) {
                    return Err(std::io::Error::other("reading output bytes"));
                }
                self.0.extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut writer = Capped(Vec::new(), self.budget.max_response_bytes);
        serde_json::to_writer(&mut writer, v).map_err(|_| budget_error())?;
        self.work(writer.0.len())?;
        Ok(writer.0)
    }
    pub(super) fn validate(&mut self, reference: &str, raw: &[u8]) -> Result<()> {
        self.work(raw.len())?;
        let schema = self.bytes(Root::Software, reference, false)?;
        let parsed = self.parse(&schema)?;
        let uri = string(&parsed, "$id")?.to_owned();
        // Existing bounded offline backend, no remote schema retrieval.
        let probe = SchemaBackendProbe::new(
            [SchemaResource {
                uri: uri.clone(),
                raw: schema,
            }],
            FormatProfile::LegacyPythonObserved20260923,
        )
        .map_err(|_| corrupt("installed reading schema invalid"))?;
        self.charge.schema_validations += 1;
        self.charge.schema_input_bytes += raw.len() as u64;
        if !probe.is_valid_raw(&uri, raw).map_err(|e| match e {
            tos_validation::SchemaProbeError::BudgetExceeded => budget_error(),
            _ => corrupt("reading schema validation failed"),
        })? {
            return Err(corrupt("reading packet violates installed schema"));
        }
        check(self.probe)
    }
    pub(super) fn finish(self, body: Vec<u8>) -> Result<ReadingSearchResult> {
        let mut result = ReadingSearchResult {
            charge: self.charge,
            body,
            pins: self.pins.into_values().collect(),
            roots: self.root_pins,
        };
        result.charge.response_bytes = result.body.len() as u64;
        result.recheck(self.probe)?;
        Ok(result)
    }
}

fn embedded(reference: &str) -> Result<&'static [u8]> {
    match reference {
        READING_SCHEMA_REF => Ok(include_bytes!(
            "../../../../../ToS/candidate-intake/zarathustra/reading-workbench-v1/reading-search-result.v1.schema.json"
        )),
        CONCEPT_SCHEMA_REF => Ok(include_bytes!(
            "../../../../../ToS/candidate-intake/zarathustra/concept-workbench-v1/concept-search-result.v1.schema.json"
        )),
        REQUEST_SCHEMA_REF => Ok(include_bytes!(
            "../../../../../ToS/candidate-intake/zarathustra/concept-workbench-v1/concept-request.v2.schema.json"
        )),
        READING_ADAPTER_REF => Ok(include_bytes!("../reading_search.rs")),
        CONCEPT_ADAPTER_REF => Ok(include_bytes!("concept.rs")),
        _ => Err(corrupt("software reading ref not embedded")),
    }
}
