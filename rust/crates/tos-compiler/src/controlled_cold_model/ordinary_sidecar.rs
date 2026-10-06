//! Reference v3 candidate carrier over one authenticated normalized snapshot.
//! Filesystem admission, publication and stable-path custody remain with Access.
//! This owner never opens a path, allocates a counter, or hydrates from the cache.
use super::*;
use tos_foundation::Digest256Hasher;
use tos_source_store::PinnedSqliteConnection;

/// Rebuild is a cache admission outcome, never an operation failure.
#[derive(Debug)]
pub enum SearchSidecarAdmissionError {
    RebuildRequired,
    Operation(Error),
}
pub const SCHEMA: &str = "tos_knowledge_search_read_model_v3";
pub const NORMALIZATION: &str = "lower-json-sort-keys-ensure-ascii-false-surrogatepass-v1";
const CREATE: &str = r#"
CREATE TABLE metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL);
CREATE TABLE search_documents(kind TEXT NOT NULL,position INTEGER NOT NULL,id TEXT NOT NULL,
source_graph TEXT NOT NULL,kind_id TEXT NOT NULL,predicate_id TEXT NOT NULL,
id_lower TEXT NOT NULL,native_id_lower TEXT NOT NULL,identity_values TEXT NOT NULL,
visible_values TEXT NOT NULL,document_chars INTEGER NOT NULL,document_digest BLOB NOT NULL,
PRIMARY KEY(kind,position)) WITHOUT ROWID;
CREATE TABLE search_grams(kind TEXT NOT NULL,n INTEGER NOT NULL,gram BLOB NOT NULL,
position INTEGER NOT NULL,PRIMARY KEY(kind,n,gram,position)) WITHOUT ROWID;
CREATE TABLE search_gram_stats(kind TEXT NOT NULL,n INTEGER NOT NULL,gram BLOB NOT NULL,
postings INTEGER NOT NULL,PRIMARY KEY(kind,n,gram)) WITHOUT ROWID;
CREATE INDEX search_document_filter ON search_documents(kind,source_graph,kind_id,predicate_id,position);
"#;

pub struct ControlledSidecarModel<'borrow, 'model, 'state, 'budget> {
    source: &'borrow mut ControlledKnowledgeModel<'model, 'state, 'budget>,
    cache: &'borrow PinnedSqliteConnection,
    check_cache: &'borrow dyn Fn() -> Result<()>,
}

impl ControlledSidecarModel<'_, '_, '_, '_> {
    pub fn check_pin(&self) -> Result<()> {
        self.source.check_pin()?;
        (self.check_cache)()?;
        self.source.check_pin()
    }
    pub fn selection(&self) -> &KnowledgeSelectedExpectation {
        self.source.selection()
    }
    pub fn source_basis(&self) -> &KnowledgeSourceBasis {
        self.source.source_basis()
    }
    pub fn search_index_profile(&self) -> &'static str {
        self.source.search_index_profile()
    }
    pub fn navigation_original_receipt(&self) -> Option<&crate::NavigationOriginalReceipt> {
        self.source.navigation_original_receipt()
    }
    pub fn philosophy_original_receipt(&self) -> Option<&crate::PhilosophyOriginalReceipt> {
        self.source.philosophy_original_receipt()
    }
    pub fn corpus_original_receipt(&self) -> Option<&crate::CorpusOriginalReceipt> {
        self.source.corpus_original_receipt()
    }
    pub fn charge_query_work(&self, n: usize) -> Result<()> {
        self.check_pin()?;
        self.source.charge_query_work(n)?;
        self.check_pin()
    }
    pub fn check_query_open_vm_admission(&self, n: u64) -> Result<()> {
        self.source.check_query_open_vm_admission(n)?;
        self.check_pin()
    }
    pub fn available_query_workspace_bytes(&self) -> Result<usize> {
        self.check_pin()?;
        self.source.available_query_workspace_bytes()
    }
    pub fn with_owned_query_workspace(
        &mut self,
        n: usize,
        run: impl FnOnce(&mut Self) -> Result<()>,
    ) -> Result<()> {
        if n == 0 {
            return Err(Error::Budget("sidecar query workspace"));
        }
        let context = self.source.context;
        context.check()?;
        let result = context.with_reserved_state(n, || run(self));
        context.check()?;
        result
    }
    pub fn controlled_binding_workspace_upper_bound(
        &self,
        v: &QueryVocabulary,
        d: &[u8],
    ) -> Result<usize> {
        self.source.controlled_binding_workspace_upper_bound(v, d)
    }
    pub fn verify_authored_vocabulary(&self, v: &QueryVocabulary, d: &[u8]) -> Result<()> {
        self.source.verify_authored_vocabulary(v, d)
    }
    pub fn with_owned_binding_workspace(
        &mut self,
        v: &QueryVocabulary,
        d: &[u8],
        run: impl for<'v> FnOnce(&mut Self, &'v tos_foundation::JsonValue) -> Result<()>,
    ) -> Result<()> {
        let n = self.controlled_binding_workspace_upper_bound(v, d)?;
        let context = self.source.context;
        context.with_reserved_state(n, || {
            self.verify_authored_vocabulary(v, d)?;
            let limits = JsonLimits::new(d.len(), 64, 100_000, 4096)
                .map_err(|_| Error::Budget("sidecar binding JSON"))?;
            context.with_foundation_owned_with_limits(d, limits, |value| run(self, value))
        })
    }
    pub fn exact_candidate(
        &mut self,
        kind: ControlledSearchKind,
        position: u64,
        vm: u64,
        decoded: u64,
        payload: usize,
        field: usize,
        chars: u64,
    ) -> Result<ControlledSearchCandidate> {
        self.check_pin()?;
        let mut selected = self
            .source
            .exact_candidate(kind, position, vm, decoded, payload, field, chars)?;
        // Cache row validation is independent of normalized model validation.
        let context = self.source.context;
        let _hold = context
            .owned_state()
            .hold(PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound())?;
        let (cache_decoded, cache_vm) = crate::knowledge_payload_read::with_query_vm_window(
            context,
            self.cache,
            vm,
            || {
                let mut stmt=self.cache.prepare_cached("SELECT id,source_graph,kind_id,predicate_id,id_lower,native_id_lower,identity_values,visible_values,document_chars,document_digest FROM search_documents WHERE kind=?1 AND position=?2")?;
                let mut rows = stmt.query(params![kind_name(kind), position as i64])?;
                let row = rows
                    .next()?
                    .ok_or(Error::Invalid("sidecar selected document absent"))?;
                let strings = [
                    &selected.id,
                    &selected.source_graph,
                    &selected.kind_id,
                    &selected.predicate_id,
                    &selected.id_lower,
                    &selected.native_id_lower,
                    &selected.identity_values,
                    &selected.visible_values,
                ];
                let mut cache_decoded = 40u64;
                for (i, expected) in strings.into_iter().enumerate() {
                    match row.get_ref(i)? {
                        rusqlite::types::ValueRef::Text(actual)
                            if actual == expected.as_bytes() =>
                        {
                            ()
                        }
                        _ => return Err(Error::Invalid("sidecar selected document fields")),
                    }
                    context.charge_work(expected.len())?;
                    cache_decoded = cache_decoded
                        .checked_add(expected.len() as u64)
                        .ok_or(Error::Budget("sidecar candidate decoded size"))?;
                }
                if row.get::<_, i64>(8)? != selected.document_chars as i64
                    || blob(row.get_ref(9)?)? != selected.document_digest.as_bytes()
                {
                    return Err(Error::Invalid("sidecar selected document digest"));
                }
                Ok(cache_decoded)
            },
        )?;
        selected.vm_steps = selected
            .vm_steps
            .checked_add(cache_vm)
            .filter(|n| *n <= vm)
            .ok_or(Error::Budget("sidecar candidate VM page allowance"))?;
        selected.decoded_bytes = selected
            .decoded_bytes
            .checked_add(cache_decoded)
            .filter(|n| *n <= decoded)
            .ok_or(Error::Budget("sidecar candidate decoded page allowance"))?;
        selected.rows = selected
            .rows
            .checked_add(1)
            .ok_or(Error::Budget("sidecar candidate rows"))?;
        self.check_pin()?;
        Ok(selected)
    }
    pub fn gram_stat(
        &mut self,
        kind: ControlledSearchKind,
        gram: &str,
        vm: u64,
        max_rows: u64,
        decoded: u64,
    ) -> Result<ControlledGramStat> {
        self.check_pin()?;
        if gram.chars().count() != 3 || gram.len() > 12 || vm == 0 || max_rows == 0 || decoded < 8 {
            return Err(Error::Budget("sidecar gram stat"));
        }
        let context = self.source.context;
        let _hold = context
            .owned_state()
            .hold(PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound())?;
        context.charge_work(gram.len() + 16)?;
        let (value, vm_steps) = crate::knowledge_payload_read::with_query_vm_window(
            context,
            self.cache,
            vm,
            || {
                Ok(self.cache.query_row("SELECT CASE WHEN typeof(postings)='integer' THEN postings END FROM search_gram_stats WHERE kind=?1 AND n=3 AND gram=?2",params![kind_name(kind),gram.as_bytes()],|row|row.get::<_,Option<i64>>(0)).optional()?)
            },
        )?;
        let value = value.flatten();
        if value.is_some_and(|n| n < 0) {
            return Err(Error::Invalid("sidecar gram stat row"));
        }
        self.check_pin()?;
        let rows = u64::from(value.is_some());
        Ok(ControlledGramStat {
            postings: value.map(|n| n as u64),
            vm_steps,
            rows,
            decoded_bytes: rows * 8,
        })
    }
    pub fn seek_postings(
        &mut self,
        kind: ControlledSearchKind,
        gram: &str,
        after: Option<u64>,
        max_rows: usize,
        vm: u64,
        decoded: u64,
    ) -> Result<ControlledPostingPage> {
        self.check_pin()?;
        if gram.chars().count() != 3
            || gram.len() > 12
            || max_rows == 0
            || max_rows > 1024
            || vm == 0
            || after.is_some_and(|p| p > i64::MAX as u64)
        {
            return Err(Error::Budget("sidecar posting request"));
        }
        let bytes = max_rows
            .checked_mul(8)
            .ok_or(Error::Budget("sidecar posting bytes"))?;
        if bytes as u64 > decoded {
            return Err(Error::Budget("sidecar posting decoded bytes"));
        }
        let context = self.source.context;
        let _hold = context.owned_state().hold(
            bytes
                + std::mem::size_of::<Vec<u64>>()
                + PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound(),
        )?;
        context.charge_work(gram.len() + 16)?;
        let (mut positions, vm_steps) = crate::knowledge_payload_read::with_query_vm_window(
            context,
            self.cache,
            vm,
            || {
                let mut result = Vec::new();
                result
                    .try_reserve_exact(max_rows)
                    .map_err(|_| Error::Budget("sidecar posting allocation"))?;
                let mut stmt=self.cache.prepare_cached("SELECT position FROM search_grams WHERE kind=?1 AND n=3 AND gram=?2 AND position>?3 ORDER BY position LIMIT ?4")?;
                let mut rows = stmt.query(params![
                    kind_name(kind),
                    gram.as_bytes(),
                    after.map_or(-1, |p| p as i64),
                    max_rows as i64
                ])?;
                let mut previous = after;
                while let Some(row) = rows.next()? {
                    context.check()?;
                    let p: i64 = row.get(0)?;
                    if p < 0 || previous.is_some_and(|n| p as u64 <= n) {
                        return Err(Error::Invalid("sidecar posting position"));
                    }
                    previous = Some(p as u64);
                    result.push(p as u64);
                    context.charge_work(8)?;
                }
                Ok(result)
            },
        )?;
        let rows = positions.len() as u64;
        let exhausted = positions.len() < max_rows;
        positions.truncate(max_rows);
        self.check_pin()?;
        Ok(ControlledPostingPage {
            positions,
            exhausted,
            vm_steps,
            rows,
            decoded_bytes: rows * 8,
        })
    }
}
fn blob(value: rusqlite::types::ValueRef<'_>) -> Result<&[u8]> {
    match value {
        rusqlite::types::ValueRef::Blob(v) => Ok(v),
        _ => Err(Error::Invalid("sidecar blob shape")),
    }
}
fn text(value: rusqlite::types::ValueRef<'_>) -> Result<&str> {
    match value {
        rusqlite::types::ValueRef::Text(v) => {
            std::str::from_utf8(v).map_err(|_| Error::Invalid("sidecar text encoding"))
        }
        _ => Err(Error::Invalid("sidecar text shape")),
    }
}
fn kind_name(kind: ControlledSearchKind) -> &'static str {
    match kind {
        ControlledSearchKind::Nodes => "nodes",
        ControlledSearchKind::Relations => "relations",
    }
}

impl ControlledKnowledgeModel<'_, '_, '_> {
    /// A native cache operation may narrow the authentic cutoff; it must use
    /// the original cancellation owner, never a fresh independently live flag.
    pub fn verify_search_cache_operation(
        &self,
        deadline: std::time::Instant,
        cancelled: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<()> {
        self.context.check()?;
        if deadline != self.context.owned_state().operation_deadline()
            || !std::sync::Arc::ptr_eq(cancelled, &self.context.owned_state().cancellation_handle())
        {
            return Err(Error::Invalid(
                "sidecar original cutoff/cancellation owner differs",
            ));
        }
        self.check_pin()
    }
    /// Unit-only cache ownership scope. The borrowed charge function targets
    /// this model's authentic original state and cannot create/reset a grant.
    pub fn with_owned_search_cache_workspace(
        &mut self,
        forecast: usize,
        run: impl FnOnce(&mut Self, &dyn Fn(usize) -> Result<()>) -> Result<()>,
    ) -> Result<()> {
        let context = self.context;
        self.with_owned_query_workspace(forecast, |source| {
            let charge = |bytes| {
                context.check()?;
                context.charge_work(bytes)?;
                context.check()
            };
            run(source, &charge)
        })
    }
    /// Authenticated search_documents were verified during cold admission.
    /// Ordered document digests reproduce Reference's full searchable snapshot.
    pub fn search_sidecar_snapshot_digest(&self) -> Result<Digest256> {
        self.check_pin()?;
        let state = self.context.owned_state();
        let _hold = state
            .hold(PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound() + 256)?;
        let mut digest = Digest256Hasher::new();
        for (kind, count) in [
            ("nodes", self.selection.node_count),
            ("relations", self.selection.relation_count),
        ] {
            let mut stmt=self.connection.prepare_cached("SELECT position,document_digest FROM search_documents WHERE kind=?1 ORDER BY position")?;
            let mut rows = stmt.query([kind])?;
            let mut expected = 0u64;
            while let Some(row) = rows.next()? {
                self.context.check()?;
                let position: i64 = row.get(0)?;
                if position < 0 || position as u64 != expected {
                    return Err(Error::Invalid("sidecar source document order"));
                }
                let bytes = blob(row.get_ref(1)?)?;
                if bytes.len() != 32 {
                    return Err(Error::Invalid("sidecar source digest shape"));
                }
                // Decimal formatting uses a fixed stack buffer, no per-row heap.
                let mut decimal = [0u8; 20];
                let mut n = expected;
                let mut start = 20;
                loop {
                    start -= 1;
                    decimal[start] = b'0' + (n % 10) as u8;
                    n /= 10;
                    if n == 0 {
                        break;
                    }
                }
                state.charge_work(kind.len() + 2 + 20 + 32)?;
                digest.update(kind.as_bytes());
                digest.update(b"\0");
                digest.update(&decimal[start..]);
                digest.update(b"\0");
                digest.update(bytes);
                expected = expected
                    .checked_add(1)
                    .ok_or(Error::Budget("sidecar source positions"))?;
            }
            if expected != count {
                return Err(Error::Invalid("sidecar source document coverage"));
            }
        }
        self.check_pin()?;
        Ok(digest.finalize())
    }
    pub fn with_search_sidecar(
        &mut self,
        cache: &PinnedSqliteConnection,
        graph_schema: &str,
        check_cache: &dyn Fn() -> Result<()>,
        run: impl FnOnce(&mut ControlledSidecarModel<'_, '_, '_, '_>) -> Result<()>,
    ) -> Result<()> {
        self.check_pin()?;
        check_cache()?;
        let context = self.context;
        let state = context.owned_state();
        let _hold = state.hold(
            PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound()
                + RuntimeKnowledgeReadContext::sql_callback_retained_state_bytes()
                + 1024,
        )?;
        self.admit_search_sidecar(cache, graph_schema, check_cache)
            .map_err(|e| match e {
                SearchSidecarAdmissionError::RebuildRequired => {
                    Error::Invalid("sidecar requires rebuild")
                }
                SearchSidecarAdmissionError::Operation(e) => e,
            })?;
        check_cache()?;
        self.check_pin()?;
        let hook = context.install_operation_sql_controller(cache, state.sql_vm_limit())?;
        let result = {
            let mut model = ControlledSidecarModel {
                source: self,
                cache,
                check_cache,
            };
            run(&mut model)
        };
        context.check()?;
        check_cache()?;
        self.check_pin()?;
        drop(hook);
        result
    }
    /// Access calls this before deciding whether to rebuild. Cache read/schema
    /// failures are distinct from source, cutoff, cancellation and VM failures.
    pub fn admit_search_sidecar(
        &self,
        cache: &PinnedSqliteConnection,
        graph_schema: &str,
        check_cache: &dyn Fn() -> Result<()>,
    ) -> std::result::Result<(), SearchSidecarAdmissionError> {
        use SearchSidecarAdmissionError::{Operation, RebuildRequired};
        self.check_pin().map_err(Operation)?;
        check_cache().map_err(Operation)?;
        let context = self.context;
        let state = context.owned_state();
        let _hold = state
            .hold(
                PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound()
                    + RuntimeKnowledgeReadContext::sql_callback_retained_state_bytes()
                    + 1024,
            )
            .map_err(Operation)?;
        let hook = context
            .install_operation_sql_controller(cache, state.sql_vm_limit())
            .map_err(Operation)?;
        let snapshot = self
            .search_sidecar_snapshot_digest()
            .map_err(Operation)?
            .to_hex();
        let revision = self
            .source_basis
            .source_revision()
            .ok_or(Operation(Error::Invalid("sidecar source revision absent")))?;
        for (key, value) in [
            ("schema", SCHEMA),
            ("complete", "true"),
            ("normalization", NORMALIZATION),
            ("ngram_size", "3"),
            ("graph_schema", graph_schema),
            ("source_revision", revision),
            ("snapshot_digest", snapshot.as_str()),
        ] {
            state
                .charge_work(key.len() + value.len())
                .map_err(Operation)?;
            let read = (|| -> rusqlite::Result<bool> {
                let mut stmt = cache.prepare_cached("SELECT value FROM metadata WHERE key=?1")?;
                let mut rows = stmt.query([key])?;
                match rows.next()? {
                    None => Ok(false),
                    Some(row) => Ok(
                        matches!(row.get_ref(0)?,rusqlite::types::ValueRef::Text(v) if v==value.as_bytes()),
                    ),
                }
            })();
            context.check().map_err(Operation)?;
            check_cache().map_err(Operation)?;
            self.check_pin().map_err(Operation)?;
            match read {
                Ok(true) => (),
                Ok(false) => return Err(RebuildRequired),
                Err(e) => {
                    use rusqlite::ErrorCode::*;
                    if e.sqlite_error_code().is_some_and(|code| {
                        matches!(
                            code,
                            OperationInterrupted
                                | DatabaseBusy
                                | DatabaseLocked
                                | OutOfMemory
                                | SystemIoFailure
                                | DiskFull
                        )
                    }) {
                        return Err(Operation(Error::from(e)));
                    }
                    return Err(RebuildRequired);
                }
            }
        }
        drop(hook);
        Ok(())
    }
    /// Write one complete v3 candidate carrier into an Access-owned fresh
    /// private inode. On error Access discards it; publication occurs only after
    /// this unit-only callback and the SQLite close have succeeded.
    pub fn build_search_sidecar(
        &mut self,
        cache: &PinnedSqliteConnection,
        graph_schema: &str,
        max_bytes: u64,
        max_postings: u64,
        check_cache: &dyn Fn() -> Result<()>,
    ) -> Result<()> {
        if max_bytes < 4096 || max_postings == 0 || max_bytes / 4096 > i64::MAX as u64 {
            return Err(Error::Budget("sidecar build caps"));
        }
        self.check_pin()?;
        check_cache()?;
        let context = self.context;
        let state = context.owned_state();
        let _hold = state.hold(
            3 * PinnedSqliteConnection::bounded_statement_rust_workspace_upper_bound()
                + RuntimeKnowledgeReadContext::sql_callback_retained_state_bytes()
                + crate::MAX_POSTING_DELTA_BYTES
                + crate::MAX_POSTINGS_PER_BLOCK as usize * 8
                + 2048,
        )?;
        let hook = context.install_operation_sql_controller(cache, state.sql_vm_limit())?;
        cache.execute_batch("PRAGMA journal_mode=OFF; PRAGMA synchronous=FULL; PRAGMA temp_store=MEMORY; PRAGMA mmap_size=0;")?;
        let page_size: i64 = cache.query_row("PRAGMA page_size", [], |r| r.get(0))?;
        if page_size != 4096 {
            return Err(Error::Invalid("sidecar SQLite page size"));
        }
        cache.pragma_update(None, "max_page_count", (max_bytes / 4096) as i64)?;
        let tables: i64 =
            cache.query_row("SELECT count(*) FROM sqlite_schema", [], |r| r.get(0))?;
        if tables != 0 {
            return Err(Error::Invalid("sidecar build requires fresh database"));
        }
        cache.execute_batch(CREATE)?;
        cache.execute_batch("BEGIN IMMEDIATE")?;
        let result = (|| {
            let snapshot = self.search_sidecar_snapshot_digest()?.to_hex();
            let revision = self
                .source_basis
                .source_revision()
                .ok_or(Error::Invalid("sidecar source revision absent"))?;
            for (key, value) in [
                ("schema", SCHEMA),
                ("complete", "false"),
                ("normalization", NORMALIZATION),
                ("ngram_size", "3"),
                ("graph_schema", graph_schema),
                ("source_revision", revision),
                ("snapshot_digest", snapshot.as_str()),
            ] {
                context.check()?;
                state.charge_work(key.len() + value.len())?;
                cache.execute(
                    "INSERT INTO metadata(key,value) VALUES(?1,?2)",
                    params![key, value],
                )?;
            }
            for (kind, count) in [
                ("nodes", self.selection.node_count),
                ("relations", self.selection.relation_count),
            ] {
                let mut source=self.connection.prepare_cached("SELECT kind,position,id,source_graph,kind_id,predicate_id,id_lower,native_id_lower,identity_values,visible_values,document_chars,document_digest FROM search_documents WHERE kind=?1 ORDER BY position")?;
                let mut rows = source.query([kind])?;
                let mut write = cache.prepare_cached(
                    "INSERT INTO search_documents VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                )?;
                let mut position = 0u64;
                while let Some(row) = rows.next()? {
                    self.check_pin()?;
                    check_cache()?;
                    let actual: i64 = row.get(1)?;
                    let chars: i64 = row.get(10)?;
                    if actual < 0 || actual as u64 != position || chars < 0 || chars > 8_000_000 {
                        return Err(Error::Invalid("sidecar source document coverage/size"));
                    }
                    let values = [
                        row.get_ref(0)?,
                        row.get_ref(1)?,
                        row.get_ref(2)?,
                        row.get_ref(3)?,
                        row.get_ref(4)?,
                        row.get_ref(5)?,
                        row.get_ref(6)?,
                        row.get_ref(7)?,
                        row.get_ref(8)?,
                        row.get_ref(9)?,
                        row.get_ref(10)?,
                        row.get_ref(11)?,
                    ];
                    for value in values {
                        state.charge_work(match value {
                            rusqlite::types::ValueRef::Text(v)
                            | rusqlite::types::ValueRef::Blob(v) => v.len(),
                            _ => 8,
                        })?;
                    }
                    write.execute(rusqlite::params_from_iter(
                        values.map(rusqlite::types::ToSqlOutput::Borrowed),
                    ))?;
                    position = position
                        .checked_add(1)
                        .ok_or(Error::Budget("sidecar document count"))?;
                }
                if position != count {
                    return Err(Error::Invalid("sidecar source document coverage"));
                }
            }
            let mut source=self.connection.prepare_cached("SELECT kind,n,gram,first_position,last_position,postings,deltas FROM search_posting_blocks ORDER BY kind,n,gram,last_position")?;
            let mut rows = source.query([])?;
            let mut write = cache.prepare_cached(
                "INSERT INTO search_grams(kind,n,gram,position) VALUES(?1,?2,?3,?4)",
            )?;
            let mut total = 0u64;
            while let Some(row) = rows.next()? {
                self.check_pin()?;
                check_cache()?;
                let kind = text(row.get_ref(0)?)?;
                let n: i64 = row.get(1)?;
                let gram = blob(row.get_ref(2)?)?;
                let first: i64 = row.get(3)?;
                let last: i64 = row.get(4)?;
                let postings: i64 = row.get(5)?;
                let deltas = blob(row.get_ref(6)?)?;
                if !matches!(kind, "nodes" | "relations")
                    || n != 3
                    || gram.len() > 12
                    || first < 0
                    || last < first
                    || postings < 1
                    || postings > crate::MAX_POSTINGS_PER_BLOCK as i64
                    || deltas.len() > crate::MAX_POSTING_DELTA_BYTES
                {
                    return Err(Error::Invalid("sidecar source block shape"));
                }
                total = total
                    .checked_add(postings as u64)
                    .ok_or(Error::Budget("sidecar posting count"))?;
                if total > max_postings {
                    return Err(Error::Budget("sidecar rebuild posting count"));
                }
                state.charge_work(kind.len() + gram.len() + deltas.len() + 24)?;
                let positions = crate::decode_posting_block(
                    first as u64,
                    last as u64,
                    postings as u16,
                    deltas,
                )?;
                for position in positions {
                    context.check()?;
                    state.charge_work(gram.len() + kind.len() + 16)?;
                    write.execute(params![kind, n, gram, position as i64])?;
                }
            }
            cache.execute_batch("INSERT INTO search_gram_stats SELECT kind,n,gram,count(*) FROM search_grams GROUP BY kind,n,gram; UPDATE metadata SET value='true' WHERE key='complete';")?;
            let pages: i64 = cache.query_row("PRAGMA page_count", [], |r| r.get(0))?;
            if pages < 0
                || (pages as u64)
                    .checked_mul(4096)
                    .is_none_or(|n| n > max_bytes)
            {
                return Err(Error::Budget("sidecar rebuild page bytes"));
            }
            self.check_pin()?;
            check_cache()?;
            context.check()?;
            cache.execute_batch("COMMIT")?;
            self.check_pin()?;
            check_cache()?;
            context.check()
        })();
        if result.is_err() {
            let _ = cache.execute_batch("ROLLBACK");
        }
        drop(hook);
        result
    }
}
