//! Explicit empty-store bulk alternative over the maintained search storage v3.
//!
//! The owner keeps the main transaction, carrier writes and rollback. A new
//! exclusive scratch file holds compressed tails, never per-posting SQL rows.
//! It is removed on success or failure and never selected as a publication.
//! Disk reservation, main progress policy and whole-operation deadline belong
//! to the caller; the same deadline also governs this private scratch engine.

use crate::{Error, Result, local_prepared_search as search};
use rusqlite::{Connection, OpenFlags, OptionalExtension, Params, Statement, params};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    rc::Rc,
    time::Instant,
};
use tos_foundation::{Digest256Hasher, JsonValue};

const MAX_BLOCK_BYTES: usize = search::BLOCK_SIZE * 8;
const TAIL_DOMAIN: &[u8] = b"tos-search-bulk-tail-v1\0";

/// The exact independent scratch and work-buffer bounds of the Python owner.
/// Byte counters describe retained payload and fixed frames, never RSS.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BulkBootstrapLimits {
    pub max_bytes: u64,
    pub max_mutations: u64,
    pub max_cached_terms: usize,
    pub max_cached_bytes: usize,
    pub batch_size: usize,
    pub max_cached_tails: usize,
    pub max_tail_bytes: usize,
}

impl BulkBootstrapLimits {
    /// Main/scratch storage admission remains explicit; work buffers have the
    /// maintained defaults rather than growing with the cohort.
    pub fn new(max_bytes: u64, max_mutations: u64) -> Self {
        Self {
            max_bytes,
            max_mutations,
            max_cached_terms: 8192,
            max_cached_bytes: 8 * 1024 * 1024,
            batch_size: 1024,
            max_cached_tails: 8192,
            max_tail_bytes: 8 * 1024 * 1024,
        }
    }

    pub fn validate(self) -> Result<()> {
        if !(65536..=(1 << 40)).contains(&self.max_bytes)
            || !(1..=search::MAX_ADDRESS).contains(&self.max_mutations)
            || !(1..=8192).contains(&self.max_cached_terms)
            || !(1..=8 * 1024 * 1024).contains(&self.max_cached_bytes)
            || !(1..=1024).contains(&self.batch_size)
            || !(1..=8192).contains(&self.max_cached_tails)
            || !(1..=8 * 1024 * 1024).contains(&self.max_tail_bytes)
        {
            return Err(Error::Invalid("bulk scratch and work-buffer limits"));
        }
        Ok(())
    }
}

/// `search.mutations` stays main-only. `mutations` is the actual joint charge;
/// the owner's independent carrier writes are accounted by its publisher.
#[derive(Clone, Debug, Default)]
pub struct BulkBootstrapReport {
    pub search: search::SearchWriteReport,
    pub main_mutations: u64,
    pub scratch_mutations: u64,
    pub mutations: u64,
    pub main_write_calls: u64,
    pub scratch_write_calls: u64,
    pub documents: u64,
    pub high_water: u64,
    pub database_bytes: u64,
    pub scratch_peak_bytes: u64,
    pub dictionary_hits: u64,
    pub dictionary_misses: u64,
    pub dictionary_peak_entries: usize,
    pub dictionary_peak_bytes: usize,
    pub reverse_memberships: u64,
    pub tail_hits: u64,
    pub tail_misses: u64,
    pub tail_evictions: u64,
    pub scratch_read_calls: u64,
    pub tail_peak_entries: usize,
    pub tail_peak_bytes: usize,
    pub load_micros: u64,
    pub stage_micros: u64,
    pub pack_micros: u64,
    pub total_micros: u64,
}

fn deadline(limit: Option<Instant>) -> Result<()> {
    if limit.is_some_and(|limit| Instant::now() >= limit) {
        return Err(Error::Budget("bulk whole-operation deadline"));
    }
    Ok(())
}

fn add(counter: &mut u64, amount: u64, label: &'static str) -> Result<()> {
    *counter = counter.checked_add(amount).ok_or(Error::Budget(label))?;
    Ok(())
}

fn micros(start: Instant, end: Instant) -> u64 {
    end.duration_since(start).as_micros().min(u64::MAX as u128) as u64
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DictionaryToken {
    kind: String,
    plane: u8,
    n: u8,
    key: Vec<u8>,
}

struct DictionaryEntry {
    term: u64,
    size: usize,
    recency: u64,
}

struct BulkWriter<'a> {
    physical: search::Writer<'a>,
    maximum: u64,
    scratch_mutations: u64,
    limits: BulkBootstrapLimits,
    deadline: Option<Instant>,
    cache: BTreeMap<Rc<DictionaryToken>, DictionaryEntry>,
    lru: BTreeMap<u64, Rc<DictionaryToken>>,
    recency: u64,
    cached_bytes: usize,
    peak_cached_bytes: usize,
    peak_cached_terms: usize,
    dictionary_hits: u64,
    dictionary_misses: u64,
    reverse_memberships: u64,
}

impl<'a> BulkWriter<'a> {
    fn new(
        db: &'a Connection,
        maximum: u64,
        limits: BulkBootstrapLimits,
        deadline: Option<Instant>,
    ) -> Result<Self> {
        Ok(Self {
            physical: search::Writer::new(db, maximum, true)?,
            maximum,
            scratch_mutations: 0,
            limits,
            deadline,
            cache: BTreeMap::new(),
            lru: BTreeMap::new(),
            recency: 0,
            cached_bytes: 0,
            peak_cached_bytes: 0,
            peak_cached_terms: 0,
            dictionary_hits: 0,
            dictionary_misses: 0,
            reverse_memberships: 0,
        })
    }

    fn admit(&mut self, count: u64) -> Result<()> {
        deadline(self.deadline)?;
        if self
            .physical
            .report
            .mutations
            .checked_add(self.scratch_mutations)
            .and_then(|used| used.checked_add(count))
            .is_none_or(|used| used > self.maximum)
        {
            return Err(Error::Budget(
                "bulk main/scratch mutations; abort owner transaction",
            ));
        }
        // Shared physical helpers may issue multiple writes. They enforce this
        // reduced allowance on every write, including trigger change counts.
        self.physical.maximum = self.maximum - self.scratch_mutations;
        Ok(())
    }

    fn write<P: Params>(&mut self, sql: &str, parameters: P) -> Result<usize> {
        self.admit(1)?;
        let result = self.physical.write(sql, parameters)?;
        self.admit(0)?;
        Ok(result)
    }

    fn term(&mut self, kind: &str, plane: u8, n: u8, key: Vec<u8>) -> Result<u64> {
        deadline(self.deadline)?;
        add(
            &mut self.physical.report.term_lookups,
            1,
            "bulk dictionary lookups",
        )?;
        let token = DictionaryToken {
            kind: kind.to_owned(),
            plane,
            n,
            key,
        };
        if let Some((token, mut entry)) = self.cache.remove_entry(&token) {
            if self.lru.remove(&entry.recency).as_deref() != Some(token.as_ref()) {
                return Err(Error::Invalid("bulk dictionary LRU closure"));
            }
            add(&mut self.dictionary_hits, 1, "bulk dictionary hits")?;
            add(&mut self.recency, 1, "bulk dictionary recency")?;
            entry.recency = self.recency;
            let term = entry.term;
            self.lru.insert(entry.recency, Rc::clone(&token));
            self.cache.insert(token, entry);
            return Ok(term);
        }
        add(&mut self.dictionary_misses, 1, "bulk dictionary misses")?;
        let found: Option<u64> = self
            .physical
            .db
            .query_row(
                "SELECT term_id FROM search_terms WHERE kind=? AND plane=? AND n=? AND term_key=?",
                params![token.kind, token.plane, token.n, token.key],
                |row| row.get(0),
            )
            .optional()?;
        let term = if let Some(term) = found {
            term
        } else {
            self.write(
                "INSERT INTO search_terms(kind,plane,n,term_key,posting_count) VALUES (?,?,?,?,0)",
                params![token.kind, token.plane, token.n, token.key],
            )?;
            u64::try_from(self.physical.db.last_insert_rowid())
                .ok()
                .filter(|term| *term > 0)
                .ok_or(Error::Invalid("bulk dictionary address"))?
        };
        if term == 0 || term > i64::MAX as u64 {
            return Err(Error::Invalid("bulk dictionary address"));
        }
        let size = 192 + token.key.len();
        if size <= self.limits.max_cached_bytes {
            while !self.cache.is_empty()
                && (self.cache.len() >= self.limits.max_cached_terms
                    || self.cached_bytes + size > self.limits.max_cached_bytes)
            {
                deadline(self.deadline)?;
                let (_, oldest) = self
                    .lru
                    .pop_first()
                    .ok_or(Error::Invalid("bulk dictionary eviction LRU"))?;
                let entry = self
                    .cache
                    .remove(&oldest)
                    .ok_or(Error::Invalid("bulk dictionary eviction closure"))?;
                self.cached_bytes -= entry.size;
            }
            add(&mut self.recency, 1, "bulk dictionary recency")?;
            let token = Rc::new(token);
            self.lru.insert(self.recency, Rc::clone(&token));
            self.cache.insert(
                token,
                DictionaryEntry {
                    term,
                    size,
                    recency: self.recency,
                },
            );
            self.cached_bytes += size;
            self.peak_cached_bytes = self.peak_cached_bytes.max(self.cached_bytes);
            self.peak_cached_terms = self.peak_cached_terms.max(self.cache.len());
        }
        Ok(term)
    }

    fn insert_document(&mut self, document: search::PreparedSearchDocument) -> Result<()> {
        deadline(self.deadline)?;
        let (key, identifier, filters) = document.metadata()?;
        let terms = search::document_terms(&document)?;
        self.physical.report.peak_document_term_bytes = self
            .physical
            .report
            .peak_document_term_bytes
            .max(terms.iter().map(|(_, _, key)| key.len() + 2).sum());
        // Existing PK/UNIQUE constraints reject duplicate addresses, exact
        // identities and within-kind sort keys without old-row scans.
        self.write(
            "INSERT INTO search_documents VALUES (?,?,?,?,?)",
            params![document.doc_id, document.kind, identifier, key, filters],
        )?;
        self.admit(1)?;
        self.physical.save_values(&document)?;
        self.admit(0)?;
        let mut ids = BTreeSet::new();
        let term_count = terms.len();
        for (plane, n, key) in terms {
            ids.insert(self.term(&document.kind, plane, n, key)?);
        }
        if ids.len() != term_count {
            return Err(Error::Invalid(
                "bulk dictionary aliases distinct document terms",
            ));
        }
        let ids: Vec<_> = ids.into_iter().collect();
        let (payload, digest) =
            search::encode_search_reverse(document.doc_id, &document.kind, &ids)?;
        self.write(
            "INSERT INTO search_document_terms VALUES (?,?,?,?)",
            params![document.doc_id, ids.len(), payload, &digest[..]],
        )?;
        add(
            &mut self.reverse_memberships,
            ids.len() as u64,
            "bulk reverse memberships",
        )?;
        add(
            &mut self.physical.report.documents_consumed,
            1,
            "bulk consumed documents",
        )?;
        Ok(())
    }

    fn block(&mut self, term: u64, tail: &Tail) -> Result<()> {
        self.admit(1)?;
        // The shared physical method encodes once. An upper bound protects its
        // counters without adding a second codec pass for every packed block.
        let payload_bytes = (tail.addresses.len() * 8) as u64;
        self.physical
            .report
            .blocks_written
            .checked_add(1)
            .ok_or(Error::Budget("bulk block accounting"))?;
        self.physical
            .report
            .payload_bytes_written
            .checked_add(payload_bytes)
            .ok_or(Error::Budget("bulk posting-byte accounting"))?;
        let fence = if tail.total <= search::BLOCK_SIZE as u64 {
            Vec::new()
        } else {
            let first = *tail
                .addresses
                .first()
                .ok_or(Error::Invalid("bulk empty block"))?;
            self.physical
                .report
                .document_key_reads
                .checked_add(1)
                .ok_or(Error::Budget("bulk fence-key accounting"))?;
            self.physical
                .keys(&[first])?
                .remove(&first)
                .ok_or(Error::Invalid("bulk block fence closure"))?
        };
        self.physical.block(term, &fence, &tail.addresses)?;
        self.admit(0)?;
        Ok(())
    }

    fn clear_dictionary_cache(&mut self) {
        self.cache.clear();
        self.lru.clear();
        self.cached_bytes = 0;
    }
}

fn ordered<'a>(db: &'a Connection, sql: &str) -> Result<Statement<'a>> {
    let mut explain = db.prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
    let mut rows = explain.query([])?;
    while let Some(row) = rows.next()? {
        let detail: String = row.get(3)?;
        if detail.to_ascii_uppercase().contains("TEMP B-TREE") {
            return Err(Error::Invalid("bulk ordering requires an existing index"));
        }
    }
    Ok(db.prepare(sql)?)
}

struct Tail {
    total: u64,
    last_rank: u64,
    addresses: Vec<u64>,
}

impl Tail {
    fn size(&self) -> usize {
        256 + 8 * self.addresses.len()
    }
}

fn tail_digest(term: u64, tail: &Tail, payload: &[u8]) -> [u8; 32] {
    let mut hash = Digest256Hasher::new();
    hash.update(TAIL_DOMAIN);
    hash.update(&term.to_be_bytes());
    hash.update(&tail.total.to_be_bytes());
    hash.update(&tail.last_rank.to_be_bytes());
    hash.update(payload);
    *hash.finalize().as_bytes()
}

struct Scratch {
    path: PathBuf,
    created: Option<(u64, u64)>,
    db: Option<Connection>,
    limits: BulkBootstrapLimits,
    deadline: Option<Instant>,
    page_size: u64,
    peak_bytes: u64,
    write_calls: u64,
    read_calls: Cell<u64>,
    cache: BTreeMap<u64, (Tail, u64)>,
    lru: BTreeMap<u64, u64>,
    recency: u64,
    cached_bytes: usize,
    peak_cached_bytes: usize,
    peak_cached_tails: usize,
    hits: u64,
    misses: u64,
    evictions: u64,
}

impl Scratch {
    fn open(
        path: &Path,
        limits: BulkBootstrapLimits,
        deadline_limit: Option<Instant>,
    ) -> Result<Self> {
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        let parent = path.parent().ok_or(Error::Invalid("bulk scratch parent"))?;
        if fs::canonicalize(parent)? != parent {
            return Err(Error::Invalid("bulk scratch parent traverses symlinks"));
        }
        for suffix in ["-journal", "-wal", "-shm"] {
            let mut adjacent = path.as_os_str().to_os_string();
            adjacent.push(suffix);
            match fs::symlink_metadata(PathBuf::from(adjacent)) {
                Ok(_) => return Err(Error::Invalid("bulk scratch sidecar already exists")),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        let mut scratch = Self {
            path,
            created: None,
            db: None,
            limits,
            deadline: deadline_limit,
            page_size: 0,
            peak_bytes: 0,
            write_calls: 0,
            read_calls: Cell::new(0),
            cache: BTreeMap::new(),
            lru: BTreeMap::new(),
            recency: 0,
            cached_bytes: 0,
            peak_cached_bytes: 0,
            peak_cached_tails: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
        };
        deadline(scratch.deadline)?;
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&scratch.path)?;
        let stamp = file.metadata()?;
        scratch.created = Some((stamp.dev(), stamp.ino()));
        drop(file);
        scratch.db = Some(Connection::open_with_flags(
            &scratch.path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?);
        scratch.identity()?;
        let limit = scratch.deadline;
        scratch.connection().progress_handler(
            1000,
            Some(move || limit.is_some_and(|limit| Instant::now() >= limit)),
        );
        // Only this disposable private file disables its rollback journal.
        // It is never recovered, admitted or retained for reuse.
        scratch
            .connection()
            .execute_batch("PRAGMA journal_mode=OFF;PRAGMA cache_size=-2048;PRAGMA mmap_size=0;")?;
        scratch.page_size = scratch
            .connection()
            .query_row("PRAGMA page_size", [], |row| row.get(0))?;
        search::page_cap(scratch.connection(), limits.max_bytes / scratch.page_size)?;
        scratch.connection().execute_batch("BEGIN;CREATE TABLE tails (term_id INTEGER PRIMARY KEY,total INTEGER NOT NULL,last_rank INTEGER NOT NULL,payload BLOB NOT NULL,digest BLOB NOT NULL);")?;
        scratch.measure()?;
        Ok(scratch)
    }

    fn connection(&self) -> &Connection {
        self.db.as_ref().expect("live private bulk scratch")
    }

    fn identity(&self) -> Result<()> {
        let current = fs::symlink_metadata(&self.path)?;
        if Some((current.dev(), current.ino())) != self.created {
            return Err(Error::Invalid("exclusive bulk scratch inode changed"));
        }
        Ok(())
    }

    fn measure(&mut self) -> Result<()> {
        deadline(self.deadline)?;
        self.identity()?;
        let pages: u64 = self
            .connection()
            .query_row("PRAGMA page_count", [], |row| row.get(0))?;
        let size = pages
            .checked_mul(self.page_size)
            .ok_or(Error::Budget("bulk scratch byte accounting"))?;
        self.peak_bytes = self.peak_bytes.max(size);
        if size > self.limits.max_bytes {
            return Err(Error::Budget("bulk scratch pages"));
        }
        Ok(())
    }

    fn save(&mut self, writer: &mut BulkWriter<'_>, term: u64, tail: &Tail) -> Result<()> {
        deadline(self.deadline)?;
        writer.admit(1)?;
        if writer
            .scratch_mutations
            .checked_add(1)
            .is_none_or(|used| used > self.limits.max_mutations)
        {
            return Err(Error::Budget("bulk scratch mutations"));
        }
        let payload = search::encode_search_postings(&tail.addresses)?;
        let digest = tail_digest(term, tail, &payload);
        let before = self.connection().total_changes();
        add(&mut self.write_calls, 1, "bulk scratch write calls")?;
        self.connection().execute("INSERT INTO tails VALUES (?,?,?,?,?) ON CONFLICT(term_id) DO UPDATE SET total=excluded.total,last_rank=excluded.last_rank,payload=excluded.payload,digest=excluded.digest",
            params![term,tail.total,tail.last_rank,payload,&digest[..]])?;
        let changed = self
            .connection()
            .total_changes()
            .checked_sub(before)
            .ok_or(Error::Invalid("bulk scratch change count"))?;
        add(
            &mut writer.scratch_mutations,
            changed,
            "bulk scratch mutation accounting",
        )?;
        if changed != 1 {
            return Err(Error::Invalid("unexpected bulk scratch write count"));
        }
        writer.admit(0)?;
        // max_page_count enforces every allocation; pages never shrink in this
        // private transaction, so measuring each batch avoids PRAGMA per miss.
        if self.write_calls % self.limits.batch_size as u64 == 0 {
            self.measure()?;
        }
        Ok(())
    }

    fn count_read(&self) -> Result<()> {
        self.read_calls.set(
            self.read_calls
                .get()
                .checked_add(1)
                .ok_or(Error::Budget("bulk scratch read calls"))?,
        );
        Ok(())
    }

    fn load(&self, term: u64, required: bool) -> Result<Tail> {
        deadline(self.deadline)?;
        self.count_read()?;
        let probe: Option<(i64,i64,i64,String,i64,String)> = self.connection().query_row(
            "SELECT total,last_rank,length(payload),typeof(payload),length(digest),typeof(digest) FROM tails WHERE term_id=?",
            [term],|row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?))
        ).optional()?;
        let Some((total, rank, size, payload_type, digest_size, digest_type)) = probe else {
            if required {
                return Err(Error::Invalid("missing staged bulk term tail"));
            }
            return Ok(Tail {
                total: 0,
                last_rank: 0,
                addresses: Vec::new(),
            });
        };
        if !(1..=search::MAX_ADDRESS as i64).contains(&total)
            || !(total..=search::MAX_ADDRESS as i64).contains(&rank)
            || !(0..=MAX_BLOCK_BYTES as i64).contains(&size)
            || payload_type != "blob"
            || digest_type != "blob"
            || digest_size != 32
        {
            return Err(Error::Invalid("invalid or oversized staged bulk tail"));
        }
        self.count_read()?;
        let (payload, digest): (Vec<u8>, Vec<u8>) = self.connection().query_row(
            "SELECT payload,digest FROM tails WHERE term_id=?",
            [term],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let mut tail = Tail {
            total: total as u64,
            last_rank: rank as u64,
            addresses: Vec::new(),
        };
        let expected = tail_digest(term, &tail, &payload);
        if digest
            .iter()
            .zip(expected)
            .fold(0u8, |diff, (&a, b)| diff | (a ^ b))
            != 0
        {
            return Err(Error::Invalid("staged bulk tail digest mismatch"));
        }
        tail.addresses = search::decode_search_postings(&payload)?;
        if tail.addresses.len() as u64 != tail.total % search::BLOCK_SIZE as u64
            || search::encode_search_postings(&tail.addresses)? != payload
        {
            return Err(Error::Invalid(
                "noncanonical staged bulk tail or count mismatch",
            ));
        }
        Ok(tail)
    }

    fn append(
        &mut self,
        writer: &mut BulkWriter<'_>,
        term: u64,
        rank: u64,
        doc_id: u64,
    ) -> Result<()> {
        deadline(self.deadline)?;
        let mut tail = if let Some((tail, recency)) = self.cache.remove(&term) {
            if self.lru.remove(&recency) != Some(term) {
                return Err(Error::Invalid("bulk tail LRU closure"));
            }
            add(&mut self.hits, 1, "bulk tail hits")?;
            self.cached_bytes -= tail.size();
            tail
        } else {
            add(&mut self.misses, 1, "bulk tail misses")?;
            self.load(term, false)?
        };
        if rank <= tail.last_rank || rank > search::MAX_ADDRESS {
            return Err(Error::Invalid(
                "bulk document traversal is not strictly monotonic",
            ));
        }
        tail.last_rank = rank;
        tail.total = tail
            .total
            .checked_add(1)
            .filter(|total| *total <= search::MAX_ADDRESS)
            .ok_or(Error::Budget("bulk tail membership count"))?;
        tail.addresses.push(doc_id);
        if tail.addresses.len() == search::BLOCK_SIZE {
            writer.block(term, &tail)?;
            tail.addresses.clear();
            // Retain only the declared numeric payload; full-block capacity is
            // released rather than hidden behind an empty accounted frame.
            tail.addresses.shrink_to_fit();
        }
        if tail.size() > self.limits.max_tail_bytes {
            return self.save(writer, term, &tail);
        }
        while !self.cache.is_empty()
            && (self.cache.len() >= self.limits.max_cached_tails
                || self.cached_bytes + tail.size() > self.limits.max_tail_bytes)
        {
            deadline(self.deadline)?;
            let (_, oldest) = self
                .lru
                .pop_first()
                .ok_or(Error::Invalid("bulk tail eviction LRU"))?;
            let (previous, _) = self
                .cache
                .remove(&oldest)
                .ok_or(Error::Invalid("bulk tail eviction closure"))?;
            self.cached_bytes -= previous.size();
            add(&mut self.evictions, 1, "bulk tail evictions")?;
            self.save(writer, oldest, &previous)?;
        }
        add(&mut self.recency, 1, "bulk tail recency")?;
        self.cached_bytes += tail.size();
        self.cache.insert(term, (tail, self.recency));
        self.lru.insert(self.recency, term);
        self.peak_cached_bytes = self.peak_cached_bytes.max(self.cached_bytes);
        self.peak_cached_tails = self.peak_cached_tails.max(self.cache.len());
        Ok(())
    }

    fn flush(&mut self, writer: &mut BulkWriter<'_>) -> Result<()> {
        while let Some((_, term)) = self.lru.pop_first() {
            deadline(self.deadline)?;
            let (tail, _) = self
                .cache
                .remove(&term)
                .ok_or(Error::Invalid("bulk tail flush closure"))?;
            self.cached_bytes -= tail.size();
            self.save(writer, term, &tail)?;
        }
        self.measure()
    }

    fn cleanup(&mut self) -> Result<()> {
        // Close first; never delete a replacement inode, sidecar or existing
        // file. Keeping the stamp on an error lets Drop retry only this inode.
        drop(self.db.take());
        if self.created.is_some() {
            match self.identity() {
                Ok(()) => {
                    fs::remove_file(&self.path)?;
                    self.created = None;
                }
                Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                    self.created = None;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

fn fresh_reverse(writer: &mut BulkWriter<'_>, doc_id: u64, kind: &str) -> Result<Vec<u64>> {
    deadline(writer.deadline)?;
    let probe: Option<(i64,i64,i64,String,String)> = writer.physical.db.query_row(
        "SELECT term_count,length(payload),length(digest),typeof(payload),typeof(digest) FROM search_document_terms WHERE doc_id=?",
        [doc_id],|row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))
    ).optional()?;
    let (count, bytes, digest_bytes, payload_type, digest_type) =
        probe.ok_or(Error::Invalid("missing bulk reverse frame"))?;
    if !(1..=search::MAX_TERMS as i64).contains(&count)
        || bytes < count
        || bytes > count * 9
        || digest_bytes != 32
        || payload_type != "blob"
        || digest_type != "blob"
    {
        return Err(Error::Invalid("bulk reverse frame bounds"));
    }
    let (payload, digest): (Vec<u8>, Vec<u8>) = writer.physical.db.query_row(
        "SELECT payload,digest FROM search_document_terms WHERE doc_id=?",
        [doc_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let terms = search::decode_search_reverse(doc_id, kind, count as usize, &payload, &digest)?;
    add(
        &mut writer.physical.report.reverse_reads,
        1,
        "bulk reverse reads",
    )?;
    // This private fresh producer resolved every term in the same still-owned
    // transaction; dictionary revalidation belongs to addressed old deltas.
    Ok(terms)
}

fn stage(writer: &mut BulkWriter<'_>, scratch: &mut Scratch) -> Result<u64> {
    let db = writer.physical.db;
    let mut documents = ordered(
        db,
        "SELECT doc_id,CASE WHEN length(CAST(kind AS BLOB))<=8 THEN kind ELSE NULL END FROM search_documents ORDER BY kind,sort_key",
    )?;
    let mut rows = documents.query([])?;
    let (mut count, mut memberships) = (0u64, 0u64);
    while let Some(row) = rows.next()? {
        deadline(writer.deadline)?;
        add(&mut count, 1, "bulk temporary document rank")?;
        if count > search::MAX_ADDRESS {
            return Err(Error::Budget("bulk temporary document rank"));
        }
        let doc_id: u64 = row.get(0)?;
        let kind: String = row.get(1)?;
        let terms = fresh_reverse(writer, doc_id, &kind)?;
        add(
            &mut memberships,
            terms.len() as u64,
            "bulk staged memberships",
        )?;
        for term in terms {
            scratch.append(writer, term, count, doc_id)?;
        }
    }
    if memberships != writer.reverse_memberships {
        return Err(Error::Invalid("bulk staged membership count differs"));
    }
    scratch.flush(writer)?;
    Ok(count)
}

fn pack(writer: &mut BulkWriter<'_>, scratch: &Scratch) -> Result<()> {
    let mut terms = ordered(
        scratch.connection(),
        "SELECT term_id FROM tails ORDER BY term_id",
    )?;
    let mut rows = terms.query([])?;
    let (mut count, mut memberships) = (0u64, 0u64);
    while let Some(row) = rows.next()? {
        deadline(writer.deadline)?;
        let term: u64 = row.get(0)?;
        let tail = scratch.load(term, true)?;
        if !tail.addresses.is_empty() {
            writer.block(term, &tail)?;
        }
        if writer.write(
            "UPDATE search_terms SET posting_count=? WHERE term_id=?",
            params![tail.total, term],
        )? != 1
        {
            return Err(Error::Invalid(
                "staged bulk tail references missing dictionary term",
            ));
        }
        add(&mut count, 1, "bulk packed dictionary terms")?;
        add(&mut memberships, tail.total, "bulk packed memberships")?;
    }
    let dictionary_count: u64 =
        writer
            .physical
            .db
            .query_row("SELECT count(*) FROM search_terms", [], |row| row.get(0))?;
    if memberships != writer.reverse_memberships || count != dictionary_count {
        return Err(Error::Invalid("bulk packed term/membership totals differ"));
    }
    Ok(())
}

/// Explicit alternative to the ordinary initializer, never an automatic retry.
/// The callback streams source carrier writes and prepared search documents in
/// the caller's unchanged transaction. Scratch opens before main caps or DDL.
/// Any error requires owner rollback; this function never manages main locks.
pub fn initialize_bulk_with(
    db: &Connection,
    binding: &JsonValue,
    scratch_path: &Path,
    scratch_limits: BulkBootstrapLimits,
    max_mutations: u64,
    max_bytes: u64,
    produce: impl FnOnce(&mut dyn FnMut(search::PreparedSearchDocument) -> Result<()>) -> Result<()>,
    deadline_limit: Option<Instant>,
) -> Result<BulkBootstrapReport> {
    search::require_transaction(db)?;
    deadline(deadline_limit)?;
    let framed = search::header(binding)?;
    if !(65536..=(1 << 40)).contains(&max_bytes) {
        return Err(Error::Invalid("bulk main whole-file byte limit"));
    }
    scratch_limits.validate()?;
    let started = Instant::now();
    let mut writer = BulkWriter::new(db, max_mutations, scratch_limits, deadline_limit)?;
    let mut scratch = Scratch::open(scratch_path, scratch_limits, deadline_limit)?;
    let result = (|| {
        let page_size: u64 = db.query_row("PRAGMA page_size", [], |row| row.get(0))?;
        let max_pages = search::page_cap(db, max_bytes / page_size)?;
        for statement in search::DDL {
            deadline(deadline_limit)?;
            db.execute(statement, [])?;
            add(
                &mut writer.physical.report.ddl_statements,
                1,
                "bulk main DDL statements",
            )?;
        }
        writer.physical.report.setup_micros = micros(started, Instant::now());
        let (mut high_water, mut count) = (0u64, 0u64);
        produce(&mut |document| {
            deadline(deadline_limit)?;
            let doc_id = document.doc_id;
            writer.insert_document(document)?;
            add(&mut count, 1, "bulk document count")?;
            high_water = high_water.max(doc_id);
            Ok(())
        })?;
        let loaded = Instant::now();
        writer.clear_dictionary_cache();
        if stage(&mut writer, &mut scratch)? != count {
            return Err(Error::Invalid("bulk document staging count differs"));
        }
        let staged = Instant::now();
        pack(&mut writer, &scratch)?;
        let packed = Instant::now();
        writer.write(
            "INSERT INTO search_header VALUES (1,?,?,?,?)",
            params![framed, &search::cursor_key()?[..], high_water, max_pages],
        )?;
        writer.physical.report.database_bytes = search::database_bytes(db)?;
        let finished = Instant::now();
        writer.physical.report.elapsed_micros = micros(started, finished);
        writer.physical.report.memberships = writer.reverse_memberships;
        let main_mutations = writer.physical.report.mutations;
        let mutations = main_mutations
            .checked_add(writer.scratch_mutations)
            .ok_or(Error::Budget("bulk joint mutation report"))?;
        Ok(BulkBootstrapReport {
            main_mutations,
            scratch_mutations: writer.scratch_mutations,
            mutations,
            main_write_calls: writer.physical.report.write_calls,
            scratch_write_calls: scratch.write_calls,
            documents: count,
            high_water,
            database_bytes: writer.physical.report.database_bytes,
            scratch_peak_bytes: scratch.peak_bytes,
            dictionary_hits: writer.dictionary_hits,
            dictionary_misses: writer.dictionary_misses,
            dictionary_peak_entries: writer.peak_cached_terms,
            dictionary_peak_bytes: writer.peak_cached_bytes,
            reverse_memberships: writer.reverse_memberships,
            tail_hits: scratch.hits,
            tail_misses: scratch.misses,
            tail_evictions: scratch.evictions,
            scratch_read_calls: scratch.read_calls.get(),
            tail_peak_entries: scratch.peak_cached_tails,
            tail_peak_bytes: scratch.peak_cached_bytes,
            load_micros: micros(started, loaded),
            stage_micros: micros(loaded, staged),
            pack_micros: micros(staged, packed),
            total_micros: micros(started, finished),
            search: writer.physical.report.clone(),
        })
    })();
    let cleanup = scratch.cleanup();
    match (result, cleanup) {
        (Ok(report), Ok(())) => Ok(report),
        (Err(error), _) | (_, Err(error)) => Err(error),
    }
}
