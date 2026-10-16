//! Explicit local byte restoration, without selecting data or granting authority.
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_foundation::{Digest256, JsonLimits};
use tos_source_store::{CaptureRestoreLimits, ReadLimits, SoftwareCaptureSelectionV1};

pub fn run_if_requested(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<i32> {
    if args.first().is_none_or(|x| x != "capture-restore") {
        return None;
    }
    let started = Instant::now();
    let result = run(&args[1..], started);
    Some(match result {
        Ok(()) => {
            if writeln!(
                out,
                "{{\"restored\":true,\"source_admission\":false,\"consumer_switched\":false}}"
            )
            .is_ok()
            {
                0
            } else {
                2
            }
        }
        Err(detail) => {
            let _ = writeln!(err, "capture_restore_refused: {detail}");
            2
        }
    })
}
fn run(args: &[String], started: Instant) -> Result<(), &'static str> {
    const KEYS: [&str; 11] = [
        "--capture",
        "--output",
        "--source-commit",
        "--source-tree",
        "--manifest-sha256",
        "--max-archive-bytes",
        "--max-decoded-bytes",
        "--max-source-bytes",
        "--max-metadata-bytes",
        "--max-members",
        "--max-seconds",
    ];
    if args.len() != KEYS.len() * 2 {
        return Err("requires exact named selection and finite limits; see --help");
    }
    let mut values = BTreeMap::new();
    for pair in args.chunks_exact(2) {
        if !KEYS.contains(&pair[0].as_str())
            || values.insert(pair[0].as_str(), pair[1].as_str()).is_some()
        {
            return Err("unknown or duplicate option");
        }
    }
    let number = |key: &str| -> Result<u64, &'static str> {
        let value: u64 = values[key].parse().map_err(|_| "invalid finite limit")?;
        if value == 0 || value == u64::MAX {
            Err("invalid finite limit")
        } else {
            Ok(value)
        }
    };
    let capture = Path::new(values["--capture"]);
    let output = Path::new(values["--output"]);
    if !capture.is_absolute() || !output.is_absolute() {
        return Err("capture/output must be absolute");
    }
    let deadline = started
        .checked_add(Duration::from_secs(number("--max-seconds")?))
        .ok_or("deadline overflow")?;
    let metadata_bytes =
        usize::try_from(number("--max-metadata-bytes")?).map_err(|_| "metadata limit overflow")?;
    let members = usize::try_from(number("--max-members")?).map_err(|_| "member limit overflow")?;
    let source_bytes = number("--max-source-bytes")?;
    let selection = SoftwareCaptureSelectionV1 {
        source_git_commit: values["--source-commit"].into(),
        source_git_tree: values["--source-tree"].into(),
        capture_manifest_sha256: Digest256::from_hex(values["--manifest-sha256"])
            .map_err(|_| "invalid manifest digest")?,
    };
    let limits = CaptureRestoreLimits {
        metadata: ReadLimits {
            max_manifest_bytes: metadata_bytes,
            max_manifest_entries: members,
            max_selected_object_bytes: source_bytes,
            json: JsonLimits {
                max_bytes: metadata_bytes,
                ..JsonLimits::default()
            },
        },
        max_archive_bytes: number("--max-archive-bytes")?,
        max_decoded_bytes: number("--max-decoded-bytes")?,
        max_source_bytes: source_bytes,
    };
    tos_source_store::restore_capture(
        capture,
        output,
        &selection,
        limits,
        deadline,
        &AtomicBool::new(false),
    )
    .map_err(|e| e.detail)
}
