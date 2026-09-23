//! Private, disk-indexed full-knowledge staging. Source admission and the
//! filesystem spill quota are supplied by independent owner/host guards.

use crate::{
    Error, Limits, Result, SourceBinding, file_digest, safe_open, sqlite_budget, stream_digest,
};
use fs2::FileExt;
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Seek, SeekFrom, Write},
    os::fd::AsRawFd,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicU64},
};
use tos_foundation::{Digest256, Digest256Hasher};

// A selected descriptor permits up to 4,096 source registrations. A full
// source family can contribute several independently sealed collections.
const MAX_COLLECTIONS: usize = 16_384;
const MAX_NAME_BYTES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WritePhase {
    Create,
    SqliteOpen,
    Schema,
    Input,
    Normalized,
    Catalog,
    Search,
    Sort,
    Finalize,
}

/// The host implementation must hold an exclusive quota-backed private
/// temp/output namespace for this stage's entire lifetime, including SQLite's
/// fallback directories and rollback files. No other writer may replace a
/// stage path or write the retained inodes. `verify` must reject absent or
/// exhausted kernel-enforced quotas; a before/after size sample alone cannot
/// cap a single SQLite statement's spill. There is no permissive implementation.
pub trait StageIsolation {
    fn verify(&self, candidate: &Path, limits: StageLimits, phase: WritePhase) -> Result<()>;
}

/// A source-owner implementation checks the selected registration and retains
/// the immutable sealed cut until the final recheck. Stage hashes alone do not
/// establish source completeness or admission.
pub trait StageOwner {
    fn verify_receipt(&self, receipt: &ExactInputReceipt) -> Result<()>;
    fn recheck_sealed_cut(&self, receipt: &ExactInputReceipt) -> Result<()>;
}

#[derive(Clone, Copy, Debug)]
pub struct StageLimits {
    pub sqlite: Limits,
    pub max_temp_bytes: u64,
    pub max_seek_rows: usize,
    pub max_seek_bytes: u64,
}
impl StageLimits {
    fn validate(self) -> Result<()> {
        self.sqlite.validate()?;
        if self.max_temp_bytes == 0
            || self.max_seek_rows == 0
            || self.max_seek_rows > 1024
            || self.max_seek_bytes == 0
            || self.max_seek_bytes > 64 * 1024 * 1024
        {
            return Err(Error::Budget("stage temp/seek limits must be positive"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct InputCollectionReceipt {
    pub source_graph: String,
    pub collection: String,
    pub input_role: String,
    pub adapter_profile: String,
    pub expected_count: u64,
    /// SHA-256 over sorted (length-prefixed ID, binary payload SHA-256) pairs.
    pub expected_root_sha256: String,
}

#[derive(Clone, Debug)]
pub struct ExactInputReceipt {
    pub binding: SourceBinding,
    pub collections: Vec<InputCollectionReceipt>,
}
impl ExactInputReceipt {
    fn validate(&self) -> Result<()> {
        self.binding.validate()?;
        if self.collections.is_empty() || self.collections.len() > MAX_COLLECTIONS {
            return Err(Error::Invalid("input collection registration count"));
        }
        let mut seen = BTreeSet::new();
        for entry in &self.collections {
            for value in [
                &entry.source_graph,
                &entry.collection,
                &entry.input_role,
                &entry.adapter_profile,
            ] {
                if value.is_empty() || value.len() > MAX_NAME_BYTES {
                    return Err(Error::Invalid("input collection registration field"));
                }
            }
            Digest256::from_hex(&entry.expected_root_sha256)
                .map_err(|_| Error::Invalid("input collection root digest"))?;
            if !seen.insert((&entry.source_graph, &entry.collection)) {
                return Err(Error::Invalid("duplicate input collection registration"));
            }
        }
        Ok(())
    }
}

pub struct InputRow<'a> {
    pub source_graph: &'a str,
    pub collection: &'a str,
    pub id: &'a str,
    pub payload: &'a [u8],
}
pub struct NodeRow<'a> {
    pub id: &'a str,
    pub source_graph: &'a str,
    pub native_id: Option<&'a str>,
    pub entity_id: Option<&'a str>,
    pub kind_id: &'a str,
    pub type_id: &'a str,
    pub source_order: i64,
    pub payload: &'a [u8],
}
pub struct RelationRow<'a> {
    pub id: &'a str,
    pub source_graph: &'a str,
    pub native_id: Option<&'a str>,
    pub from_id: &'a str,
    pub to_id: &'a str,
    pub predicate_id: &'a str,
    pub relation_type_id: &'a str,
    pub source_order: i64,
    pub payload: &'a [u8],
}

#[derive(Clone, Debug)]
pub struct StageReceipt {
    pub source_cut: String,
    pub membership_root: String,
    pub input_collections: usize,
    pub verified_inputs: Vec<InputCollectionReceipt>,
    pub input_rows: u64,
    pub node_rows: u64,
    pub relation_rows: u64,
    pub node_root_sha256: String,
    pub relation_root_sha256: String,
    pub sqlite_sha256: String,
    pub sqlite_size_bytes: u64,
}

/// Provisional derived row roots for the full-model seal. `finish` repeats
/// these scans after its owner cut recheck; this is not source admission.
#[derive(Clone, Debug)]
pub(crate) struct CoreRoots {
    pub nodes: u64,
    pub relations: u64,
    pub node_sha256: String,
    pub relation_sha256: String,
}

#[derive(Clone, Debug)]
pub struct SeekRow {
    pub id: String,
    pub source_graph: String,
    pub source_order: Option<i64>,
    pub payload: Vec<u8>,
    pub payload_sha256: String,
}

#[derive(Clone, Debug)]
pub struct ScanPage {
    pub rows: Vec<SeekRow>,
    /// Pass this as `after_id` for the next page. `None` means exhausted.
    pub next_id: Option<String>,
}

pub struct KnowledgeStage<'a> {
    candidate: PathBuf,
    inode: (u64, u64),
    lease_path: PathBuf,
    lease_inode: (u64, u64),
    lease: Option<fs::File>,
    db: Option<Connection>,
    vm_used: Option<Arc<AtomicU64>>,
    limits: StageLimits,
    receipt: ExactInputReceipt,
    registrations: BTreeMap<String, BTreeSet<String>>,
    owner: &'a dyn StageOwner,
    isolation: &'a dyn StageIsolation,
    total_rows: u64,
    work_bytes: u64,
    poisoned: bool,
    keep: bool,
    selected_full: bool,
    fresh_selected: Option<PathBuf>,
}

impl<'a> KnowledgeStage<'a> {
    pub(crate) fn registered_source(&self, source_graph: &str) -> bool {
        self.registrations.contains_key(source_graph)
    }

    fn registered(&self, source_graph: &str, collection: &str) -> bool {
        self.registrations
            .get(source_graph)
            .is_some_and(|collections| collections.contains(collection))
    }

    pub(crate) fn exact_receipt(&self) -> &ExactInputReceipt {
        &self.receipt
    }
    pub(crate) fn poison(&mut self) {
        self.poisoned = true;
    }
    pub(crate) fn mark_selected_full(&mut self) -> Result<()> {
        if self.poisoned || self.selected_full {
            return Err(Error::Invalid("selected full-model stage state"));
        }
        self.selected_full = true;
        Ok(())
    }

    pub fn create(
        candidate: &Path,
        limits: StageLimits,
        receipt: ExactInputReceipt,
        owner: &'a dyn StageOwner,
        isolation: &'a dyn StageIsolation,
    ) -> Result<Self> {
        limits.validate()?;
        receipt.validate()?;
        let mut registrations: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for entry in &receipt.collections {
            registrations
                .entry(entry.source_graph.clone())
                .or_default()
                .insert(entry.collection.clone());
        }
        owner.verify_receipt(&receipt)?;
        let parent = candidate
            .parent()
            .ok_or(Error::Invalid("stage candidate parent"))?;
        if !parent.is_dir() || parent.is_symlink() || candidate.exists() || candidate.is_symlink() {
            return Err(Error::Invalid("stage candidate path"));
        }
        if sqlite_sidecar_paths(candidate)
            .iter()
            .any(|path| path.exists() || path.is_symlink())
        {
            return Err(Error::Invalid("stage SQLite sidecar path exists"));
        }
        let fresh = fresh_selected_path(candidate);
        if fresh.exists() || fresh.is_symlink() {
            return Err(Error::Invalid("stage fresh selected path exists"));
        }
        let lease_path = lease_path(candidate)?;
        if lease_path.exists() || lease_path.is_symlink() {
            return Err(Error::Invalid("stage lease exists"));
        }
        isolation.verify(candidate, limits, WritePhase::Create)?;
        let mut lease = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&lease_path)?;
        if lease.try_lock_exclusive().is_err() {
            let _ = fs::remove_file(&lease_path);
            return Err(Error::Invalid("stage lease busy"));
        }
        let lease_metadata = lease.metadata()?;
        let file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(candidate)
        {
            Ok(file) => file,
            Err(error) => {
                let _ = fs::remove_file(&lease_path);
                return Err(Error::Io(error));
            }
        };
        let metadata = file.metadata()?;
        drop(file);
        let mut stage = Self {
            candidate: candidate.to_owned(),
            inode: (metadata.dev(), metadata.ino()),
            lease_path,
            lease_inode: (lease_metadata.dev(), lease_metadata.ino()),
            lease: Some(lease),
            db: None,
            vm_used: None,
            limits,
            receipt,
            registrations,
            owner,
            isolation,
            total_rows: 0,
            work_bytes: 0,
            poisoned: false,
            keep: false,
            selected_full: false,
            fresh_selected: None,
        };
        let lease = stage.lease.as_mut().expect("stage lease open");
        write!(
            lease,
            "tos-knowledge-stage-v1 {} {}\n",
            metadata.dev(),
            metadata.ino()
        )?;
        lease.sync_all()?;
        stage.check(WritePhase::SqliteOpen)?;
        let db = Connection::open(candidate)?;
        stage.db = Some(db);
        stage.vm_used = Some(sqlite_budget::configure(stage.db(), limits.sqlite)?);
        stage.check(WritePhase::Schema)?;
        stage.db().execute_batch(SCHEMA)?;
        stage.check(WritePhase::Schema)?;
        Ok(stage)
    }

    fn db(&self) -> &Connection {
        self.db.as_ref().expect("stage database open")
    }
    fn check(&self, phase: WritePhase) -> Result<()> {
        self.isolation.verify(&self.candidate, self.limits, phase)
    }
    /// A bounded producer may add catalog/search tables and indexed joins to
    /// this private database. The caller must keep its own row/byte budgets;
    /// the stage holds the SQLite VM/page/cache limits and host quota guard.
    /// Any callback or quota failure poisons the stage, so `finish` refuses it.
    pub(crate) fn with_connection<T>(
        &mut self,
        phase: WritePhase,
        f: impl FnOnce(&mut Connection) -> Result<T>,
    ) -> Result<T> {
        if self.poisoned {
            return Err(Error::Invalid("stage poisoned by prior failure"));
        }
        let result = (|| {
            self.check(phase)?;
            let value = f(self.db.as_mut().expect("stage database open"))?;
            self.check(phase)?;
            Ok(value)
        })();
        self.poisoned |= result.is_err();
        result
    }

    pub(crate) fn core_roots(&mut self) -> Result<CoreRoots> {
        self.with_connection(WritePhase::Sort, |db| {
            let (nodes, node_sha256) = output_root(db, "knowledge_nodes")?;
            let (relations, relation_sha256) = output_root(db, "knowledge_relations")?;
            Ok(CoreRoots {
                nodes,
                relations,
                node_sha256,
                relation_sha256,
            })
        })
    }
    pub(crate) fn charge(&mut self, payload: &[u8]) -> Result<()> {
        if payload.len() > self.limits.sqlite.max_row_bytes {
            return Err(Error::Budget("stage row bytes"));
        }
        self.total_rows = self
            .total_rows
            .checked_add(1)
            .ok_or(Error::Budget("stage rows"))?;
        if self.total_rows > self.limits.sqlite.max_rows {
            return Err(Error::Budget("stage rows"));
        }
        self.work_bytes = self
            .work_bytes
            .checked_add(payload.len() as u64)
            .ok_or(Error::Budget("stage work bytes"))?;
        if self.work_bytes > self.limits.sqlite.max_work_bytes {
            return Err(Error::Budget("stage work bytes"));
        }
        Ok(())
    }
    /// Charge a disk-backed external-sort copy before the SQL statement that
    /// writes final rows. A later failure poisons the stage and removes it.
    pub(crate) fn charge_materialized(&mut self, rows: u64, bytes: u64) -> Result<()> {
        self.total_rows = self
            .total_rows
            .checked_add(rows)
            .ok_or(Error::Budget("stage rows"))?;
        self.work_bytes = self
            .work_bytes
            .checked_add(bytes)
            .ok_or(Error::Budget("stage work bytes"))?;
        if self.total_rows > self.limits.sqlite.max_rows
            || self.work_bytes > self.limits.sqlite.max_work_bytes
        {
            return Err(Error::Budget("stage materialized rows/work bytes"));
        }
        Ok(())
    }
    pub fn ingest_input(&mut self, row: InputRow<'_>) -> Result<()> {
        let result = self.ingest_input_inner(row);
        self.poisoned |= result.is_err();
        result
    }
    fn ingest_input_inner(&mut self, row: InputRow<'_>) -> Result<()> {
        if !self.registered(row.source_graph, row.collection) {
            return Err(Error::Invalid("unregistered input collection"));
        }
        valid_id(row.id)?;
        self.charge(row.payload)?;
        self.check(WritePhase::Input)?;
        let digest = Digest256::of_bytes(row.payload);
        self.db().execute(
            "INSERT INTO raw_records VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                row.source_graph,
                row.collection,
                row.id,
                row.payload.len() as i64,
                &digest.as_bytes()[..],
                row.payload
            ],
        )?;
        self.check(WritePhase::Input)?;
        Ok(())
    }
    pub fn insert_node(&mut self, row: NodeRow<'_>) -> Result<()> {
        let result = self.insert_node_inner(row);
        self.poisoned |= result.is_err();
        result
    }
    fn insert_node_inner(&mut self, row: NodeRow<'_>) -> Result<()> {
        if !self.registrations.contains_key(row.source_graph) {
            return Err(Error::Invalid("unregistered node source"));
        }
        for value in [row.id, row.kind_id, row.type_id] {
            valid_id(value)?;
        }
        for value in [row.native_id, row.entity_id].into_iter().flatten() {
            valid_id(value)?;
        }
        if row.source_order < 0 {
            return Err(Error::Invalid("negative source order"));
        }
        self.charge(row.payload)?;
        self.check(WritePhase::Normalized)?;
        let digest = Digest256::of_bytes(row.payload);
        self.db().execute(
            "INSERT INTO knowledge_nodes VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                row.id,
                row.source_graph,
                row.native_id,
                row.entity_id,
                row.kind_id,
                row.type_id,
                row.source_order,
                row.payload.len() as i64,
                &digest.as_bytes()[..],
                row.payload
            ],
        )?;
        self.check(WritePhase::Normalized)?;
        Ok(())
    }
    pub fn insert_relation(&mut self, row: RelationRow<'_>) -> Result<()> {
        let result = self.insert_relation_inner(row);
        self.poisoned |= result.is_err();
        result
    }
    fn insert_relation_inner(&mut self, row: RelationRow<'_>) -> Result<()> {
        if !self.registrations.contains_key(row.source_graph) {
            return Err(Error::Invalid("unregistered relation source"));
        }
        for value in [
            row.id,
            row.from_id,
            row.to_id,
            row.predicate_id,
            row.relation_type_id,
        ] {
            valid_id(value)?;
        }
        if let Some(native_id) = row.native_id {
            valid_id(native_id)?;
        }
        if row.source_order < 0 {
            return Err(Error::Invalid("negative source order"));
        }
        self.charge(row.payload)?;
        self.check(WritePhase::Normalized)?;
        let digest = Digest256::of_bytes(row.payload);
        self.db().execute(
            "INSERT INTO knowledge_relations VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                row.id,
                row.source_graph,
                row.native_id,
                row.from_id,
                row.to_id,
                row.predicate_id,
                row.relation_type_id,
                row.source_order,
                row.payload.len() as i64,
                &digest.as_bytes()[..],
                row.payload
            ],
        )?;
        self.check(WritePhase::Normalized)?;
        Ok(())
    }

    /// Indexed exact ID seek. One row is transferred only after its actual
    /// length predicate passes; returned bytes and digest are verified.
    pub fn raw_by_id(
        &self,
        source_graph: &str,
        collection: &str,
        id: &str,
    ) -> Result<Option<SeekRow>> {
        if !self.registered(source_graph, collection) {
            return Err(Error::Invalid("unregistered input collection"));
        }
        valid_id(id)?;
        let row = self
            .db()
            .query_row(
                "SELECT id,source_graph,NULL,payload,payload_sha256 FROM raw_records
             WHERE source_graph=?1 AND collection=?2 AND id=?3
               AND length(payload)<=?4 AND payload_len=length(payload)",
                params![
                    source_graph,
                    collection,
                    id,
                    self.limits.sqlite.max_row_bytes as i64
                ],
                read_seek_row,
            )
            .optional()?;
        row.map(|row| verify_seek_row(row, self.limits.sqlite.max_row_bytes))
            .transpose()
    }

    /// Ordered raw input page using the `(source_graph,collection,id)` primary
    /// index. The page is bounded by both row and total payload bytes, so the
    /// caller can normalize it after the immutable stage borrow ends.
    pub fn scan_input(
        &self,
        source_graph: &str,
        collection: &str,
        after_id: Option<&str>,
        max_rows: usize,
    ) -> Result<ScanPage> {
        if !self.registered(source_graph, collection) {
            return Err(Error::Invalid("unregistered input collection"));
        }
        if let Some(id) = after_id {
            valid_id(id)?;
        }
        if max_rows == 0 || max_rows > self.limits.max_seek_rows {
            return Err(Error::Budget("stage seek rows"));
        }
        let sql = if after_id.is_some() {
            "SELECT id,source_graph,NULL,payload,payload_sha256 FROM raw_records
             WHERE source_graph=?1 AND collection=?2 AND id>?3
               AND length(payload)<=?4 AND payload_len=length(payload)
             ORDER BY id LIMIT ?5"
        } else {
            "SELECT id,source_graph,NULL,payload,payload_sha256 FROM raw_records
             WHERE source_graph=?1 AND collection=?2
               AND length(payload)<=?3 AND payload_len=length(payload)
             ORDER BY id LIMIT ?4"
        };
        let mut statement = self.db().prepare(sql)?;
        let lookahead = (max_rows + 1) as i64;
        let mut rows = if let Some(id) = after_id {
            statement.query(params![
                source_graph,
                collection,
                id,
                self.limits.sqlite.max_row_bytes as i64,
                lookahead
            ])?
        } else {
            statement.query(params![
                source_graph,
                collection,
                self.limits.sqlite.max_row_bytes as i64,
                lookahead
            ])?
        };
        let mut page = Vec::new();
        let mut bytes = 0u64;
        let mut has_more = false;
        while let Some(row) = rows.next()? {
            if page.len() == max_rows {
                has_more = true;
                break;
            }
            let item = verify_seek_row(read_seek_row(row)?, self.limits.sqlite.max_row_bytes)?;
            let next_bytes = bytes
                .checked_add(item.payload.len() as u64)
                .ok_or(Error::Budget("stage seek bytes"))?;
            if next_bytes > self.limits.max_seek_bytes {
                if page.is_empty() {
                    return Err(Error::Budget("stage seek bytes"));
                }
                has_more = true;
                break;
            }
            bytes = next_bytes;
            page.push(item);
        }
        let next_id = if has_more {
            page.last().map(|row| row.id.clone())
        } else {
            None
        };
        Ok(ScanPage {
            rows: page,
            next_id,
        })
    }

    /// Bounded index walk in deterministic `(source_order,id)` order.
    pub fn outgoing(
        &self,
        from_id: &str,
        after_order: i64,
        max_rows: usize,
        sink: &mut dyn FnMut(SeekRow) -> Result<()>,
    ) -> Result<usize> {
        valid_id(from_id)?;
        self.seek_indexed(
            "SELECT id,source_graph,source_order,payload,payload_sha256 FROM knowledge_relations
              WHERE from_id=?1 AND source_order>?2 AND length(payload)<=?3
                AND payload_len=length(payload)
              ORDER BY source_order,id LIMIT ?4",
            from_id,
            after_order,
            max_rows,
            sink,
        )
    }
    pub fn incoming(
        &self,
        to_id: &str,
        after_order: i64,
        max_rows: usize,
        sink: &mut dyn FnMut(SeekRow) -> Result<()>,
    ) -> Result<usize> {
        valid_id(to_id)?;
        self.seek_indexed(
            "SELECT id,source_graph,source_order,payload,payload_sha256 FROM knowledge_relations
              WHERE to_id=?1 AND source_order>?2 AND length(payload)<=?3
                AND payload_len=length(payload)
              ORDER BY source_order,id LIMIT ?4",
            to_id,
            after_order,
            max_rows,
            sink,
        )
    }
    fn seek_indexed(
        &self,
        sql: &str,
        endpoint: &str,
        after_order: i64,
        max_rows: usize,
        sink: &mut dyn FnMut(SeekRow) -> Result<()>,
    ) -> Result<usize> {
        if max_rows == 0 || max_rows > self.limits.max_seek_rows {
            return Err(Error::Budget("stage seek rows"));
        }
        let mut statement = self.db().prepare(sql)?;
        let mut rows = statement.query(params![
            endpoint,
            after_order,
            self.limits.sqlite.max_row_bytes as i64,
            max_rows as i64
        ])?;
        let mut count = 0usize;
        let mut bytes = 0u64;
        while let Some(row) = rows.next()? {
            let item = verify_seek_row(read_seek_row(row)?, self.limits.sqlite.max_row_bytes)?;
            bytes = bytes
                .checked_add(item.payload.len() as u64)
                .ok_or(Error::Budget("stage seek bytes"))?;
            if bytes > self.limits.max_seek_bytes {
                return Err(Error::Budget("stage seek bytes"));
            }
            sink(item)?;
            count += 1;
        }
        Ok(count)
    }

    pub fn finish(mut self) -> Result<StageReceipt> {
        if self.poisoned {
            return Err(Error::Invalid("stage poisoned by prior failed row"));
        }
        self.owner.recheck_sealed_cut(&self.receipt)?;
        self.check(WritePhase::Sort)?;
        let mut input_rows = 0u64;
        for entry in &self.receipt.collections {
            let (count, root) = input_root(self.db(), entry)?;
            self.check(WritePhase::Sort)?;
            if count != entry.expected_count || root != entry.expected_root_sha256 {
                return Err(Error::Invalid("input collection count/root mismatch"));
            }
            input_rows = input_rows
                .checked_add(count)
                .ok_or(Error::Budget("input rows"))?;
        }
        let (node_rows, node_root) = output_root(self.db(), "knowledge_nodes")?;
        self.check(WritePhase::Sort)?;
        let (relation_rows, relation_root) = output_root(self.db(), "knowledge_relations")?;
        self.check(WritePhase::Sort)?;
        let dangling: Option<String> = self
            .db()
            .query_row(
                "SELECT r.id FROM knowledge_relations r
             WHERE NOT EXISTS (SELECT 1 FROM knowledge_nodes n WHERE n.id=r.from_id)
                OR NOT EXISTS (SELECT 1 FROM knowledge_nodes n WHERE n.id=r.to_id)
             LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if dangling.is_some() {
            return Err(Error::Invalid("stage relation endpoint absent"));
        }
        self.check(WritePhase::Sort)?;
        let mut selected_file = None;
        if self.selected_full {
            self.check(WritePhase::Finalize)?;
            preflight_selected_vacuum(self.db(), &self.candidate, self.inode, self.limits)?;
            // Owner input is removed from the private stage only after exact
            // root checks. VACUUM INTO then creates a different SQLite inode
            // containing the allowlisted logical tables; the private stage
            // inode is never the selected artifact.
            self.db()
                .execute_batch("PRAGMA secure_delete=ON; DROP TABLE raw_records")?;
            selected_table_closure(self.db())?;
            self.check(WritePhase::Finalize)?;
            let fresh = fresh_selected_path(&self.candidate);
            self.isolation
                .verify(&fresh, self.limits, WritePhase::Finalize)?;
            let fresh_utf8 = fresh
                .to_str()
                .ok_or(Error::Invalid("stage fresh selected path encoding"))?;
            self.fresh_selected = Some(fresh.clone());
            self.db().execute("VACUUM INTO ?1", [fresh_utf8])?;
            self.check(WritePhase::Finalize)?;
            fs::set_permissions(&fresh, fs::Permissions::from_mode(0o600))?;
            let pinned = safe_open::open_regular(&fresh, self.limits.sqlite.max_output_bytes)?;
            verify_fresh_selected(
                &fresh,
                &pinned,
                self.limits.sqlite,
                Arc::clone(self.vm_used.as_ref().expect("stage VM counter")),
            )?;
            selected_file = Some(pinned);
        }
        self.check(WritePhase::Finalize)?;
        if self
            .db()
            .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))?
            != "ok"
        {
            return Err(Error::Invalid("stage SQLite integrity"));
        }
        self.check(WritePhase::Finalize)?;
        self.owner.recheck_sealed_cut(&self.receipt)?;
        let db = self.db.take().expect("stage database open");
        db.close().map_err(|(_, e)| Error::Sql(e))?;
        let output_path = self.fresh_selected.as_deref().unwrap_or(&self.candidate);
        let (sqlite_sha256, sqlite_size_bytes) = if let Some(pinned) = selected_file.as_ref() {
            let mut digest_file = pinned.try_clone()?;
            digest_file.rewind()?;
            stream_digest(&mut digest_file)?
        } else {
            file_digest(output_path)?
        };
        if sqlite_size_bytes > self.limits.sqlite.max_output_bytes {
            return Err(Error::Budget("stage final output bytes"));
        }
        if let Some(pinned) = selected_file.as_ref() {
            pinned.sync_all()?;
        } else {
            fs::File::open(output_path)?.sync_all()?;
        }
        if let Some(fresh) = self.fresh_selected.as_ref() {
            let old = fs::symlink_metadata(&self.candidate)?;
            let new = fs::symlink_metadata(fresh)?;
            let pinned = selected_file
                .as_ref()
                .ok_or(Error::Invalid("fresh selected file not retained"))?
                .metadata()?;
            if !old.file_type().is_file()
                || (old.dev(), old.ino()) != self.inode
                || !new.file_type().is_file()
                || (new.dev(), new.ino()) != (pinned.dev(), pinned.ino())
                || new.len() != sqlite_size_bytes
                || old.uid() != new.uid()
            {
                return Err(Error::Invalid("selected stage inode changed"));
            }
            cleanup_sqlite_sidecars(&self.candidate, old.uid())?;
            fs::rename(fresh, &self.candidate)?;
            self.inode = (new.dev(), new.ino());
            self.fresh_selected = None;
            let installed = fs::symlink_metadata(&self.candidate)?;
            if !installed.file_type().is_file()
                || (installed.dev(), installed.ino()) != self.inode
                || installed.len() != sqlite_size_bytes
            {
                return Err(Error::Invalid("selected installed inode changed"));
            }
        }
        fs::File::open(self.candidate.parent().expect("stage parent"))?.sync_all()?;
        self.remove_lease()?;
        self.keep = true;
        Ok(StageReceipt {
            source_cut: self.receipt.binding.source_cut.clone(),
            membership_root: self.receipt.binding.membership_root.clone(),
            input_collections: self.receipt.collections.len(),
            verified_inputs: self.receipt.collections.clone(),
            input_rows,
            node_rows,
            relation_rows,
            node_root_sha256: node_root,
            relation_root_sha256: relation_root,
            sqlite_sha256,
            sqlite_size_bytes,
        })
    }
}

/// VACUUM builds a second database while the current one is still present.
/// Reserve two current-file equivalents in the declared private temp budget
/// before dropping owner input or starting that rebuild. The host isolation
/// guard remains responsible for enforcing the actual peak across temp,
/// rollback, fallback paths and output files; this arithmetic is an early
/// refusal, not a filesystem quota implementation.
fn preflight_selected_vacuum(
    db: &Connection,
    candidate: &Path,
    inode: (u64, u64),
    limits: StageLimits,
) -> Result<()> {
    let page_count: i64 = db.query_row("PRAGMA page_count", [], |row| row.get(0))?;
    let page_size: i64 = db.query_row("PRAGMA page_size", [], |row| row.get(0))?;
    let (page_count, page_size) = (
        u64::try_from(page_count).map_err(|_| Error::Invalid("selected page count"))?,
        u64::try_from(page_size).map_err(|_| Error::Invalid("selected page size"))?,
    );
    if page_count == 0 || page_size == 0 {
        return Err(Error::Invalid("selected SQLite page geometry"));
    }
    let database_bytes = page_count
        .checked_mul(page_size)
        .ok_or(Error::Budget("selected SQLite page bytes"))?;
    let rebuild_reserve = database_bytes
        .checked_mul(2)
        .ok_or(Error::Budget("selected VACUUM rebuild reserve"))?;
    let metadata = fs::symlink_metadata(candidate)?;
    if !metadata.file_type().is_file()
        || (metadata.dev(), metadata.ino()) != inode
        || metadata.len() != database_bytes
    {
        return Err(Error::Invalid("selected SQLite file/page mismatch"));
    }
    if database_bytes > limits.sqlite.max_output_bytes || rebuild_reserve > limits.max_temp_bytes {
        return Err(Error::Budget("selected VACUUM output/temp reserve"));
    }
    Ok(())
}

pub(crate) fn selected_table_closure(db: &Connection) -> Result<()> {
    const TABLES: &[&str] = &[
        "metadata",
        "graph_header",
        "knowledge_nodes",
        "knowledge_relations",
        "source_scope",
        "search_documents",
        "search_grams",
        "search_gram_stats",
        "catalog_index_meta",
        "catalog_facet_fields",
        "catalog_facets",
        "catalog_routes",
        "catalog_source_counts",
    ];
    let mut statement = db.prepare(
        "SELECT CASE WHEN typeof(name)='text' AND length(CAST(name AS BLOB))<=128 THEN name ELSE NULL END
         FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let mut rows = statement.query([])?;
    let mut seen = std::collections::BTreeSet::new();
    while let Some(row) = rows.next()? {
        let name: Option<String> = row.get(0)?;
        let Some(name) = name else {
            return Err(Error::Budget("selected knowledge table name bytes"));
        };
        if !TABLES.contains(&name.as_str()) || !seen.insert(name) {
            return Err(Error::Invalid("unexpected selected knowledge table"));
        }
    }
    if seen.len() != TABLES.len() {
        return Err(Error::Invalid("missing selected knowledge table"));
    }
    const EXPLICIT_INDEXES: &[(&str, &str)] = &[
        (
            "knowledge_nodes_source_order",
            "CREATE INDEX knowledge_nodes_source_order ON knowledge_nodes(source_graph,source_order,id)",
        ),
        (
            "knowledge_nodes_kind",
            "CREATE INDEX knowledge_nodes_kind ON knowledge_nodes(kind_id,source_order)",
        ),
        (
            "knowledge_nodes_entity",
            "CREATE INDEX knowledge_nodes_entity ON knowledge_nodes(entity_id,source_order)",
        ),
        (
            "knowledge_relations_source_order",
            "CREATE INDEX knowledge_relations_source_order ON knowledge_relations(source_graph,source_order,id)",
        ),
        (
            "knowledge_relations_from",
            "CREATE INDEX knowledge_relations_from ON knowledge_relations(from_id,source_order,id)",
        ),
        (
            "knowledge_relations_to",
            "CREATE INDEX knowledge_relations_to ON knowledge_relations(to_id,source_order,id)",
        ),
        (
            "knowledge_relations_predicate",
            "CREATE INDEX knowledge_relations_predicate ON knowledge_relations(predicate_id,source_order)",
        ),
        (
            "search_document_filter",
            "CREATE INDEX search_document_filter ON search_documents(kind,source_graph,kind_id,predicate_id,position)",
        ),
    ];
    let mut statement = db.prepare(
        "SELECT CASE WHEN typeof(name)='text' AND length(CAST(name AS BLOB))<=128 THEN name ELSE NULL END,
                CASE WHEN typeof(sql)='text' AND length(CAST(sql AS BLOB))<=1024 THEN sql ELSE NULL END
         FROM sqlite_master WHERE type='index' AND sql IS NOT NULL ORDER BY name",
    )?;
    let mut rows = statement.query([])?;
    let mut indexes = BTreeSet::new();
    while let Some(row) = rows.next()? {
        let name: Option<String> = row.get(0)?;
        let sql: Option<String> = row.get(1)?;
        let (Some(name), Some(sql)) = (name, sql) else {
            return Err(Error::Budget("selected knowledge schema text bytes"));
        };
        if !EXPLICIT_INDEXES
            .iter()
            .any(|(expected_name, expected_sql)| name == *expected_name && sql == *expected_sql)
            || !indexes.insert(name)
        {
            return Err(Error::Invalid("unexpected selected knowledge index"));
        }
    }
    if indexes.len() != EXPLICIT_INDEXES.len() {
        return Err(Error::Invalid("missing selected knowledge index"));
    }
    let extra: Option<i64> = db
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type NOT IN ('table','index') LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if extra.is_some() {
        return Err(Error::Invalid(
            "unexpected selected knowledge schema object",
        ));
    }
    Ok(())
}

fn verify_fresh_selected(
    path: &Path,
    pinned: &fs::File,
    limits: Limits,
    used: Arc<AtomicU64>,
) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    let opened = pinned.metadata()?;
    if !metadata.file_type().is_file()
        || (metadata.dev(), metadata.ino()) != (opened.dev(), opened.ino())
        || opened.len() > limits.max_output_bytes
    {
        return Err(Error::Budget("fresh selected SQLite bytes/type"));
    }
    if sqlite_sidecar_paths(path)
        .iter()
        .any(|sidecar| sidecar.exists() || sidecar.is_symlink())
    {
        return Err(Error::Invalid("fresh selected SQLite sidecar"));
    }
    let uri = format!(
        "file:/proc/self/fd/{}?mode=ro&immutable=1",
        pinned.as_raw_fd()
    );
    let db = Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;
    sqlite_budget::install_progress(&db, limits, used);
    db.pragma_update(None, "cache_size", -(limits.sqlite_cache_kib as i64))?;
    db.execute_batch("PRAGMA temp_store=FILE")?;
    crate::knowledge_selected::verify_schema(&db)?;
    let integrity: String = db.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    let freelist: u64 = db.query_row("PRAGMA freelist_count", [], |row| row.get(0))?;
    if integrity != "ok" || freelist != 0 {
        return Err(Error::Invalid("fresh selected SQLite integrity/pages"));
    }
    db.close().map_err(|(_, error)| Error::Sql(error))?;
    Ok(())
}

impl Drop for KnowledgeStage<'_> {
    fn drop(&mut self) {
        self.db.take();
        if !self.keep {
            if let Some(fresh) = self.fresh_selected.as_ref() {
                if let (Ok(metadata), Some(lease)) =
                    (fs::symlink_metadata(fresh), self.lease.as_ref())
                {
                    if metadata.file_type().is_file()
                        && lease
                            .metadata()
                            .is_ok_and(|lease| lease.uid() == metadata.uid())
                    {
                        let _ = cleanup_sqlite_sidecars(fresh, metadata.uid());
                        let _ = fs::remove_file(fresh);
                    }
                }
            }
            if let Ok(metadata) = fs::symlink_metadata(&self.candidate) {
                if metadata.file_type().is_file() && (metadata.dev(), metadata.ino()) == self.inode
                {
                    let _ = cleanup_sqlite_sidecars(&self.candidate, metadata.uid());
                    let _ = fs::remove_file(&self.candidate);
                }
            }
            let _ = self.remove_lease();
        }
    }
}

impl KnowledgeStage<'_> {
    fn remove_lease(&mut self) -> Result<()> {
        if let Ok(metadata) = fs::symlink_metadata(&self.lease_path) {
            if !metadata.file_type().is_file()
                || (metadata.dev(), metadata.ino()) != self.lease_inode
            {
                return Err(Error::Invalid("stage lease path changed"));
            }
            fs::remove_file(&self.lease_path)?;
        }
        self.lease.take();
        Ok(())
    }
}

fn lease_path(candidate: &Path) -> Result<PathBuf> {
    let name = candidate
        .file_name()
        .ok_or(Error::Invalid("stage candidate filename"))?;
    let mut name = name.to_os_string();
    name.push(".stage-lease");
    Ok(candidate.with_file_name(name))
}

fn fresh_selected_path(candidate: &Path) -> PathBuf {
    let mut path = candidate.as_os_str().to_os_string();
    path.push(".fresh-selected");
    PathBuf::from(path)
}

fn sqlite_sidecar_paths(candidate: &Path) -> [PathBuf; 3] {
    ["-journal", "-wal", "-shm"].map(|suffix| {
        let mut path = candidate.as_os_str().to_os_string();
        path.push(suffix);
        PathBuf::from(path)
    })
}
fn cleanup_sqlite_sidecars(candidate: &Path, uid: u32) -> Result<()> {
    for path in sqlite_sidecar_paths(candidate) {
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if !metadata.file_type().is_file() || metadata.uid() != uid {
                    return Err(Error::Invalid("stage SQLite sidecar changed"));
                }
                fs::remove_file(path)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(Error::Io(error)),
        }
    }
    Ok(())
}

/// Recover one abandoned private candidate after the owner has established
/// that its producer process is gone. An active lock or changed inode refuses;
/// this never touches selected models or arbitrary neighboring files.
pub fn reap_abandoned_private_stage(candidate: &Path) -> Result<bool> {
    let lease_path = lease_path(candidate)?;
    if !lease_path.exists() && !lease_path.is_symlink() {
        return Ok(false);
    }
    let mut lease = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&lease_path)?;
    if !lease.metadata()?.file_type().is_file() {
        return Err(Error::Invalid("stage lease type"));
    }
    lease
        .try_lock_exclusive()
        .map_err(|_| Error::Invalid("stage producer still active"))?;
    let mut marker = String::new();
    lease.seek(SeekFrom::Start(0))?;
    if lease.metadata()?.len() > 128 {
        return Err(Error::Invalid("stage lease marker bytes"));
    }
    Read::by_ref(&mut lease)
        .take(128)
        .read_to_string(&mut marker)?;
    let mut fields = marker.split_whitespace();
    if fields.next() != Some("tos-knowledge-stage-v1") {
        return Err(Error::Invalid("stage lease marker"));
    }
    let dev: u64 = fields
        .next()
        .ok_or(Error::Invalid("stage lease device"))?
        .parse()
        .map_err(|_| Error::Invalid("stage lease device"))?;
    let ino: u64 = fields
        .next()
        .ok_or(Error::Invalid("stage lease inode"))?
        .parse()
        .map_err(|_| Error::Invalid("stage lease inode"))?;
    if fields.next().is_some() {
        return Err(Error::Invalid("stage lease marker trailing data"));
    }
    let metadata = fs::symlink_metadata(candidate)?;
    if !metadata.file_type().is_file() || (metadata.dev(), metadata.ino()) != (dev, ino) {
        return Err(Error::Invalid("stage candidate changed since lease"));
    }
    let fresh = fresh_selected_path(candidate);
    match fs::symlink_metadata(&fresh) {
        Ok(output) => {
            if !output.file_type().is_file() || output.uid() != metadata.uid() {
                return Err(Error::Invalid("abandoned fresh selected path changed"));
            }
            cleanup_sqlite_sidecars(&fresh, output.uid())?;
            fs::remove_file(&fresh)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(Error::Io(error)),
    }
    cleanup_sqlite_sidecars(candidate, metadata.uid())?;
    fs::remove_file(candidate)?;
    fs::remove_file(&lease_path)?;
    FileExt::unlock(&lease)?;
    fs::File::open(candidate.parent().ok_or(Error::Invalid("stage parent"))?)?.sync_all()?;
    Ok(true)
}

fn valid_id(value: &str) -> Result<()> {
    if value.is_empty() || value.len() > MAX_NAME_BYTES {
        return Err(Error::Invalid("stage ID bytes"));
    }
    Ok(())
}
fn root_item(hash: &mut Digest256Hasher, id: &str, digest: &[u8]) {
    hash.update(&(id.len() as u64).to_be_bytes());
    hash.update(id.as_bytes());
    hash.update(digest);
}
fn input_root(db: &Connection, entry: &InputCollectionReceipt) -> Result<(u64, String)> {
    let mut statement = db.prepare(
        "SELECT id,payload_sha256 FROM raw_records
      WHERE source_graph=?1 AND collection=?2 ORDER BY id",
    )?;
    let mut rows = statement.query(params![entry.source_graph, entry.collection])?;
    let mut hash = Digest256Hasher::new();
    let mut count = 0u64;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let digest: Vec<u8> = row.get(1)?;
        if digest.len() != 32 {
            return Err(Error::Invalid("stage payload digest size"));
        }
        root_item(&mut hash, &id, &digest);
        count = count.checked_add(1).ok_or(Error::Budget("input rows"))?;
    }
    Ok((count, hash.finalize().to_hex()))
}
fn output_root(db: &Connection, table: &str) -> Result<(u64, String)> {
    let sql = match table {
        "knowledge_nodes" => {
            "SELECT id,source_graph,source_order,payload_sha256 FROM knowledge_nodes ORDER BY source_graph,id"
        }
        "knowledge_relations" => {
            "SELECT id,source_graph,source_order,payload_sha256 FROM knowledge_relations ORDER BY source_graph,id"
        }
        _ => return Err(Error::Invalid("unknown stage output table")),
    };
    let mut statement = db.prepare(sql)?;
    let mut rows = statement.query([])?;
    let mut hash = Digest256Hasher::new();
    let mut count = 0u64;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let order: i64 = row.get(2)?;
        let digest: Vec<u8> = row.get(3)?;
        if digest.len() != 32 || order < 0 || order as u64 != count {
            return Err(Error::Invalid("stage output source order/digest"));
        }
        root_item(&mut hash, &id, &digest);
        count = count
            .checked_add(1)
            .ok_or(Error::Budget("stage output rows"))?;
    }
    Ok((count, hash.finalize().to_hex()))
}
fn read_seek_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SeekRow> {
    let digest: Vec<u8> = row.get(4)?;
    Ok(SeekRow {
        id: row.get(0)?,
        source_graph: row.get(1)?,
        source_order: row.get(2)?,
        payload: row.get(3)?,
        payload_sha256: digest.iter().map(|b| format!("{b:02x}")).collect(),
    })
}
fn verify_seek_row(row: SeekRow, max_row_bytes: usize) -> Result<SeekRow> {
    if row.payload.len() > max_row_bytes {
        return Err(Error::Budget("stage seek row bytes"));
    }
    if Digest256::of_bytes(&row.payload).to_hex() != row.payload_sha256 {
        return Err(Error::Invalid("stage seek payload digest"));
    }
    Ok(row)
}

const SCHEMA: &str = r#"
CREATE TABLE raw_records(
 source_graph TEXT NOT NULL,collection TEXT NOT NULL,id TEXT NOT NULL,
 payload_len INTEGER NOT NULL,payload_sha256 BLOB NOT NULL,payload BLOB NOT NULL,
 PRIMARY KEY(source_graph,collection,id)) WITHOUT ROWID;
CREATE TABLE knowledge_nodes(
 id TEXT PRIMARY KEY,source_graph TEXT NOT NULL,native_id TEXT,entity_id TEXT,
 kind_id TEXT NOT NULL,type_id TEXT NOT NULL,source_order INTEGER NOT NULL UNIQUE,
 payload_len INTEGER NOT NULL,payload_sha256 BLOB NOT NULL,payload BLOB NOT NULL) WITHOUT ROWID;
CREATE INDEX knowledge_nodes_source_order ON knowledge_nodes(source_graph,source_order,id);
CREATE INDEX knowledge_nodes_kind ON knowledge_nodes(kind_id,source_order);
CREATE INDEX knowledge_nodes_entity ON knowledge_nodes(entity_id,source_order);
CREATE TABLE knowledge_relations(
 id TEXT PRIMARY KEY,source_graph TEXT NOT NULL,native_id TEXT,
 from_id TEXT NOT NULL,to_id TEXT NOT NULL,predicate_id TEXT NOT NULL,
 relation_type_id TEXT NOT NULL,source_order INTEGER NOT NULL UNIQUE,
 payload_len INTEGER NOT NULL,payload_sha256 BLOB NOT NULL,payload BLOB NOT NULL) WITHOUT ROWID;
CREATE INDEX knowledge_relations_source_order ON knowledge_relations(source_graph,source_order,id);
CREATE INDEX knowledge_relations_from ON knowledge_relations(from_id,source_order,id);
CREATE INDEX knowledge_relations_to ON knowledge_relations(to_id,source_order,id);
CREATE INDEX knowledge_relations_predicate ON knowledge_relations(predicate_id,source_order);
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    const RAW_ROOT: &str = "4a6512ce1f0842dc9246eb059c186dfb2b08bd6f2515f000474c792273958ca4";
    const EMPTY_ROOT: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    struct Owner {
        checks: AtomicUsize,
    }
    impl StageOwner for Owner {
        fn verify_receipt(&self, receipt: &ExactInputReceipt) -> Result<()> {
            if receipt.collections[0].adapter_profile != "fixture-adapter-v1" {
                return Err(Error::Invalid("fixture adapter"));
            }
            self.checks.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn recheck_sealed_cut(&self, _: &ExactInputReceipt) -> Result<()> {
            self.checks.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
    struct TestQuota {
        calls: AtomicUsize,
        deny: bool,
    }
    impl StageIsolation for TestQuota {
        fn verify(&self, _: &Path, limits: StageLimits, _: WritePhase) -> Result<()> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.deny || limits.max_temp_bytes == 0 {
                return Err(Error::Budget("test host temp quota"));
            }
            Ok(())
        }
    }
    fn limits() -> StageLimits {
        StageLimits {
            sqlite: Limits::default(),
            max_temp_bytes: 64 * 1024 * 1024,
            max_seek_rows: 8,
            max_seek_bytes: 1024,
        }
    }
    fn exact_receipt(root: &str) -> ExactInputReceipt {
        ExactInputReceipt {
            binding: SourceBinding {
                owner_profile: "fixture-owner".into(),
                source_cut: "sealed-cut-7".into(),
                through_commit_seq: 7,
                membership_root: "0".repeat(64),
                index_generation: "gen-1".into(),
                route_map_version: "routes-1".into(),
                reader_abi: "reader-1".into(),
                projection_root_sha256: "1".repeat(64),
                complete: true,
            },
            collections: vec![
                InputCollectionReceipt {
                    source_graph: "fixture.graph".into(),
                    collection: "fixture/raw".into(),
                    input_role: "fixture".into(),
                    adapter_profile: "fixture-adapter-v1".into(),
                    expected_count: 1,
                    expected_root_sha256: root.into(),
                },
                InputCollectionReceipt {
                    source_graph: "fixture.graph".into(),
                    collection: "fixture/empty".into(),
                    input_role: "fixture".into(),
                    adapter_profile: "fixture-adapter-v1".into(),
                    expected_count: 0,
                    expected_root_sha256: EMPTY_ROOT.into(),
                },
            ],
        }
    }

    #[test]
    fn registration_index_accepts_more_than_legacy_collection_ceiling() {
        let candidate = stage_path("many-collections");
        let owner = Owner {
            checks: AtomicUsize::new(0),
        };
        let quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: false,
        };
        let mut receipt = exact_receipt(RAW_ROOT);
        for index in 0..300 {
            receipt.collections.push(InputCollectionReceipt {
                source_graph: "fixture.graph".into(),
                collection: format!("extra/{index}"),
                input_role: "fixture".into(),
                adapter_profile: "fixture-adapter-v1".into(),
                expected_count: 0,
                expected_root_sha256: EMPTY_ROOT.into(),
            });
        }
        let mut stage =
            KnowledgeStage::create(&candidate, limits(), receipt, &owner, &quota).unwrap();
        assert!(stage.registered("fixture.graph", "extra/299"));
        assert!(!stage.registered("fixture.graph", "extra/300"));
        stage
            .ingest_input(InputRow {
                source_graph: "fixture.graph",
                collection: "extra/299",
                id: "row.1",
                payload: b"row",
            })
            .unwrap();
        drop(stage);
        fs::remove_dir_all(candidate.parent().unwrap()).unwrap();
    }
    fn stage_path(label: &str) -> PathBuf {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "tos-knowledge-stage-{label}-{}-{tick}",
            std::process::id()
        ));
        fs::create_dir(&dir).unwrap();
        dir.join("candidate.sqlite3")
    }
    fn ingest_fixture(stage: &mut KnowledgeStage<'_>) {
        stage
            .ingest_input(InputRow {
                source_graph: "fixture.graph",
                collection: "fixture/raw",
                id: "raw.1",
                payload: b"raw",
            })
            .unwrap();
        stage
            .insert_node(NodeRow {
                id: "node.1",
                source_graph: "fixture.graph",
                native_id: None,
                entity_id: None,
                kind_id: "kind.1",
                type_id: "type.1",
                source_order: 0,
                payload: b"node",
            })
            .unwrap();
        stage
            .insert_relation(RelationRow {
                id: "rel.1",
                source_graph: "fixture.graph",
                native_id: None,
                from_id: "node.1",
                to_id: "node.1",
                predicate_id: "pred.1",
                relation_type_id: "reltype.1",
                source_order: 0,
                payload: b"relation",
            })
            .unwrap();
    }

    #[test]
    fn exact_owner_root_and_indexed_seek_yield_private_complete_stage() {
        let candidate = stage_path("complete");
        let owner = Owner {
            checks: AtomicUsize::new(0),
        };
        let quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: false,
        };
        let mut stage = KnowledgeStage::create(
            &candidate,
            limits(),
            exact_receipt(RAW_ROOT),
            &owner,
            &quota,
        )
        .unwrap();
        ingest_fixture(&mut stage);
        assert_eq!(
            stage
                .raw_by_id("fixture.graph", "fixture/raw", "raw.1")
                .unwrap()
                .unwrap()
                .payload,
            b"raw"
        );
        let page = stage
            .scan_input("fixture.graph", "fixture/raw", None, 1)
            .unwrap();
        assert_eq!(page.rows.len(), 1);
        assert_eq!(
            page.rows[0].payload_sha256,
            Digest256::of_bytes(b"raw").to_hex()
        );
        assert!(page.next_id.is_none());
        assert!(
            stage
                .scan_input("fixture.graph", "fixture/raw", Some("raw.1"), 1)
                .unwrap()
                .rows
                .is_empty()
        );
        let empty = stage
            .scan_input("fixture.graph", "fixture/empty", None, 1)
            .unwrap();
        assert!(empty.rows.is_empty() && empty.next_id.is_none());
        let mut ids = Vec::new();
        assert_eq!(
            stage
                .outgoing("node.1", -1, 2, &mut |row| {
                    ids.push(row.id);
                    Ok(())
                })
                .unwrap(),
            1
        );
        assert_eq!(ids, ["rel.1"]);
        let receipt = stage.finish().unwrap();
        assert_eq!(
            (
                receipt.input_collections,
                receipt.input_rows,
                receipt.node_rows,
                receipt.relation_rows
            ),
            (2, 1, 1, 1)
        );
        assert_eq!(receipt.source_cut, "sealed-cut-7");
        assert!(candidate.is_file());
        assert_eq!(owner.checks.load(Ordering::SeqCst), 3);
        assert!(quota.calls.load(Ordering::SeqCst) >= 6);
        fs::remove_dir_all(candidate.parent().unwrap()).unwrap();
    }

    #[test]
    fn omitted_input_and_ignored_insert_error_cannot_finish() {
        let owner = Owner {
            checks: AtomicUsize::new(0),
        };
        let quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: false,
        };
        let omitted = stage_path("omitted");
        let stage =
            KnowledgeStage::create(&omitted, limits(), exact_receipt(RAW_ROOT), &owner, &quota)
                .unwrap();
        assert!(stage.finish().is_err());
        assert!(!omitted.exists());
        fs::remove_dir_all(omitted.parent().unwrap()).unwrap();

        let duplicate = stage_path("duplicate");
        let mut stage = KnowledgeStage::create(
            &duplicate,
            limits(),
            exact_receipt(RAW_ROOT),
            &owner,
            &quota,
        )
        .unwrap();
        stage
            .ingest_input(InputRow {
                source_graph: "fixture.graph",
                collection: "fixture/raw",
                id: "raw.1",
                payload: b"raw",
            })
            .unwrap();
        assert!(
            stage
                .ingest_input(InputRow {
                    source_graph: "fixture.graph",
                    collection: "fixture/raw",
                    id: "raw.1",
                    payload: b"raw"
                })
                .is_err()
        );
        assert!(stage.finish().is_err());
        assert!(!duplicate.exists());
        fs::remove_dir_all(duplicate.parent().unwrap()).unwrap();
    }

    #[test]
    fn owner_root_and_quota_gate_fail_before_candidate_admission() {
        let owner = Owner {
            checks: AtomicUsize::new(0),
        };
        let quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: false,
        };
        let wrong = stage_path("root");
        let mut stage =
            KnowledgeStage::create(&wrong, limits(), exact_receipt(EMPTY_ROOT), &owner, &quota)
                .unwrap();
        ingest_fixture(&mut stage);
        assert!(stage.finish().is_err());
        assert!(!wrong.exists());
        fs::remove_dir_all(wrong.parent().unwrap()).unwrap();

        let denied = stage_path("quota");
        let denied_quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: true,
        };
        assert!(
            KnowledgeStage::create(
                &denied,
                limits(),
                exact_receipt(RAW_ROOT),
                &owner,
                &denied_quota
            )
            .is_err()
        );
        assert!(!denied.exists());
        fs::remove_dir_all(denied.parent().unwrap()).unwrap();
    }

    #[test]
    fn selected_vacuum_refuses_unadmitted_rebuild_before_private_drop() {
        let candidate = stage_path("vacuum-reserve");
        let owner = Owner {
            checks: AtomicUsize::new(0),
        };
        let quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: false,
        };
        let mut tight = limits();
        tight.max_temp_bytes = 1;
        let mut stage =
            KnowledgeStage::create(&candidate, tight, exact_receipt(RAW_ROOT), &owner, &quota)
                .unwrap();
        stage
            .ingest_input(InputRow {
                source_graph: "fixture.graph",
                collection: "fixture/raw",
                id: "raw.1",
                payload: b"raw",
            })
            .unwrap();
        stage.mark_selected_full().unwrap();
        let result = stage.finish();
        assert!(matches!(
            result,
            Err(Error::Budget("selected VACUUM output/temp reserve"))
        ));
        assert!(!candidate.exists());
        fs::remove_dir_all(candidate.parent().unwrap()).unwrap();
    }

    #[test]
    fn noncontiguous_source_order_is_rejected_after_external_sort() {
        let candidate = stage_path("order");
        let owner = Owner {
            checks: AtomicUsize::new(0),
        };
        let quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: false,
        };
        let mut stage = KnowledgeStage::create(
            &candidate,
            limits(),
            exact_receipt(RAW_ROOT),
            &owner,
            &quota,
        )
        .unwrap();
        stage
            .ingest_input(InputRow {
                source_graph: "fixture.graph",
                collection: "fixture/raw",
                id: "raw.1",
                payload: b"raw",
            })
            .unwrap();
        stage
            .insert_node(NodeRow {
                id: "node.1",
                source_graph: "fixture.graph",
                native_id: None,
                entity_id: None,
                kind_id: "kind.1",
                type_id: "type.1",
                source_order: 1,
                payload: b"node",
            })
            .unwrap();
        assert!(stage.finish().is_err());
        assert!(!candidate.exists());
        fs::remove_dir_all(candidate.parent().unwrap()).unwrap();
    }

    #[test]
    fn endpoint_closure_and_composer_failure_cannot_finish() {
        let owner = Owner {
            checks: AtomicUsize::new(0),
        };
        let quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: false,
        };
        let candidate = stage_path("endpoint");
        let mut stage = KnowledgeStage::create(
            &candidate,
            limits(),
            exact_receipt(RAW_ROOT),
            &owner,
            &quota,
        )
        .unwrap();
        stage
            .ingest_input(InputRow {
                source_graph: "fixture.graph",
                collection: "fixture/raw",
                id: "raw.1",
                payload: b"raw",
            })
            .unwrap();
        stage
            .insert_node(NodeRow {
                id: "node.1",
                source_graph: "fixture.graph",
                native_id: None,
                entity_id: None,
                kind_id: "kind.1",
                type_id: "type.1",
                source_order: 0,
                payload: b"node",
            })
            .unwrap();
        stage
            .insert_relation(RelationRow {
                id: "rel.1",
                source_graph: "fixture.graph",
                native_id: None,
                from_id: "node.1",
                to_id: "node.absent",
                predicate_id: "pred.1",
                relation_type_id: "reltype.1",
                source_order: 0,
                payload: b"relation",
            })
            .unwrap();
        assert!(stage.finish().is_err());
        assert!(!candidate.exists());
        fs::remove_dir_all(candidate.parent().unwrap()).unwrap();

        let candidate = stage_path("composer");
        let mut stage = KnowledgeStage::create(
            &candidate,
            limits(),
            exact_receipt(RAW_ROOT),
            &owner,
            &quota,
        )
        .unwrap();
        ingest_fixture(&mut stage);
        assert!(
            stage
                .with_connection::<()>(WritePhase::Catalog, |_| Err(Error::Invalid(
                    "catalog failure"
                )))
                .is_err()
        );
        assert!(stage.finish().is_err());
        assert!(!candidate.exists());
        fs::remove_dir_all(candidate.parent().unwrap()).unwrap();
    }

    #[test]
    fn abandoned_exact_private_stage_is_reaped_but_live_stage_refuses() {
        let owner = Owner {
            checks: AtomicUsize::new(0),
        };
        let quota = TestQuota {
            calls: AtomicUsize::new(0),
            deny: false,
        };
        let live = stage_path("live");
        let stage =
            KnowledgeStage::create(&live, limits(), exact_receipt(RAW_ROOT), &owner, &quota)
                .unwrap();
        assert!(reap_abandoned_private_stage(&live).is_err());
        drop(stage);
        assert!(!live.exists());
        fs::remove_dir_all(live.parent().unwrap()).unwrap();

        let orphan = stage_path("orphan");
        fs::write(&orphan, b"private interrupted stage").unwrap();
        let metadata = fs::metadata(&orphan).unwrap();
        fs::write(
            lease_path(&orphan).unwrap(),
            format!(
                "tos-knowledge-stage-v1 {} {}\n",
                metadata.dev(),
                metadata.ino()
            ),
        )
        .unwrap();
        assert!(reap_abandoned_private_stage(&orphan).unwrap());
        assert!(!orphan.exists());
        assert!(!lease_path(&orphan).unwrap().exists());
        fs::remove_dir_all(orphan.parent().unwrap()).unwrap();
    }
}
