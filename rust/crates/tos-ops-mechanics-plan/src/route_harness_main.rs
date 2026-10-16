//! Source-only native candidate. Maintained Python/default lanes stay active.
use std::env;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, Ordering};
use tos_ops_mechanics_plan::{route_cards, route_harness};

static CANCEL: AtomicI32 = AtomicI32::new(0);
#[cfg(target_os = "linux")]
extern "C" fn cancelled(signal: i32) {
    CANCEL.store(signal, Ordering::Relaxed);
}

fn usage() -> &'static str {
    "usage: tos-agents-route-harness --repo-root ABSOLUTE_PATH [--check] [--output PATH] [--source-ref REF] [--volatile-timing]"
}
fn run() -> Result<i32, String> {
    let mut root = None;
    let mut output = None;
    let mut source_ref = None;
    let mut check = false;
    let mut include_timing = false;
    let mut args = env::args().skip(1);
    let mut arguments = 0;
    while let Some(arg) = args.next() {
        arguments += 1;
        if arguments > 16 || arg.len() > 4096 {
            return Err("harness argument budget exceeded".into());
        }
        let mut value = || -> Result<String, String> {
            let value = args
                .next()
                .ok_or_else(|| format!("missing value for {arg}"))?;
            if value.len() > 4096 || value.contains('\0') {
                return Err("harness argument byte budget exceeded".into());
            }
            Ok(value)
        };
        match arg.as_str() {
            "--repo-root" => root = Some(PathBuf::from(value()?)),
            "--output" => output = Some(PathBuf::from(value()?)),
            "--source-ref" => source_ref = Some(value()?),
            "--check" => check = true,
            "--volatile-timing" => include_timing = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return Ok(0);
            }
            _ => return Err(format!("unknown argument: {arg}\n{}", usage())),
        }
    }
    let root = root.ok_or_else(|| format!("--repo-root is required\n{}", usage()))?;
    #[cfg(target_os = "linux")]
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = cancelled as *const () as usize;
        libc::sigemptyset(&mut action.sa_mask);
        for signal in [libc::SIGINT, libc::SIGTERM] {
            if libc::sigaction(signal, &action, std::ptr::null_mut()) != 0 {
                return Err(io::Error::last_os_error().to_string());
            }
        }
    }
    let mut result =
        route_harness::build_result(&root, include_timing, &CANCEL).map_err(|e| e.to_string())?;
    result["source_ref"] = route_harness::requested_source_ref(
        result["source_ref"]
            .as_str()
            .ok_or("invalid observed source ref")?,
        source_ref.as_deref(),
    )
    .into();
    let rendered = route_harness::render_result(&result).map_err(|e| e.to_string())?;
    if let Some(path) = output {
        let output = if path.is_absolute() {
            path
        } else {
            root.join(path)
        };
        route_cards::write_output(&root, &output, &rendered).map_err(|e| e.to_string())?;
        println!("wrote {}", output.display());
    } else if !check {
        print!("{rendered}");
    }
    let failures = result["tasks"]
        .as_array()
        .ok_or("invalid tasks")?
        .iter()
        .filter(|task| task["route_success"] != true)
        .count();
    if check {
        if failures != 0 {
            println!(
                "AGENTS route harness failed for {failures}/{} task routes",
                result["task_count"]
            );
            return Ok(1);
        }
        println!(
            "AGENTS route harness passed for {} task routes",
            result["task_count"]
        );
    }
    Ok(0)
}
fn main() {
    let code = match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error}");
            1
        }
    };
    let signal = CANCEL.load(Ordering::Relaxed);
    std::process::exit(if signal == 0 { code } else { 128 + signal });
}
