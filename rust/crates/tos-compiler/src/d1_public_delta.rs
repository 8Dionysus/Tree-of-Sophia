//! Bounded row-baseline comparison for the disposable public D1 delta.
//! The full SQL and row baseline remain the fallback; only an exact prior v9
//! baseline with its auxiliary publication can authorize an affected-pair SQL.

use crate::{
    Error, Result,
    d1_public_baseline::{PublicRowIndex, TABLES},
    d1_public_capture::PublicCapture,
    d1_public_metadata::{python_reader_binding_parts, reader_binding_parts},
    d1_public_sql::{
        MAX_INSERT_ROWS, MAX_ROW_VALUE_BYTES, MAX_STATEMENT_BYTES, quote, single_insert_body_digest,
    },
    safe_open,
};
use rusqlite::{Connection, params};
use serde::{
    Deserialize, Deserializer,
    de::{DeserializeSeed, Error as _, IgnoredAny, MapAccess, SeqAccess, Visitor},
};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    ops::Range,
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
            // Admit the raw transfer and its decoded string payload before
            // reading. Header structures have their separate field cap.
            self.capture
                .charge_work((next as u64) * 2)
                .map_err(|error| {
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
    tables: std::collections::BTreeSet<String>,
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
        // Historical base-only v9 indexes can omit the two optional stores.
        // This profile is mechanically parsed only; load_prior never grants
        // it a PriorProof or delta without auxiliary publication metadata.
        let historical = self.header.auxiliary.as_ref().is_none_or(Value::is_null);
        if TABLES.iter().any(|(name, _)| {
            !seen.contains(*name)
                && !(historical
                    && matches!(
                        *name,
                        "knowledge_compact_lens" | "knowledge_lens_memberships"
                    ))
        }) {
            return Err(M::Error::custom("incomplete public D1 prior tables"));
        }
        self.header.tables = seen;
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
struct PriorValues(Vec<String>);
impl<'de> Deserialize<'de> for PriorValues {
    fn deserialize<D: Deserializer<'de>>(de: D) -> std::result::Result<Self, D::Error> {
        struct ValuesVisitor;
        impl<'de> Visitor<'de> for ValuesVisitor {
            type Value = PriorValues;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("at most four D1 key literals")
            }
            fn visit_seq<S: SeqAccess<'de>>(
                self,
                mut seq: S,
            ) -> std::result::Result<Self::Value, S::Error> {
                let mut values = Vec::with_capacity(4);
                while values.len() < 4 {
                    let Some(value) = seq.next_element::<String>()? else {
                        return Ok(PriorValues(values));
                    };
                    values.push(value);
                }
                if seq.next_element::<IgnoredAny>()?.is_some() {
                    return Err(S::Error::custom("D1 prior key field count"));
                }
                Ok(PriorValues(values))
            }
        }
        de.deserialize_seq(ValuesVisitor)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PriorRow {
    digest: String,
    values: PriorValues,
}
struct EqualJson<'a> {
    expected: &'a [u8],
    offset: usize,
}
impl Write for EqualJson<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let end = self
            .offset
            .checked_add(bytes.len())
            .filter(|end| *end <= self.expected.len())
            .ok_or_else(|| io::Error::other("D1 prior key identity"))?;
        if &self.expected[self.offset..end] != bytes {
            return Err(io::Error::other("D1 prior key identity"));
        }
        self.offset = end;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
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
            self.capture
                .charge_work(std::mem::size_of::<String>() as u64)
                .map_err(|error| {
                    let message = error.to_string();
                    *self.failed.borrow_mut() = Some(error);
                    M::Error::custom(message)
                })?;
            let Some(key) = map.next_key::<String>()? else {
                break;
            };
            if key.len() > KEY_BYTES {
                return Err(M::Error::custom("D1 prior row key bytes"));
            }
            self.left.set(ROW_BYTES);
            let fixed_state = std::mem::size_of::<PriorRow>() + 4 * std::mem::size_of::<String>();
            self.capture
                .charge_work(fixed_state as u64)
                .map_err(|error| {
                    let message = error.to_string();
                    *self.failed.borrow_mut() = Some(error);
                    M::Error::custom(message)
                })?;
            let value: PriorRow = map.next_value()?;
            if !hex_digest(&value.digest) {
                return Err(M::Error::custom("D1 prior digest"));
            }
            let values = &value.values.0;
            if values.len() != keys.len() || values.iter().any(|value| !key_literal(value)) {
                return Err(M::Error::custom("D1 prior key literal"));
            }
            if key.len() > MAX_STATEMENT_BYTES {
                return Err(M::Error::custom("D1 prior key identity"));
            }
            self.capture
                .charge_work(key.len() as u64 + 128)
                .map_err(|error| {
                    let message = error.to_string();
                    *self.failed.borrow_mut() = Some(error);
                    M::Error::custom(message)
                })?;
            let mut equal = EqualJson {
                expected: key.as_bytes(),
                offset: 0,
            };
            serde_json::to_writer(&mut equal, values).map_err(M::Error::custom)?;
            if equal.offset != key.len() {
                return Err(M::Error::custom("D1 prior key identity"));
            }
            let sequence = i64::try_from(self.header.count).map_err(M::Error::custom)?;
            self.db.execute("INSERT INTO prior_rows(table_name,sequence,row_key,digest,values_json) VALUES (?1,?2,?3,?4,?5)",
                params![&self.table,sequence,&key,&value.digest,&key]).map_err(|error| {
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
    if before.size > max_bytes {
        return Err(Error::Budget("public D1 prior baseline bytes"));
    }
    // Admit the whole already measured read before the first kernel read.
    capture.charge_work(
        before
            .size
            .checked_add(1)
            .ok_or(Error::Budget("public D1 prior baseline read bytes"))?,
    )?;
    let mut buffer = [0u8; 64 * 1024];
    let mut hash = Digest256Hasher::new();
    let mut size = 0u64;
    loop {
        let remaining = before.size.saturating_sub(size);
        let take = remaining.saturating_add(1).min(buffer.len() as u64) as usize;
        let n = file.read(&mut buffer[..take])?;
        if n == 0 {
            break;
        }
        size = size
            .checked_add(n as u64)
            .filter(|v| *v <= before.size)
            .ok_or(Error::Invalid("public D1 prior baseline changed"))?;
        capture.charge_work(0)?;
        hash.update(&buffer[..n]);
    }
    let after = identity(path)?;
    if after != before || size != before.size {
        return Err(Error::Invalid("public D1 prior baseline changed"));
    }
    Ok((hash.finalize(), size, before))
}

#[derive(Default)]
pub(crate) struct PriorLoad {
    pub proof: Option<PriorProof>,
    pub auxiliary_migration: bool,
}

pub(crate) fn load_prior(
    index: &PublicRowIndex,
    runtime: &Path,
    capture: &PublicCapture,
    max_bytes: u64,
) -> Result<PriorLoad> {
    let deployed = runtime.join("read-model.deployed.rows.json");
    let current = runtime.join("read-model.rows.json");
    let path = if deployed.exists() || deployed.is_symlink() {
        deployed
    } else {
        current
    };
    if !path.exists() || path.is_symlink() {
        return Ok(PriorLoad::default());
    }
    if identity(&path)?.size > max_bytes {
        return Ok(PriorLoad::default());
    }
    let (digest, size, source_identity) = hash_file(&path, capture, max_bytes)?;
    if size == 0 {
        return Ok(PriorLoad::default());
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
        return Ok(PriorLoad::default());
    }
    db.execute_batch("COMMIT")?;
    let revision = header
        .revision
        .ok_or(Error::Invalid("public D1 prior revision"))?;
    let (again, again_size, again_identity) = hash_file(&path, capture, max_bytes)?;
    if again != digest || again_size != size || again_identity != source_identity {
        return Err(Error::Invalid(
            "public D1 prior baseline changed during import",
        ));
    }
    // A mechanically valid historical v9 row baseline cannot admit a delta
    // without the optional-store publication proof. Preserve full SQL fallback
    // and report the maintained initial-migration disposition explicitly.
    let auxiliary = match header.auxiliary {
        None | Some(Value::Null) => {
            db.execute_batch("DROP TABLE prior_rows")?;
            return Ok(PriorLoad {
                proof: None,
                auxiliary_migration: true,
            });
        }
        Some(value) => value,
    };
    // JSON member order cannot turn the historical parse allowance into a
    // complete optional-store proof: auxiliary metadata may follow rows.
    if header.tables.len() != TABLES.len() {
        db.execute_batch("DROP TABLE prior_rows")?;
        return Err(Error::Invalid(
            "public D1 prior auxiliary table completeness",
        ));
    }
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
        return Err(Error::Invalid("public D1 prior auxiliary publication"));
    }
    Ok(PriorLoad {
        proof: Some(PriorProof {
            path,
            digest,
            size,
            identity: source_identity,
            revision,
            top,
        }),
        auxiliary_migration: false,
    })
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
            // The queued rows, joined rows and final statement coexist until
            // the write completes. Admit both new copies before constructing.
            self.capture.charge_work(
                (batch.bytes as u64)
                    .checked_mul(2)
                    .ok_or(Error::Budget("public D1 delta batch materialization"))?,
            )?;
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
    fn key(&mut self, stage: &str, values_json: &str) -> Result<bool> {
        let decoded_state = (values_json.len() as u64)
            .checked_add(
                (std::mem::size_of::<Vec<String>>() + 4 * std::mem::size_of::<String>()) as u64,
            )
            .ok_or(Error::Budget("public D1 delta key state"))?;
        self.capture.charge_work(decoded_state)?;
        let PriorValues(values) =
            serde_json::from_str(values_json).map_err(|e| Error::Source(e.to_string()))?;
        if values.iter().any(|v| !key_literal(v)) {
            return Err(Error::Invalid("public D1 delta key literal"));
        }
        let value_bytes = values
            .iter()
            .try_fold(2usize, |total, value| {
                total
                    .checked_add(value.len())
                    .ok_or(Error::Budget("public D1 delta key bytes"))
            })?
            .checked_add(values.len().saturating_sub(1) * 2)
            .ok_or(Error::Budget("public D1 delta key bytes"))?;
        let prefix_bytes = "INSERT INTO ".len() + stage.len() + "_keys VALUES ".len();
        let statement_bytes = prefix_bytes
            .checked_add(value_bytes)
            .and_then(|bytes| bytes.checked_add(1))
            .ok_or(Error::Budget("public D1 delta key bytes"))?;
        if statement_bytes > MAX_STATEMENT_BYTES {
            return Ok(false);
        }
        self.capture.charge_work(
            (value_bytes as u64)
                .checked_mul(2)
                .and_then(|bytes| bytes.checked_add(prefix_bytes as u64))
                .ok_or(Error::Budget("public D1 delta key materialization"))?,
        )?;
        let prefix = format!("INSERT INTO {stage}_keys VALUES ");
        self.push(true, &prefix, format!("({})", values.join(", ")))?;
        Ok(true)
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
fn stage_insert<'a>(statement: &'a str, table: &str) -> Result<(&'a str, &'a str)> {
    let rest = source_insert_rest(statement, table)?;
    let (columns, values) = rest
        .split_once(") VALUES (")
        .ok_or(Error::Invalid("public D1 delta INSERT shape"))?;
    let values = values
        .strip_suffix(");")
        .ok_or(Error::Invalid("public D1 delta INSERT end"))?;
    if columns.is_empty() || values.is_empty() {
        return Err(Error::Invalid("public D1 delta INSERT fields"));
    }
    Ok((columns, values))
}

fn source_insert_rest<'a>(statement: &'a str, table: &str) -> Result<&'a str> {
    statement
        .strip_prefix("INSERT INTO ")
        .and_then(|rest| rest.strip_prefix(table))
        .and_then(|rest| rest.strip_prefix("_next ("))
        .ok_or(Error::Invalid("public D1 delta source INSERT"))
}

fn parse_decimal(bytes: &[u8], cursor: &mut usize) -> Result<u64> {
    let start = *cursor;
    let mut value = 0u64;
    while let Some(byte) = bytes.get(*cursor).filter(|byte| byte.is_ascii_digit()) {
        value = value
            .checked_mul(10)
            .and_then(|number| number.checked_add((*byte - b'0') as u64))
            .ok_or(Error::Invalid("public D1 delta segment number"))?;
        *cursor += 1;
    }
    if *cursor == start {
        return Err(Error::Invalid("public D1 delta segment number"));
    }
    if *cursor - start > 1 && bytes[start] == b'0' {
        return Err(Error::Invalid("public D1 delta segment number"));
    }
    Ok(value)
}

/// Inspect the compact numeric segment JSON without allocating. The row
/// index writes this exact serde form; a multi-segment chunked row returns
/// `None` and follows its established extraction path only when changed.
fn single_segment_json(value: &str) -> Result<Option<(u64, u64)>> {
    let bytes = value.as_bytes();
    if bytes.first() != Some(&b'[') {
        return Err(Error::Invalid("public D1 delta SQL segments"));
    }
    let mut cursor = 1usize;
    let mut count = 0usize;
    let mut single = None;
    loop {
        if bytes.get(cursor) != Some(&b'[') {
            return Err(Error::Invalid("public D1 delta SQL segments"));
        }
        cursor += 1;
        let offset = parse_decimal(bytes, &mut cursor)?;
        if bytes.get(cursor) != Some(&b',') {
            return Err(Error::Invalid("public D1 delta SQL segments"));
        }
        cursor += 1;
        let length = parse_decimal(bytes, &mut cursor)?;
        if bytes.get(cursor) != Some(&b']') || length == 0 || length > MAX_STATEMENT_BYTES as u64 {
            return Err(Error::Invalid("public D1 delta SQL segments"));
        }
        offset
            .checked_add(length)
            .ok_or(Error::Invalid("public D1 delta SQL segment range"))?;
        cursor += 1;
        count = count
            .checked_add(1)
            .filter(|count| *count <= MAX_INSERT_ROWS)
            .ok_or(Error::Invalid("public D1 delta SQL segment count"))?;
        if count == 1 {
            single = Some((offset, length));
        }
        match bytes.get(cursor) {
            Some(b',') => cursor += 1,
            Some(b']') => {
                cursor += 1;
                break;
            }
            _ => return Err(Error::Invalid("public D1 delta SQL segments")),
        }
    }
    if count == 0 || cursor != bytes.len() {
        return Err(Error::Invalid("public D1 delta SQL segments"));
    }
    Ok((count == 1).then_some(single.expect("one SQL segment")))
}

fn batch_columns(table: &str) -> Option<&'static [&'static str]> {
    match table {
        "knowledge_lens_memberships" => Some(&["kind", "field", "value", "id", "sort_key"]),
        "knowledge_search_grams" => Some(&["kind", "n", "gram", "position"]),
        "knowledge_search_gram_stats" => Some(&["kind", "n", "gram", "postings"]),
        _ => None,
    }
}

fn joined_fields_match(text: &str, fields: &[&str]) -> bool {
    let mut cursor = 0usize;
    for (index, field) in fields.iter().enumerate() {
        if index != 0 {
            if text.as_bytes().get(cursor) != Some(&b',') {
                return false;
            }
            cursor += 1;
        }
        let Some(end) = cursor.checked_add(field.len()) else {
            return false;
        };
        if text.get(cursor..end) != Some(*field) {
            return false;
        }
        cursor = end;
    }
    cursor == text.len()
}

struct CachedInsertBatch {
    offset: u64,
    length: u64,
    statement: String,
    columns: Range<usize>,
    rows: Vec<Range<usize>>,
}

fn scan_tuple(statement: &str, open: usize) -> Result<(Range<usize>, usize)> {
    let bytes = statement.as_bytes();
    if bytes.get(open) != Some(&b'(') {
        return Err(Error::Invalid("public D1 delta batch tuple"));
    }
    let mut cursor = open + 1;
    let start = cursor;
    let mut nested = 0usize;
    let mut quoted = false;
    while let Some(byte) = bytes.get(cursor).copied() {
        if byte == b'\'' {
            if quoted && bytes.get(cursor + 1) == Some(&b'\'') {
                cursor += 2;
                continue;
            }
            quoted = !quoted;
        } else if !quoted {
            match byte {
                b'(' => {
                    nested = nested
                        .checked_add(1)
                        .ok_or(Error::Invalid("public D1 delta batch nesting"))?;
                }
                b')' if nested == 0 => return Ok((start..cursor, cursor + 1)),
                b')' => nested -= 1,
                _ => {}
            }
        }
        cursor += 1;
    }
    Err(Error::Invalid("public D1 delta batch tuple end"))
}

fn parse_insert_batch(
    statement: String,
    table: &str,
    offset: u64,
    length: u64,
    capture: &PublicCapture,
) -> Result<CachedInsertBatch> {
    let expected = batch_columns(table).ok_or(Error::Invalid("public D1 delta batch table"))?;
    let parser_state = MAX_INSERT_ROWS
        .checked_mul(std::mem::size_of::<Range<usize>>())
        .ok_or(Error::Budget("public D1 delta batch parser state"))?;
    capture.charge_work(
        (statement.len() as u64)
            .checked_add(parser_state as u64)
            .ok_or(Error::Budget("public D1 delta batch parser state"))?,
    )?;
    let prefix_bytes = "INSERT INTO "
        .len()
        .checked_add(table.len())
        .and_then(|bytes| bytes.checked_add("_next (".len()))
        .ok_or(Error::Budget("public D1 delta batch prefix"))?;
    let rest = source_insert_rest(&statement, table)
        .map_err(|_| Error::Invalid("public D1 delta batch INSERT"))?;
    let (columns, tuples) = rest
        .split_once(") VALUES ")
        .ok_or(Error::Invalid("public D1 delta batch columns"))?;
    if !joined_fields_match(columns, expected) || !statement.ends_with(';') {
        return Err(Error::Invalid("public D1 delta batch columns"));
    }
    if statement
        .len()
        .checked_add(1)
        .filter(|bytes| *bytes <= MAX_STATEMENT_BYTES)
        .is_none()
    {
        return Err(Error::Budget("public D1 delta batch statement bytes"));
    }
    let columns_start = prefix_bytes;
    let columns_end = columns_start
        .checked_add(columns.len())
        .ok_or(Error::Budget("public D1 delta batch columns"))?;
    let columns = columns_start..columns_end;
    let mut rows = Vec::with_capacity(MAX_INSERT_ROWS);
    let mut cursor = statement.len() - tuples.len();
    loop {
        if rows.len() >= MAX_INSERT_ROWS {
            return Err(Error::Invalid("public D1 delta batch row count"));
        }
        let (row, after_tuple) = scan_tuple(&statement, cursor)?;
        rows.push(row);
        match statement.as_bytes().get(after_tuple) {
            Some(b',') => cursor = after_tuple + 1,
            Some(b';') if after_tuple + 1 == statement.len() => break,
            _ => return Err(Error::Invalid("public D1 delta batch separator")),
        }
    }
    Ok(CachedInsertBatch {
        offset,
        length,
        statement,
        columns,
        rows,
    })
}

struct ActiveSegmentGroup {
    offset: u64,
    length: u64,
    rows: usize,
    cached: Option<CachedInsertBatch>,
}

fn finish_segment_group(group: &mut Option<ActiveSegmentGroup>) -> Result<()> {
    if let Some(group) = group.take() {
        if group
            .cached
            .as_ref()
            .is_some_and(|cached| cached.rows.len() != group.rows)
        {
            return Err(Error::Invalid("public D1 delta batch index cardinality"));
        }
    }
    Ok(())
}

fn add_segment_row(
    group: &mut Option<ActiveSegmentGroup>,
    segment: Option<(u64, u64)>,
) -> Result<Option<usize>> {
    if group
        .as_ref()
        .is_some_and(|active| segment != Some((active.offset, active.length)))
    {
        finish_segment_group(group)?;
    }
    let Some((offset, length)) = segment else {
        return Ok(None);
    };
    if group.is_none() {
        *group = Some(ActiveSegmentGroup {
            offset,
            length,
            rows: 0,
            cached: None,
        });
    }
    let active = group.as_mut().expect("active SQL segment group");
    let ordinal = active.rows;
    active.rows = active
        .rows
        .checked_add(1)
        .filter(|count| *count <= MAX_INSERT_ROWS)
        .ok_or(Error::Invalid("public D1 delta batch index cardinality"))?;
    Ok(Some(ordinal))
}

fn scan_fields(text: &str, capture: &PublicCapture) -> Result<Vec<Range<usize>>> {
    let bytes = text.as_bytes();
    capture.charge_work(
        (bytes.len() as u64)
            .checked_mul(2)
            .ok_or(Error::Budget("public D1 delta row parser work"))?,
    )?;
    let mut count = 1usize;
    let mut nested = 0usize;
    let mut quoted = false;
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\'' => {
                if quoted && bytes.get(cursor + 1) == Some(&b'\'') {
                    cursor += 2;
                    continue;
                }
                quoted = !quoted;
            }
            b'(' if !quoted => {
                nested = nested
                    .checked_add(1)
                    .ok_or(Error::Invalid("public D1 delta row nesting"))?;
            }
            b')' if !quoted => {
                nested = nested
                    .checked_sub(1)
                    .ok_or(Error::Invalid("public D1 delta row nesting"))?;
            }
            b',' if !quoted && nested == 0 => {
                count = count
                    .checked_add(1)
                    .filter(|count| *count <= 128)
                    .ok_or(Error::Invalid("public D1 delta row field count"))?;
            }
            _ => {}
        }
        cursor += 1;
    }
    if quoted || nested != 0 || text.is_empty() {
        return Err(Error::Invalid("public D1 delta row fields"));
    }
    let range_state = count
        .checked_mul(std::mem::size_of::<Range<usize>>())
        .ok_or(Error::Budget("public D1 delta row field state"))?;
    capture.charge_work(range_state as u64)?;
    let mut ranges = Vec::with_capacity(count);
    let mut start = 0usize;
    nested = 0;
    quoted = false;
    cursor = 0;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\'' => {
                if quoted && bytes.get(cursor + 1) == Some(&b'\'') {
                    cursor += 2;
                    continue;
                }
                quoted = !quoted;
            }
            b'(' if !quoted => {
                nested = nested
                    .checked_add(1)
                    .ok_or(Error::Invalid("public D1 delta row nesting"))?;
            }
            b')' if !quoted => {
                nested = nested
                    .checked_sub(1)
                    .ok_or(Error::Invalid("public D1 delta row nesting"))?;
            }
            b',' if !quoted && nested == 0 => {
                if start == cursor {
                    return Err(Error::Invalid("public D1 delta empty row field"));
                }
                ranges.push(start..cursor);
                start = cursor + 1;
            }
            _ => {}
        }
        cursor += 1;
    }
    if start == bytes.len() {
        return Err(Error::Invalid("public D1 delta empty row field"));
    }
    ranges.push(start..bytes.len());
    if ranges.len() != count {
        return Err(Error::Invalid("public D1 delta row field count"));
    }
    Ok(ranges)
}

fn validate_indexed_row(
    table: &str,
    columns: &str,
    values: &str,
    values_json: &str,
    digest: &str,
    capture: &PublicCapture,
) -> Result<()> {
    if !hex_digest(digest) {
        return Err(Error::Invalid("public D1 delta row digest"));
    }
    if let Some(expected) = batch_columns(table)
        && !joined_fields_match(columns, expected)
    {
        return Err(Error::Invalid("public D1 delta batch columns"));
    }
    let column_ranges = scan_fields(columns, capture)?;
    let value_ranges = scan_fields(values, capture)?;
    if column_ranges.len() != value_ranges.len() {
        return Err(Error::Invalid("public D1 delta row shape"));
    }
    let value_bytes = value_ranges.iter().try_fold(1024usize, |total, range| {
        total
            .checked_add(range.len())
            .ok_or(Error::Budget("public D1 delta row value bytes"))
    })?;
    if value_bytes > MAX_ROW_VALUE_BYTES {
        return Err(Error::Budget("public D1 delta row value bytes"));
    }
    capture.charge_work(
        (values_json.len() as u64)
            .checked_add((4 * std::mem::size_of::<String>()) as u64)
            .ok_or(Error::Budget("public D1 delta indexed row state"))?,
    )?;
    let indexed = serde_json::from_str::<PriorValues>(values_json)
        .map_err(|e| Error::Source(e.to_string()))?;
    let keys = TABLES
        .iter()
        .find(|(name, _)| *name == table)
        .map(|(_, keys)| *keys)
        .ok_or(Error::Invalid("public D1 delta row table"))?;
    if indexed.0.len() != keys.len() {
        return Err(Error::Invalid("public D1 delta row key count"));
    }
    for (key, expected) in keys.iter().zip(indexed.0.iter()) {
        if !key_literal(expected) {
            return Err(Error::Invalid("public D1 delta key literal"));
        }
        let mut found = None;
        for (index, column) in column_ranges.iter().enumerate() {
            let field = &columns[column.clone()];
            if !field
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            {
                return Err(Error::Invalid("public D1 delta column name"));
            }
            if field == *key {
                if found.replace(index).is_some() {
                    return Err(Error::Invalid("public D1 delta duplicate key column"));
                }
            }
        }
        let position = found.ok_or(Error::Invalid("public D1 delta key column"))?;
        let actual = &values[value_ranges[position].clone()];
        if actual != expected.as_str() || !key_literal(actual) {
            return Err(Error::Invalid("public D1 delta row identity"));
        }
    }
    if batch_columns(table).is_some() {
        let statement_bytes = "INSERT INTO "
            .len()
            .checked_add(table.len())
            .and_then(|size| size.checked_add("_next (".len()))
            .and_then(|size| size.checked_add(columns.len()))
            .and_then(|size| size.checked_add(") VALUES (".len()))
            .and_then(|size| size.checked_add(values.len()))
            .and_then(|size| size.checked_add(");\n".len()))
            .ok_or(Error::Budget("public D1 delta batch row bytes"))?;
        if statement_bytes > MAX_STATEMENT_BYTES {
            return Err(Error::Budget("public D1 delta batch row statement bytes"));
        }
    }
    let digest_work = "INSERT INTO "
        .len()
        .checked_add(table.len())
        .and_then(|size| size.checked_add("_next (".len()))
        .and_then(|size| size.checked_add(columns.len()))
        .and_then(|size| size.checked_add(") VALUES (".len()))
        .and_then(|size| size.checked_add(values.len()))
        .and_then(|size| size.checked_add(");".len()))
        .ok_or(Error::Budget("public D1 delta row digest work"))?;
    capture.charge_work(digest_work as u64)?;
    let actual_digest = single_insert_body_digest(table, columns, values).to_hex();
    if actual_digest.as_str() != digest {
        return Err(Error::Invalid("public D1 delta row digest mismatch"));
    }
    Ok(())
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
        let mut stmt=db.prepare("SELECT length(CAST(n.digest AS BLOB)),length(CAST(n.values_json AS BLOB)),length(CAST(n.segments_json AS BLOB)),coalesce(length(CAST(p.digest AS BLOB)),0),n.digest,n.values_json,n.segments_json,p.digest FROM rows AS n LEFT JOIN prior_rows AS p ON p.table_name=n.table_name AND p.row_key=n.row_key WHERE n.table_name=?1 ORDER BY n.sequence")?;
        let mut rows = stmt.query(params![*table])?;
        let mut active_segment_group = None;
        while let Some(row) = rows.next()? {
            let mut lookup_bytes = (4 * std::mem::size_of::<String>() + 128) as u64;
            for column in 0..4 {
                let length: i64 = row.get(column)?;
                lookup_bytes = lookup_bytes
                    .checked_add(
                        u64::try_from(length)
                            .map_err(|_| Error::Invalid("public D1 delta lookup length"))?,
                    )
                    .ok_or(Error::Budget("public D1 delta lookup work"))?;
            }
            capture.charge_work(lookup_bytes)?;
            let digest: String = row.get(4)?;
            let values_json: String = row.get(5)?;
            let segments_json: String = row.get(6)?;
            let old: Option<String> = row.get(7)?;
            let indexed_segment = single_segment_json(&segments_json)?;
            let row_ordinal = add_segment_row(&mut active_segment_group, indexed_segment)?;
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
            if let Some((offset, length)) = indexed_segment {
                let (columns, values) = if batch_columns(table).is_some() {
                    let ordinal =
                        row_ordinal.ok_or(Error::Invalid("public D1 delta batch row ordinal"))?;
                    let active = active_segment_group
                        .as_mut()
                        .ok_or(Error::Invalid("public D1 delta batch group absent"))?;
                    if active.offset != offset || active.length != length {
                        return Err(Error::Invalid("public D1 delta batch group identity"));
                    }
                    if active.cached.is_none() {
                        let statement = segment(&mut sql, offset, length, capture)?;
                        let cached = parse_insert_batch(statement, table, offset, length, capture)?;
                        if ordinal >= cached.rows.len() {
                            return Err(Error::Invalid("public D1 delta batch row ordinal"));
                        }
                        active.cached = Some(cached);
                    }
                    let cached = active
                        .cached
                        .as_ref()
                        .ok_or(Error::Invalid("public D1 delta batch cache absent"))?;
                    let row_range = cached
                        .rows
                        .get(ordinal)
                        .ok_or(Error::Invalid("public D1 delta batch row ordinal"))?;
                    let columns = &cached.statement[cached.columns.clone()];
                    let values = &cached.statement[row_range.clone()];
                    validate_indexed_row(table, columns, values, &values_json, &digest, capture)?;
                    let retained = columns
                        .len()
                        .checked_add(values.len())
                        .and_then(|bytes| bytes.checked_add(2))
                        .ok_or(Error::Budget("public D1 delta row materialization"))?;
                    capture.charge_work(retained as u64)?;
                    (columns.to_owned(), values.to_owned())
                } else {
                    let statement = segment(&mut sql, offset, length, capture)?;
                    let (columns, values) = stage_insert(&statement, table)?;
                    validate_indexed_row(table, columns, values, &values_json, &digest, capture)?;
                    let retained = columns
                        .len()
                        .checked_add(values.len())
                        .and_then(|bytes| bytes.checked_add(2))
                        .ok_or(Error::Budget("public D1 delta row materialization"))?;
                    capture.charge_work(retained as u64)?;
                    (columns.to_owned(), values.to_owned())
                };
                if !writer.key(&stage, &values_json)? {
                    return Ok(None);
                }
                let prefix_bytes = "INSERT INTO "
                    .len()
                    .checked_add(stage.len())
                    .and_then(|bytes| bytes.checked_add(" (".len()))
                    .and_then(|bytes| bytes.checked_add(columns.len()))
                    .and_then(|bytes| bytes.checked_add(") VALUES ".len()))
                    .ok_or(Error::Budget("public D1 delta row bytes"))?;
                let row_bytes = values
                    .len()
                    .checked_add(2)
                    .ok_or(Error::Budget("public D1 delta row bytes"))?;
                let full_bytes = prefix_bytes
                    .checked_add(row_bytes)
                    .and_then(|bytes| bytes.checked_add(1))
                    .ok_or(Error::Budget("public D1 delta row bytes"))?;
                if full_bytes > MAX_STATEMENT_BYTES {
                    return Ok(None);
                }
                capture.charge_work(full_bytes as u64)?;
                writer.push(
                    false,
                    &format!("INSERT INTO {stage} ({columns}) VALUES "),
                    format!("({values})"),
                )?;
            } else {
                if !writer.key(&stage, &values_json)? {
                    return Ok(None);
                }
                // The captured row index contains only numeric [offset,length]
                // pairs. Admit the tuple slots before serde allocates the Vec.
                let segment_slots = segments_json.bytes().filter(|byte| *byte == b'[').count();
                capture.charge_work(
                    (segment_slots as u64)
                        .checked_mul(std::mem::size_of::<(u64, u64)>() as u64)
                        .ok_or(Error::Budget("public D1 delta segment state"))?,
                )?;
                let segments: Vec<(u64, u64)> = serde_json::from_str(&segments_json)
                    .map_err(|e| Error::Source(e.to_string()))?;
                if segments.len() < 2 {
                    return Err(Error::Invalid("public D1 delta row SQL absent"));
                }
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
                    let stage_bytes = target
                        .len()
                        .checked_add(tail.len())
                        .ok_or(Error::Budget("public D1 delta chunk bytes"))?;
                    if stage_bytes > MAX_STATEMENT_BYTES {
                        return Ok(None);
                    }
                    capture.charge_work(stage_bytes as u64)?;
                    writer.line(&format!("{target}{tail}"))?;
                }
            }
        }
        finish_segment_group(&mut active_segment_group)?;
        drop(rows);
        drop(stmt);
        let mut stmt=db.prepare("SELECT length(CAST(p.values_json AS BLOB)),p.values_json FROM prior_rows AS p LEFT JOIN rows AS n ON n.table_name=p.table_name AND n.row_key=p.row_key WHERE p.table_name=?1 AND n.row_key IS NULL ORDER BY p.sequence")?;
        let mut rows = stmt.query(params![*table])?;
        while let Some(row) = rows.next()? {
            let length: i64 = row.get(0)?;
            let length = u64::try_from(length)
                .map_err(|_| Error::Invalid("public D1 removed key length"))?;
            capture.charge_work(
                length
                    .checked_add(std::mem::size_of::<String>() as u64 + 128)
                    .ok_or(Error::Budget("public D1 removed key work"))?,
            )?;
            let values_json: String = row.get(1)?;
            removed += 1;
            key_counts[number] += 1;
            if !writer.key(&stage, &values_json)? {
                return Ok(None);
            }
        }
        writer.flush()?;
    }
    writer.line("CREATE TABLE IF NOT EXISTS tos_delta_publications (revision TEXT PRIMARY KEY, base_revision TEXT NOT NULL);")?;
    let publish = format!("{prefix}_publish");
    writer.line(&format!("DROP TRIGGER IF EXISTS {publish};"))?;
    let current = "(SELECT json_extract(group_concat(json_chunk, ''), '$.sha256') FROM (SELECT json_chunk FROM edge_meta WHERE key = 'data_revision' ORDER BY part))";
    // At most one trigger is built. Reserve its whole existing statement
    // envelope before any variable operation or copy is materialized.
    capture.charge_work((MAX_STATEMENT_BYTES as u64) * 3)?;
    let (guards, seals) = expected_auxiliary_operations(prior.top(), next_top)?;
    let mut operations = format!(
        "SELECT CASE WHEN {current} IS NOT '{}' THEN RAISE(ABORT, 'stale delta baseline') END;{guards}",
        prior.revision()
    );
    if operations.len() > MAX_STATEMENT_BYTES {
        return Ok(None);
    }
    for (number, (table, keys)) in TABLES.iter().enumerate() {
        if key_counts[number] == 0 {
            continue;
        }
        let stage = format!("{prefix}_{table}");
        let count_guard = format!(
            "SELECT CASE WHEN (SELECT count(*) FROM {stage}) != {} OR (SELECT count(*) FROM {stage}_keys) != {} THEN RAISE(ABORT, 'incomplete delta staging') END;",
            changed_counts[number], key_counts[number]
        );
        if operations.len().saturating_add(count_guard.len()) > MAX_STATEMENT_BYTES {
            return Ok(None);
        }
        operations.push_str(&count_guard);
        let same = keys
            .iter()
            .map(|key| format!("target.{key} IS changed.{key}"))
            .collect::<Vec<_>>()
            .join(" AND ");
        let replace = format!(
            "DELETE FROM {table} WHERE rowid IN (SELECT target.rowid FROM {stage}_keys AS changed CROSS JOIN {table} AS target WHERE {same});INSERT INTO {table} SELECT * FROM {stage};"
        );
        if operations.len().saturating_add(replace.len()) > MAX_STATEMENT_BYTES {
            return Ok(None);
        }
        operations.push_str(&replace);
    }
    if operations.len().saturating_add(seals.len()) > MAX_STATEMENT_BYTES {
        return Ok(None);
    }
    operations.push_str(&seals);
    let final_guard = format!(
        "SELECT CASE WHEN {current} IS NOT '{next_revision}' THEN RAISE(ABORT, 'delta revision mismatch') END;"
    );
    let trigger_framing = "CREATE TRIGGER ".len()
        + publish.len()
        + " AFTER INSERT ON tos_delta_publications WHEN NEW.revision = '' BEGIN  END;".len()
        + next_revision.len();
    let trigger_bytes = trigger_framing
        .checked_add(operations.len())
        .and_then(|bytes| bytes.checked_add(final_guard.len()))
        .ok_or(Error::Budget("public D1 delta trigger bytes"))?;
    if trigger_bytes > MAX_STATEMENT_BYTES {
        return Ok(None);
    }
    operations.push_str(&final_guard);
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

#[cfg(test)]
mod batch_delta_parser_tests {
    use super::*;

    #[test]
    fn segment_groups_reject_trailing_commas_and_keep_ordinals() {
        assert_eq!(single_segment_json("[[10,90]]").unwrap(), Some((10, 90)));
        assert_eq!(single_segment_json("[[10,90],[100,80]]").unwrap(), None);
        assert!(single_segment_json("[[10,90],]").is_err());

        let mut complete = None;
        assert_eq!(
            add_segment_row(&mut complete, Some((10, 90))).unwrap(),
            Some(0)
        );
        assert_eq!(
            add_segment_row(&mut complete, Some((10, 90))).unwrap(),
            Some(1)
        );
        complete.as_mut().unwrap().cached = Some(CachedInsertBatch {
            offset: 10,
            length: 90,
            statement: String::new(),
            columns: 0..0,
            rows: vec![0..1, 1..2],
        });
        assert!(finish_segment_group(&mut complete).is_ok());

        let mut short_index = None;
        add_segment_row(&mut short_index, Some((10, 90))).unwrap();
        short_index.as_mut().unwrap().cached = Some(CachedInsertBatch {
            offset: 10,
            length: 90,
            statement: String::new(),
            columns: 0..0,
            rows: vec![0..1, 1..2],
        });
        assert!(finish_segment_group(&mut short_index).is_err());
    }

    #[test]
    fn tuple_scanner_keeps_quoted_commas_and_doubled_quotes_inside_each_row() {
        let statement = "('node',3,'comma, doubled ''quote'' value',7),('relation',3,'x',8);";
        let (first, after_first) = scan_tuple(statement, 0).unwrap();
        assert_eq!(
            &statement[first],
            "'node',3,'comma, doubled ''quote'' value',7"
        );
        assert_eq!(statement.as_bytes()[after_first], b',');
        let (second, after_second) = scan_tuple(statement, after_first + 1).unwrap();
        assert_eq!(&statement[second], "'relation',3,'x',8");
        assert_eq!(statement.as_bytes()[after_second], b';');
    }

    #[test]
    fn indexed_row_digest_matches_the_exact_single_insert_form() {
        let digest = single_insert_body_digest(
            "knowledge_search_grams",
            "kind,n,gram,position",
            "'nodes',3,'a,b''c',12",
        )
        .to_hex();
        let expected = Digest256::of_bytes(
            b"INSERT INTO knowledge_search_grams_next (kind,n,gram,position) VALUES ('nodes',3,'a,b''c',12);",
        )
        .to_hex();
        assert_eq!(digest, expected);
    }
}
