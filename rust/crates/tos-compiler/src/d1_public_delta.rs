//! Bounded row-baseline comparison for the disposable public D1 delta.
//! The full SQL and row baseline remain the fallback; only an exact prior v9
//! baseline with its auxiliary publication can authorize an affected-pair SQL.

use crate::{
    Error, Result,
    d1_public_baseline::{PublicRowIndex, TABLES},
    d1_public_capture::PublicCapture,
    d1_public_metadata::{python_reader_binding_parts, reader_binding_parts},
    d1_public_sql::{MAX_INSERT_ROWS, MAX_ROW_VALUE_BYTES, MAX_STATEMENT_BYTES, quote},
    safe_open,
};
use rusqlite::{Connection, params};
use serde::{
    Deserializer,
    de::{DeserializeSeed, Error as _, MapAccess, Visitor},
};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    os::unix::fs::MetadataExt,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    rc::Rc,
};
use tos_foundation::{Digest256, Digest256Hasher};

const SCHEMA: &str = "tos_cloudflare_edge_read_model_v9";
const AUX_SCHEMA: &str = "tos_d1_lens_auxiliary_publication_v1";
const KEY_BYTES: usize = 2 * (6 * MAX_STATEMENT_BYTES + 4096) + 4096;
const ROW_BYTES: usize = 6 * MAX_ROW_VALUE_BYTES + 4096;
const HEADER_BYTES: usize = 8192;
const CLOCK: &str = "(SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1)";

struct FieldReader<'a> {
    inner: BufReader<File>,
    left: Rc<Cell<usize>>,
    capture: &'a PublicCapture,
    prepaid: usize,
    remaining_file: u64,
    failed: Rc<RefCell<Option<Error>>>,
}
impl Read for FieldReader<'_> {
    fn read(&mut self, target: &mut [u8]) -> io::Result<usize> {
        if target.is_empty() {
            return Ok(0);
        }
        if self.left.get() == 0 {
            return Err(io::Error::other("public D1 prior row value bytes"));
        }
        if self.prepaid == 0 {
            let next = self.remaining_file.min(64 * 1024) as usize;
            if next == 0 {
                return Ok(0);
            }
            self.capture.charge_work(next as u64).map_err(|error| {
                let message = error.to_string();
                *self.failed.borrow_mut() = Some(error);
                io::Error::other(message)
            })?;
            self.prepaid = next;
        }
        let take = target.len().min(self.left.get()).min(self.prepaid);
        let n = self.inner.read(&mut target[..take]).map_err(|error| {
            let message = error.to_string();
            *self.failed.borrow_mut() = Some(Error::Io(error));
            io::Error::other(message)
        })?;
        if n == 0 && self.remaining_file != 0 {
            *self.failed.borrow_mut() = Some(Error::Invalid(
                "public D1 prior baseline changed during import",
            ));
            return Err(io::Error::other(
                "public D1 prior baseline changed during import",
            ));
        }
        self.left.set(self.left.get() - n);
        self.prepaid -= n;
        self.remaining_file -= n as u64;
        Ok(n)
    }
}

#[derive(Default)]
struct PriorHeader {
    schema: Option<String>,
    revision: Option<String>,
    auxiliary: Option<Value>,
    rows: bool,
    count: u64,
}
struct HeaderSeed<'a> {
    db: &'a Connection,
    capture: &'a PublicCapture,
    left: Rc<Cell<usize>>,
    failed: Rc<RefCell<Option<Error>>>,
    header: &'a mut PriorHeader,
}
impl<'de> DeserializeSeed<'de> for HeaderSeed<'_> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<(), D::Error> {
        deserializer.deserialize_map(HeaderVisitor {
            db: self.db,
            capture: self.capture,
            left: self.left,
            failed: self.failed,
            header: self.header,
        })
    }
}
struct HeaderVisitor<'a> {
    db: &'a Connection,
    capture: &'a PublicCapture,
    left: Rc<Cell<usize>>,
    failed: Rc<RefCell<Option<Error>>>,
    header: &'a mut PriorHeader,
}
impl<'de> Visitor<'de> for HeaderVisitor<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("public D1 row baseline")
    }
    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> std::result::Result<(), M::Error> {
        loop {
            self.left.set(HEADER_BYTES);
            let Some(key) = map.next_key::<String>()? else {
                break;
            };
            match key.as_str() {
                "schema" if self.header.schema.is_none() => {
                    self.left.set(HEADER_BYTES);
                    self.header.schema = Some(map.next_value()?);
                }
                "revision" if self.header.revision.is_none() => {
                    self.left.set(HEADER_BYTES);
                    self.header.revision = Some(map.next_value()?);
                }
                "auxiliary_publication" if self.header.auxiliary.is_none() => {
                    self.left.set(131_072);
                    self.header.auxiliary = Some(map.next_value()?);
                }
                "rows" if !self.header.rows => {
                    if self.header.schema.as_deref() != Some(SCHEMA)
                        || self
                            .header
                            .revision
                            .as_deref()
                            .is_none_or(|v| !hex_digest(v))
                        || self.header.auxiliary.is_none()
                    {
                        return Err(M::Error::custom("incompatible public D1 prior baseline"));
                    }
                    self.header.rows = true;
                    map.next_value_seed(TablesSeed {
                        db: self.db,
                        capture: self.capture,
                        left: Rc::clone(&self.left),
                        failed: Rc::clone(&self.failed),
                        header: &mut *self.header,
                    })?;
                }
                _ => {
                    return Err(M::Error::custom(
                        "unknown or duplicate public D1 prior baseline field",
                    ));
                }
            }
        }
        Ok(())
    }
}
struct TablesSeed<'a> {
    db: &'a Connection,
    capture: &'a PublicCapture,
    left: Rc<Cell<usize>>,
    failed: Rc<RefCell<Option<Error>>>,
    header: &'a mut PriorHeader,
}
impl<'de> DeserializeSeed<'de> for TablesSeed<'_> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, de: D) -> std::result::Result<(), D::Error> {
        de.deserialize_map(TablesVisitor {
            db: self.db,
            capture: self.capture,
            left: self.left,
            failed: self.failed,
            header: self.header,
        })
    }
}
struct TablesVisitor<'a> {
    db: &'a Connection,
    capture: &'a PublicCapture,
    left: Rc<Cell<usize>>,
    failed: Rc<RefCell<Option<Error>>>,
    header: &'a mut PriorHeader,
}
impl<'de> Visitor<'de> for TablesVisitor<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("registered D1 tables")
    }
    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> std::result::Result<(), M::Error> {
        let mut seen = std::collections::BTreeSet::new();
        loop {
            self.left.set(HEADER_BYTES);
            let Some(table) = map.next_key::<String>()? else {
                break;
            };
            if !TABLES.iter().any(|(name, _)| *name == table.as_str())
                || !seen.insert(table.clone())
            {
                return Err(M::Error::custom(
                    "unknown or duplicate public D1 prior table",
                ));
            }
            map.next_value_seed(TableSeed {
                db: self.db,
                capture: self.capture,
                left: Rc::clone(&self.left),
                failed: Rc::clone(&self.failed),
                table,
                header: &mut *self.header,
            })?;
        }
        if seen.len() != TABLES.len() {
            return Err(M::Error::custom("incomplete public D1 prior tables"));
        }
        Ok(())
    }
}
struct TableSeed<'a> {
    db: &'a Connection,
    capture: &'a PublicCapture,
    left: Rc<Cell<usize>>,
    failed: Rc<RefCell<Option<Error>>>,
    table: String,
    header: &'a mut PriorHeader,
}
impl<'de> DeserializeSeed<'de> for TableSeed<'_> {
    type Value = ();
    fn deserialize<D: Deserializer<'de>>(self, de: D) -> std::result::Result<(), D::Error> {
        de.deserialize_map(TableVisitor {
            db: self.db,
            capture: self.capture,
            left: self.left,
            failed: self.failed,
            table: self.table,
            header: self.header,
        })
    }
}
struct TableVisitor<'a> {
    db: &'a Connection,
    capture: &'a PublicCapture,
    left: Rc<Cell<usize>>,
    failed: Rc<RefCell<Option<Error>>>,
    table: String,
    header: &'a mut PriorHeader,
}
impl<'de> Visitor<'de> for TableVisitor<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("bounded D1 prior rows")
    }
    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> std::result::Result<(), M::Error> {
        let keys = TABLES
            .iter()
            .find(|(name, _)| *name == self.table.as_str())
            .map(|(_, keys)| *keys)
            .ok_or_else(|| M::Error::custom("unregistered D1 prior table"))?;
        loop {
            self.left.set(KEY_BYTES);
            let Some(key) = map.next_key::<String>()? else {
                break;
            };
            if key.len() > KEY_BYTES {
                return Err(M::Error::custom("D1 prior row key bytes"));
            }
            self.left.set(ROW_BYTES);
            let value: Value = map.next_value()?;
            let digest = value
                .get("digest")
                .and_then(Value::as_str)
                .filter(|v| hex_digest(v))
                .ok_or_else(|| M::Error::custom("D1 prior digest"))?;
            let values = value
                .get("values")
                .and_then(Value::as_array)
                .ok_or_else(|| M::Error::custom("D1 prior key values"))?;
            if value.as_object().is_none_or(|o| o.len() != 2)
                || values.len() != keys.len()
                || values
                    .iter()
                    .any(|v| v.as_str().is_none_or(|v| !key_literal(v)))
            {
                return Err(M::Error::custom("D1 prior key literal"));
            }
            let values_json = serde_json::to_string(values).map_err(M::Error::custom)?;
            if values_json != key || values_json.len() > MAX_STATEMENT_BYTES {
                return Err(M::Error::custom("D1 prior key identity"));
            }
            self.capture
                .charge_work((key.len() + values_json.len() + 128) as u64)
                .map_err(|error| {
                    let message = error.to_string();
                    *self.failed.borrow_mut() = Some(error);
                    M::Error::custom(message)
                })?;
            let sequence = i64::try_from(self.header.count).map_err(M::Error::custom)?;
            self.db.execute("INSERT INTO prior_rows(table_name,sequence,row_key,digest,values_json) VALUES (?1,?2,?3,?4,?5)",
                params![&self.table,sequence,&key,digest,&values_json]).map_err(|error| {
                    let message = error.to_string();
                    *self.failed.borrow_mut() = Some(Error::Sql(error));
                    M::Error::custom(message)
                })?;
            self.header.count = self
                .header
                .count
                .checked_add(1)
                .ok_or_else(|| M::Error::custom("D1 prior row count"))?;
            if self.header.count % 4096 == 0 {
                self.db
                    .execute_batch("COMMIT; BEGIN IMMEDIATE")
                    .map_err(|error| {
                        let message = error.to_string();
                        *self.failed.borrow_mut() = Some(Error::Sql(error));
                        M::Error::custom(message)
                    })?;
            }
        }
        Ok(())
    }
}

fn hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
fn key_literal(value: &str) -> bool {
    if value.contains('\0') {
        return false;
    }
    if value == "NULL" {
        return true;
    }
    if let Some(digits) = value.strip_prefix('-') {
        return !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit());
    }
    if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) {
        return true;
    }
    if value.len() < 2 || !(value.starts_with('\'') && value.ends_with('\'')) {
        return false;
    }
    let inside = &value[1..value.len() - 1];
    let mut bytes = inside.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'\'' && bytes.next() != Some(b'\'') {
            return false;
        }
    }
    true
}

pub(crate) struct PriorProof {
    path: PathBuf,
    digest: Digest256,
    size: u64,
    identity: FileIdentity,
    revision: String,
    top: Value,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    dev: u64,
    ino: u64,
    size: u64,
    mtime: i64,
    mtime_ns: i64,
    ctime: i64,
    ctime_ns: i64,
}
fn identity(path: &Path) -> Result<FileIdentity> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(Error::Invalid("public D1 prior baseline path"));
    }
    Ok(FileIdentity {
        dev: metadata.dev(),
        ino: metadata.ino(),
        size: metadata.size(),
        mtime: metadata.mtime(),
        mtime_ns: metadata.mtime_nsec(),
        ctime: metadata.ctime(),
        ctime_ns: metadata.ctime_nsec(),
    })
}
impl PriorProof {
    pub(crate) fn revision(&self) -> &str {
        &self.revision
    }
    pub(crate) fn top(&self) -> &Value {
        &self.top
    }
    pub(crate) fn recheck(&self, capture: &PublicCapture, max_bytes: u64) -> Result<()> {
        let (digest, size, identity) = hash_file(&self.path, capture, max_bytes)?;
        if digest != self.digest || size != self.size || identity != self.identity {
            return Err(Error::Invalid("public D1 prior baseline changed"));
        }
        Ok(())
    }
}
fn hash_file(
    path: &Path,
    capture: &PublicCapture,
    max_bytes: u64,
) -> Result<(Digest256, u64, FileIdentity)> {
    let before = identity(path)?;
    let mut file = safe_open::open_regular(path, max_bytes)?;
    let opened = file.metadata()?;
    if opened.dev() != before.dev || opened.ino() != before.ino {
        return Err(Error::Invalid("public D1 prior baseline changed"));
    }
    let mut buffer = [0u8; 64 * 1024];
    let mut hash = Digest256Hasher::new();
    let mut size = 0u64;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        size = size
            .checked_add(n as u64)
            .filter(|v| *v <= max_bytes)
            .ok_or(Error::Budget("public D1 prior baseline bytes"))?;
        capture.charge_work(n as u64)?;
        hash.update(&buffer[..n]);
    }
    let after = identity(path)?;
    if after != before || size != before.size {
        return Err(Error::Invalid("public D1 prior baseline changed"));
    }
    Ok((hash.finalize(), size, before))
}

pub(crate) fn load_prior(
    index: &PublicRowIndex,
    runtime: &Path,
    capture: &PublicCapture,
    max_bytes: u64,
) -> Result<Option<PriorProof>> {
    let deployed = runtime.join("read-model.deployed.rows.json");
    let current = runtime.join("read-model.rows.json");
    let path = if deployed.exists() || deployed.is_symlink() {
        deployed
    } else {
        current
    };
    if !path.exists() || path.is_symlink() {
        return Ok(None);
    }
    if identity(&path)?.size > max_bytes {
        return Ok(None);
    }
    let (digest, size, source_identity) = hash_file(&path, capture, max_bytes)?;
    if size == 0 {
        return Ok(None);
    }
    let file = safe_open::open_regular(&path, max_bytes)?;
    let opened = file.metadata()?;
    if opened.dev() != source_identity.dev || opened.ino() != source_identity.ino {
        return Err(Error::Invalid("public D1 prior baseline changed"));
    }
    let db = index.connection();
    db.execute_batch("CREATE TABLE prior_rows(table_name TEXT NOT NULL,sequence INTEGER NOT NULL,row_key TEXT NOT NULL,digest TEXT NOT NULL,values_json TEXT NOT NULL,PRIMARY KEY(table_name,row_key)) WITHOUT ROWID; CREATE INDEX prior_rows_sequence ON prior_rows(table_name,sequence); BEGIN IMMEDIATE")?;
    let left = Rc::new(Cell::new(HEADER_BYTES));
    let failed = Rc::new(RefCell::new(None));
    let reader = FieldReader {
        inner: BufReader::with_capacity(64 * 1024, file),
        left: Rc::clone(&left),
        capture,
        prepaid: 0,
        remaining_file: size,
        failed: Rc::clone(&failed),
    };
    let mut de = serde_json::Deserializer::from_reader(reader);
    let mut header = PriorHeader::default();
    let parsed = (|| {
        HeaderSeed {
            db,
            capture,
            left: Rc::clone(&left),
            failed: Rc::clone(&failed),
            header: &mut header,
        }
        .deserialize(&mut de)?;
        left.set(HEADER_BYTES);
        de.end()
    })();
    if parsed.is_err() || !header.rows || header.schema.as_deref() != Some(SCHEMA) {
        let _ = db.execute_batch("ROLLBACK");
        if let Some(error) = failed.borrow_mut().take() {
            return Err(error);
        }
        db.execute_batch("DROP TABLE IF EXISTS prior_rows")?;
        capture.charge_work(0)?;
        return Ok(None);
    }
    db.execute_batch("COMMIT")?;
    let revision = header
        .revision
        .ok_or(Error::Invalid("public D1 prior revision"))?;
    let auxiliary = header
        .auxiliary
        .ok_or(Error::Invalid("public D1 prior auxiliary"))?;
    let stores = serde_json::json!({"knowledge_compact_lens":"tos_compact_lens_carrier_v1",
        "knowledge_lens_memberships":"tos_lens_membership_index_v1"});
    let top = auxiliary.get("reader_top").cloned().unwrap_or(Value::Null);
    let required_top = [
        "schema",
        "read_model_schema",
        "source_revision",
        "data_revision",
        "graph_schema",
        "normalization_binding",
        "catalog_sha256",
        "row_integrity",
        "authority_boundary",
        "lens_sha256",
    ];
    if auxiliary.as_object().is_none_or(|a| a.len() != 3)
        || auxiliary["schema"] != AUX_SCHEMA
        || auxiliary["stores"] != stores
        || top.as_object().is_none_or(|object| {
            object.len() != required_top.len()
                || required_top.iter().any(|key| !object.contains_key(*key))
        })
        || top["data_revision"] != revision
        || top["read_model_schema"] != SCHEMA
        || top["source_revision"]
            .as_str()
            .is_none_or(|v| v.is_empty() || v.len() > 128)
        || top["catalog_sha256"]
            .as_str()
            .is_none_or(|v| !hex_digest(v))
        || top["lens_sha256"].as_str().is_none_or(|v| !hex_digest(v))
        || reader_binding_parts(&top).is_err()
    {
        db.execute_batch("DROP TABLE prior_rows")?;
        return Ok(None);
    }
    let (again, again_size, again_identity) = hash_file(&path, capture, max_bytes)?;
    if again != digest || again_size != size || again_identity != source_identity {
        return Err(Error::Invalid(
            "public D1 prior baseline changed during import",
        ));
    }
    Ok(Some(PriorProof {
        path,
        digest,
        size,
        identity: source_identity,
        revision,
        top,
    }))
}

pub(crate) struct DeltaOutput {
    pub(crate) path: PathBuf,
    pub(crate) summary: Value,
    pub(crate) prior: PriorProof,
}
struct InsertBatch {
    prefix: String,
    rows: Vec<String>,
    bytes: usize,
}
struct DeltaWriter<'a> {
    path: PathBuf,
    out: BufWriter<File>,
    capture: &'a PublicCapture,
    max_bytes: u64,
    bytes: u64,
    statements: u64,
    keys: Option<InsertBatch>,
    values: Option<InsertBatch>,
    complete: bool,
}
impl DeltaWriter<'_> {
    fn line(&mut self, statement: &str) -> Result<()> {
        if statement.len() > MAX_STATEMENT_BYTES {
            return Err(Error::Budget("public D1 delta statement bytes"));
        }
        self.bytes = self
            .bytes
            .checked_add(statement.len() as u64 + 1)
            .filter(|n| *n <= self.max_bytes)
            .ok_or(Error::Budget("public D1 delta output bytes"))?;
        self.capture.charge_work((statement.len() as u64 + 1) * 2)?;
        self.out.write_all(statement.as_bytes())?;
        self.out.write_all(b"\n")?;
        self.statements += 1;
        Ok(())
    }
    fn flush_one(&mut self, keys: bool) -> Result<()> {
        let batch = if keys {
            self.keys.take()
        } else {
            self.values.take()
        };
        if let Some(batch) = batch {
            self.capture.charge_work(batch.bytes as u64)?;
            self.line(&format!("{}{};", batch.prefix, batch.rows.join(",")))?;
        }
        Ok(())
    }
    fn flush(&mut self) -> Result<()> {
        self.flush_one(true)?;
        self.flush_one(false)
    }
    fn push(&mut self, keys: bool, prefix: &str, row: String) -> Result<()> {
        let slot = if keys {
            &mut self.keys
        } else {
            &mut self.values
        };
        if slot.as_ref().is_some_and(|batch| {
            batch.prefix != prefix
                || batch.rows.len() >= MAX_INSERT_ROWS
                || batch.bytes + row.len() + 1 > MAX_STATEMENT_BYTES
        }) {
            self.flush_one(keys)?;
        }
        let slot = if keys {
            &mut self.keys
        } else {
            &mut self.values
        };
        if slot.is_none() {
            *slot = Some(InsertBatch {
                prefix: prefix.to_owned(),
                rows: Vec::new(),
                bytes: prefix.len() + 1,
            });
        }
        let batch = slot.as_mut().expect("D1 delta batch exists");
        if batch.bytes + row.len() + usize::from(!batch.rows.is_empty()) > MAX_STATEMENT_BYTES {
            return Err(Error::Budget("public D1 delta row statement bytes"));
        }
        batch.bytes += row.len() + usize::from(!batch.rows.is_empty());
        batch.rows.push(row);
        Ok(())
    }
    fn key(&mut self, stage: &str, values_json: &str) -> Result<()> {
        self.capture.charge_work((values_json.len() as u64) * 2)?;
        let values: Vec<String> =
            serde_json::from_str(values_json).map_err(|e| Error::Source(e.to_string()))?;
        if values.iter().any(|v| !key_literal(v)) {
            return Err(Error::Invalid("public D1 delta key literal"));
        }
        let prefix = format!("INSERT INTO {stage}_keys VALUES ");
        self.push(true, &prefix, format!("({})", values.join(", ")))
    }
}
impl Drop for DeltaWriter<'_> {
    fn drop(&mut self) {
        if !self.complete {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn segment(file: &mut File, offset: u64, length: u64, capture: &PublicCapture) -> Result<String> {
    if length == 0 || length > MAX_STATEMENT_BYTES as u64 {
        return Err(Error::Invalid("public D1 delta SQL segment"));
    }
    capture.charge_work(length)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = vec![0u8; length as usize];
    file.read_exact(&mut bytes)?;
    String::from_utf8(bytes).map_err(|e| Error::Source(e.to_string()))
}
fn stage_insert<'a>(statement: &'a str, table: &str) -> Result<(String, &'a str)> {
    let prefix = format!("INSERT INTO {table}_next (");
    let rest = statement
        .strip_prefix(&prefix)
        .ok_or(Error::Invalid("public D1 delta source INSERT"))?;
    let (columns, values) = rest
        .split_once(") VALUES (")
        .ok_or(Error::Invalid("public D1 delta INSERT shape"))?;
    let values = values
        .strip_suffix(");")
        .ok_or(Error::Invalid("public D1 delta INSERT end"))?;
    if columns.is_empty() || values.is_empty() {
        return Err(Error::Invalid("public D1 delta INSERT fields"));
    }
    Ok((columns.to_owned(), values))
}
fn binding_expr(top: &Value) -> Result<String> {
    let (_, prefix, suffix) = reader_binding_parts(top)?;
    Ok(format!("{}||{CLOCK}||{}", quote(&prefix)?, quote(&suffix)?))
}
fn python_binding_expr(top: &Value) -> Result<String> {
    let (_, prefix, suffix) = python_reader_binding_parts(top)?;
    Ok(format!("{}||{CLOCK}||{}", quote(&prefix)?, quote(&suffix)?))
}
fn expected_auxiliary_operations(prior: &Value, next: &Value) -> Result<(String, String)> {
    let old = binding_expr(prior)?;
    let previous_binding = match python_binding_expr(prior) {
        Ok(python) if python != old => format!("(binding=({old}) OR binding=({python}))"),
        Ok(_) | Err(Error::Invalid(_)) => format!("binding=({old})"),
        Err(error) => return Err(error),
    };
    let new = binding_expr(next)?;
    let mut guards = String::new();
    let mut seals = String::new();
    for (state, schema) in [
        (
            "knowledge_compact_lens_state",
            "tos_compact_lens_carrier_v1",
        ),
        (
            "knowledge_lens_membership_state",
            "tos_lens_membership_index_v1",
        ),
    ] {
        guards.push_str(&format!("SELECT CASE WHEN typeof({CLOCK})!='integer' OR {CLOCK}<0 OR {CLOCK}>9007199254740991 OR NOT EXISTS(SELECT 1 FROM {state} WHERE singleton=1 AND schema={} AND {previous_binding} AND valid=1) THEN RAISE(ABORT,'stale lens auxiliary publication') END;",quote(schema)?));
        seals.push_str(&format!("SELECT CASE WHEN typeof({CLOCK})!='integer' OR {CLOCK}<0 OR {CLOCK}>9007199254740991 THEN RAISE(ABORT,'invalid auxiliary successor epoch') END;UPDATE {state} SET binding=({new}),valid=1 WHERE singleton=1;"));
    }
    Ok((guards, seals))
}

pub(crate) fn produce(
    index: &PublicRowIndex,
    prior: PriorProof,
    full_sql: &Path,
    pending: &Path,
    capture: &PublicCapture,
    next_revision: &str,
    next_top: &Value,
    max_bytes: u64,
) -> Result<Option<DeltaOutput>> {
    if !hex_digest(next_revision)
        || next_top["data_revision"] != next_revision
        || pending.exists()
        || pending.is_symlink()
    {
        return Err(Error::Invalid("public D1 delta output identity"));
    }
    let db = index.connection();
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(pending)?;
    let mut writer = DeltaWriter {
        path: pending.to_owned(),
        out: BufWriter::new(file),
        capture,
        max_bytes,
        bytes: 0,
        statements: 0,
        keys: None,
        values: None,
        complete: false,
    };
    let mut sql = safe_open::open_regular(full_sql, max_bytes)?;
    let prefix = format!("tos_delta_{}", &next_revision[..16]);
    let mut changed = 0u64;
    let mut reused = 0u64;
    let mut removed = 0u64;
    let mut changed_counts = vec![0u64; TABLES.len()];
    let mut key_counts = vec![0u64; TABLES.len()];
    for (number, (table, keys)) in TABLES.iter().enumerate() {
        let stage = format!("{prefix}_{table}");
        writer.line(&format!("DROP TABLE IF EXISTS {stage};"))?;
        writer.line(&format!(
            "CREATE TABLE {stage} AS SELECT * FROM {table} WHERE 0;"
        ))?;
        writer.line(&format!("DROP TABLE IF EXISTS {stage}_keys;"))?;
        writer.line(&format!(
            "CREATE TABLE {stage}_keys AS SELECT {} FROM {table} WHERE 0;",
            keys.join(", ")
        ))?;
        writer.line(&format!(
            "CREATE UNIQUE INDEX {stage}_keys_idx ON {stage}_keys ({});",
            keys.join(", ")
        ))?;
        let mut stmt=db.prepare("SELECT n.digest,n.values_json,n.segments_json,p.digest FROM rows AS n LEFT JOIN prior_rows AS p ON p.table_name=n.table_name AND p.row_key=n.row_key WHERE n.table_name=?1 ORDER BY n.sequence")?;
        let mut rows = stmt.query(params![*table])?;
        while let Some(row) = rows.next()? {
            let digest: String = row.get(0)?;
            let values_json: String = row.get(1)?;
            let segments_json: String = row.get(2)?;
            let old: Option<String> = row.get(3)?;
            let lookup_bytes = digest
                .len()
                .checked_add(values_json.len())
                .and_then(|bytes| bytes.checked_add(segments_json.len()))
                .and_then(|bytes| bytes.checked_add(old.as_ref().map_or(0, String::len)))
                .and_then(|bytes| bytes.checked_add(128))
                .ok_or(Error::Budget("public D1 delta lookup work"))?;
            capture.charge_work(lookup_bytes as u64)?;
            if values_json.len() > MAX_STATEMENT_BYTES {
                return Ok(None);
            }
            if old.as_deref() == Some(digest.as_str()) {
                reused += 1;
                continue;
            }
            changed += 1;
            changed_counts[number] += 1;
            key_counts[number] += 1;
            writer.key(&stage, &values_json)?;
            let segments: Vec<(u64, u64)> =
                serde_json::from_str(&segments_json).map_err(|e| Error::Source(e.to_string()))?;
            if segments.is_empty() {
                return Err(Error::Invalid("public D1 delta row SQL absent"));
            }
            if segments.len() == 1 {
                let statement = segment(&mut sql, segments[0].0, segments[0].1, capture)?;
                let (columns, values) = stage_insert(&statement, table)?;
                writer.push(
                    false,
                    &format!("INSERT INTO {stage} ({columns}) VALUES "),
                    format!("({values})"),
                )?;
            } else {
                writer.flush()?;
                for (index, (offset, length)) in segments.into_iter().enumerate() {
                    let statement = segment(&mut sql, offset, length, capture)?;
                    let source = if index == 0 {
                        format!("INSERT INTO {table}_next")
                    } else {
                        format!("UPDATE {table}_next")
                    };
                    let target = if index == 0 {
                        format!("INSERT INTO {stage}")
                    } else {
                        format!("UPDATE {stage}")
                    };
                    let tail = statement
                        .strip_prefix(&source)
                        .ok_or(Error::Invalid("public D1 delta chunk source"))?;
                    writer.line(&format!("{target}{tail}"))?;
                }
            }
        }
        drop(rows);
        drop(stmt);
        let mut stmt=db.prepare("SELECT p.values_json FROM prior_rows AS p LEFT JOIN rows AS n ON n.table_name=p.table_name AND n.row_key=p.row_key WHERE p.table_name=?1 AND n.row_key IS NULL ORDER BY p.sequence")?;
        let mut rows = stmt.query(params![*table])?;
        while let Some(row) = rows.next()? {
            let values_json: String = row.get(0)?;
            capture.charge_work(values_json.len() as u64 + 128)?;
            removed += 1;
            key_counts[number] += 1;
            writer.key(&stage, &values_json)?;
        }
        writer.flush()?;
    }
    writer.line("CREATE TABLE IF NOT EXISTS tos_delta_publications (revision TEXT PRIMARY KEY, base_revision TEXT NOT NULL);")?;
    let publish = format!("{prefix}_publish");
    writer.line(&format!("DROP TRIGGER IF EXISTS {publish};"))?;
    let current = "(SELECT json_extract(group_concat(json_chunk, ''), '$.sha256') FROM (SELECT json_chunk FROM edge_meta WHERE key = 'data_revision' ORDER BY part))";
    let (guards, seals) = expected_auxiliary_operations(prior.top(), next_top)?;
    let mut operations = format!(
        "SELECT CASE WHEN {current} IS NOT '{}' THEN RAISE(ABORT, 'stale delta baseline') END;{guards}",
        prior.revision()
    );
    for (number, (table, keys)) in TABLES.iter().enumerate() {
        if key_counts[number] == 0 {
            continue;
        }
        let stage = format!("{prefix}_{table}");
        operations.push_str(&format!("SELECT CASE WHEN (SELECT count(*) FROM {stage}) != {} OR (SELECT count(*) FROM {stage}_keys) != {} THEN RAISE(ABORT, 'incomplete delta staging') END;",changed_counts[number],key_counts[number]));
        let same = keys
            .iter()
            .map(|key| format!("target.{key} IS changed.{key}"))
            .collect::<Vec<_>>()
            .join(" AND ");
        operations.push_str(&format!("DELETE FROM {table} WHERE rowid IN (SELECT target.rowid FROM {stage}_keys AS changed CROSS JOIN {table} AS target WHERE {same});"));
        operations.push_str(&format!("INSERT INTO {table} SELECT * FROM {stage};"));
    }
    operations.push_str(&seals);
    operations.push_str(&format!("SELECT CASE WHEN {current} IS NOT '{next_revision}' THEN RAISE(ABORT, 'delta revision mismatch') END;"));
    writer.line(&format!("CREATE TRIGGER {publish} AFTER INSERT ON tos_delta_publications WHEN NEW.revision = '{next_revision}' BEGIN {operations} END;"))?;
    writer.line(&format!("INSERT OR REPLACE INTO tos_delta_publications SELECT '{next_revision}', '{}' WHERE {current} IS NOT '{next_revision}';",prior.revision()))?;
    writer.line(&format!("DROP TRIGGER {publish};"))?;
    for (table, _) in TABLES {
        let stage = format!("{prefix}_{table}");
        writer.line(&format!("DROP TABLE {stage};"))?;
        writer.line(&format!("DROP TABLE {stage}_keys;"))?;
    }
    writer.out.flush()?;
    writer.out.get_ref().sync_all()?;
    writer.complete = true;
    let summary = serde_json::json!({"available":true,"base_revision":prior.revision(),
        "target_revision":next_revision,"changed_rows":changed,"reused_rows":reused,
        "removed_rows":removed,"sql_statements":writer.statements,
        "publication":"single-statement-transaction",
        "resume":"replay-staging-and-idempotent-publication"});
    Ok(Some(DeltaOutput {
        path: pending.to_owned(),
        summary,
        prior,
    }))
}
