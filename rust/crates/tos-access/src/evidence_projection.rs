//! Maintained, installed Evidence Lens builder/check/validator command adapter.
use std::{
    fs,
    io::Write,
    path::PathBuf,
    time::{Duration, Instant},
};
use tos_compiler::{PublicCaptureLimits, epistemic_evidence};
fn run(args: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    let verb = args
        .get(1)
        .map(String::as_str)
        .ok_or("evidence-projection requires build, check or validate")?;
    if !matches!(verb, "build" | "check" | "validate") {
        return Err("unknown Evidence Lens operation".into());
    }
    let (mut root, mut staging, mut output, mut seconds) = (None, None, None, None);
    let mut options = args.iter().skip(2);
    while let Some(option) = options.next() {
        let slot = match option.as_str() {
            "--source-root" => &mut root,
            "--staging" => &mut staging,
            "--output" => &mut output,
            "--max-seconds" => &mut seconds,
            _ => return Err(format!("unknown Evidence Lens option: {option}")),
        };
        let value = options
            .next()
            .ok_or_else(|| format!("{option} requires a value"))?;
        if slot.replace(value.as_str()).is_some() {
            return Err(format!("duplicate Evidence Lens option: {option}"));
        }
    }
    let root = PathBuf::from(root.ok_or("--source-root is required")?);
    let staging = PathBuf::from(staging.ok_or("--staging fresh SQLite path is required")?);
    if !root.is_absolute() || !staging.is_absolute() {
        return Err("Evidence Lens source and staging paths must be absolute".into());
    }
    let seconds = seconds
        .ok_or("--max-seconds is required")?
        .parse::<u64>()
        .map_err(|_| "invalid Evidence Lens seconds")?;
    if seconds == 0 || seconds > 86400 {
        return Err("Evidence Lens seconds outside 1..86400".into());
    }
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(seconds))
        .ok_or("Evidence Lens deadline overflow")?;
    let limits = PublicCaptureLimits {
        max_input_bytes: 1024 * 1024 * 1024,
        max_rows: 10_000_000,
        max_staging_bytes: 1024 * 1024 * 1024,
        max_work_bytes: 8 * 1024 * 1024 * 1024,
        max_sql_vm_steps: 1_000_000_000,
        sqlite_cache_kib: 4096,
    };
    if verb == "build" {
        let output = PathBuf::from(output.ok_or("build requires --output fresh absolute file")?);
        if !output.is_absolute() {
            return Err("Evidence Lens output must be absolute".into());
        }
        let rendered = epistemic_evidence::build(&root, &staging, limits, deadline)
            .map_err(|e| e.to_string())?;
        if Instant::now() >= deadline {
            return Err("Evidence Lens deadline".into());
        }
        // Fresh explicit output leaves authoritative checked-in projection intact.
        use std::io::Write as _;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)
            .map_err(|e| e.to_string())?;
        file.write_all(&rendered)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        if Instant::now() >= deadline {
            return Err(
                "Evidence Lens deadline after output sync; fresh output is not accepted".into(),
            );
        }
        writeln!(stdout, "[ok] wrote ToS Evidence Lens projection").map_err(|e| e.to_string())?;
        if Instant::now() >= deadline {
            return Err("Evidence Lens deadline after receipt".into());
        }
        Ok(())
    } else {
        if output.is_some() {
            return Err("check/validate do not accept --output".into());
        }
        epistemic_evidence::check(&root, &staging, limits, deadline).map_err(|e| e.to_string())?;
        if Instant::now() >= deadline {
            return Err("Evidence Lens deadline".into());
        }
        writeln!(
            stdout,
            "[ok] {} ToS Evidence Lens projection",
            if verb == "check" {
                "verified"
            } else {
                "validated"
            }
        )
        .map_err(|e| e.to_string())?;
        if Instant::now() >= deadline {
            return Err("Evidence Lens deadline after receipt".into());
        }
        Ok(())
    }
}
pub fn run_if_requested(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Option<i32> {
    if args.first().is_none_or(|v| v != "evidence-projection") {
        return None;
    }
    Some(match run(args, stdout) {
        Ok(()) => 0,
        Err(e) => {
            let _ = writeln!(stderr, "Evidence Lens: {e}");
            2
        }
    })
}
