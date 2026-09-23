//! Native local selector. A source-owner implementation supplies the authority
//! check; this module only enforces exact candidate and pointer mechanics.

use crate::{
    CandidateReceipt, Error, MODEL_ABI, Result, SELECTION_PROFILE, SourceBinding, file_digest,
    safe_open, stream_digest,
};
use fs2::FileExt;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use tos_foundation::{JsonLimits, JsonMode, parse_json};

const POINTER_SCHEMA: &str = "tos_compiler_selected_navigation_v1";
const POINTER_CAP: usize = 8192;

/// A source-owner fence remains live through the durable pointer decision.
/// External freshness observed once before this point is insufficient.
pub trait SelectionFence {
    fn receipt_id(&self) -> &str;
    fn recheck_held(&self) -> Result<()>;
}

/// The source owner verifies the exact cut, current policy/rights obligations
/// and consumer ABI, then holds an ordered fence until this call returns.
/// This crate deliberately has no permissive production implementation.
pub trait PublicationAuthority {
    type Fence: SelectionFence;
    fn acquire_selection_fence(
        &mut self,
        source: &SourceBinding,
        candidate: &CandidateReceipt,
    ) -> Result<Self::Fence>;
}

#[derive(Clone, Debug)]
pub struct PublishedReceipt {
    pub selected_model_sha256: String,
    pub selected_path: std::path::PathBuf,
    pub pointer_path: std::path::PathBuf,
    pub authority_receipt_id: String,
}

pub(crate) fn selected_packet(path: &Path) -> Result<Option<Value>> {
    let mut file = match safe_open::open_regular(path, POINTER_CAP as u64) {
        Ok(file) => file,
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut raw = Vec::with_capacity(file.metadata()?.len() as usize);
    Read::by_ref(&mut file)
        .take(POINTER_CAP as u64 + 1)
        .read_to_end(&mut raw)?;
    if raw.len() > POINTER_CAP {
        return Err(Error::Budget("selected pointer bytes"));
    }
    parse_json(
        &raw,
        JsonMode::PublishedStrict,
        JsonLimits::new(POINTER_CAP, 32, 10000, 256).map_err(|e| Error::Source(e.to_string()))?,
    )
    .map_err(|e| Error::Source(e.to_string()))?;
    let pointer: Value = serde_json::from_slice(&raw).map_err(|e| Error::Source(e.to_string()))?;
    if pointer.get("schema").and_then(Value::as_str) != Some(POINTER_SCHEMA)
        || pointer.get("model_abi").and_then(Value::as_str) != Some(MODEL_ABI)
        || pointer.get("selection_profile").and_then(Value::as_str) != Some(SELECTION_PROFILE)
    {
        return Err(Error::Invalid("selected pointer profile mismatch"));
    }
    let digest = pointer
        .get("model_sha256")
        .and_then(Value::as_str)
        .ok_or(Error::Invalid("selected pointer digest absent"))?;
    tos_foundation::Digest256::from_hex(digest)
        .map_err(|_| Error::Invalid("selected pointer digest invalid"))?;
    Ok(Some(pointer))
}

fn selected_digest(path: &Path) -> Result<Option<String>> {
    Ok(selected_packet(path)?.map(|pointer| {
        pointer["model_sha256"]
            .as_str()
            .expect("validated digest")
            .to_owned()
    }))
}

/// Atomically select an immutable local candidate under a CAS-style pointer
/// check. The caller's authority hook is invoked under the selection lock.
/// A newer unrelated source cut does not invalidate a still-pinned exact cut.
pub fn publish_candidate<A: PublicationAuthority>(
    candidate: &Path,
    source: &SourceBinding,
    receipt: &CandidateReceipt,
    publication_dir: &Path,
    expected_previous_sha256: Option<&str>,
    authority: &mut A,
) -> Result<PublishedReceipt> {
    if !publication_dir.is_dir()
        || publication_dir.is_symlink()
        || candidate.parent() != Some(publication_dir)
    {
        return Err(Error::Invalid("publication directory/candidate scope"));
    }
    let mut pinned = safe_open::open_regular(candidate, receipt.sqlite_size_bytes)?;
    if source.source_cut != receipt.source_cut
        || source.projection_root_sha256 != receipt.projection_root_sha256
    {
        return Err(Error::Invalid("candidate source binding mismatch"));
    }
    let lock_path = publication_dir.join(".selection.lock");
    let lock = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(&lock_path)?;
    if !lock.metadata()?.file_type().is_file() {
        return Err(Error::Invalid("selection lock is not a regular file"));
    }
    lock.try_lock_exclusive()
        .map_err(|_| Error::Invalid("selected pointer is busy"))?;
    let outcome = (|| {
        let pointer = publication_dir.join("selected.json");
        let previous = selected_digest(&pointer)?;
        if previous.as_deref() != expected_previous_sha256 {
            return Err(Error::Invalid("selected pointer changed"));
        }
        let (digest, size) = stream_digest(&mut pinned)?;
        if digest != receipt.sqlite_sha256 || size != receipt.sqlite_size_bytes {
            return Err(Error::Invalid("candidate bytes changed"));
        }
        let fence = authority.acquire_selection_fence(source, receipt)?;
        let owner_receipt = fence.receipt_id().to_owned();
        if owner_receipt.is_empty() {
            return Err(Error::Invalid("empty owner authority receipt"));
        }
        let selected = publication_dir.join(format!("{digest}.sqlite3"));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&selected)
        {
            Ok(mut installed) => {
                let install_result: Result<()> = (|| {
                    pinned.seek(SeekFrom::Start(0))?;
                    let copied = std::io::copy(
                        &mut Read::by_ref(&mut pinned).take(size.saturating_add(1)),
                        &mut installed,
                    )?;
                    installed.sync_all()?;
                    drop(installed);
                    let (installed_digest, installed_size) = file_digest(&selected)?;
                    if copied != size || installed_digest != digest || installed_size != size {
                        return Err(Error::Invalid("installed candidate bytes changed"));
                    }
                    Ok(())
                })();
                if install_result.is_err() {
                    let _ = fs::remove_file(&selected);
                }
                install_result?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let (old_digest, old_size) = file_digest(&selected)?;
                if old_digest != digest || old_size != size {
                    return Err(Error::Invalid("existing selected artifact mismatch"));
                }
            }
            Err(error) => return Err(Error::Io(error)),
        }
        fs::File::open(&selected)?.sync_all()?;
        fs::File::open(publication_dir)?.sync_all()?;
        let packet = json!({
            "schema":POINTER_SCHEMA,"model_abi":MODEL_ABI,
            "selection_profile":SELECTION_PROFILE,"model_sha256":digest.clone(),
            "model_size_bytes":size,"source_cut":source.source_cut,
            "through_commit_seq":source.through_commit_seq,
            "projection_root_sha256":source.projection_root_sha256,
            "index_generation":source.index_generation,
            "route_map_version":source.route_map_version,
            "owner_authority_receipt_id":owner_receipt.clone(),
        });
        let raw = serde_json::to_vec(&packet).map_err(|e| Error::Source(e.to_string()))?;
        if raw.len() + 1 > POINTER_CAP {
            return Err(Error::Budget("selected pointer bytes"));
        }
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Error::Invalid("clock unavailable"))?
            .as_nanos();
        let temporary =
            publication_dir.join(format!(".selected.{}.{tick}.pending", std::process::id()));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        let write_result: Result<()> = (|| {
            file.write_all(&raw)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            fence.recheck_held()?;
            fs::rename(&temporary, &pointer)?;
            fs::File::open(publication_dir)?.sync_all()?;
            Ok(())
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        write_result?;
        // The private candidate pathname may have been replaced by external
        // owner code. Cleanup belongs to the caller's private-stage janitor;
        // never unlink a path we no longer own after selecting this inode.
        Ok(PublishedReceipt {
            selected_model_sha256: digest,
            selected_path: selected,
            pointer_path: pointer,
            authority_receipt_id: owner_receipt,
        })
    })();
    FileExt::unlock(&lock)?;
    outcome
}
