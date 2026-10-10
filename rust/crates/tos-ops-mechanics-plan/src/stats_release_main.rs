use std::{
    collections::BTreeMap,
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicI32, Ordering},
};
use tos_ops_mechanics_plan::{kag_release, stats_release};
static CANCEL: AtomicI32 = AtomicI32::new(0);
#[cfg(target_os = "linux")]
extern "C" fn cancelled(signal: i32) {
    CANCEL.store(signal, Ordering::Relaxed);
    kag_release::cancel(signal);
}
fn main() {
    #[cfg(target_os = "linux")]
    unsafe {
        libc::signal(libc::SIGINT, cancelled as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, cancelled as *const () as libc::sighandler_t);
    }
    let result = (|| {
        let mut args = std::env::args().skip(1);
        let operation = args
            .next()
            .ok_or("expected build, status, verify or validate")?;
        if operation == "--help" {
            println!(
                "usage: tos-stats-release build --source-root PATH --stats-root PATH --release-root PATH --python ABSOLUTE_INTERPRETER\n       tos-stats-release status --release-root PATH --expected-revision SHA256\n       tos-stats-release verify --release PATH\n       tos-stats-release validate --stats-root PATH --port PATH --python ABSOLUTE_INTERPRETER"
            );
            return Ok(());
        }
        let allowed: &[&str] = match operation.as_str() {
            "build" => &[
                "--source-root",
                "--stats-root",
                "--release-root",
                "--python",
            ],
            "status" => &["--release-root", "--expected-revision"],
            "verify" => &["--release"],
            "validate" => &["--stats-root", "--port", "--python"],
            _ => return Err("expected build, status, verify or validate".to_owned()),
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
                return Err("duplicate option".to_owned());
            }
        }
        let get = |key: &str| {
            options
                .get(key)
                .map(String::as_str)
                .ok_or_else(|| format!("missing {key}"))
        };
        let path = |key: &str| get(key).map(PathBuf::from);
        let value = match operation.as_str() {
            "build" => {
                let manifest = stats_release::build_release(
                    &path("--source-root")?,
                    &path("--stats-root")?,
                    &path("--release-root")?,
                    &path("--python")?,
                    &CANCEL,
                )
                .map_err(|e| e.to_string())?;
                serde_json::json!({"source_kind":stats_release::SOURCE_KIND,
                    "source_revision":manifest["source_revision"],"integration_revision":manifest["integration_revision"]})
            }
            "status" => {
                stats_release::status_release(&path("--release-root")?, get("--expected-revision")?)
                    .map_err(|e| e.to_string())?
            }
            "verify" => {
                stats_release::verify_integration(&path("--release")?).map_err(|e| e.to_string())?
            }
            "validate" => {
                let (code, out, err) = stats_release::validate_port(
                    &path("--stats-root")?,
                    &path("--port")?,
                    &path("--python")?,
                    &CANCEL,
                )
                .map_err(|e| e.to_string())?;
                std::io::stdout()
                    .write_all(&out)
                    .map_err(|e| e.to_string())?;
                std::io::stderr()
                    .write_all(&err)
                    .map_err(|e| e.to_string())?;
                if code != 0 {
                    std::process::exit(if (1..=255).contains(&code) { code } else { 1 });
                }
                return Ok(());
            }
            _ => unreachable!(),
        };
        std::io::stdout()
            .write_all(&kag_release::canonical(&value).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    })();
    if let Err(error) = result {
        eprintln!("stats release rejected: {error}");
        std::process::exit(1);
    }
}
