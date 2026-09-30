//! Explicit operator-selected backup/restore; credentials never enter argv or output.
use crate::backup_recovery::{BackupSelection, PgTool, RestoreSelection};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use tos_segment_store::SegmentLimits;

pub const HELP: &str = "usage: tos-native-owner-command backup|restore OPTIONS
Connection: private environment TOS_BACKUP_PG_URL (PostgreSQL 16).
Local NoTls only: one numeric loopback host, explicit user/database, one port.
Required TLS, hostaddr, sockets/multiple hosts and session selection are refused.
Required options:
  --domain DOMAIN --store-root ABS --backup-root ABS
  --pg-tool ABS --pg-tool-sha256 HEX64
  --max-segment-bytes N --max-frame-bytes N --max-frames N
  --max-journal-bytes N --max-seconds N
Backup: --confirm-quiescent-owner yes
Restore: --confirm-fresh-target-owner yes --receipt-sha256 HEX64
Confirmations declare owner preconditions; they do not stop concurrent writers.
Restore requires a fresh database (existing public tables are refused), a different
database name and an empty
independent store; it does not initialize schema or activate restored data.
Roots must be private owned mode 0700; select exact PG16 tool or pinned wrapper.
Transport cap: store 64MiB/dump 64MiB, 256 files/512 entries; soft+hard FSIZE<=64MiB.
Tools use stdout/stdin, each <=60s within remaining max-seconds.
Use an admitted hard whole supervisor and owned container cleanup if applicable.
A failed operation may leave partial output; do not consume it as a restore.
";

pub fn run(args: &[String]) -> Result<serde_json::Value, &'static str> {
    let started = Instant::now();
    let operation = args
        .first()
        .map(String::as_str)
        .ok_or("missing operation")?;
    if !matches!(operation, "backup" | "restore") {
        return Err("unknown operation");
    }
    let common = [
        "--domain",
        "--store-root",
        "--backup-root",
        "--pg-tool",
        "--pg-tool-sha256",
        "--max-segment-bytes",
        "--max-frame-bytes",
        "--max-frames",
        "--max-journal-bytes",
        "--max-seconds",
    ];
    let mut values = BTreeMap::new();
    if (args.len() - 1) % 2 != 0 {
        return Err("options require values");
    }
    for pair in args[1..].chunks_exact(2) {
        let key = pair[0].as_str();
        let extra = if operation == "backup" {
            key == "--confirm-quiescent-owner"
        } else {
            matches!(key, "--confirm-fresh-target-owner" | "--receipt-sha256")
        };
        if (!common.contains(&key) && !extra) || values.insert(key, pair[1].as_str()).is_some() {
            return Err("unknown or duplicate option");
        }
    }
    let get = |key: &str| values.get(key).copied().ok_or("missing required option");
    let number = |key: &str| -> Result<u64, &'static str> {
        let n = get(key)?.parse::<u64>().map_err(|_| "invalid limit")?;
        if n == 0 || n == u64::MAX {
            return Err("invalid limit");
        }
        Ok(n)
    };
    let limits = SegmentLimits {
        max_segment_bytes: number("--max-segment-bytes")?,
        max_frame_bytes: number("--max-frame-bytes")?,
        max_frames: u32::try_from(number("--max-frames")?).map_err(|_| "frame count overflow")?,
        max_journal_bytes: usize::try_from(number("--max-journal-bytes")?)
            .map_err(|_| "journal limit overflow")?,
    }
    .validate()
    .map_err(|_| "invalid segment limits")?;
    let deadline = started
        .checked_add(Duration::from_secs(number("--max-seconds")?))
        .ok_or("deadline overflow")?;
    let store = Path::new(get("--store-root")?);
    let backup = Path::new(get("--backup-root")?);
    let tool_path = Path::new(get("--pg-tool")?);
    if !store.is_absolute() || !backup.is_absolute() || !tool_path.is_absolute() {
        return Err("paths must be absolute");
    }
    let sha = get("--pg-tool-sha256")?;
    let valid_sha = |s: &str| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    if !valid_sha(sha) {
        return Err("invalid tool digest");
    }
    let domain = get("--domain")?;
    if domain.is_empty() {
        return Err("empty domain");
    }
    let url = std::env::var("TOS_BACKUP_PG_URL").map_err(|_| "TOS_BACKUP_PG_URL required")?;
    if url.is_empty() {
        return Err("TOS_BACKUP_PG_URL required");
    }
    let cancelled = AtomicBool::new(false);
    let tool = PgTool {
        path: tool_path,
        sha256: sha,
    };
    let result = if operation == "backup" {
        if get("--confirm-quiescent-owner")? != "yes" {
            return Err("explicit quiescent owner confirmation required");
        }
        crate::backup_recovery::backup_quiescent(
            &BackupSelection {
                pg_url: &url,
                domain,
                store_root: store,
                backup_root: backup,
                tool,
                store_limits: limits,
                quiescent_owner_confirmed: true,
            },
            deadline,
            &cancelled,
        )
    } else {
        if get("--confirm-fresh-target-owner")? != "yes" {
            return Err("explicit fresh target owner confirmation required");
        }
        let receipt = get("--receipt-sha256")?;
        if !valid_sha(receipt) {
            return Err("invalid receipt digest");
        }
        crate::backup_recovery::restore_into_fresh(
            &RestoreSelection {
                pg_url: &url,
                domain,
                backup_root: backup,
                receipt_sha256: receipt,
                store_root: store,
                tool,
                store_limits: limits,
                fresh_target_owner_confirmed: true,
            },
            deadline,
            &cancelled,
        )
    };
    result.map_err(|error| match error.kind() {
        std::io::ErrorKind::TimedOut => "backup/restore deadline exceeded; preserve partial output",
        std::io::ErrorKind::Interrupted => "backup/restore cancelled; preserve partial output",
        std::io::ErrorKind::InvalidInput => "backup/restore selection or limits refused",
        std::io::ErrorKind::InvalidData => "backup/restore integrity or format refused",
        std::io::ErrorKind::PermissionDenied => "backup/restore owner or access refused",
        std::io::ErrorKind::AlreadyExists => "backup/restore requires a fresh destination",
        std::io::ErrorKind::NotFound => "backup/restore selected input or tool is absent",
        _ => "backup/restore IO or database refused; preserve partial output",
    })
}
