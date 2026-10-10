//! Disposable CLI: Git capture retains the crate's dedicated executor custody.
use std::env;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, Ordering};
use tos_ops_mechanics_plan::route_cards;
static CANCEL: AtomicI32 = AtomicI32::new(0);
#[cfg(target_os = "linux")]
extern "C" fn cancelled(signal: i32) {
    CANCEL.store(signal, Ordering::Relaxed);
}
fn run() -> Result<i32, String> {
    let mut args = env::args().skip(1);
    let mut root = None;
    let mut mode = None;
    let mut check = false;
    let mut output = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--repo-root" => {
                root = Some(PathBuf::from(args.next().ok_or("missing repository root")?))
            }
            "build" | "validate" if mode.is_none() => mode = Some(arg),
            "--check" => check = true,
            "--output" => output = Some(PathBuf::from(args.next().ok_or("missing output")?)),
            "--help" | "-h" => {
                println!(
                    "usage: tos-route-cards --repo-root ABSOLUTE_PATH build [--check] [--output PATH] | validate"
                );
                return Ok(0);
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    let root = root.ok_or("--repo-root is required")?;
    let mode = mode.ok_or("build or validate is required")?;
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
    if mode == "validate" {
        if check || output.is_some() {
            return Err("validate accepts no build flags".into());
        }
        let issues = route_cards::run_validation(&root, &CANCEL).map_err(|e| e.to_string())?;
        if !issues.is_empty() {
            println!("Nested AGENTS route-card check failed.");
            for (location, message) in issues {
                println!("- {location}: {message}");
            }
            return Ok(1);
        }
        let inventory = route_cards::load_inventory(&root).map_err(|e| e.to_string())?;
        let count = route_cards::discover_route_cards(&root, &inventory)
            .map_err(|e| e.to_string())?
            .len();
        println!("Nested AGENTS route-card check passed for {count} files.");
        return Ok(0);
    }
    let value = route_cards::build_currentness(&root, &CANCEL).map_err(|e| e.to_string())?;
    let rendered = route_cards::render_currentness(&value).map_err(|e| e.to_string())?;
    let chosen = match output {
        Some(path) => path,
        None => {
            let inventory = route_cards::load_inventory(&root).map_err(|e| e.to_string())?;
            PathBuf::from(
                inventory["currentness"]
                    .as_str()
                    .ok_or("missing currentness output path")?,
            )
        }
    };
    let path = if chosen.is_absolute() {
        chosen
    } else {
        root.join(chosen)
    };
    let display = path.strip_prefix(&root).unwrap_or(&path).display();
    if check {
        let actual = route_cards::read_output(&path).map_err(|e| e.to_string())?;
        if actual.as_deref() != Some(&rendered) {
            println!("AGENTS route currentness is stale or missing: {display}");
            return Ok(1);
        }
        println!("AGENTS route currentness is current: {display}");
    } else {
        route_cards::write_output(&root, &path, &rendered).map_err(|e| e.to_string())?;
        println!("wrote {display}");
    }
    Ok(0)
}
fn main() {
    match run() {
        Ok(code) => {
            let signal = CANCEL.load(Ordering::Relaxed);
            std::process::exit(if signal == 0 { code } else { 128 + signal });
        }
        Err(error) => {
            eprintln!("error: {error}");
            let signal = CANCEL.load(Ordering::Relaxed);
            std::process::exit(if signal == 0 { 1 } else { 128 + signal });
        }
    }
}
