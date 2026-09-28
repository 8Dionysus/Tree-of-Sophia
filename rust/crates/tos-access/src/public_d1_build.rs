//! Explicit offline public D1 builder. It publishes disposable SQL/static
//! carriers only; no installed-current or managed selection is implied.

use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};
use tos_compiler::{PublicD1Build, build_public_d1, portable_public_d1_limits};

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
    let limits = portable_public_d1_limits(seconds).map_err(|e| e.to_string())?;
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
