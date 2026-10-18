use std::{collections::BTreeMap, io::Write, path::PathBuf};
use tos_ops_mechanics_plan::{release_files, stats_release};
#[cfg(target_os = "linux")]
extern "C" fn cancelled(signal: i32) {
    release_files::cancel(signal);
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
                "usage: tos-stats-release build --source-root PATH --release-root PATH\n       tos-stats-release status --release-root PATH --expected-revision SHA256\n       tos-stats-release verify --release PATH\n       tos-stats-release validate --port PATH"
            );
            return Ok(());
        }
        let allowed: &[&str] = match operation.as_str() {
            "build" => &["--source-root", "--release-root"],
            "status" => &["--release-root", "--expected-revision"],
            "verify" => &["--release"],
            "validate" => &["--port"],
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
                let manifest =
                    stats_release::build_release(&path("--source-root")?, &path("--release-root")?)
                        .map_err(|e| e.to_string())?;
                serde_json::json!({"source_kind":stats_release::SOURCE_KIND,
                    "source_revision":manifest["source_revision"],"export_revision":manifest["export_revision"]})
            }
            "status" => {
                stats_release::status_release(&path("--release-root")?, get("--expected-revision")?)
                    .map_err(|e| e.to_string())?
            }
            "verify" => {
                stats_release::verify_export(&path("--release")?).map_err(|e| e.to_string())?
            }
            "validate" => {
                stats_release::validate_port(&path("--port")?).map_err(|e| e.to_string())?
            }
            _ => unreachable!(),
        };
        std::io::stdout()
            .write_all(&release_files::canonical(&value).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    })();
    if let Err(error) = result {
        eprintln!("stats release rejected: {error}");
        std::process::exit(1);
    }
}
