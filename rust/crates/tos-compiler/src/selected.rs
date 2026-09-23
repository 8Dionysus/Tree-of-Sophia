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
use tos_foundation::{Digest256, JsonMode};

/// This expectation comes from the source/selection owner, independently of
/// the local pointer bytes. It carries no grant to disclose current content.
pub struct SelectedExpectation<'a> {
    pub source: &'a SourceBinding,
    pub model_sha256: &'a str,
    pub model_size_bytes: u64,
    pub owner_receipt_id: &'a str,
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
        Ok(())
    }
    /// New warm reader against the same admitted inode. Its SQLite startup
    /// has the same explicit VM budget, while the query adapter installs a
    /// separate per-operation progress cap before each seek.
    pub fn fork_reader(&self) -> Result<Self> {
        self.check_pin()?;
        let pinned = self.pinned.try_clone()?;
        let (connection, vm_counter) = open_sqlite(&pinned, self.cold_open_vm_steps)?;
        Ok(Self {
            connection,
            pinned,
            selection: self.selection.clone(),
            cold_open_vm_steps: self.cold_open_vm_steps,
            open_vm_steps: vm_counter.load(Ordering::Relaxed),
        })
    }
    pub fn into_parts(self) -> (Connection, File, VerifiedSelection) {
        (self.connection, self.pinned, self.selection)
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
    Ok(VerifiedSelectedModel {
        connection: db,
        pinned,
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
