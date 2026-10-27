use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;
use tos_ops_mechanics_plan::{executor, validation_lanes};

static CANCEL: AtomicI32 = AtomicI32::new(0);

#[cfg(target_os = "linux")]
extern "C" fn cancelled(signal: i32) {
    CANCEL.store(signal, Ordering::Relaxed);
}

struct Options {
    root: PathBuf,
    check: bool,
    sequence: Option<String>,
    run: Option<String>,
    rust_group: Option<validation_lanes::RustGroup>,
    limits: executor::Limits,
    growth_jobs: usize,
    cargo_timings: bool,
}

fn usage() -> &'static str {
    "usage: tos-validation-lanes --repo-root PATH [--check] [--sequence ID] [--run ID] [--rust-group conformance|workspace|cold-install] [--growth-jobs 1|2] [--cargo-timings] [--command-timeout-ms N] [--lane-timeout-ms N] [--cleanup-grace-ms N] [--max-output-bytes N]"
}

fn options() -> Result<Options, String> {
    let mut args = env::args().skip(1);
    let mut root = None;
    let mut check = false;
    let mut sequence = None;
    let mut run = None;
    let mut rust_group = None;
    let mut limits = executor::Limits::default();
    let mut growth_jobs = 1;
    let mut cargo_timings = false;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--repo-root" => root = Some(PathBuf::from(args.next().ok_or("missing repo root")?)),
            "--check" => check = true,
            "--sequence" => sequence = Some(args.next().ok_or("missing sequence ID")?),
            "--run" => run = Some(args.next().ok_or("missing run ID")?),
            "--rust-group" => {
                let value = args.next().ok_or("missing Rust group")?;
                rust_group = Some(
                    validation_lanes::RustGroup::parse(&value)
                        .ok_or("Rust group must be conformance, workspace, or cold-install")?,
                );
            }
            "--cargo-timings" => cargo_timings = true,
            "--growth-jobs" => {
                growth_jobs = args
                    .next()
                    .ok_or("missing Growth jobs")?
                    .parse()
                    .map_err(|_| "invalid Growth jobs")?;
                if !(1..=2).contains(&growth_jobs) {
                    return Err("Growth jobs must be 1 or 2".into());
                }
            }
            "--command-timeout-ms"
            | "--lane-timeout-ms"
            | "--cleanup-grace-ms"
            | "--max-output-bytes" => {
                let value: u64 = args
                    .next()
                    .ok_or("missing limit")?
                    .parse()
                    .map_err(|_| "invalid numeric limit")?;
                match argument.as_str() {
                    "--command-timeout-ms" => limits.command_wall = Duration::from_millis(value),
                    "--lane-timeout-ms" => limits.lane_wall = Duration::from_millis(value),
                    "--cleanup-grace-ms" => limits.cleanup_grace = Duration::from_millis(value),
                    _ => {
                        limits.output_bytes =
                            usize::try_from(value).map_err(|_| "output limit overflow")?
                    }
                }
            }
            "--help" | "-h" => {
                println!("{}", usage());
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {argument}")),
        }
    }
    if rust_group.is_some()
        && ((sequence.is_none() && run.is_none())
            || sequence
                .as_deref()
                .into_iter()
                .chain(run.as_deref())
                .any(|id| id != "rust_workspace"))
    {
        return Err("--rust-group requires --sequence/--run rust_workspace".into());
    }
    if cargo_timings && run.is_none() {
        return Err("--cargo-timings requires --run".into());
    }
    Ok(Options {
        root: root.ok_or("--repo-root is required")?,
        check,
        sequence,
        run,
        rust_group,
        limits,
        growth_jobs,
        cargo_timings,
    })
}

fn main() {
    let options = options().unwrap_or_else(|error| {
        eprintln!("{error}\n{}", usage());
        std::process::exit(2);
    });
    if !options.check && options.sequence.is_none() && options.run.is_none() {
        println!("{}", usage());
        return;
    }
    if options.check {
        match validation_lanes::validate_manifest(&options.root) {
            Ok(issues) if issues.is_empty() => {
                println!("[ok] validated ToS validation lane manifest")
            }
            Ok(issues) => {
                eprintln!("Validation lane manifest check failed.");
                for (location, message) in issues {
                    eprintln!("- {location}: {message}");
                }
                std::process::exit(1);
            }
            Err(error) => {
                eprintln!(
                    "Validation lane manifest check failed.\n- docs/validation/validation_lanes.json: {error}"
                );
                std::process::exit(1);
            }
        }
    }
    if let Some(sequence) = options.sequence.as_deref() {
        let steps = validation_lanes::command_sequence_with_rust_group(
            &options.root,
            sequence,
            options.rust_group,
        )
        .unwrap_or_else(|error| {
            eprintln!("error: {error}");
            std::process::exit(1);
        });
        for ((label, command), _) in steps {
            println!("{label}: {}", command.join(" "));
        }
    }
    if let Some(sequence) = options.run.as_deref() {
        let steps = validation_lanes::command_sequence_with_rust_group(
            &options.root,
            sequence,
            options.rust_group,
        )
        .unwrap_or_else(|error| {
            eprintln!("error: {error}");
            std::process::exit(1);
        });
        let _ = io::stdout().flush();
        #[cfg(target_os = "linux")]
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = cancelled as *const () as usize;
            libc::sigemptyset(&mut action.sa_mask);
            for signal in [libc::SIGINT, libc::SIGTERM] {
                if libc::sigaction(signal, &action, std::ptr::null_mut()) != 0 {
                    eprintln!("error: {}", io::Error::last_os_error());
                    std::process::exit(1);
                }
            }
        }
        let code = executor::run_validation_sequence_with_options(
            &options.root,
            &steps,
            options.limits,
            &CANCEL,
            options.growth_jobs,
            options.cargo_timings,
        )
        .unwrap_or_else(|error| {
            eprintln!("error: validation sequence: {error}");
            let signal = CANCEL.load(Ordering::Relaxed);
            std::process::exit(if signal == 0 { 1 } else { 128 + signal });
        });
        std::process::exit(code);
    }
}
