use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicI32, Ordering},
};
use tos_compiler::source_registry as rules;
use tos_ops_mechanics_plan::{source_registry, source_registry_views as views};
static CANCEL: AtomicI32 = AtomicI32::new(0);
extern "C" fn cancelled(signal: i32) {
    CANCEL.store(signal, Ordering::Relaxed);
}
fn output(value: &serde_json::Value) -> Result<(), String> {
    std::io::stdout()
        .write_all(&rules::encoded(value)?)
        .map_err(|e| e.to_string())
}
fn run() -> Result<i32, String> {
    let mut args = std::env::args().skip(1);
    let operation = args
        .next()
        .ok_or("expected normalize, check, validate, inspect, reconcile or coverage")?;
    if operation == "--help" {
        println!(
            "usage: tos-source-registry normalize|check|validate --source-root PATH --packet-root PATH [--input-root PATH]\n       tos-source-registry inspect --packet-root PATH --corpus ID --document ID [--record ID]\n       tos-source-registry reconcile --source-root PATH --output-root PATH [--check]\n       tos-source-registry coverage --source-root PATH --output-root PATH [--check]\n       tos-source-registry coverage --source-root PATH [--verify-local|--remaining|--document ID]\ncheck reproduces the recorded snapshot with current Rust rules, preserving its recorded producer provenance."
        );
        return Ok(0);
    }
    if operation == "--version" {
        println!("tos-source-registry {}", env!("CARGO_PKG_VERSION"));
        return Ok(0);
    }
    let allowed: &[&str] = match operation.as_str() {
        "normalize" => &["--source-root", "--packet-root", "--input-root"],
        "check" | "validate" => &["--source-root", "--packet-root"],
        "inspect" => &["--packet-root", "--corpus", "--document", "--record"],
        "reconcile" => &["--source-root", "--output-root", "--check"],
        "coverage" => &[
            "--source-root",
            "--output-root",
            "--check",
            "--verify-local",
            "--remaining",
            "--document",
        ],
        _ => return Err("unknown registry operation".into()),
    };
    let mut options = BTreeMap::new();
    let mut flags = BTreeSet::new();
    while let Some(key) = args.next() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("unexpected option {key}"));
        }
        if ["--check", "--verify-local", "--remaining"].contains(&key.as_str()) {
            if !flags.insert(key) {
                return Err("duplicate flag".into());
            }
        } else {
            let value = args
                .next()
                .ok_or_else(|| format!("missing value for {key}"))?;
            if options.insert(key, value).is_some() {
                return Err("duplicate option".into());
            }
        }
    }
    let get = |key: &str| {
        options
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| format!("missing {key}"))
    };
    let path = |key: &str| get(key).map(PathBuf::from);
    if operation == "reconcile" || operation == "coverage" {
        let source = path("--source-root")?;
        let is_coverage = operation == "coverage";
        let live = flags.contains("--verify-local");
        let remaining = flags.contains("--remaining");
        let document = options.get("--document");
        let check = flags.contains("--check");
        let stdout = live || remaining || document.is_some();
        if check && stdout {
            return Err("--check cannot be combined with stdout selectors".into());
        }
        let output_root = if stdout {
            None
        } else {
            Some(path("--output-root")?)
        };
        let value = if is_coverage {
            views::coverage(&source, live, &CANCEL)
        } else {
            views::reconciliation(&source, &CANCEL)
        }
        .map_err(|e| e.to_string())?;
        if live {
            output(
                &serde_json::json!({"observed_at":tos_ops_mechanics_plan::kag_downstream_status::observation_time().map_err(|e|e.to_string())?,"custody_scope":value["custody_scope"],"targets":value["targets"]}),
            )?;
            return Ok(i32::from(
                value["targets"]
                    .as_array()
                    .ok_or("missing targets")?
                    .iter()
                    .any(|t| t["local_now"]["state"] != "verified"),
            ));
        }
        if stdout {
            for r in value["records"].as_array().ok_or("missing records")? {
                if r["kind"] == "registry"
                    && document.is_none_or(|d| r["document_id"] == *d)
                    && (!remaining || r["status"] != "selected_versions_planted")
                {
                    output(r)?
                }
            }
            return Ok(0);
        }
        views::publish_view(
            &source,
            output_root.as_deref().unwrap(),
            &value,
            is_coverage,
            check,
            &CANCEL,
        )
        .map_err(|e| e.to_string())?;
        output(&value["summary"])?;
        return Ok(0);
    }
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
    output(&value)?;
    Ok(0)
}
fn main() {
    unsafe {
        libc::signal(libc::SIGINT, cancelled as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, cancelled as *const () as libc::sighandler_t);
    }
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("registry operation rejected: {error}");
            std::process::exit(1)
        }
    }
}
