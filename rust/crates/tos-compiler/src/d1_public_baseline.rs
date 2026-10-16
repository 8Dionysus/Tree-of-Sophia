//! Disposable disk index for the existing full D1 row-baseline JSON contract.
//! It records exact emitted key literals and statement digests, not a selected
//! predecessor or permission to publish a future affected pair.

use crate::{
    Error, Result,
    d1_public_capture::{PublicCapture, PublicCaptureLimits},
    d1_public_sql::MAX_STATEMENT_BYTES,
    sqlite_budget,
};
use rusqlite::{Connection, params};
use serde_json::Value;
use std::{
    fs::{self, OpenOptions},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
};
use tos_foundation::Digest256;

pub(crate) const TABLES: &[(&str, &[&str])] = &[
    ("edge_meta", &["key", "part"]),
    ("philosophy_nodes", &["id"]),
    ("philosophy_edges", &["id"]),
    ("philosophy_aux", &["collection", "ord"]),
    ("philosophy_clusters", &["id", "part"]),
    ("philosophy_cluster_nodes", &["cluster_id", "member_ord"]),
    ("philosophy_cluster_edges", &["cluster_id", "member_ord"]),
    ("philosophy_review_packets", &["view_id"]),
    ("corpus_items", &["collection", "ord"]),
    ("corpus_edges", &["ord"]),
    ("corpus_packs", &["id"]),
    ("knowledge_nodes", &["id"]),
    ("knowledge_relations", &["id"]),
    ("knowledge_search_documents", &["kind", "position"]),
    ("knowledge_search_grams", &["kind", "n", "gram", "position"]),
    ("knowledge_search_gram_stats", &["kind", "n", "gram"]),
    ("knowledge_lens_order", &["kind", "id"]),
    ("source_navigation_nodes", &["node_id"]),
    ("source_navigation_node_payload", &["id", "part"]),
    ("source_navigation_edges", &["edge_id"]),
    ("source_navigation_edge_payload", &["id", "part"]),
    ("source_navigation_rights", &["rights_id"]),
    ("source_navigation_rights_payload", &["id", "part"]),
    ("knowledge_compact_lens", &["kind", "id"]),
    (
        "knowledge_lens_memberships",
        &["kind", "field", "value", "id"],
    ),
];
const MAX_KEY_JSON_BYTES: usize = 6 * MAX_STATEMENT_BYTES + 4096;

struct KeyCount {
    len: usize,
    exceeded: bool,
}
impl Write for KeyCount {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(next) = self
            .len
            .checked_add(bytes.len())
            .filter(|len| *len <= MAX_KEY_JSON_BYTES)
        else {
            self.exceeded = true;
            return Err(io::Error::other("public D1 baseline key bytes"));
        };
        self.len = next;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) struct PublicRowIndex {
    path: PathBuf,
    db: Connection,
    rows: u64,
}
struct PendingBaseline<'a> {
    path: &'a Path,
    finished: bool,
}
impl Drop for PendingBaseline<'_> {
    fn drop(&mut self) {
        if !self.finished {
            let _ = fs::remove_file(self.path);
        }
    }
}
impl PublicRowIndex {
    pub(crate) fn create(
        path: &Path,
        capture: &PublicCapture,
        limits: PublicCaptureLimits,
    ) -> Result<Self> {
        if path.exists() || path.is_symlink() || limits.max_staging_bytes < 65536 {
            return Err(Error::Invalid("public D1 row index path/budget"));
        }
        let db = Connection::open(path)?;
        sqlite_budget::install_progress_until(
            &db,
            limits.sqlite(),
            capture.vm_counter(),
            capture.deadline(),
        );
        db.execute_batch("PRAGMA page_size=4096; PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA temp_store=FILE;")?;
        let pages = limits.max_staging_bytes / 4096;
        if pages < 16 || pages > i64::MAX as u64 {
            return Err(Error::Budget("public D1 row index pages"));
        }
        let admitted: i64 = db.query_row(&format!("PRAGMA max_page_count={pages}"), [], |row| {
            row.get(0)
        })?;
        let temp: i64 =
            db.query_row(&format!("PRAGMA temp.max_page_count={pages}"), [], |row| {
                row.get(0)
            })?;
        if admitted != pages as i64 || temp != pages as i64 {
            return Err(Error::Budget("public D1 row index page admission"));
        }
        db.execute_batch("CREATE TABLE rows(table_name TEXT NOT NULL,sequence INTEGER NOT NULL,row_key TEXT NOT NULL,digest TEXT NOT NULL,values_json TEXT NOT NULL,segments_json TEXT NOT NULL,PRIMARY KEY(table_name,row_key)) WITHOUT ROWID; CREATE INDEX rows_table_sequence ON rows(table_name,sequence); BEGIN IMMEDIATE;")?;
        Ok(Self {
            path: path.to_owned(),
            db,
            rows: 0,
        })
    }
    pub(crate) fn record(
        &mut self,
        table: &str,
        columns: &[&str],
        values: &[&str],
        digest: Digest256,
        segments: &[(u64, u64)],
        capture: &PublicCapture,
    ) -> Result<()> {
        let base = table
            .strip_suffix("_next")
            .ok_or(Error::Invalid("public D1 indexed table suffix"))?;
        let keys = TABLES
            .iter()
            .find(|(name, _)| *name == base)
            .map(|(_, keys)| *keys)
            .ok_or(Error::Invalid("public D1 unregistered baseline table"))?;
        if columns.len() != values.len() {
            return Err(Error::Invalid("public D1 indexed row shape"));
        }
        let mut selected = Vec::with_capacity(keys.len());
        for key in keys {
            let position = columns
                .iter()
                .position(|column| column == key)
                .ok_or(Error::Invalid("public D1 indexed key column"))?;
            selected.push(values[position]);
        }
        if segments.is_empty()
            || segments
                .iter()
                .any(|(_, length)| *length == 0 || *length > MAX_STATEMENT_BYTES as u64)
        {
            return Err(Error::Invalid("public D1 baseline SQL segments"));
        }
        let mut count = KeyCount {
            len: 0,
            exceeded: false,
        };
        serde_json::to_writer(&mut count, &selected).map_err(|error| {
            if count.exceeded {
                Error::Budget("public D1 baseline key bytes")
            } else {
                Error::Source(error.to_string())
            }
        })?;
        let segment_state = segments
            .len()
            .checked_mul(48)
            .ok_or(Error::Budget("public D1 baseline segments"))?;
        let materialization_work = (count.len as u64)
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(segment_state as u64))
            .ok_or(Error::Budget("public D1 baseline materialization work"))?;
        capture.charge_work(materialization_work)?;
        let values_json =
            serde_json::to_string(&selected).map_err(|e| Error::Source(e.to_string()))?;
        let segments_json =
            serde_json::to_string(segments).map_err(|e| Error::Source(e.to_string()))?;
        if values_json.len() != count.len || segments_json.len() > segment_state {
            return Err(Error::Invalid("public D1 baseline serialized size"));
        }
        let sequence =
            i64::try_from(self.rows).map_err(|_| Error::Budget("public D1 baseline row count"))?;
        self.db.execute("INSERT INTO rows(table_name,sequence,row_key,digest,values_json,segments_json) VALUES (?1,?2,?3,?4,?5,?6)",
            params![base,sequence,&values_json,digest.to_hex(),&values_json,&segments_json])?;
        self.rows = self
            .rows
            .checked_add(1)
            .ok_or(Error::Budget("public D1 baseline row count"))?;
        if self.rows % 4096 == 0 {
            self.db.execute_batch("COMMIT; BEGIN IMMEDIATE;")?;
        }
        Ok(())
    }
    pub(crate) fn emit_json(
        &mut self,
        target: &Path,
        capture: &PublicCapture,
        revision: &str,
        reader_top: &Value,
        max_bytes: u64,
    ) -> Result<u64> {
        if target.exists() || target.is_symlink() || max_bytes == 0 || revision.len() != 64 {
            return Err(Error::Invalid("public D1 baseline output"));
        }
        self.db.execute_batch("COMMIT")?;
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(target)?;
        let mut pending = PendingBaseline {
            path: target,
            finished: false,
        };
        let mut out = BufWriter::new(file);
        let mut bytes = 0u64;
        let mut write = |part: &str| -> Result<()> {
            bytes = bytes
                .checked_add(part.len() as u64)
                .filter(|value| *value <= max_bytes)
                .ok_or(Error::Budget("public D1 baseline output bytes"))?;
            capture.charge_work(part.len() as u64)?;
            out.write_all(part.as_bytes())?;
            Ok(())
        };
        let descriptor = serde_json::json!({
            "schema":"tos_d1_lens_auxiliary_publication_v1",
            "stores":{"knowledge_compact_lens":"tos_compact_lens_carrier_v1",
                "knowledge_lens_memberships":"tos_lens_membership_index_v1"},
            "reader_top":reader_top,
        });
        let descriptor =
            serde_json::to_string(&descriptor).map_err(|e| Error::Source(e.to_string()))?;
        if descriptor.len() > 131072 {
            return Err(Error::Budget("public D1 baseline publication bytes"));
        }
        write("{\"schema\":\"tos_cloudflare_edge_read_model_v9\",\"revision\":")?;
        write(&serde_json::to_string(revision).map_err(|e| Error::Source(e.to_string()))?)?;
        write(",\"auxiliary_publication\":")?;
        write(&descriptor)?;
        write(",\"rows\":{")?;
        for (index, (table, _)) in TABLES.iter().enumerate() {
            if index > 0 {
                write(",")?;
            }
            write(&serde_json::to_string(table).map_err(|e| Error::Source(e.to_string()))?)?;
            write(":{")?;
            let mut statement=self.db.prepare("SELECT CASE WHEN length(row_key)<=?2 THEN row_key ELSE NULL END,digest,CASE WHEN length(values_json)<=?2 THEN values_json ELSE NULL END FROM rows WHERE table_name=?1 ORDER BY sequence")?;
            let mut rows = statement.query(params![table, MAX_KEY_JSON_BYTES as i64])?;
            let mut first = true;
            while let Some(row) = rows.next()? {
                let key: Option<String> = row.get(0)?;
                let digest: String = row.get(1)?;
                let values: Option<String> = row.get(2)?;
                let (Some(key), Some(values)) = (key, values) else {
                    return Err(Error::Budget("public D1 baseline row bytes"));
                };
                if !first {
                    write(",")?;
                }
                first = false;
                write(&serde_json::to_string(&key).map_err(|e| Error::Source(e.to_string()))?)?;
                write(":{\"digest\":")?;
                write(&serde_json::to_string(&digest).map_err(|e| Error::Source(e.to_string()))?)?;
                write(",\"values\":")?;
                write(&values)?;
                write("}")?;
            }
            write("}")?;
        }
        write("}}\n")?;
        out.flush()?;
        out.get_ref().sync_all()?;
        pending.finished = true;
        Ok(bytes)
    }
    pub(crate) fn connection(&self) -> &Connection {
        &self.db
    }
}
impl Drop for PublicRowIndex {
    fn drop(&mut self) {
        let _ = self.db.execute_batch("ROLLBACK");
        for suffix in ["-journal", "-wal", "-shm", ""] {
            let mut path = self.path.as_os_str().to_os_string();
            path.push(suffix);
            let _ = fs::remove_file(PathBuf::from(path));
        }
    }
}
