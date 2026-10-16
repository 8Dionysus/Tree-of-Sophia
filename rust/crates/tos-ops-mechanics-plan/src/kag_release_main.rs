use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, Ordering};
use tos_ops_mechanics_plan::kag_release;
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
        let mut options = std::collections::BTreeMap::new();
        let mut operation = None;
        while let Some(arg) = args.next() {
            if matches!(
                arg.as_str(),
                "build" | "status" | "verify" | "export-build" | "export-verify"
            ) {
                if operation.replace(arg).is_some() {
                    return Err("duplicate operation".to_owned());
                }
            } else {
                if !matches!(
                    arg.as_str(),
                    "--repo-root"
                        | "--python"
                        | "--store"
                        | "--revision"
                        | "--kag-root"
                        | "--release-root"
                        | "--release"
                        | "--expected-revision"
                        | "--output"
                ) {
                    return Err(format!("unknown argument: {arg}"));
                }
                let value = args.next().ok_or_else(|| format!("missing value: {arg}"))?;
                if options.insert(arg, value).is_some() {
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
        let allowed: &[&str] = match operation.as_deref() {
            Some("build") => &[
                "--repo-root",
                "--python",
                "--store",
                "--revision",
                "--kag-root",
                "--release-root",
            ],
            Some("status") => &[
                "--repo-root",
                "--python",
                "--release-root",
                "--expected-revision",
            ],
            Some("verify") => &[
                "--repo-root",
                "--python",
                "--release",
                "--expected-revision",
            ],
            Some("export-build") => &[
                "--repo-root",
                "--python",
                "--store",
                "--revision",
                "--output",
            ],
            Some("export-verify") => &["--repo-root", "--python", "--release"],
            _ => return Err("expected KAG operation".into()),
        };
        if options.keys().any(|key| !allowed.contains(&key.as_str())) {
            return Err("option is not applicable to selected operation".into());
        }
        let value = match operation.as_deref() {
            Some("build") => kag_release::build_release(
                &path("--repo-root")?,
                &path("--store")?,
                get("--revision")?,
                &path("--kag-root")?,
                &path("--release-root")?,
                &path("--python")?,
                &CANCEL,
            ),
            Some("status") => {
                kag_release::status_release(&path("--release-root")?, get("--expected-revision")?)
            }
            Some("verify") => {
                kag_release::verify_integration(&path("--release")?, get("--expected-revision")?)
            }
            Some("export-build") => tos_ops_mechanics_plan::kag_corpus_export::build_export(
                &path("--repo-root")?,
                &path("--store")?,
                get("--revision")?,
                &path("--output")?,
            ),
            Some("export-verify") => {
                tos_ops_mechanics_plan::kag_corpus_export::verify_export(&path("--release")?)
            }
            _ => {
                return Err("expected build, status, verify, export-build or export-verify".into());
            }
        }
        .map_err(|e| e.to_string())?;
        let raw = kag_release::canonical(&value).map_err(|e| e.to_string())?;
        use std::io::Write;
        std::io::stdout().write_all(&raw).map_err(|e| e.to_string())
    })();
    if let Err(error) = result {
        eprintln!("KAG release rejected: {error}");
        std::process::exit(1);
    }
}
