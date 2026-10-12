//! Explicit operator-selected backup/restore; credentials never enter argv or output.
use crate::backup_recovery::{BackupProfile, BackupSelection, PgTool, RestoreSelection};
use std::os::unix::fs::MetadataExt;
use std::{
    collections::BTreeMap,
    io::Read,
    path::Path,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};
use tos_segment_store::SegmentLimits;

const MAX_PROFILE_BYTES: u64 = 4096;

pub const HELP: &str = "usage: tos-native-owner-command backup|restore OPTIONS
Connection: private environment TOS_BACKUP_PG_URL (PostgreSQL 16).
Local NoTls only: one numeric loopback host, explicit user/database, one port.
Required TLS, hostaddr, sockets/multiple hosts and session selection are refused.
Required options:
  --domain DOMAIN --store-root ABS --backup-root ABS
  --pg-tool ABS --pg-tool-sha256 HEX64
  --max-segment-bytes N --max-frame-bytes N --max-frames N
  --max-journal-bytes N --max-seconds N
Optional for both: --profile ABSOLUTE_JSON (owned regular file, <=4KiB, no symlink traversal).
The profile has exact BackupProfile fields; unknown fields or invalid limits are refused.
Omit it to retain the compatibility defaults.
Backup: --confirm-quiescent-owner yes
Restore: --confirm-fresh-target-owner yes --receipt-sha256 HEX64
Confirmations declare owner preconditions; they do not stop concurrent writers.
Restore requires a fresh database (existing public tables are refused), a different
database name and an empty
independent store; it does not initialize schema or activate restored data.
Roots must be private owned mode 0700; select exact PG16 tool or pinned wrapper.
Compatibility default transport caps: store 64MiB/dump 64MiB, 256 files/512 entries,
and 256 history/log/member rows; the selected dump cap must cover process FSIZE.
Tools use stdout/stdin, each <=60s within remaining max-seconds.
Use an admitted hard whole supervisor and owned container cleanup if applicable.
A failed operation may leave partial output; do not consume it as a restore.
";

fn parse_profile_json(raw: &[u8]) -> Result<BackupProfile, &'static str> {
    let profile: BackupProfile =
        serde_json::from_slice(raw).map_err(|_| "invalid backup profile JSON")?;
    profile
        .validate()
        .map_err(|_| "invalid backup profile limits")
}

fn read_profile_file(path: &Path) -> Result<BackupProfile, &'static str> {
    if !path.is_absolute() {
        return Err("backup profile path must be absolute");
    }
    let mut file = tos_fd_open::open_absolute_regular(path, MAX_PROFILE_BYTES)
        .map_err(|_| "backup profile must be a bounded regular file without symlink traversal")?;
    let before = file
        .metadata()
        .map_err(|_| "backup profile metadata unavailable")?;
    if before.uid() != rustix::process::geteuid().as_raw() || before.nlink() != 1 {
        return Err("backup profile must be owned by the invoking user and have no hard links");
    }
    let mut raw = Vec::with_capacity(
        usize::try_from(before.len()).map_err(|_| "backup profile exceeds its byte limit")?,
    );
    (&mut file)
        .take(MAX_PROFILE_BYTES + 1)
        .read_to_end(&mut raw)
        .map_err(|_| "backup profile read failed")?;
    let after = file
        .metadata()
        .map_err(|_| "backup profile metadata unavailable")?;
    if raw.len() as u64 > MAX_PROFILE_BYTES
        || raw.len() as u64 != before.len()
        || (
            before.dev(),
            before.ino(),
            before.len(),
            before.mtime(),
            before.mtime_nsec(),
            before.ctime(),
            before.ctime_nsec(),
        ) != (
            after.dev(),
            after.ino(),
            after.len(),
            after.mtime(),
            after.mtime_nsec(),
            after.ctime(),
            after.ctime_nsec(),
        )
    {
        return Err("backup profile changed while being read or exceeded its byte limit");
    }
    parse_profile_json(&raw)
}

const INTEGRITY_PREDICATE_PREFIX: &str =
    "owner-selected backup/restore integrity check failed at predicate line ";

fn refusal_diagnostic(operation: &str, error: &std::io::Error) -> Option<String> {
    if error.kind() != std::io::ErrorKind::InvalidData {
        return None;
    }
    let predicate_line = error
        .to_string()
        .strip_prefix(INTEGRITY_PREDICATE_PREFIX)
        .and_then(|line| line.parse::<u32>().ok())
        .filter(|line| *line != 0);
    Some(match predicate_line {
        Some(line) => {
            format!("native recovery diagnostic: operation={operation} predicate_line={line}")
        }
        None => format!(
            "native recovery diagnostic: operation={operation} predicate=integrity-or-format"
        ),
    })
}

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
        "--profile",
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
    let profile = match values.get("--profile") {
        Some(path) => read_profile_file(Path::new(path))?,
        None => BackupProfile::default()
            .validate()
            .map_err(|_| "invalid default backup profile")?,
    };
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
        crate::backup_recovery::backup_quiescent_with_profile(
            &BackupSelection {
                pg_url: &url,
                domain,
                store_root: store,
                backup_root: backup,
                tool,
                store_limits: limits,
                quiescent_owner_confirmed: true,
            },
            profile,
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
        crate::backup_recovery::restore_into_fresh_with_profile(
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
            profile,
            deadline,
            &cancelled,
        )
    };
    result.map_err(|error| {
        if let Some(diagnostic) = refusal_diagnostic(operation, &error) {
            eprintln!("{diagnostic}");
        }
        match error.kind() {
            std::io::ErrorKind::TimedOut => {
                "backup/restore deadline exceeded; preserve partial output"
            }
            std::io::ErrorKind::Interrupted => "backup/restore cancelled; preserve partial output",
            std::io::ErrorKind::InvalidInput => "backup/restore selection or limits refused",
            std::io::ErrorKind::InvalidData => "backup/restore integrity or format refused",
            std::io::ErrorKind::PermissionDenied => "backup/restore owner or access refused",
            std::io::ErrorKind::AlreadyExists => "backup/restore requires a fresh destination",
            std::io::ErrorKind::NotFound => "backup/restore selected input or tool is absent",
            _ => "backup/restore IO or database refused; preserve partial output",
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::symlink};

    #[test]
    fn integrity_refusal_diagnostic_exposes_only_a_recognized_predicate() {
        let exact = std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{INTEGRITY_PREDICATE_PREFIX}441"),
        );
        assert_eq!(
            refusal_diagnostic("backup", &exact).as_deref(),
            Some("native recovery diagnostic: operation=backup predicate_line=441")
        );

        let unrecognized = std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "private database URL and password",
        );
        assert_eq!(
            refusal_diagnostic("restore", &unrecognized).as_deref(),
            Some("native recovery diagnostic: operation=restore predicate=integrity-or-format")
        );

        let other_kind = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "private path");
        assert_eq!(refusal_diagnostic("backup", &other_kind), None);
    }

    #[test]
    fn explicit_profile_is_bounded_owned_and_strict() {
        let directory = tempfile::tempdir().unwrap();
        let profile_path = directory.path().join("profile.json");
        let mut profile = BackupProfile::default();
        profile.max_history_rows = 512;
        profile.max_log_rows = 1024;
        fs::write(&profile_path, serde_json::to_vec(&profile).unwrap()).unwrap();
        assert_eq!(read_profile_file(&profile_path).unwrap(), profile);

        let link_path = directory.path().join("profile-link.json");
        symlink(&profile_path, &link_path).unwrap();
        assert!(read_profile_file(&link_path).is_err());

        let mut unknown = serde_json::to_value(profile).unwrap();
        unknown["unrecognized"] = serde_json::json!(1);
        assert!(parse_profile_json(&serde_json::to_vec(&unknown).unwrap()).is_err());

        let oversized_path = directory.path().join("oversized-profile.json");
        fs::write(
            &oversized_path,
            vec![b' '; usize::try_from(MAX_PROFILE_BYTES).unwrap() + 1],
        )
        .unwrap();
        assert!(read_profile_file(&oversized_path).is_err());

        profile.max_files = 8;
        profile.max_entries = 4;
        assert!(parse_profile_json(&serde_json::to_vec(&profile).unwrap()).is_err());
    }
}
