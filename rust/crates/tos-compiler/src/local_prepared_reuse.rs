//! Explicit new-file bootstrap search reuse from one independently selected
//! donor. The retained donor connection is read-only and owns its transaction;
//! the target transaction/lifecycle, fresh-source two-pass digest, replacement
//! raw-row bound and final commit belong to `local_prepared`.
//!
//! This ports maintained prepared_search_reuse.py, not a new posting algorithm.
//! Every old carrier is inspected, every old term/text/reverse frame is checked,
//! and all six tables are streamed once into bounded copy batches. Forward and
//! reverse membership closure is checked before success. The donor is untouched.
//! A held read transaction proves its selected snapshot, never external WAL
//! currentness. Inode checks concern the selected path, not source authority.
use crate::{
    Error, Result, local_prepared as owner,
    local_prepared_read::{PreparedReadLimits, PreparedReadTransaction},
    local_prepared_search as search, safe_open,
};
use rusqlite::{
    Connection, OpenFlags, Params, params,
    types::{Value, ValueRef},
};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet, HashMap},
    fs::{self, File},
    os::unix::fs::MetadataExt,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};
use tos_foundation::{
    CanonicalProfile, Digest256, Digest256Hasher, JsonLimits, JsonNumber, JsonNumberKind,
    JsonString, JsonValue, canonical_bytes_v1, python_lower_unicode16_v1,
};

const TOP: &str = "knowledge_reader_top";
const LENS: &str = "knowledge_lens_top";
const CATALOG: &str = "knowledge_catalog";
const MAX_HISTOGRAM_CELLS: usize = 16384;
const BATCH_ROWS: usize = 512;
// The reused owner admission currently issues 7 table checks, 10 named
// index-list/layout pairs, 5 primary-index pairs and 3 snapshot statements.
// Precharge before its SQL; refuse an owner procedure change until reviewed.
const ADMISSION_STATEMENTS: u64 = 40;
const RECHECK_STATEMENTS: u64 = 3;
const TABLES: &[(&str, &[&str])] = &[
    (
        "search_documents",
        &["doc_id", "kind", "identifier", "sort_key", "filters"],
    ),
    (
        "search_values",
        &["doc_id", "category", "field", "byte_length"],
    ),
    (
        "search_text_chunks",
        &["doc_id", "category", "field", "chunk", "payload"],
    ),
    (
        "search_terms",
        &["term_id", "kind", "plane", "n", "term_key", "posting_count"],
    ),
    (
        "search_blocks",
        &["term_id", "lower_fence", "posting_count", "payload"],
    ),
    (
        "search_document_terms",
        &["doc_id", "term_count", "payload", "digest"],
    ),
];

#[derive(Clone, Debug)]
pub struct PreparedSearchReuse {
    pub path: PathBuf,
    pub binding: JsonValue,
    pub max_source_bytes: u64,
    pub max_copy_bytes: u64,
    pub max_copy_rows: u64,
    pub max_batch_bytes: usize,
    pub max_queries: u64,
    pub max_vm_steps: u64,
    pub max_validation_state_bytes: usize,
    /// Callback progress belongs to the maintained Python profile. Native
    /// request parsing must reject it explicitly rather than silently ignore it.
    pub progress: bool,
}
impl PreparedSearchReuse {
    pub fn new(path: PathBuf, binding: JsonValue) -> Self {
        Self {
            path,
            binding,
            max_source_bytes: 256 * 1024 * 1024,
            max_copy_bytes: 256 * 1024 * 1024,
            max_copy_rows: 2_000_000,
            max_batch_bytes: 8 * 1024 * 1024,
            max_queries: 2_000_000,
            max_vm_steps: 1_000_000_000,
            max_validation_state_bytes: 128 * 1024 * 1024,
            progress: false,
        }
    }
    fn validate(&self) -> Result<()> {
        if self.progress {
            return Err(Error::PreparedUnsupported(
                "native donor progress callback; use maintained Python profile",
            ));
        }
        if self.max_source_bytes == 0
            || self.max_copy_bytes == 0
            || self.max_copy_rows == 0
            || self.max_batch_bytes == 0
            || self.max_queries == 0
            || self.max_vm_steps == 0
            || self.max_validation_state_bytes == 0
        {
            return Err(Error::Invalid("positive search donor limits required"));
        }
        search::header(&self.binding)?;
        Ok(())
    }
}
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct ReuseReport {
    /// Selected whole-file length; returned-scalar source_bytes below excludes
    /// SQLite pager IO and neither value measures process RSS.
    pub donor_file_bytes: u64,
    pub population_count_reads: u64,
    pub term_dictionary_scans: u64,
    pub copy_table_scans: u64,
    /// Returned scalar bytes, plus a conservative 128-byte allowance per
    /// reused admission row for its separately unreported numeric columns.
    pub source_bytes: u64,
    pub source_rows: u64,
    pub queries: u64,
    /// Conservative VM charge: callback steps plus one interval per statement
    /// for otherwise invisible tails. No VM counter is reset between phases.
    pub vm_steps_charged: u64,
    pub validation_state_bytes: usize,
    pub validation_state_peak_bytes: usize,
    pub changed_documents: usize,
    pub changed_bytes: usize,
    pub observed_documents: u64,
    pub copied_rows: u64,
    pub copied_bytes: u64,
    pub mutations: u64,
    pub write_calls: u64,
    pub batches: u64,
    pub peak_batch_rows: usize,
    pub peak_batch_bytes: usize,
    pub elapsed_micros: u64,
    pub temp_objects_created: u64,
}
struct Meter {
    source_bytes: Cell<u64>,
    source_rows: Cell<u64>,
    queries: Cell<u64>,
    state_bytes: Cell<usize>,
    state_peak: Cell<usize>,
    vm: Arc<AtomicU64>,
    interval: u64,
    deadline: Option<Instant>,
}
impl Meter {
    fn check(&self) -> Result<()> {
        if self.deadline.is_some_and(|d| Instant::now() >= d) {
            Err(Error::Budget("search donor whole operation deadline"))
        } else {
            Ok(())
        }
    }
    fn begin(&self, request: &PreparedSearchReuse) -> Result<()> {
        self.check()?;
        let queries = self
            .queries
            .get()
            .checked_add(1)
            .filter(|n| *n <= request.max_queries)
            .ok_or(Error::Budget("search donor query cap"))?;
        self.queries.set(queries);
        let vm = self
            .vm
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                used.checked_add(self.interval)
            })
            .map_err(|_| Error::Budget("search donor VM counter overflow"))?
            + self.interval;
        if vm > request.max_vm_steps {
            return Err(Error::Budget("search donor cumulative VM tail cap"));
        }
        Ok(())
    }
    fn charge_bytes(&self, bytes: usize, request: &PreparedSearchReuse) -> Result<()> {
        self.source_bytes.set(
            self.source_bytes
                .get()
                .checked_add(bytes as u64)
                .filter(|n| *n <= request.max_source_bytes)
                .ok_or(Error::Budget("search donor source read cap"))?,
        );
        Ok(())
    }
    fn row(&self, row: &rusqlite::Row<'_>, request: &PreparedSearchReuse) -> Result<()> {
        self.check()?;
        let mut bytes = 0usize;
        for column in 0..row.as_ref().column_count() {
            let size = match row.get_ref(column)? {
                ValueRef::Null => 0,
                ValueRef::Integer(n) => n.to_string().len(),
                ValueRef::Real(_) => {
                    return Err(Error::Invalid(
                        "search donor unexpected floating SQL scalar",
                    ));
                }
                ValueRef::Text(v) | ValueRef::Blob(v) => v.len(),
            };
            bytes = bytes
                .checked_add(size)
                .ok_or(Error::Budget("search donor row read bytes"))?;
        }
        self.charge_bytes(bytes, request)?;
        self.source_rows.set(
            self.source_rows
                .get()
                .checked_add(1)
                .ok_or(Error::Budget("search donor source row count"))?,
        );
        Ok(())
    }
    fn retain(&self, bytes: usize, request: &PreparedSearchReuse) -> Result<()> {
        let total = self
            .state_bytes
            .get()
            .checked_add(bytes)
            .filter(|n| *n <= request.max_validation_state_bytes)
            .ok_or(Error::Budget("search donor validation state cap"))?;
        self.state_bytes.set(total);
        self.state_peak.set(self.state_peak.get().max(total));
        Ok(())
    }
}
struct TermState {
    kind: String,
    plane: u8,
    n: u8,
    key: Vec<u8>,
    expected: u64,
    copied: u64,
}
struct DocumentState {
    kind: String,
    key: Vec<u8>,
    reverse_count: usize,
    reverse_digest: Digest256,
    forward_digest: Digest256Hasher,
    forward_count: usize,
}

/// File-owned donor. Dropping it rolls back only its private read transaction
/// and closes only its donor connection. It never manages the target lifecycle.
pub struct SearchDonor {
    request: PreparedSearchReuse,
    limits: owner::PublicationLimits,
    db: Connection,
    anchor: File,
    stamp: (u64, u64),
    meter: Meter,
    started: Instant,
    count: u64,
    observed: u64,
    generation: Vec<u8>,
    descriptor: JsonValue,
    lens: JsonValue,
    terms: HashMap<u64, TermState>,
    documents: HashMap<u64, DocumentState>,
    changed: BTreeSet<(String, String)>,
    changed_bytes: usize,
    rows_digest: Digest256Hasher,
    histograms: [BTreeMap<[String; 3], u64>; 2],
    finished: bool,
    copied: bool,
    report: ReuseReport,
}
impl Drop for SearchDonor {
    fn drop(&mut self) {
        self.db.progress_handler(0, None::<fn() -> bool>);
        if !self.db.is_autocommit() {
            let _ = self.db.execute_batch("ROLLBACK");
        }
    }
}
impl SearchDonor {
    pub fn open(
        mut request: PreparedSearchReuse,
        limits: owner::PublicationLimits,
        deadline: Option<Instant>,
    ) -> Result<Self> {
        request.validate()?;
        limits.validate()?;
        if !request.path.is_absolute() {
            request.path = std::env::current_dir()?.join(&request.path);
        }
        let started = Instant::now();
        let anchor = safe_open::open_regular(&request.path, u64::MAX)?;
        let metadata = anchor.metadata()?;
        let stamp = (metadata.dev(), metadata.ino());
        let db = Connection::open_with_flags(
            &request.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let interval = request.max_vm_steps.min(1000);
        let vm = Arc::new(AtomicU64::new(0));
        let callback_vm = Arc::clone(&vm);
        let maximum = request.max_vm_steps;
        db.progress_handler(
            interval as i32,
            Some(move || {
                callback_vm
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                        used.checked_add(interval)
                    })
                    .map_or(true, |previous| previous + interval > maximum)
                    || deadline.is_some_and(|d| Instant::now() >= d)
            }),
        );
        let length = limits
            .max_row_bytes
            .max(limits.max_metadata_bytes)
            .max(request.max_batch_bytes)
            .checked_add(65536)
            .and_then(|n| i32::try_from(n).ok())
            .ok_or(Error::Budget("search donor SQLite scalar length cap"))?;
        // `limits` is not an enabled rusqlite feature in this source profile.
        // The connection is exclusively owned here; use its SQLite length gate
        // before any SQL can allocate a donor result beyond its byte probes.
        unsafe {
            rusqlite::ffi::sqlite3_limit(db.handle(), rusqlite::ffi::SQLITE_LIMIT_LENGTH, length);
        }
        let mut donor = Self {
            request,
            limits,
            db,
            anchor,
            stamp,
            meter: Meter {
                source_bytes: Cell::new(0),
                source_rows: Cell::new(0),
                queries: Cell::new(0),
                state_bytes: Cell::new(0),
                state_peak: Cell::new(0),
                vm,
                interval,
                deadline,
            },
            started,
            count: 0,
            observed: 0,
            generation: Vec::new(),
            descriptor: JsonValue::Null,
            lens: JsonValue::Null,
            terms: HashMap::new(),
            documents: HashMap::new(),
            changed: BTreeSet::new(),
            changed_bytes: 0,
            rows_digest: Digest256Hasher::new(),
            histograms: Default::default(),
            finished: false,
            copied: false,
            report: ReuseReport::default(),
        };
        donor.report.donor_file_bytes = metadata.len();
        donor.check_path()?;
        donor.execute("PRAGMA query_only=ON")?;
        donor.execute("BEGIN")?;
        donor.validate_open()?;
        donor.check_path()?;
        Ok(donor)
    }
    fn execute(&self, sql: &str) -> Result<()> {
        self.meter.begin(&self.request)?;
        self.db.execute_batch(sql)?;
        Ok(())
    }
    fn one<P: Params>(&self, sql: &str, params: P) -> Result<Option<Vec<Value>>> {
        self.meter.begin(&self.request)?;
        let mut statement = self.db.prepare(sql)?;
        let mut rows = statement.query(params)?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        self.meter.row(row, &self.request)?;
        let values = (0..row.as_ref().column_count())
            .map(|i| row.get(i))
            .collect::<std::result::Result<Vec<Value>, _>>()?;
        if let Some(row) = rows.next()? {
            self.meter.row(row, &self.request)?;
            return Err(Error::Invalid(
                "search donor ambiguous singleton/point read",
            ));
        }
        Ok(Some(values))
    }
    fn metadata(&self, key: &str, cap: usize) -> Result<JsonValue> {
        self.meter.begin(&self.request)?;
        let mut statement = self.db.prepare("SELECT part,CASE WHEN length(CAST(json_chunk AS BLOB))<=131072 THEN json_chunk ELSE NULL END FROM edge_meta WHERE key=? ORDER BY part LIMIT 257")?;
        let mut rows = statement.query([key])?;
        let mut raw = String::new();
        let mut part = 0i64;
        while let Some(row) = rows.next()? {
            self.meter.row(row, &self.request)?;
            let actual: i64 = row.get(0)?;
            let chunk: Option<String> = row.get(1)?;
            if actual != part || part >= 256 {
                return Err(Error::Invalid("search donor metadata chunk order/count"));
            }
            let chunk = chunk.ok_or(Error::Budget("search donor metadata chunk bytes"))?;
            if raw.len().checked_add(chunk.len()).is_none_or(|n| n > cap) {
                return Err(Error::Budget("search donor metadata frame cap"));
            }
            raw.push_str(&chunk);
            part += 1;
        }
        if part == 0 {
            return Err(Error::Invalid("search donor metadata absent"));
        }
        let value = owner::parse(&raw, cap)?;
        if owner::compact(&value, cap)? != raw {
            return Err(Error::Invalid("search donor metadata compact framing"));
        }
        Ok(value)
    }
    fn validate_open(&mut self) -> Result<()> {
        // Reuse the existing immutable base/index admission; absorb all its
        // returned metadata/schema bytes, rows and statement-tail charges.
        for _ in 0..ADMISSION_STATEMENTS {
            self.meter.begin(&self.request)?;
        }
        let admission = PreparedReadTransaction::admit(
            &self.db,
            &self.request.binding,
            PreparedReadLimits {
                max_row_bytes: self.limits.max_metadata_bytes.max(65536),
                max_response_bytes: self.limits.max_metadata_bytes.max(65536),
                max_rows: self.request.max_queries.min(usize::MAX as u64) as usize,
                max_vm_steps: self.request.max_vm_steps,
                max_bytes: self
                    .request
                    .max_source_bytes
                    .saturating_sub(self.meter.source_bytes.get())
                    .min(usize::MAX as u64) as usize,
            },
        )
        .map_err(|e| Error::Source(e.to_string()))?;
        let top = admission.top().clone();
        self.meter.charge_bytes(
            admission
                .read_bytes()
                .checked_add(
                    admission
                        .read_rows()
                        .checked_mul(128)
                        .ok_or(Error::Budget("search donor admission scalar allowance"))?,
                )
                .ok_or(Error::Budget("search donor admission source bytes"))?,
            &self.request,
        )?;
        self.meter.source_rows.set(
            self.meter
                .source_rows
                .get()
                .checked_add(admission.read_rows() as u64)
                .ok_or(Error::Budget("search donor admission rows"))?,
        );
        if admission.statement_count() != ADMISSION_STATEMENTS {
            return Err(Error::Invalid(
                "search donor immutable admission procedure changed",
            ));
        }
        drop(admission);
        self.verify_search_schema()?;
        self.verify_metadata(&top)?;
        let framed = search::header(&self.request.binding)?;
        let row = self.one("SELECT CASE WHEN length(CAST(header AS BLOB))<=65536 THEN header END,CASE WHEN length(cursor_key)=32 THEN cursor_key END,high_water FROM search_header WHERE singleton=1", [])?.ok_or(Error::Invalid("search donor header absent"))?;
        if string(&row[0])? != framed {
            return Err(Error::Invalid("stale search donor search profile/binding"));
        }
        self.generation = blob(&row[1])?.to_vec();
        if self.generation.len() != 32 {
            return Err(Error::Invalid("search donor cursor incarnation"));
        }
        let high = positive_or_zero(&row[2])?;
        let prepared = self
            .one(
                "SELECT high_water FROM prepared_state WHERE singleton=1",
                [],
            )?
            .ok_or(Error::Invalid("search donor prepared state absent"))?;
        let mut populations = Vec::new();
        for table in [
            "prepared_documents",
            "search_documents",
            "search_document_terms",
            "knowledge_nodes",
            "knowledge_relations",
        ] {
            let row = self
                .one(&format!("SELECT count(*) FROM {table}"), [])?
                .ok_or(Error::Invalid("search donor population count"))?;
            populations.push(positive_or_zero(&row[0])?);
            self.report.population_count_reads += 1;
        }
        self.count = populations[3]
            .checked_add(populations[4])
            .filter(|n| *n <= self.limits.max_mutations && *n <= search::MAX_ADDRESS)
            .ok_or(Error::Budget("search donor dense population"))?;
        if high != self.count
            || positive_or_zero(&prepared[0])? != high
            || populations[..3].iter().any(|n| *n != self.count)
        {
            return Err(Error::Invalid(
                "search donor requires complete dense bootstrap population",
            ));
        }
        self.meter.begin(&self.request)?;
        self.report.term_dictionary_scans += 1;
        let mut statement = self.db.prepare("SELECT term_id,CASE WHEN length(kind)<=8 THEN kind END,plane,n,CASE WHEN length(term_key)<=3072 THEN term_key END,posting_count FROM search_terms ORDER BY term_id")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            self.meter.row(row, &self.request)?;
            let term: i64 = row.get(0)?;
            let kind: String = row.get(1)?;
            let plane: i64 = row.get(2)?;
            let n: i64 = row.get(3)?;
            let key: Vec<u8> = row.get(4)?;
            let total: i64 = row.get(5)?;
            if term < 1
                || term as u64 > search::MAX_ADDRESS
                || !matches!(kind.as_str(), "node" | "relation")
                || !(0..=3).contains(&plane)
                || !(0..=3).contains(&n)
                || total < 1
                || total as u64 > self.count
            {
                return Err(Error::Invalid("search donor term dictionary framing"));
            }
            self.meter.retain(512 + key.len(), &self.request)?;
            self.terms.insert(
                term as u64,
                TermState {
                    kind,
                    plane: plane as u8,
                    n: n as u8,
                    key,
                    expected: total as u64,
                    copied: 0,
                },
            );
        }
        Ok(())
    }
    fn verify_search_schema(&self) -> Result<()> {
        for ddl in search::DDL {
            let tokens: Vec<_> = ddl.split_whitespace().collect();
            let (kind, name, table) = if tokens[1] == "TABLE" {
                ("table", tokens[2], tokens[2])
            } else {
                ("index", tokens[2], "search_blocks")
            };
            let actual = self.one("SELECT type,name,tbl_name,CASE WHEN length(CAST(sql AS BLOB))<=65536 THEN sql END FROM sqlite_master WHERE name=?", [name])?.ok_or(Error::Invalid("search donor search schema object absent"))?;
            if string(&actual[0])? != kind
                || string(&actual[1])? != name
                || string(&actual[2])? != table
                || normalize_sql(string(&actual[3])?) != normalize_sql(ddl)
            {
                return Err(Error::Invalid("search donor exact storage schema differs"));
            }
        }
        for (name, table) in [
            ("sqlite_autoindex_search_documents_1", "search_documents"),
            ("sqlite_autoindex_search_documents_2", "search_documents"),
            ("sqlite_autoindex_search_terms_1", "search_terms"),
        ] {
            let actual = self
                .one(
                    "SELECT type,name,tbl_name,sql FROM sqlite_master WHERE name=?",
                    [name],
                )?
                .ok_or(Error::Invalid("search donor unique storage index absent"))?;
            if string(&actual[0])? != "index"
                || string(&actual[1])? != name
                || string(&actual[2])? != table
                || actual[3] != Value::Null
            {
                return Err(Error::Invalid("search donor unique storage index differs"));
            }
        }
        Ok(())
    }
    fn verify_metadata(&mut self, top: &JsonValue) -> Result<()> {
        let selected = self.one("SELECT CASE WHEN length(CAST(descriptor AS BLOB))<=? THEN descriptor END FROM prepared_state WHERE singleton=1", [self.limits.max_metadata_bytes as u64])?.ok_or(Error::Invalid("search donor descriptor absent"))?;
        let raw = string(&selected[0])?;
        let descriptor = owner::parse(raw, self.limits.max_metadata_bytes)?;
        owner::verify_descriptor(&descriptor, raw, top)?;
        if field(&descriptor, "mode")?.as_str() != Some("bootstrap") {
            return Err(Error::Invalid(
                "search donor requires current bootstrap descriptor",
            ));
        }
        let revision = required(top, "data_revision")?;
        let revision_meta = self.metadata("data_revision", 1024)?;
        if revision_meta.as_object().is_none_or(|o| o.len() != 1)
            || revision_meta
                .object_get("sha256")
                .and_then(JsonValue::as_str)
                != Some(revision)
        {
            return Err(Error::Invalid("search donor data revision differs"));
        }
        let catalog = self.metadata(CATALOG, self.limits.max_metadata_bytes)?;
        let lens = self.metadata(LENS, 1_048_576)?;
        let header = field(&descriptor, "header")?;
        owner::validate_header(header, &catalog)?;
        for (top_key, header_key) in [
            ("source_revision", "source_revision"),
            ("graph_schema", "schema"),
            ("normalization_binding", "normalization_binding"),
            ("authority_boundary", "authority_boundary"),
        ] {
            if !same(
                field(top, top_key)?,
                field(header, header_key)?,
                self.limits.max_metadata_bytes,
            )? {
                return Err(Error::Invalid("search donor top/header closure"));
            }
        }
        if digest(&catalog, self.limits.max_metadata_bytes)?
            != required(&descriptor, "catalog_sha256")?
            || digest(&catalog, self.limits.max_metadata_bytes)? != required(top, "catalog_sha256")?
            || digest(&lens, 1_048_576)? != required(top, "lens_sha256")?
        {
            return Err(Error::Invalid(
                "search donor catalog/lens metadata checksum",
            ));
        }
        verify_lens(&lens, header)?;
        let rows_sha = required(&descriptor, "rows_sha256")?;
        Digest256::from_hex(rows_sha)
            .map_err(|_| Error::Invalid("search donor complete rows digest"))?;
        self.meter.retain(
            raw.len()
                .checked_mul(8)
                .and_then(|n| {
                    owner::compact(&lens, 1_048_576)
                        .ok()
                        .and_then(|v| n.checked_add(v.len() * 8))
                })
                .ok_or(Error::Budget("search donor retained metadata bytes"))?,
            &self.request,
        )?;
        self.descriptor = descriptor;
        self.lens = lens;
        Ok(())
    }
    /// Observe every fresh row in the exact dense first-pass node/relation
    /// encounter order. Old donor bytes, not fresh replacement semantics, own
    /// donor search validation. Replacement contents remain owner-authoritative.
    pub fn observe(
        &mut self,
        kind: &str,
        identifier: &str,
        new_raw: &str,
        address: u64,
        token: u64,
    ) -> Result<()> {
        self.meter.check()?;
        if self.finished
            || self.copied
            || !matches!(kind, "node" | "relation")
            || identifier.is_empty()
            || identifier.chars().count() > 4096
            || address != self.observed + 1
            || address > self.count
            || token > search::MAX_ADDRESS
            || new_raw.len() > self.limits.max_row_bytes
        {
            return Err(Error::Invalid("search donor source observation framing"));
        }
        let mapping = self
            .one(
                "SELECT doc_id,source_order FROM prepared_documents WHERE kind=? AND id=?",
                params![kind, identifier],
            )?
            .ok_or(Error::Invalid(
                "search donor identity absent; full bootstrap required",
            ))?;
        if positive_or_zero(&mapping[0])? != address || positive_or_zero(&mapping[1])? != token {
            return Err(Error::Invalid(
                "search donor identity/address/source order differs; full bootstrap required",
            ));
        }
        let columns = columns(kind);
        let selected = columns
            .iter()
            .map(|key| format!("CASE WHEN length(CAST({key} AS BLOB))<=? THEN {key} END"))
            .chain(std::iter::once(
                "CASE WHEN length(CAST(json AS BLOB))<=? THEN json END".to_owned(),
            ))
            .collect::<Vec<_>>()
            .join(",");
        let mut parameters =
            vec![
                Value::Integer(self.limits.max_row_bytes.min(i64::MAX as usize) as i64);
                columns.len() + 1
            ];
        parameters.push(Value::Text(identifier.to_owned()));
        let carrier = self
            .one(
                &format!("SELECT {selected} FROM knowledge_{kind}s WHERE id=?"),
                rusqlite::params_from_iter(parameters),
            )?
            .ok_or(Error::Invalid("search donor carrier missing"))?;
        let old_raw = string(&carrier[columns.len()])?;
        let item = owner::parse(old_raw, self.limits.max_row_bytes)?;
        if required(&item, "id")? != identifier
            || owner::compact(&item, self.limits.max_row_bytes)? != old_raw
        {
            return Err(Error::Invalid(
                "search donor full carrier framing/identity differs",
            ));
        }
        for (i, column) in columns.iter().enumerate() {
            if string(&carrier[i])? != owner::index_value(&item, column)? {
                return Err(Error::Invalid(
                    "search donor carrier duplicated index column differs",
                ));
            }
        }
        let row_sha = Digest256::of_bytes(old_raw.as_bytes()).to_hex();
        let row_meta = self.metadata(&format!("knowledge_{kind}_digest:{identifier}"), 1024)?;
        if row_meta.as_object().is_none_or(|o| o.len() != 1)
            || row_meta.object_get("sha256").and_then(JsonValue::as_str) != Some(row_sha.as_str())
        {
            return Err(Error::Invalid("search donor carrier checksum differs"));
        }
        let encoded = search::default_json(&text(identifier), search::MAX_ROW_BYTES)?;
        let key = search::order_key(&text(identifier), token)?;
        let identity = self.one("SELECT CASE WHEN length(kind)<=8 THEN kind END,CASE WHEN length(identifier)<=1900000 THEN identifier END,CASE WHEN length(sort_key)<=1900000 THEN sort_key END FROM search_documents WHERE doc_id=?", [address])?.ok_or(Error::Invalid("search donor search document absent"))?;
        if string(&identity[0])? != kind
            || blob(&identity[1])? != encoded
            || blob(&identity[2])? != key
        {
            return Err(Error::Invalid(
                "search donor document identity/address/order differs",
            ));
        }
        let lens_order = self.one("SELECT CASE WHEN length(CAST(sort_key AS BLOB))<=65536 THEN sort_key END,CASE WHEN length(CAST(from_id AS BLOB))<=65536 THEN from_id END,CASE WHEN length(CAST(to_id AS BLOB))<=65536 THEN to_id END FROM knowledge_lens_order WHERE kind=? AND id=?", params![kind, identifier])?.ok_or(Error::Invalid("search donor lens order row absent"))?;
        let lower_id = python_lower_unicode16_v1(identifier, 4096, 12288, 65536)
            .map_err(|e| Error::Source(e.to_string()))?;
        let from = if kind == "relation" {
            owner::index_value(&item, "from_id")?
        } else {
            String::new()
        };
        let to = if kind == "relation" {
            owner::index_value(&item, "to_id")?
        } else {
            String::new()
        };
        if string(&lens_order[0])? != lower_id
            || string(&lens_order[1])? != from
            || string(&lens_order[2])? != to
        {
            return Err(Error::Invalid(
                "search donor lens index/body closure differs",
            ));
        }
        for endpoint in [&from, &to].into_iter().filter(|_| kind == "relation") {
            if self
                .one("SELECT id FROM knowledge_nodes WHERE id=?", [endpoint])?
                .is_none()
            {
                return Err(Error::Invalid("search donor relation endpoint absent"));
            }
        }
        let frame = owner::compact(
            &JsonValue::Array(vec![
                text(kind),
                text(identifier),
                number(address),
                number(token),
                text(&row_sha),
            ]),
            131072,
        )?;
        self.rows_digest.update(frame.as_bytes());
        self.rows_digest.update(b"\n");
        let index = usize::from(kind == "relation");
        let dims = dimensions(kind);
        let cell = [
            owner::index_value(&item, dims[0])?,
            owner::index_value(&item, dims[1])?,
            owner::index_value(&item, dims[2])?,
        ];
        if !self.histograms[index].contains_key(&cell) {
            if self.histograms[index].len() >= MAX_HISTOGRAM_CELLS {
                return Err(Error::Budget("search donor source histogram cells"));
            }
            self.meter.retain(
                512 + cell.iter().map(|s| s.len() * 4).sum::<usize>(),
                &self.request,
            )?;
        }
        let n = self.histograms[index].entry(cell).or_default();
        *n = n
            .checked_add(1)
            .ok_or(Error::Budget("search donor histogram count"))?;
        self.verify_document(search::PreparedSearchDocument::from_item(
            address, kind, &item, token,
        )?)?;
        if new_raw != old_raw {
            if self.changed.len() >= self.limits.max_changes {
                return Err(Error::Budget(
                    "search donor changed document count; full bootstrap required",
                ));
            }
            self.changed_bytes = self
                .changed_bytes
                .checked_add(new_raw.len())
                .filter(|n| *n <= self.limits.max_change_bytes)
                .ok_or(Error::Budget(
                    "search donor changed raw bytes; full bootstrap required",
                ))?;
            self.meter.retain(
                new_raw
                    .len()
                    .checked_mul(16)
                    .and_then(|n| n.checked_add(1024))
                    .ok_or(Error::Budget("search donor changed retained bytes"))?,
                &self.request,
            )?;
            self.changed
                .insert((kind.to_owned(), identifier.to_owned()));
        }
        self.observed = address;
        Ok(())
    }
    fn verify_document(&mut self, document: search::PreparedSearchDocument) -> Result<()> {
        let filters = self.one("SELECT CASE WHEN length(filters)<=1900000 THEN filters END FROM search_documents WHERE doc_id=?", [document.doc_id])?.ok_or(Error::Invalid("search donor filters absent"))?;
        if blob(&filters[0])? != search::default_json(&document.filters, search::MAX_ROW_BYTES)? {
            return Err(Error::Invalid("search donor filters/source differs"));
        }
        let mut values = BTreeMap::new();
        for (category, entries) in [
            ("identity", &document.identities[..]),
            ("visible", &document.visible[..]),
            ("full", std::slice::from_ref(&document.searchable)),
        ] {
            for (field, value) in entries.iter().enumerate() {
                values.insert((category, field), value.as_bytes());
            }
        }
        self.meter.begin(&self.request)?;
        let mut statement = self.db.prepare(
            "SELECT category,field,byte_length FROM search_values WHERE doc_id=? LIMIT ?",
        )?;
        let mut rows = statement.query(params![document.doc_id, values.len() + 1])?;
        let mut seen = BTreeSet::new();
        while let Some(row) = rows.next()? {
            self.meter.row(row, &self.request)?;
            let category: String = row.get(0)?;
            let field: i64 = row.get(1)?;
            let length: i64 = row.get(2)?;
            let Some(value) = usize::try_from(field)
                .ok()
                .and_then(|field| values.get(&(category.as_str(), field)))
            else {
                return Err(Error::Invalid("search donor unexpected rank value"));
            };
            if length != value.len() as i64 || !seen.insert((category, field)) {
                return Err(Error::Invalid("search donor value/source framing differs"));
            }
        }
        if seen.len() != values.len() {
            return Err(Error::Invalid("search donor value population differs"));
        }
        drop(rows);
        drop(statement);
        let expected_chunks: usize = values
            .values()
            .map(|v| v.len().div_ceil(search::CHUNK_SIZE))
            .sum();
        self.meter.begin(&self.request)?;
        let mut statement = self.db.prepare("SELECT category,field,chunk,CASE WHEN length(payload)<=32768 THEN payload END FROM search_text_chunks WHERE doc_id=? ORDER BY category,field,chunk LIMIT ?")?;
        let mut rows = statement.query(params![document.doc_id, expected_chunks + 1])?;
        let mut count = 0usize;
        while let Some(row) = rows.next()? {
            self.meter.row(row, &self.request)?;
            let category: String = row.get(0)?;
            let field: i64 = row.get(1)?;
            let chunk: i64 = row.get(2)?;
            let payload: Vec<u8> = row.get(3)?;
            let value = usize::try_from(field)
                .ok()
                .and_then(|field| values.get(&(category.as_str(), field)))
                .ok_or(Error::Invalid("search donor unexpected text value"))?;
            let start = usize::try_from(chunk)
                .ok()
                .and_then(|n| n.checked_mul(search::CHUNK_SIZE))
                .ok_or(Error::Invalid("search donor text chunk index"))?;
            if payload.is_empty()
                || value.get(start..start.saturating_add(search::CHUNK_SIZE).min(value.len()))
                    != Some(payload.as_slice())
            {
                return Err(Error::Invalid("search donor text/source bytes differ"));
            }
            count += 1;
        }
        if count != expected_chunks {
            return Err(Error::Invalid("search donor text chunk population differs"));
        }
        drop(rows);
        drop(statement);
        let reverse = self.one("SELECT term_count,CASE WHEN length(payload) BETWEEN term_count AND min(1800000,9*term_count) THEN payload END,CASE WHEN length(digest)=32 THEN digest END FROM search_document_terms WHERE doc_id=?", [document.doc_id])?.ok_or(Error::Invalid("search donor reverse absent"))?;
        let term_count = positive_or_zero(&reverse[0])?
            .try_into()
            .map_err(|_| Error::Budget("search donor reverse count"))?;
        let reverse = search::decode_search_reverse(
            document.doc_id,
            &document.kind,
            term_count,
            blob(&reverse[1])?,
            blob(&reverse[2])?,
        )?;
        let mut expected = search::document_terms(&document)?;
        let mut digest = Digest256Hasher::new();
        for term in &reverse {
            self.meter.check()?;
            let entry = self.terms.get(term).ok_or(Error::Invalid(
                "search donor reverse dictionary term absent",
            ))?;
            if entry.kind != document.kind
                || !expected.remove(&(entry.plane, entry.n, entry.key.clone()))
            {
                return Err(Error::Invalid(
                    "search donor reverse/source membership differs",
                ));
            }
            digest.update(&term.to_be_bytes());
        }
        if !expected.is_empty() {
            return Err(Error::Invalid("search donor reverse omits source terms"));
        }
        let key = search::order_key(&document.identifier, document.source_order)?;
        self.meter.retain(768 + key.len(), &self.request)?;
        if self
            .documents
            .insert(
                document.doc_id,
                DocumentState {
                    kind: document.kind,
                    key,
                    reverse_count: reverse.len(),
                    reverse_digest: digest.finalize(),
                    forward_digest: Digest256Hasher::new(),
                    forward_count: 0,
                },
            )
            .is_some()
        {
            return Err(Error::Invalid(
                "duplicate search donor document observation",
            ));
        }
        Ok(())
    }
    pub fn finish_observation(&mut self, count: u64) -> Result<()> {
        self.meter.check()?;
        if self.finished
            || self.observed != self.count
            || count != self.count
            || self.documents.len() as u64 != count
        {
            return Err(Error::Invalid(
                "search donor complete population differs; full bootstrap required",
            ));
        }
        if self.rows_digest.clone().finalize().to_hex()
            != required(&self.descriptor, "rows_sha256")?
        {
            return Err(Error::Invalid(
                "search donor complete carrier digest differs",
            ));
        }
        for (i, name) in ["node_counts", "relation_counts"].iter().enumerate() {
            let counts = JsonValue::Array(
                self.histograms[i]
                    .iter()
                    .map(|(key, n)| {
                        JsonValue::Array(vec![
                            text(&key[0]),
                            text(&key[1]),
                            text(&key[2]),
                            number(*n),
                        ])
                    })
                    .collect(),
            );
            if !same(&counts, field(&self.lens, name)?, 1_048_576)? {
                return Err(Error::Invalid(
                    "search donor lens complete population differs",
                ));
            }
        }
        self.finished = true;
        Ok(())
    }
    pub fn should_replace(&self, kind: &str, identifier: &str) -> bool {
        self.changed
            .contains(&(kind.to_owned(), identifier.to_owned()))
    }
    pub fn changed_documents(&self) -> usize {
        self.changed.len()
    }
    pub fn binding(&self) -> &JsonValue {
        &self.request.binding
    }
    /// Copy the complete validated six-table population plus old header. The
    /// caller MUST first complete its second fresh carrier/hash pass, then apply
    /// exactly `changed_documents()` addressed updates using the remaining
    /// mutation allowance and a fresh binding via search::apply_delta.
    pub fn copy_into(
        &mut self,
        target: &Connection,
        maximum_pages: u64,
        max_mutations: u64,
    ) -> Result<ReuseReport> {
        if !self.finished
            || self.copied
            || target.is_autocommit()
            || maximum_pages == 0
            || max_mutations < 2
        {
            return Err(Error::Invalid(
                "search donor copy lifecycle/transaction/budget",
            ));
        }
        self.recheck_before_commit()?;
        let current: u64 = target.query_row("PRAGMA max_page_count", [], |r| r.get(0))?;
        let effective = maximum_pages.min(current);
        let actual: u64 = target.query_row("PRAGMA page_count", [], |r| r.get(0))?;
        if actual > effective {
            return Err(Error::Budget("search donor target whole main page cap"));
        }
        target.pragma_update(None, "max_page_count", effective)?;
        if target.query_row("PRAGMA max_page_count", [], |r| r.get::<_, u64>(0))? != effective {
            return Err(Error::Budget("search donor target pager cap enforcement"));
        }
        for ddl in search::DDL {
            target.execute(ddl, [])?;
        }
        let mut previous_term = None;
        let mut previous_last: Option<Vec<u8>> = None;
        for (table, columns) in TABLES {
            self.meter.check()?;
            self.report.copy_table_scans += 1;
            let size = columns
                .iter()
                .map(|column| format!("coalesce(length(CAST({column} AS BLOB)),0)"))
                .collect::<Vec<_>>()
                .join("+");
            let selected = columns
                .iter()
                .map(|column| format!("CASE WHEN ({size})<=? THEN {column} END"))
                .collect::<Vec<_>>()
                .join(",");
            let order = if *table == "search_blocks" {
                " ORDER BY term_id,lower_fence"
            } else {
                ""
            };
            let query = format!("SELECT {selected} FROM {table}{order}");
            self.meter.begin(&self.request)?;
            let mut statement = self.db.prepare(&query)?;
            let mut rows = statement.query(rusqlite::params_from_iter(std::iter::repeat_n(
                self.request.max_batch_bytes as u64,
                columns.len(),
            )))?;
            let insert = format!(
                "INSERT INTO {table} VALUES ({})",
                vec!["?"; columns.len()].join(",")
            );
            let mut batch = Vec::new();
            let mut batch_bytes = 0usize;
            while let Some(row) = rows.next()? {
                self.meter.row(row, &self.request)?;
                let values = (0..columns.len())
                    .map(|i| row.get(i))
                    .collect::<std::result::Result<Vec<Value>, _>>()?;
                if values
                    .iter()
                    .any(|v| matches!(v, Value::Null | Value::Real(_)))
                {
                    return Err(Error::Invalid(
                        "search donor row absent/noncanonical/over copy byte cap",
                    ));
                }
                if matches!(*table, "search_values" | "search_text_chunks")
                    && !self.documents.contains_key(&positive_or_zero(&values[0])?)
                {
                    return Err(Error::Invalid("search donor orphan document value/text"));
                }
                if *table == "search_blocks" {
                    let term = positive_or_zero(&values[0])?;
                    let fence = blob(&values[1])?;
                    let declared = positive_or_zero(&values[2])?;
                    let payload = blob(&values[3])?;
                    let entry = self
                        .terms
                        .get_mut(&term)
                        .ok_or(Error::Invalid("search donor forward term missing"))?;
                    if declared > search::BLOCK_SIZE as u64
                        || payload.len() > search::BLOCK_SIZE * 8
                        || fence.len() > search::MAX_ROW_BYTES
                    {
                        return Err(Error::Invalid("search donor forward block framing"));
                    }
                    let addresses = search::decode_search_postings(payload)?;
                    if addresses.len() as u64 != declared
                        || search::encode_search_postings(&addresses)? != payload
                    {
                        return Err(Error::Invalid(
                            "search donor forward codec/count canonical closure",
                        ));
                    }
                    if previous_term != Some(term) {
                        previous_term = Some(term);
                        previous_last = None;
                    }
                    if previous_last
                        .as_ref()
                        .is_some_and(|last| last.as_slice() >= fence)
                    {
                        return Err(Error::Invalid("search donor forward fence overlap"));
                    }
                    for address in addresses {
                        let document = self
                            .documents
                            .get_mut(&address)
                            .ok_or(Error::Invalid("search donor forward document absent"))?;
                        if document.kind != entry.kind
                            || document.key.as_slice() < fence
                            || previous_last
                                .as_ref()
                                .is_some_and(|last| document.key <= *last)
                        {
                            return Err(Error::Invalid(
                                "search donor posting kind/key/fence/source-order closure",
                            ));
                        }
                        previous_last = Some(document.key.clone());
                        document.forward_digest.update(&term.to_be_bytes());
                        document.forward_count += 1;
                    }
                    entry.copied = entry
                        .copied
                        .checked_add(declared)
                        .ok_or(Error::Budget("search donor posting count"))?;
                }
                let size = scalar_bytes(&values)?;
                self.report.copied_rows = self
                    .report
                    .copied_rows
                    .checked_add(1)
                    .filter(|n| *n <= self.request.max_copy_rows.min(max_mutations - 2))
                    .ok_or(Error::Budget("search donor copy row/mutation cap"))?;
                self.report.copied_bytes = self
                    .report
                    .copied_bytes
                    .checked_add(size as u64)
                    .filter(|n| *n <= self.request.max_copy_bytes)
                    .ok_or(Error::Budget("search donor copy bytes"))?;
                if size > self.request.max_batch_bytes {
                    return Err(Error::Budget("search donor row batch bytes"));
                }
                if !batch.is_empty() && batch_bytes + size > self.request.max_batch_bytes {
                    flush(
                        target,
                        &insert,
                        &mut batch,
                        &mut self.report,
                        max_mutations,
                        self.meter.deadline,
                    )?;
                    batch_bytes = 0;
                }
                batch.push(values);
                batch_bytes += size;
                self.report.peak_batch_rows = self.report.peak_batch_rows.max(batch.len());
                self.report.peak_batch_bytes = self.report.peak_batch_bytes.max(batch_bytes);
                if batch.len() >= BATCH_ROWS {
                    flush(
                        target,
                        &insert,
                        &mut batch,
                        &mut self.report,
                        max_mutations,
                        self.meter.deadline,
                    )?;
                    batch_bytes = 0;
                }
            }
            if !batch.is_empty() {
                flush(
                    target,
                    &insert,
                    &mut batch,
                    &mut self.report,
                    max_mutations,
                    self.meter.deadline,
                )?;
            }
        }
        if self
            .terms
            .values()
            .any(|entry| entry.expected != entry.copied)
            || self.documents.values().any(|document| {
                document.reverse_count != document.forward_count
                    || document.reverse_digest != document.forward_digest.clone().finalize()
            })
        {
            return Err(Error::Invalid(
                "search donor full forward/reverse/count closure differs",
            ));
        }
        self.terms.clear();
        self.documents.clear();
        let before = target.total_changes();
        let changed = target.execute(
            "INSERT INTO search_header VALUES (1,?1,?2,?3,?4)",
            params![
                search::header(&self.request.binding)?,
                self.generation,
                self.count,
                effective
            ],
        )?;
        if changed != 1 {
            return Err(Error::Invalid("search donor old header insertion"));
        }
        charge_mutations(
            &mut self.report,
            target.total_changes().saturating_sub(before).max(1),
            max_mutations,
        )?;
        self.report.write_calls += 1;
        self.copied = true;
        self.recheck_before_commit()?;
        Ok(self.report())
    }
    /// Same held RO snapshot and same selected inode. An external publication
    /// or WAL update is not inspected by opening another connection.
    pub fn recheck_before_commit(&self) -> Result<()> {
        self.meter.check()?;
        self.check_path()?;
        for _ in 0..RECHECK_STATEMENTS {
            self.meter.begin(&self.request)?;
        }
        let admission = PreparedReadTransaction::recheck_binding(
            &self.db,
            &self.request.binding,
            PreparedReadLimits {
                max_row_bytes: self.limits.max_metadata_bytes.max(65536),
                max_response_bytes: self.limits.max_metadata_bytes.max(65536),
                max_rows: self.request.max_queries.min(usize::MAX as u64) as usize,
                max_vm_steps: self.request.max_vm_steps,
                max_bytes: self
                    .request
                    .max_source_bytes
                    .saturating_sub(self.meter.source_bytes.get())
                    .min(usize::MAX as u64) as usize,
            },
        )
        .map_err(|e| Error::Source(e.to_string()))?;
        self.meter.charge_bytes(
            admission
                .read_bytes()
                .checked_add(
                    admission
                        .read_rows()
                        .checked_mul(128)
                        .ok_or(Error::Budget("search donor recheck scalar allowance"))?,
                )
                .ok_or(Error::Budget("search donor recheck source bytes"))?,
            &self.request,
        )?;
        self.meter.source_rows.set(
            self.meter
                .source_rows
                .get()
                .checked_add(admission.read_rows() as u64)
                .ok_or(Error::Budget("search donor recheck rows"))?,
        );
        if admission.statement_count() != RECHECK_STATEMENTS {
            return Err(Error::Invalid(
                "search donor snapshot recheck procedure changed",
            ));
        }
        let expected = search::header(&self.request.binding)?;
        let frame = self.one("SELECT CASE WHEN length(CAST(header AS BLOB))<=65536 THEN header END,CASE WHEN length(cursor_key)=32 THEN cursor_key END FROM search_header WHERE singleton=1", [])?.ok_or(Error::Invalid("search donor held search header absent"))?;
        if string(&frame[0])? != expected || blob(&frame[1])? != self.generation {
            return Err(Error::Invalid("search donor held incarnation differs"));
        }
        self.check_path()
    }
    fn check_path(&self) -> Result<()> {
        let actual = fs::symlink_metadata(&self.request.path)?;
        let held = self.anchor.metadata()?;
        if !actual.is_file()
            || actual.file_type().is_symlink()
            || (actual.dev(), actual.ino()) != self.stamp
            || (held.dev(), held.ino()) != self.stamp
        {
            return Err(Error::Invalid(
                "search donor selected file identity changed",
            ));
        }
        Ok(())
    }
    pub fn report(&self) -> ReuseReport {
        let mut report = self.report.clone();
        report.source_bytes = self.meter.source_bytes.get();
        report.source_rows = self.meter.source_rows.get();
        report.queries = self.meter.queries.get();
        report.vm_steps_charged = self.meter.vm.load(Ordering::Relaxed);
        report.validation_state_bytes = self.meter.state_bytes.get();
        report.validation_state_peak_bytes = self.meter.state_peak.get();
        report.changed_documents = self.changed.len();
        report.changed_bytes = self.changed_bytes;
        report.observed_documents = self.observed;
        report.elapsed_micros = self.started.elapsed().as_micros().min(u64::MAX as u128) as u64;
        report
    }
}
fn charge_mutations(report: &mut ReuseReport, amount: u64, cap: u64) -> Result<()> {
    report.mutations = report
        .mutations
        .checked_add(amount)
        .filter(|n| *n <= cap)
        .ok_or(Error::Budget(
            "search donor copy mutation cap; abort whole target transaction",
        ))?;
    Ok(())
}
fn flush(
    target: &Connection,
    sql: &str,
    batch: &mut Vec<Vec<Value>>,
    report: &mut ReuseReport,
    cap: u64,
    deadline: Option<Instant>,
) -> Result<()> {
    let mut statement = target.prepare_cached(sql)?;
    for values in batch.iter() {
        if deadline.is_some_and(|d| Instant::now() >= d) {
            return Err(Error::Budget("search donor copy deadline"));
        }
        let before = target.total_changes();
        if statement.execute(rusqlite::params_from_iter(values))? != 1 {
            return Err(Error::Invalid("search donor copy row insertion closure"));
        }
        charge_mutations(
            report,
            target.total_changes().saturating_sub(before).max(1),
            cap,
        )?;
        report.write_calls += 1;
    }
    report.batches += 1;
    batch.clear();
    Ok(())
}
fn normalize_sql(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
fn positive_or_zero(value: &Value) -> Result<u64> {
    match value {
        Value::Integer(v) if *v >= 0 => Ok(*v as u64),
        _ => Err(Error::Invalid("search donor SQL integer")),
    }
}
fn string(value: &Value) -> Result<&str> {
    match value {
        Value::Text(v) => Ok(v),
        _ => Err(Error::Invalid("search donor missing/oversized SQL text")),
    }
}
fn blob(value: &Value) -> Result<&[u8]> {
    match value {
        Value::Blob(v) => Ok(v),
        _ => Err(Error::Invalid("search donor missing/oversized SQL blob")),
    }
}
fn scalar_bytes(values: &[Value]) -> Result<usize> {
    values.iter().try_fold(0usize, |total, value| {
        total
            .checked_add(match value {
                Value::Integer(v) => v.to_string().len(),
                Value::Text(v) => v.len(),
                Value::Blob(v) => v.len(),
                _ => return Err(Error::Invalid("search donor copied SQL scalar")),
            })
            .ok_or(Error::Budget("search donor copy scalar bytes"))
    })
}
fn text(v: &str) -> JsonValue {
    JsonValue::String(JsonString::from_utf8(v))
}
fn number(n: u64) -> JsonValue {
    JsonValue::Number(JsonNumber {
        kind: JsonNumberKind::Int,
        lexeme: n.to_string(),
    })
}
fn field<'a>(v: &'a JsonValue, key: &str) -> Result<&'a JsonValue> {
    v.object_get(key)
        .ok_or(Error::Invalid("search donor required metadata field"))
}
fn required<'a>(v: &'a JsonValue, key: &str) -> Result<&'a str> {
    field(v, key)?
        .as_str()
        .ok_or(Error::Invalid("search donor required metadata string"))
}
fn digest(v: &JsonValue, cap: usize) -> Result<String> {
    Ok(Digest256::of_bytes(owner::compact(v, cap)?.as_bytes()).to_hex())
}
fn same(a: &JsonValue, b: &JsonValue, cap: usize) -> Result<bool> {
    let limits = JsonLimits::new(cap, 96, 1_000_000, 4300)
        .map_err(|_| Error::Budget("search donor metadata comparison"))?;
    Ok(
        canonical_bytes_v1(a, CanonicalProfile::SourceRecordDigestV1, limits)
            .map_err(|e| Error::Source(e.to_string()))?
            == canonical_bytes_v1(b, CanonicalProfile::SourceRecordDigestV1, limits)
                .map_err(|e| Error::Source(e.to_string()))?,
    )
}
fn columns(kind: &str) -> &'static [&'static str] {
    if kind == "node" {
        &[
            "id",
            "entity_id",
            "native_id",
            "source_graph",
            "kind_id",
            "type_id",
        ]
    } else {
        &[
            "id",
            "native_id",
            "source_graph",
            "from_id",
            "to_id",
            "predicate_id",
            "relation_type_id",
        ]
    }
}
fn dimensions(kind: &str) -> [&'static str; 3] {
    if kind == "node" {
        ["source_graph", "kind_id", "type_id"]
    } else {
        ["source_graph", "predicate_id", "relation_type_id"]
    }
}
fn verify_lens(lens: &JsonValue, header: &JsonValue) -> Result<()> {
    let fields = [
        "schema",
        "execution_version",
        "source_revision",
        "sort_key",
        "unicode_version",
        "query_properties",
        "node_counts",
        "relation_counts",
    ];
    if lens.as_object().is_none_or(|o| {
        o.len() != fields.len()
            || o.iter()
                .any(|(key, _)| key.as_str().is_none_or(|key| !fields.contains(&key)))
    }) || required(lens, "schema")? != "tos_published_lens_metadata_v1"
        || required(lens, "execution_version")? != "tos-lens-execution-v7"
        || field(lens, "source_revision")? != field(header, "source_revision")?
        || required(lens, "sort_key")? != "python-str-or-empty-lower-v1"
        || required(lens, "unicode_version")? != "16.0.0"
    {
        return Err(Error::Invalid("search donor lens metadata profile"));
    }
    let properties = field(lens, "query_properties")?
        .as_array()
        .filter(|a| a.len() <= 4096)
        .ok_or(Error::Invalid("search donor lens query properties"))?;
    if !same(
        field(lens, "query_properties")?,
        header
            .object_get("query_properties")
            .unwrap_or(&JsonValue::Array(vec![])),
        1_048_576,
    )? {
        return Err(Error::Invalid(
            "search donor lens/header query properties differ",
        ));
    }
    for property in properties {
        for key in ["property_id", "field", "value_type"] {
            if required(property, key)?.is_empty() {
                return Err(Error::Invalid("search donor lens property identifier"));
            }
        }
        if !matches!(property.object_get("inherited"), Some(JsonValue::Bool(_))) {
            return Err(Error::Invalid("search donor lens property inherited"));
        }
        for key in ["applies_to", "operators"] {
            if field(property, key)?
                .as_array()
                .is_none_or(|a| a.iter().any(|v| v.as_str().is_none_or(str::is_empty)))
            {
                return Err(Error::Invalid("search donor lens property string lists"));
            }
        }
    }
    for name in ["node_counts", "relation_counts"] {
        let mut previous = None;
        let cells = field(lens, name)?
            .as_array()
            .filter(|a| a.len() <= MAX_HISTOGRAM_CELLS)
            .ok_or(Error::Invalid("search donor lens histogram size"))?;
        for cell in cells {
            let parts = cell
                .as_array()
                .filter(|a| a.len() == 4)
                .ok_or(Error::Invalid("search donor lens histogram cell"))?;
            let key = [
                parts[0].as_str().unwrap_or(""),
                parts[1].as_str().unwrap_or(""),
                parts[2].as_str().unwrap_or(""),
            ];
            if key.iter().any(|s| s.is_empty())
                || parts[3]
                    .as_u64()
                    .is_none_or(|n| n == 0 || n > search::MAX_ADDRESS)
                || previous.is_some_and(|p| p >= key)
            {
                return Err(Error::Invalid("search donor lens histogram order/count"));
            }
            previous = Some(key);
        }
    }
    Ok(())
}
