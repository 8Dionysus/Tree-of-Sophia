//! Explicit offline public D1 builder. It publishes disposable SQL/static
//! carriers only; no installed-current or managed selection is implied.

use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};
use tos_compiler::{Limits, PublicD1Build, build_public_d1, public_d1_limits};

fn path(value: &str) -> Result<PathBuf, String> {
    let supplied = Path::new(value);
    if supplied.is_symlink() {
        return Err("public D1 path is a symlink".into());
    }
    if supplied.exists() {
        return fs::canonicalize(supplied).map_err(|e| e.to_string());
    }
    let parent = supplied
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = supplied
        .file_name()
        .ok_or("public D1 path has no final name")?;
    Ok(fs::canonicalize(parent)
        .map_err(|e| e.to_string())?
        .join(name))
}

fn run(args: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    let mut source = None;
    let mut output = None;
    let mut runtime = None;
    let mut seconds = None;
    let mut state_bytes = None;
    let mut json_visits = None;
    let mut work_bytes = None;
    let mut model_bytes = None;
    let mut postings = None;
    let mut options = args.iter().skip(1);
    while let Some(option) = options.next() {
        let value = options
            .next()
            .ok_or_else(|| format!("public D1 option {option} requires a value"))?;
        let slot = match option.as_str() {
            "--source-root" => &mut source,
            "--output" => &mut output,
            "--runtime" => &mut runtime,
            "--max-build-seconds" => &mut seconds,
            "--max-state-bytes" => &mut state_bytes,
            "--max-json-visits" => &mut json_visits,
            "--max-work-bytes" => &mut work_bytes,
            "--max-model-bytes" => &mut model_bytes,
            "--max-postings" => &mut postings,
            _ => return Err(format!("unknown public D1 option: {option}")),
        };
        if slot.replace(value.as_str()).is_some() {
            return Err(format!("duplicate public D1 option: {option}"));
        }
    }
    // Deliberately no silent whole-build deadline. This must refuse before
    // compiler entry acquires its lock or invalidates completion markers.
    let seconds = seconds
        .map(str::to_owned)
        .or_else(|| env::var("TOS_BUILD_MAX_SECONDS").ok())
        .ok_or("TOS_BUILD_MAX_SECONDS or --max-build-seconds is required")?;
    let seconds = seconds
        .parse::<u64>()
        .map_err(|_| "invalid public D1 build seconds")?;
    let positive = |value: Option<&str>, fallback: u64, name: &str| -> Result<u64, String> {
        match value {
            Some(raw) => raw
                .parse::<u64>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| format!("invalid positive public D1 {name}")),
            None => Ok(fallback),
        }
    };
    let mut base = Limits::default();
    base.max_work_bytes = positive(work_bytes, base.max_work_bytes, "work bytes")?;
    base.max_output_bytes = positive(model_bytes, base.max_output_bytes, "model bytes")?;
    let postings = positive(postings, 10_000_000, "postings")?;
    let mut limits = public_d1_limits(seconds, base, postings).map_err(|e| e.to_string())?;
    limits.max_state_bytes = state_bytes
        .map(str::to_owned)
        .or_else(|| env::var("TOS_BUILD_MAX_STATE_BYTES").ok())
        .ok_or("TOS_BUILD_MAX_STATE_BYTES or --max-state-bytes is required")?
        .parse()
        .map_err(|_| "invalid public D1 state bytes")?;
    limits.max_json_visits = json_visits
        .map(str::to_owned)
        .or_else(|| env::var("TOS_BUILD_MAX_JSON_VISITS").ok())
        .ok_or("TOS_BUILD_MAX_JSON_VISITS or --max-json-visits is required")?
        .parse()
        .map_err(|_| "invalid public D1 JSON visits")?;
    if limits.max_state_bytes < 131072 || limits.max_json_visits == 0 {
        return Err(
            "public D1 requires --max-state-bytes >=131072 and --max-json-visits >0".into(),
        );
    }
    let source = path(source.ok_or("--source-root is required")?)?;
    let output = path(output.ok_or("--output is required")?)?;
    let runtime = path(runtime.ok_or("--runtime is required")?)?;
    let manifest = build_public_d1(PublicD1Build {
        source_root: &source,
        output: &output,
        runtime: &runtime,
        limits,
    })
    .map_err(|e| e.to_string())?;
    writeln!(stdout, "{manifest}").map_err(|e| e.to_string())
}

pub fn run_if_requested(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Option<i32> {
    if args.first().is_none_or(|arg| arg != "build-data") {
        return None;
    }
    let result = run(args, stdout);
    Some(match result {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(stderr, "public D1 build: {error}");
            2
        }
    })
}
