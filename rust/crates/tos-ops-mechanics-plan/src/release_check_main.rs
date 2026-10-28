//! Native consumer of the authored software release command sequence.

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
    phase: validation_lanes::ReleasePhase,
    test_group: Option<validation_lanes::SoftwareTestGroup>,
    limits: executor::Limits,
    cargo_timings: bool,
}

fn usage() -> &'static str {
    "usage: tos-release-check --repo-root PATH [--phase all|checks|tests] [--test-group access|query|command] [--cargo-timings] [--command-timeout-ms N] [--lane-timeout-ms N] [--cleanup-grace-ms N] [--max-output-bytes N]"
}

fn options() -> Result<Options, String> {
    let mut args = env::args().skip(1);
    let mut root = None;
    let mut phase = validation_lanes::ReleasePhase::All;
    let mut test_group = None;
    let mut limits = executor::Limits::default();
    let mut cargo_timings = false;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--test-group" => {
                test_group = Some(
                    validation_lanes::SoftwareTestGroup::parse(
                        &args.next().ok_or("missing test group")?,
                    )
                    .ok_or("test group must be access, query, or command")?,
                );
            }
            "--cargo-timings" => cargo_timings = true,
            "--repo-root" => root = Some(PathBuf::from(args.next().ok_or("missing repo root")?)),
            "--phase" => {
                phase = match args.next().ok_or("missing phase")?.as_str() {
                    "all" => validation_lanes::ReleasePhase::All,
                    "checks" => validation_lanes::ReleasePhase::Checks,
                    "tests" => validation_lanes::ReleasePhase::Tests,
                    _ => return Err("phase must be all, checks, or tests".into()),
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
    if test_group.is_some() && phase != validation_lanes::ReleasePhase::Tests {
        return Err("--test-group requires --phase tests".into());
    }
    Ok(Options {
        root: root.ok_or("--repo-root is required")?,
        phase,
        test_group,
        limits,
        cargo_timings,
    })
}

fn main() {
    let options = options().unwrap_or_else(|error| {
        eprintln!("{error}\n{}", usage());
        std::process::exit(2);
    });
    let steps = validation_lanes::release_steps_with_test_group(
        &options.root,
        options.phase,
        options.test_group,
    )
    .unwrap_or_else(|error| {
        println!("[error] {error}");
        std::process::exit(2);
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
    let code = executor::run_release_sequence_with_timings(
        &options.root,
        &steps,
        options.limits,
        &CANCEL,
        options.cargo_timings,
    )
    .unwrap_or_else(|error| {
        eprintln!("error: release sequence: {error}");
        let signal = CANCEL.load(Ordering::Relaxed);
        std::process::exit(if signal == 0 { 1 } else { 128 + signal });
    });
    std::process::exit(code);
}
