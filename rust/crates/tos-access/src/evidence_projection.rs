//! Maintained, installed Evidence Lens builder/check/validator command adapter.
use std::{
    env, fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tos_compiler::{
    PublicCaptureLimits, epistemic_evidence, research_execution::ResearchExecution,
};

const OUTPUT_ALLOWANCE: u64 = 2 * 1024 * 1024;
const MAX_STAGE: u64 = 1024 * 1024 * 1024;
const HELP: &str = "usage: tos evidence-projection build|check|validate --source-root ABS [--max-seconds 1..86400] [--staging FRESH_ABS | --staging-parent PRIVATE_ABS] [--scratch-bytes RESERVED_REMAINING_BYTES] [--output ABS] [--replace]\n\nWithout --staging, select a private scratch parent and remaining quota through the options or TOS_EVIDENCE_STAGING_PARENT and TOS_EVIDENCE_SCRATCH_BYTES. The command creates and removes only its own private SQLite directory. The explicit --staging route retains the caller-selected capture. These options do not grant host storage. Build defaults to the source root's generated Evidence companion; --replace uses the captured prior bytes under the shared native output lock. Check and validate never replace outputs. Default deadline: 180 seconds.\n";

struct Scratch {
    path: PathBuf,
    identity: (u64, u64),
}
impl Scratch {
    fn new(parent: &Path) -> Result<Self, String> {
        let held = tos_fd_open::open_absolute_directory(parent).map_err(|e| e.to_string())?;
        let metadata = held.metadata().map_err(|e| e.to_string())?;
        if metadata.uid() != unsafe { libc::geteuid() } || metadata.mode() & 0o077 != 0 {
            return Err("Evidence staging parent must be private and owned by this user".into());
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let path = parent.join(format!("tos-evidence-{}-{nonce}", std::process::id()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .map_err(|e| e.to_string())?;
        let child = tos_fd_open::open_absolute_directory(&path).map_err(|e| e.to_string())?;
        let child = child.metadata().map_err(|e| e.to_string())?;
        let scratch = Self {
            path,
            identity: (child.dev(), child.ino()),
        };
        let after = tos_fd_open::open_absolute_directory(parent)
            .map_err(|e| e.to_string())?
            .metadata()
            .map_err(|e| e.to_string())?;
        if (metadata.dev(), metadata.ino()) != (after.dev(), after.ino()) {
            return Err("Evidence staging parent changed during creation".into());
        }
        Ok(scratch)
    }
    fn cleanup(&self) -> Result<(), String> {
        let metadata = match fs::symlink_metadata(&self.path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.to_string()),
        };
        if !metadata.is_dir() || (metadata.dev(), metadata.ino()) != self.identity {
            return Err("Evidence owned staging directory identity changed; retained".into());
        }
        // Never recursively remove unknown contents or traverse a replacement.
        for name in [
            "capture.sqlite",
            "capture.sqlite-journal",
            "capture.sqlite-wal",
            "capture.sqlite-shm",
        ] {
            match fs::remove_file(self.path.join(name)) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(error.to_string()),
            }
        }
        fs::remove_dir(&self.path).map_err(|e| e.to_string())
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

fn run(args: &[String], stdout: &mut dyn Write) -> Result<(), String> {
    if args.get(1).is_none_or(|v| v == "--help" || v == "-h") {
        return write!(stdout, "{HELP}").map_err(|e| e.to_string());
    }
    let verb = args[1].as_str();
    if !matches!(verb, "build" | "check" | "validate") {
        return Err("unknown Evidence Lens operation".into());
    }
    let (mut root, mut staging, mut parent, mut quota, mut output, mut seconds) =
        (None, None, None, None, None, None);
    let mut replace = false;
    let mut options = args.iter().skip(2);
    while let Some(option) = options.next() {
        if option == "--replace" {
            if replace {
                return Err("duplicate --replace".into());
            }
            replace = true;
            continue;
        }
        let slot = match option.as_str() {
            "--source-root" => &mut root,
            "--staging" => &mut staging,
            "--staging-parent" => &mut parent,
            "--scratch-bytes" => &mut quota,
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
    if verb != "build" && (replace || output.is_some()) {
        return Err("check/validate do not accept --output or --replace".into());
    }
    if staging.is_some() && parent.is_some() {
        return Err("choose --staging or --staging-parent".into());
    }
    let root = PathBuf::from(root.ok_or("--source-root is required")?);
    let seconds = seconds
        .unwrap_or("180")
        .parse::<u64>()
        .map_err(|_| "invalid Evidence Lens seconds")?;
    let parent_env = env::var("TOS_EVIDENCE_STAGING_PARENT").ok();
    let quota_env = env::var("TOS_EVIDENCE_SCRATCH_BYTES").ok();
    let quota = quota
        .or(quota_env.as_deref())
        .map(str::parse::<u64>)
        .transpose()
        .map_err(|_| "invalid Evidence scratch bytes")?;
    // The original explicit staging route retains its declared 1 GiB SQLite
    // bound. The managed route divides the caller's already reserved space
    // between MAIN/TEMP/journal headroom and the small atomic JSON output.
    let stage_limit = match quota {
        Some(bytes) if bytes > OUTPUT_ALLOWANCE && bytes != u64::MAX => {
            ((bytes - OUTPUT_ALLOWANCE) / 4).min(MAX_STAGE)
        }
        Some(_) => return Err("Evidence scratch bytes must exceed 2 MiB and be finite".into()),
        None if staging.is_some() => MAX_STAGE,
        None => {
            return Err(
                "owned Evidence staging requires --scratch-bytes or TOS_EVIDENCE_SCRATCH_BYTES"
                    .into(),
            );
        }
    };
    if stage_limit < 65536 {
        return Err("Evidence scratch quota cannot fit the minimum SQLite capture".into());
    }
    let execution = ResearchExecution::new_evidence_output(&root, seconds, OUTPUT_ALLOWANCE)?;
    let mut owned = None;
    let staging = match staging {
        Some(path) => {
            let path = PathBuf::from(path);
            if !path.is_absolute() {
                return Err("Evidence staging must be absolute".into());
            }
            path
        }
        None => {
            let parent = Path::new(
                parent
                    .or(parent_env.as_deref())
                    .ok_or("select --staging-parent or TOS_EVIDENCE_STAGING_PARENT")?,
            );
            owned = Some(Scratch::new(parent)?);
            owned.as_ref().unwrap().path.join("capture.sqlite")
        }
    };
    let limits = PublicCaptureLimits {
        max_input_bytes: 1024 * 1024 * 1024,
        max_rows: 10_000_000,
        max_staging_bytes: stage_limit,
        max_work_bytes: 8 * 1024 * 1024 * 1024,
        max_sql_vm_steps: 1_000_000_000,
        sqlite_cache_kib: 4096,
    };
    let result = (|| {
        if verb == "build" {
            let target = output
                .map(PathBuf::from)
                .unwrap_or_else(|| root.join(epistemic_evidence::PROJECTION_REF));
            if !target.is_absolute() {
                return Err("Evidence output must be absolute".into());
            }
            let parent = target.parent().ok_or("Evidence output parent required")?;
            let leaf = target
                .file_name()
                .and_then(|v| v.to_str())
                .ok_or("Evidence output leaf must be UTF-8")?;
            let writer = execution.select_output_directory(parent, true)?;
            let previous = match fs::symlink_metadata(&target) {
                Ok(metadata) => {
                    if !replace {
                        return Err("Evidence output exists; select --replace".into());
                    }
                    if !metadata.is_file() || metadata.len() > 1024 * 1024 {
                        return Err(
                            "Evidence prior output must be a regular file within 1 MiB".into()
                        );
                    }
                    let mut file = writer.source_file(leaf, 1024 * 1024)?;
                    let mode = file.metadata().map_err(|e| e.to_string())?.mode() & 0o777;
                    Some((writer.read_file(&mut file, 1024 * 1024)?, mode))
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(error.to_string()),
            };
            let rendered = epistemic_evidence::build(&root, &staging, limits, execution.deadline())
                .map_err(|e| e.to_string())?;
            execution.check()?;
            if rendered.len() > 1024 * 1024 {
                return Err("Evidence generated output exceeds 1 MiB".into());
            }
            match previous {
                Some((raw, mode)) => writer.write_replacing_exact(leaf, &rendered, mode, &raw)?,
                None => writer.write(leaf, &rendered, 0o644, true)?,
            }
        } else {
            epistemic_evidence::check(&root, &staging, limits, execution.deadline())
                .map_err(|e| e.to_string())?;
        }
        execution.check()
    })();
    let cleanup = owned.as_ref().map(Scratch::cleanup).transpose();
    match (result, cleanup) {
        (Err(error), Err(cleanup)) => return Err(format!("{error}; staging cleanup: {cleanup}")),
        (Err(error), _) => return Err(error),
        (_, Err(error)) => return Err(error),
        _ => (),
    }
    execution.check()?;
    writeln!(
        stdout,
        "[ok] {} ToS Evidence Lens projection",
        match verb {
            "build" => "wrote",
            "check" => "verified",
            _ => "validated",
        }
    )
    .map_err(|e| e.to_string())?;
    execution.check()
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
