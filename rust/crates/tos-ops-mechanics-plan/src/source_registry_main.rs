use std::{
    collections::BTreeMap,
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicI32, Ordering},
};
use tos_ops_mechanics_plan::source_registry;
static CANCEL: AtomicI32 = AtomicI32::new(0);
extern "C" fn cancelled(signal: i32) {
    CANCEL.store(signal, Ordering::Relaxed);
}
fn main() {
    unsafe {
        libc::signal(libc::SIGINT, cancelled as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, cancelled as *const () as libc::sighandler_t);
    }
    let result = (|| -> Result<(), String> {
        let mut args = std::env::args().skip(1);
        let operation = args
            .next()
            .ok_or("expected normalize, check, validate or inspect")?;
        if operation == "--help" {
            println!(
                "usage: tos-source-registry normalize|check|validate --source-root PATH --packet-root PATH [--input-root PATH]\n       tos-source-registry inspect --packet-root PATH --corpus ID --document ID [--record ID]\ncheck reproduces the recorded snapshot with current Rust rules, preserving its recorded producer provenance."
            );
            return Ok(());
        }
        let allowed: &[&str] = match operation.as_str() {
            "normalize" => &["--source-root", "--packet-root", "--input-root"],
            "check" | "validate" => &["--source-root", "--packet-root"],
            "inspect" => &["--packet-root", "--corpus", "--document", "--record"],
            _ => return Err("unknown registry operation".into()),
        };
        let mut options = BTreeMap::new();
        while let Some(key) = args.next() {
            if !allowed.contains(&key.as_str()) {
                return Err(format!("unexpected option {key}"));
            }
            let value = args
                .next()
                .ok_or_else(|| format!("missing value for {key}"))?;
            if options.insert(key, value).is_some() {
                return Err("duplicate option".into());
            }
        }
        let get = |key: &str| {
            options
                .get(key)
                .map(String::as_str)
                .ok_or_else(|| format!("missing {key}"))
        };
        let path = |key: &str| get(key).map(PathBuf::from);
        let packet = path("--packet-root")?;
        let value = match operation.as_str() {
            "normalize" | "check" => source_registry::normalize(
                &path("--source-root")?,
                &packet,
                options.get("--input-root").map(PathBuf::from).as_deref(),
                operation == "check",
                &CANCEL,
            ),
            "validate" => source_registry::validate(&path("--source-root")?, &packet, &CANCEL),
            "inspect" => source_registry::inspect(
                &packet,
                get("--corpus")?,
                get("--document")?,
                options.get("--record").map(String::as_str),
                &CANCEL,
            ),
            _ => unreachable!(),
        }
        .map_err(|e| e.to_string())?;
        let bytes = tos_compiler::source_registry::encoded(&value)?;
        std::io::stdout()
            .write_all(&bytes)
            .map_err(|e| e.to_string())?;
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("registry operation rejected: {error}");
        std::process::exit(1)
    }
}
