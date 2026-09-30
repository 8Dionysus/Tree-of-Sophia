//! Installed native prepare command; no Python source-graph child or fallback.
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};
use tos_compiler::{
    local_prepared::PublicationLimits,
    local_prepared_bulk::BulkBootstrapLimits,
    native_prepare::{self, MaintenanceAttachmentLimits, PrepareRequest, SourceBootstrapLimits},
};
fn absolute(raw: &str, existing: bool) -> Result<PathBuf, String> {
    let path = Path::new(raw);
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        env::current_dir().map_err(|e| e.to_string())?.join(path)
    };
    if existing {
        fs::canonicalize(path).map_err(|e| e.to_string())
    } else {
        Ok(path)
    }
}
fn positive(raw: &str) -> Result<u64, String> {
    let n = raw
        .parse::<u64>()
        .map_err(|_| "positive integer required")?;
    if n == 0 {
        Err("positive integer required".into())
    } else {
        Ok(n)
    }
}
fn run(args: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    let mut source_limits: Option<SourceBootstrapLimits> = None;
    let mut source = None;
    let mut output = None;
    let mut seconds = None;
    let mut publication = PublicationLimits::default();
    let mut scratch_profile = None;
    let mut scratch_bytes = None;
    let mut scratch_mutations = None;
    let mut maintenance = None;
    let mut maintenance_mutations = None;
    let mut seen = std::collections::BTreeSet::new();
    let mut options = args.iter().skip(1);
    while let Some(option) = options.next() {
        if !seen.insert(option.as_str()) {
            return Err("duplicate prepare option".into());
        }
        if option == "--attach-maintenance" {
            maintenance = Some(MaintenanceAttachmentLimits::default());
            continue;
        }
        let raw = options.next().ok_or("prepare option requires value")?;
        match option.as_str() {
            "--source-root" => source = Some(absolute(raw, true)?),
            "--output-dir" => output = Some(absolute(raw, false)?),
            "--max-seconds" => seconds = Some(positive(raw)?),
            "--source-limits" => {
                source_limits = Some(
                    serde_json::from_str(raw).map_err(|_| "invalid source computational limits")?,
                )
            }
            "--max-bytes" => publication.max_bytes = positive(raw)?,
            "--max-mutations" => publication.max_mutations = positive(raw)?,
            "--bulk-search-scratch-bytes" => scratch_bytes = Some(positive(raw)?),
            "--bulk-search-scratch-mutations" => scratch_mutations = Some(positive(raw)?),
            "--maintenance-max-mutations" => maintenance_mutations = Some(positive(raw)?),
            // Explicit complete profiles preserve the programmatic caller's
            // existing allowance objects; they are not implicit host admission.
            "--publication-limits" => {
                publication = serde_json::from_str(raw).map_err(|_| "invalid publication limits")?
            }
            "--search-scratch-limits" => {
                if scratch_bytes.is_some() || scratch_mutations.is_some() {
                    return Err("conflicting scratch profiles".into());
                }
                let v: BulkBootstrapLimits =
                    serde_json::from_str(raw).map_err(|_| "invalid scratch limits")?;
                v.validate().map_err(|_| "invalid scratch limits")?;
                scratch_profile = Some(v);
            }
            "--maintenance-limits" => {
                maintenance =
                    Some(serde_json::from_str(raw).map_err(|_| "invalid maintenance limits")?)
            }
            _ => return Err("unknown prepare option".into()),
        }
    }
    if scratch_bytes.is_some() != scratch_mutations.is_some() {
        return Err("bulk search requires both scratch caps".into());
    }
    if let Some(n) = maintenance_mutations {
        maintenance
            .as_mut()
            .ok_or("maintenance allowance requires attachment")?
            .max_mutations = n;
    }
    let seconds = match seconds {
        Some(n) => n,
        None => positive(
            &env::var("TOS_PREPARED_MAX_SECONDS")
                .map_err(|_| "explicit prepare whole deadline required")?,
        )?,
    };
    let source = source.ok_or("--source-root required")?;
    let output = output.ok_or("--output-dir required")?;
    if scratch_profile.is_some() && (scratch_bytes.is_some() || scratch_mutations.is_some()) {
        return Err("conflicting scratch profiles".into());
    }
    let scratch = scratch_profile.or(match (scratch_bytes, scratch_mutations) {
        (Some(b), Some(m)) => Some(BulkBootstrapLimits::new(b, m)),
        _ => None,
    });
    let result = native_prepare::prepare(PrepareRequest {
        source_root: &source,
        output_dir: &output,
        publication,
        search_scratch: scratch,
        maintenance,
        max_seconds: seconds,
        source_limits,
    })
    .map_err(|_| "native prepare refused")?;
    writeln!(stdout, "{result}").map_err(|e| e.to_string())
}
pub fn run_if_requested(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Option<i32> {
    if args.first().is_none_or(|s| s != "prepare") {
        return None;
    }
    Some(match run(args, stdout) {
        Ok(()) => 0,
        Err(_) => {
            let _ = writeln!(
                stderr,
                "{{\"schema\":\"tos_offline_prepared_bootstrap_receipt_v1\",\"status\":\"failed\",\"error_type\":\"NativePrepareError\"}}"
            );
            1
        }
    })
}
