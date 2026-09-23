//! Trusted-local opening of one owner-selected immutable navigation model.
//! The caller supplies independent owner expectation; selected.json and a
//! digest named file cannot attest their own source or rights authority.

use crate::{
    Error, MODEL_ABI, Result, SELECTION_PROFILE, SourceBinding, publication::selected_packet,
    safe_open, stream_digest,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde_json::Value;
use std::{
    fs::File,
    io::Seek,
    os::fd::AsRawFd,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tos_foundation::{Digest256, Digest256Hasher, JsonMode};

/// Owner/host capability retained across every warm reader. Its provider
/// controls the selected artifact namespace and guarantees that no writer
/// alias can mutate this inode while the capability is held. Metadata/stat
/// checks alone cannot establish this condition.
pub trait ImmutableModelCustody: Send + Sync {
    fn verify_held(&self, pinned: &File, sha256: &str, size_bytes: u64) -> Result<()>;
}

/// This expectation comes from the source/selection owner, independently of
/// the local pointer bytes. It carries no grant to disclose current content.
pub struct SelectedExpectation<'a> {
    pub source: &'a SourceBinding,
    pub model_sha256: &'a str,
    pub model_size_bytes: u64,
    pub owner_receipt_id: &'a str,
    pub custody: Arc<dyn ImmutableModelCustody>,
    /// Cold-open SHA work limit. This is an admission job budget, not a
    /// per-query allowance; a model above it remains pending/unavailable.
    pub max_cold_open_bytes: u64,
    /// SQLite instructions allowed while opening schema and bound metadata.
    pub max_cold_open_vm_steps: u64,
}

#[derive(Clone, Debug)]
pub struct VerifiedSelection {
    pub model_sha256: String,
    pub model_size_bytes: u64,
    pub owner_receipt_id: String,
    pub source_cut: String,
    pub through_commit_seq: u64,
    pub membership_root: String,
    pub projection_root_sha256: String,
    pub index_generation: String,
    pub route_map_version: String,
    pub reader_abi: String,
    pub model_abi: String,
    pub selection_profile: String,
    pub authority_boundary: String,
    pub complete: bool,
}

/// The pinned descriptor lives for as long as the read-only SQLite connection.
/// Reuse the selected model across requests; digest verification is linear in
/// model bytes and must not be repeated per query.
pub struct VerifiedSelectedModel {
    connection: Connection,
    pinned: File,
    custody: Arc<dyn ImmutableModelCustody>,
    selection: VerifiedSelection,
    cold_open_vm_steps: u64,
    open_vm_steps: u64,
}
impl VerifiedSelectedModel {
    pub fn selection(&self) -> &VerifiedSelection {
        &self.selection
    }
    pub fn connection(&self) -> &Connection {
        &self.connection
    }
    pub fn connection_mut(&mut self) -> &mut Connection {
        &mut self.connection
    }
    /// Actual SQLite VM instructions charged while opening and checking this
    /// reader. This excludes the independently capped full-file SHA I/O.
    pub fn open_vm_steps(&self) -> u64 {
        self.open_vm_steps
    }
    /// Cheap local FD continuity check. The owner must separately renew the
    /// sealed source pin and current disclosure/rights fence; this does not
    /// rehash a large model on every query.
    pub fn check_pin(&self) -> Result<()> {
        let meta = self.pinned.metadata()?;
        if !meta.file_type().is_file() || meta.len() != self.selection.model_size_bytes {
            return Err(Error::Invalid("selected model pinned file changed"));
        }
        self.custody.verify_held(
            &self.pinned,
            &self.selection.model_sha256,
            self.selection.model_size_bytes,
        )?;
        Ok(())
    }
    /// New warm reader against the same admitted inode. Its SQLite startup
    /// has the same explicit VM budget, while the query adapter installs a
    /// separate per-operation progress cap before each seek.
    pub fn fork_reader(&self) -> Result<Self> {
        self.fork_reader_with_vm_budget(self.cold_open_vm_steps)
    }
    /// Admit a warm reader only within the caller's predeclared SQLite
    /// startup VM allowance. The cap cannot exceed this selection's cold
    /// admission cap and applies before any SQLite statement executes.
    pub fn fork_reader_with_vm_budget(&self, max_vm_steps: u64) -> Result<Self> {
        if max_vm_steps == 0 || max_vm_steps > self.cold_open_vm_steps {
            return Err(Error::Budget("warm-reader SQLite VM steps"));
        }
        self.check_pin()?;
        let pinned = self.pinned.try_clone()?;
        let (connection, vm_counter) = open_sqlite(&pinned, max_vm_steps)?;
        let reader = Self {
            connection,
            pinned,
            custody: Arc::clone(&self.custody),
            selection: self.selection.clone(),
            cold_open_vm_steps: max_vm_steps,
            open_vm_steps: vm_counter.load(Ordering::Relaxed),
        };
        reader.check_pin()?;
        Ok(reader)
    }
}

fn value<'a>(packet: &'a Value, key: &str) -> Result<&'a str> {
    packet
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or(Error::Invalid("selected pointer field absent"))
}
fn metadata(db: &Connection, key: &str, max_bytes: usize) -> Result<String> {
    db.query_row(
        "SELECT value FROM metadata WHERE key=?1 AND length(CAST(value AS BLOB))<=?2",
        params![key, max_bytes as u64],
        |row| row.get(0),
    )
    .optional()?
    .ok_or(Error::Invalid(
        "selected model metadata absent or oversized",
    ))
}

fn open_sqlite(pinned: &File, max_vm_steps: u64) -> Result<(Connection, Arc<AtomicU64>)> {
    if max_vm_steps == 0 {
        return Err(Error::Budget("cold-open SQLite VM steps"));
    }
    let uri = format!(
        "file:/proc/self/fd/{}?mode=ro&immutable=1",
        pinned.as_raw_fd()
    );
    let db = Connection::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let used = Arc::new(AtomicU64::new(0));
    let callback_used = Arc::clone(&used);
    db.progress_handler(
        1,
        Some(move || {
            callback_used
                .fetch_add(1, Ordering::Relaxed)
                .saturating_add(1)
                >= max_vm_steps
        }),
    );
    db.execute_batch("PRAGMA query_only=ON; PRAGMA trusted_schema=OFF;")?;
    Ok((db, used))
}

fn checked_root_item(hash: &mut Digest256Hasher, id: &str, sha256: &str) -> Result<()> {
    let digest =
        Digest256::from_hex(sha256).map_err(|_| Error::Invalid("selected row digest malformed"))?;
    hash.update(&(id.len() as u64).to_be_bytes());
    hash.update(id.as_bytes());
    hash.update(digest.as_bytes());
    Ok(())
}

fn verify_root(
    db: &Connection,
    table: &str,
    id_col: &str,
    root_key: &str,
    count_key: &str,
) -> Result<()> {
    let sql = match (table, id_col) {
        ("nodes", "node_id") => "SELECT node_id,carrier_sha256 FROM nodes ORDER BY node_id",
        ("edges", "edge_id") => "SELECT edge_id,carrier_sha256 FROM edges ORDER BY edge_id",
        ("rights", "rights_id") => "SELECT rights_id,carrier_sha256 FROM rights ORDER BY rights_id",
        _ => return Err(Error::Invalid("selected root table")),
    };
    let mut statement = db.prepare(sql)?;
    let mut rows = statement.query([])?;
    let mut hash = Digest256Hasher::new();
    let mut count = 0u64;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let digest: String = row.get(1)?;
        checked_root_item(&mut hash, &id, &digest)?;
        count = count
            .checked_add(1)
            .ok_or(Error::Budget("selected row count"))?;
    }
    if metadata(db, root_key, 64)? != hash.finalize().to_hex()
        || metadata(db, count_key, 32)? != count.to_string()
    {
        return Err(Error::Invalid("selected row root/count mismatch"));
    }
    Ok(())
}

/// One cold admission pass. Reused warm readers inherit this owner-selected
/// immutable inode and never repeat whole-file work per request.
fn verify_cold_closure(db: &Connection) -> Result<()> {
    if db.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))? != "ok" {
        return Err(Error::Invalid("selected SQLite integrity"));
    }
    for (table, id, root, count) in [
        ("nodes", "node_id", "node_root_sha256", "node_count"),
        ("edges", "edge_id", "edge_root_sha256", "edge_count"),
        ("rights", "rights_id", "rights_root_sha256", "rights_count"),
    ] {
        verify_root(db, table, id, root, count)?;
    }
    let visible_nodes: u64 =
        db.query_row("SELECT count(*) FROM nodes WHERE visible=1", [], |row| {
            row.get(0)
        })?;
    let visible_edges: u64 =
        db.query_row("SELECT count(*) FROM edges WHERE visible=1", [], |row| {
            row.get(0)
        })?;
    if metadata(db, "visible_node_count", 32)? != visible_nodes.to_string()
        || metadata(db, "visible_edge_count", 32)? != visible_edges.to_string()
    {
        return Err(Error::Invalid("selected visible counts mismatch"));
    }
    let broken_node: Option<String> = db
        .query_row(
            "SELECT node_id FROM nodes n WHERE visible=1
         AND NOT EXISTS (SELECT 1 FROM adjacency a WHERE a.from_id=n.node_id) LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let broken_edge: Option<String> = db
        .query_row(
            "SELECT edge_id FROM edges e WHERE visible=1 AND
         (NOT EXISTS (SELECT 1 FROM nodes n WHERE n.node_id=e.from_id AND n.visible=1)
          OR NOT EXISTS (SELECT 1 FROM nodes n WHERE n.node_id=e.to_id AND n.visible=1))
         LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let broken_rights: Option<String> = db
        .query_row(
            "SELECT rights_id FROM rights_scopes s WHERE
         NOT EXISTS (SELECT 1 FROM rights r WHERE r.rights_id=s.rights_id) LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if broken_node.is_some() || broken_edge.is_some() || broken_rights.is_some() {
        return Err(Error::Invalid("selected model relational closure"));
    }
    let mut adj_stmt =
        db.prepare("SELECT from_id,edge_count,edges_sha256 FROM adjacency ORDER BY from_id")?;
    let mut adj = adj_stmt.query([])?;
    let mut edge_stmt = db.prepare(
        "SELECT from_id,edge_id,carrier_sha256 FROM edges WHERE visible=1 ORDER BY from_id,edge_id",
    )?;
    let mut edges = edge_stmt.query([])?;
    let edge_tuple = |row: &rusqlite::Row<'_>| -> rusqlite::Result<(String, String, String)> {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    };
    let mut next_edge = edges.next()?.map(edge_tuple).transpose()?;
    let mut certified_edges = 0u64;
    while let Some(row) = adj.next()? {
        let from: String = row.get(0)?;
        let expected_count: i64 = row.get(1)?;
        let expected_root: String = row.get(2)?;
        if expected_count < 0 {
            return Err(Error::Invalid("selected adjacency negative count"));
        }
        let visible: Option<i64> = db
            .query_row(
                "SELECT visible FROM nodes WHERE node_id=?1",
                params![&from],
                |r| r.get(0),
            )
            .optional()?;
        if visible != Some(1) {
            return Err(Error::Invalid("selected adjacency source not visible"));
        }
        let mut hash = Digest256Hasher::new();
        let mut count = 0u64;
        while let Some((edge_from, id, digest)) = next_edge.as_ref() {
            if edge_from != &from {
                break;
            }
            checked_root_item(&mut hash, id, digest)?;
            count = count
                .checked_add(1)
                .ok_or(Error::Budget("selected adjacency count"))?;
            next_edge = edges.next()?.map(edge_tuple).transpose()?;
        }
        if count != expected_count as u64 || hash.finalize().to_hex() != expected_root {
            return Err(Error::Invalid("selected adjacency root/count mismatch"));
        }
        certified_edges = certified_edges
            .checked_add(count)
            .ok_or(Error::Budget("selected certified edges"))?;
    }
    if next_edge.is_some() || certified_edges != visible_edges {
        return Err(Error::Invalid("selected adjacency coverage mismatch"));
    }
    Ok(())
}

pub fn open_selected_model(
    publication_dir: &Path,
    expected: &SelectedExpectation<'_>,
) -> Result<VerifiedSelectedModel> {
    if expected.owner_receipt_id.is_empty()
        || expected.model_size_bytes == 0
        || !expected.source.complete
    {
        return Err(Error::Invalid("owner selection expectation incomplete"));
    }
    if expected.model_size_bytes > expected.max_cold_open_bytes {
        return Err(Error::Budget("cold-open model bytes"));
    }
    if expected.max_cold_open_vm_steps == 0 {
        return Err(Error::Budget("cold-open SQLite VM steps"));
    }
    let expected_digest = Digest256::from_hex(expected.model_sha256)
        .map_err(|_| Error::Invalid("owner model digest invalid"))?;
    Digest256::from_hex(&expected.source.membership_root)
        .map_err(|_| Error::Invalid("owner membership root digest invalid"))?;
    Digest256::from_hex(&expected.source.projection_root_sha256)
        .map_err(|_| Error::Invalid("owner projection root digest invalid"))?;
    if !publication_dir.is_absolute() || !publication_dir.is_dir() || publication_dir.is_symlink() {
        return Err(Error::Invalid("publication directory"));
    }
    let pointer = selected_packet(&publication_dir.join("selected.json"))?
        .ok_or(Error::Invalid("selected pointer absent"))?;
    if value(&pointer, "model_sha256")? != expected.model_sha256
        || pointer.get("model_size_bytes").and_then(Value::as_u64)
            != Some(expected.model_size_bytes)
        || value(&pointer, "owner_authority_receipt_id")? != expected.owner_receipt_id
        || value(&pointer, "source_cut")? != expected.source.source_cut
        || pointer.get("through_commit_seq").and_then(Value::as_u64)
            != Some(expected.source.through_commit_seq)
        || value(&pointer, "projection_root_sha256")? != expected.source.projection_root_sha256
        || value(&pointer, "index_generation")? != expected.source.index_generation
        || value(&pointer, "route_map_version")? != expected.source.route_map_version
    {
        return Err(Error::Invalid(
            "selected pointer differs from owner expectation",
        ));
    }
    let path: PathBuf = publication_dir.join(format!("{}.sqlite3", expected.model_sha256));
    let mut pinned = safe_open::open_regular(&path, expected.model_size_bytes)?;
    expected
        .custody
        .verify_held(&pinned, expected.model_sha256, expected.model_size_bytes)?;
    let (digest, size) = stream_digest(&mut pinned)?;
    if digest != expected_digest.to_hex() || size != expected.model_size_bytes {
        return Err(Error::Invalid("selected model digest/size mismatch"));
    }
    pinned.rewind()?;
    let (db, vm_counter) = open_sqlite(&pinned, expected.max_cold_open_vm_steps)?;
    for (key, expected_value) in [
        ("model_abi", MODEL_ABI),
        ("selection_profile", SELECTION_PROFILE),
        ("json_profile", JsonMode::PublishedStrict.as_str()),
        ("owner_profile", expected.source.owner_profile.as_str()),
        ("source_cut", expected.source.source_cut.as_str()),
        ("membership_root", expected.source.membership_root.as_str()),
        (
            "index_generation",
            expected.source.index_generation.as_str(),
        ),
        (
            "route_map_version",
            expected.source.route_map_version.as_str(),
        ),
        ("reader_abi", expected.source.reader_abi.as_str()),
        (
            "projection_root_sha256",
            expected.source.projection_root_sha256.as_str(),
        ),
        ("complete", "true"),
    ] {
        if metadata(&db, key, 4096)? != expected_value {
            return Err(Error::Invalid("selected model metadata binding mismatch"));
        }
    }
    if metadata(&db, "through_commit_seq", 32)? != expected.source.through_commit_seq.to_string() {
        return Err(Error::Invalid("selected model sequence mismatch"));
    }
    let authority_boundary = metadata(&db, "authority_boundary", 256 * 1024)?;
    if authority_boundary.is_empty() {
        return Err(Error::Invalid("selected model source authority absent"));
    }
    if metadata(&db, "derived_authority", 128)? != "candidate_only_no_admission" {
        return Err(Error::Invalid("selected model derived authority marker"));
    }
    verify_cold_closure(&db)?;
    expected
        .custody
        .verify_held(&pinned, expected.model_sha256, expected.model_size_bytes)?;
    Ok(VerifiedSelectedModel {
        connection: db,
        pinned,
        custody: Arc::clone(&expected.custody),
        cold_open_vm_steps: expected.max_cold_open_vm_steps,
        open_vm_steps: vm_counter.load(Ordering::Relaxed),
        selection: VerifiedSelection {
            model_sha256: digest,
            model_size_bytes: size,
            owner_receipt_id: expected.owner_receipt_id.to_owned(),
            source_cut: expected.source.source_cut.clone(),
            through_commit_seq: expected.source.through_commit_seq,
            membership_root: expected.source.membership_root.clone(),
            projection_root_sha256: expected.source.projection_root_sha256.clone(),
            index_generation: expected.source.index_generation.clone(),
            route_map_version: expected.source.route_map_version.clone(),
            reader_abi: expected.source.reader_abi.clone(),
            model_abi: MODEL_ABI.to_owned(),
            selection_profile: SELECTION_PROFILE.to_owned(),
            authority_boundary,
            complete: true,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cold_admission_refuses_missing_zero_adjacency_certificate() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL);
             CREATE TABLE nodes(node_id TEXT PRIMARY KEY,visible INTEGER,carrier_sha256 TEXT);
             CREATE TABLE edges(edge_id TEXT PRIMARY KEY,from_id TEXT,to_id TEXT,visible INTEGER,carrier_sha256 TEXT);
             CREATE TABLE rights(rights_id TEXT PRIMARY KEY,carrier_sha256 TEXT);
             CREATE TABLE rights_scopes(rights_id TEXT,scope_id TEXT);
             CREATE TABLE adjacency(from_id TEXT PRIMARY KEY,edge_count INTEGER,edges_sha256 TEXT);",
        ).unwrap();
        let empty = Digest256::of_bytes(b"").to_hex();
        for key in ["node_root_sha256", "edge_root_sha256", "rights_root_sha256"] {
            db.execute("INSERT INTO metadata VALUES (?1,?2)", params![key, &empty])
                .unwrap();
        }
        for key in [
            "node_count",
            "edge_count",
            "rights_count",
            "visible_node_count",
            "visible_edge_count",
        ] {
            db.execute("INSERT INTO metadata VALUES (?1,'0')", params![key])
                .unwrap();
        }
        verify_cold_closure(&db).unwrap();
        let row_sha = "1".repeat(64);
        db.execute("INSERT INTO nodes VALUES ('n',1,?1)", params![&row_sha])
            .unwrap();
        let mut root = Digest256Hasher::new();
        checked_root_item(&mut root, "n", &row_sha).unwrap();
        db.execute(
            "UPDATE metadata SET value=?1 WHERE key='node_root_sha256'",
            params![root.finalize().to_hex()],
        )
        .unwrap();
        db.execute(
            "UPDATE metadata SET value='1' WHERE key IN ('node_count','visible_node_count')",
            [],
        )
        .unwrap();
        assert!(verify_cold_closure(&db).is_err());
        db.execute("INSERT INTO adjacency VALUES ('n',0,?1)", params![&empty])
            .unwrap();
        verify_cold_closure(&db).unwrap();
    }
}
