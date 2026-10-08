use std::{
    collections::BTreeMap,
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicI32, Ordering},
};
use tos_ops_mechanics_plan::open_work_queue;
static CANCEL: AtomicI32 = AtomicI32::new(0);
extern "C" fn cancelled(signal: i32) {
    CANCEL.store(signal, Ordering::Relaxed);
}
fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let operation = args
        .next()
        .ok_or("expected build, check, validate or readiness")?;
    if operation == "--help" {
        println!(
            "usage: tos-open-work-queue build|check|validate --source-root PATH\n       tos-open-work-queue readiness --source-root PATH [--readiness-plan REPOSITORY_RELATIVE_PATH]\n       tos-open-work-queue build --source-root PATH --dry-run\nreadiness prints a read-only projection; it never replaces the historical queue."
        );
        return Ok(());
    }
    if operation == "--version" {
        println!("tos-open-work-queue {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if !["build", "check", "validate", "readiness"].contains(&operation.as_str()) {
        return Err("unknown queue operation".into());
    }
    let mut options = BTreeMap::new();
    let mut dry = false;
    while let Some(key) = args.next() {
        if key == "--dry-run" && operation == "build" && !dry {
            dry = true;
            continue;
        }
        if key != "--source-root" && !(key == "--readiness-plan" && operation == "readiness") {
            return Err(format!("unexpected option {key}"));
        }
        let value = args.next().ok_or_else(|| format!("missing {key} value"))?;
        if options.insert(key, value).is_some() {
            return Err("duplicate option".into());
        }
    }
    let root = PathBuf::from(
        options
            .get("--source-root")
            .ok_or("missing --source-root")?,
    );
    if operation == "validate" {
        open_work_queue::validate(&root, &CANCEL).map_err(|e| e.to_string())?;
    } else {
        let value = open_work_queue::build(
            &root,
            options.get("--readiness-plan").map(String::as_str),
            operation == "readiness",
            &CANCEL,
        )
        .map_err(|e| e.to_string())?;
        if operation == "readiness" || dry {
            let bytes = open_work_queue::render(&value).map_err(|e| e.to_string())?;
            std::io::stdout()
                .write_all(bytes.as_bytes())
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
        if operation == "check" {
            open_work_queue::check(&root, &value, &CANCEL).map_err(|e| e.to_string())?;
        } else {
            open_work_queue::write(&root, &value).map_err(|e| e.to_string())?;
        }
    }
    println!("[ok] reviewed queue {operation}; source references, ordering and receipt mechanics");
    Ok(())
}
fn main() {
    unsafe {
        libc::signal(libc::SIGINT, cancelled as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, cancelled as *const () as libc::sighandler_t);
    }
    if let Err(error) = run() {
        eprintln!("queue operation rejected: {error}");
        std::process::exit(1)
    }
}
