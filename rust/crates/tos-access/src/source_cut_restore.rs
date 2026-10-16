//! Explicit selected immutable authored-cut restoration, not writer relocation.
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_foundation::{Digest256, JsonLimits, SourceRevision};
use tos_source_store::{CorpusReader, CutReadLimits, ReadLimits};

pub fn run_if_requested(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Option<i32> {
    if args.first().is_none_or(|x| x != "restore-source-cut") {
        return None;
    }
    let started = Instant::now();
    Some(match run(&args[1..], started) {
        Ok(raw) => {
            if out
                .write_all(raw.as_bytes())
                .and_then(|_| out.flush())
                .is_ok()
            {
                0
            } else {
                2
            }
        }
        Err(detail) => {
            let _ = writeln!(err, "restore_source_cut_refused: {detail}");
            2
        }
    })
}
fn run(args: &[String], started: Instant) -> Result<String, &'static str> {
    const KEYS: [&str; 10] = [
        "--corpus-store",
        "--source-revision",
        "--output",
        "--max-revisions",
        "--max-directories",
        "--max-members",
        "--max-member-bytes",
        "--max-total-bytes",
        "--max-metadata-bytes",
        "--max-seconds",
    ];
    if args.len() != KEYS.len() * 2 {
        return Err("requires exact selection and all finite limits");
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
        let n = values[key]
            .parse::<u64>()
            .map_err(|_| "invalid finite limit")?;
        if n == 0 || n == u64::MAX {
            Err("invalid finite limit")
        } else {
            Ok(n)
        }
    };
    let count = |key: &str| -> Result<usize, &'static str> {
        usize::try_from(number(key)?).map_err(|_| "limit overflow")
    };
    let store = Path::new(values["--corpus-store"]);
    let output = Path::new(values["--output"]);
    if !store.is_absolute() || !output.is_absolute() {
        return Err("store/output must be absolute");
    }
    let revision = SourceRevision(
        Digest256::from_prefixed(values["--source-revision"])
            .map_err(|_| "invalid exact revision")?,
    );
    let deadline = started
        .checked_add(Duration::from_secs(number("--max-seconds")?))
        .ok_or("deadline overflow")?;
    let metadata = count("--max-metadata-bytes")?;
    let reader = CorpusReader::open_existing(
        store,
        ReadLimits {
            max_manifest_bytes: metadata,
            max_manifest_entries: count("--max-members")?,
            max_selected_object_bytes: number("--max-member-bytes")?,
            json: JsonLimits {
                max_bytes: metadata,
                ..JsonLimits::default()
            },
        },
    )
    .map_err(|e| e.detail)?;
    let result = tos_source_store::restore_source_cut(
        &reader,
        revision,
        output,
        CutReadLimits {
            max_revisions: count("--max-revisions")?,
            max_members: number("--max-members")?,
            max_total_bytes: number("--max-total-bytes")?,
            max_member_bytes: number("--max-member-bytes")?,
        },
        count("--max-directories")?,
        deadline,
        &AtomicBool::new(false),
    )
    .map_err(|e| e.detail)?;
    Ok(format!(
        "{{\"member_count\":{},\"membership_sha256\":\"{}\",\"restored\":true,\"source_bytes\":{},\"source_revision\":\"{}\",\"writer_grant_transferred\":false}}\n",
        result.member_count,
        result.membership_sha256.to_prefixed(),
        result.source_bytes,
        result.revision.0.to_prefixed()
    ))
}
