//! Native software CI selection and required-job gate.
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};
use tos_ops_mechanics_plan::software_ci;

static CANCEL: AtomicI32 = AtomicI32::new(0);
#[cfg(target_os = "linux")]
extern "C" fn cancelled(signal: i32) {
    CANCEL.store(signal, Ordering::Relaxed);
}

fn usage() -> &'static str {
    "usage: tos-software-ci plan --repo-root ABSOLUTE_PATH --base REF [--full] | tos-software-ci gate | tos-software-ci {executor-manifest|executor-bind|software-receipts|software-limits} --repo-root ABSOLUTE_PATH [operation paths] | tos-software-ci {verify-reader-install|verify-mechanics-install} --repo-root ABS [--installed-prefix ABS] [--command-entries-only] [--command-timeout-ms N --lane-timeout-ms N --max-output-bytes N]"
}

fn output_cancel_check() -> io::Result<()> {
    if CANCEL.load(Ordering::Relaxed) != 0 {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "CI output cancelled",
        ));
    }
    Ok(())
}

fn github_output(path: &Path, selection: &software_ci::Selection) -> io::Result<()> {
    output_cancel_check()?;
    let record = format!(
        "software_mode={}\nworker={}\nrust={}\n",
        selection.software_mode, selection.worker, selection.rust
    );
    let mut options = fs::OpenOptions::new();
    options.append(true).create(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Match the artifact-output boundary: opening a FIFO must not wait
        // for a reader before we can refuse a non-regular output.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        options.mode(0o600);
    }
    let mut stream = options.open(path)?;
    let meta = stream.metadata()?;
    if !meta.is_file()
        || meta
            .len()
            .checked_add(record.len() as u64)
            .is_none_or(|length| length > 1024 * 1024)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "GITHUB_OUTPUT must be a bounded regular file",
        ));
    }
    output_cancel_check()?;
    stream.write_all(record.as_bytes())?;
    stream.flush()?;
    output_cancel_check()
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let mode = args.next().ok_or_else(|| usage().to_owned())?;
    if mode == "--help" || mode == "-h" {
        println!("{}", usage());
        return Ok(());
    }
    if [
        "executor-manifest",
        "executor-bind",
        "software-receipts",
        "software-limits",
    ]
    .contains(&mode.as_str())
    {
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
        return tos_ops_mechanics_plan::ci_artifacts::run(
            &mode,
            &args.collect::<Vec<_>>(),
            &CANCEL,
        )
        .map_err(|e| e.to_string());
    }
    if ["verify-reader-install", "verify-mechanics-install"].contains(&mode.as_str()) {
        #[cfg(target_os = "linux")]
        {
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
            return tos_ops_mechanics_plan::ci_verification::run(
                &mode,
                &args.collect::<Vec<_>>(),
                &CANCEL,
            )
            .map_err(|e| e.to_string());
        }
        #[cfg(not(target_os = "linux"))]
        return Err("native verification requires Linux process custody".into());
    }
    if mode == "gate" {
        if args.next().is_some() {
            return Err(usage().into());
        }
        let raw = env::var("CI_NEEDS").map_err(|_| "CI_NEEDS is required")?;
        if raw.len() > 1024 * 1024 {
            return Err("CI_NEEDS byte budget exceeded".into());
        }
        let needs = serde_json::from_str(&raw).map_err(|e| format!("invalid CI_NEEDS: {e}"))?;
        software_ci::gate(&needs).map_err(|e| e.to_string())?;
        println!("All selected checks succeeded; unselected checks were skipped.");
        return Ok(());
    }
    if mode != "plan" {
        return Err(usage().into());
    }
    let mut root = None;
    let mut base = None;
    let mut full = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--repo-root" => root = Some(PathBuf::from(args.next().ok_or("missing repo root")?)),
            "--base" => base = Some(args.next().ok_or("missing base")?),
            "--full" => full = true,
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    let root = root.ok_or("--repo-root is required")?;
    let base = base.ok_or("--base is required")?;
    if base.is_empty() || base.starts_with('-') || base.len() > 4096 || base.contains('\0') {
        return Err("invalid base ref".into());
    }
    if !root.is_absolute() || fs::canonicalize(&root).map_err(|e| e.to_string())? != root {
        return Err("repository root must be absolute without symlinks".into());
    }
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
    let selection = software_ci::plan(&root, &base, full, &CANCEL).map_err(|e| e.to_string())?;
    // Python's json.dumps defaults to ensure_ascii=True. JSON syntax is ASCII;
    // escaping every non-ASCII scalar (and DEL) preserves literal semantics.
    let json = serde_json::to_string_pretty(&selection).map_err(|e| e.to_string())?;
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    for ch in json.chars() {
        if ch < '\u{7f}' {
            write!(stdout, "{ch}").map_err(|e| e.to_string())?;
        } else {
            for unit in ch.encode_utf16(&mut [0; 2]) {
                write!(stdout, "\\u{unit:04x}").map_err(|e| e.to_string())?;
            }
        }
    }
    writeln!(stdout).map_err(|e| e.to_string())?;
    stdout.flush().map_err(|e| e.to_string())?;
    drop(stdout);
    if let Some(path) = env::var_os("GITHUB_OUTPUT").filter(|p| !p.is_empty()) {
        github_output(Path::new(&path), &selection).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        let signal = CANCEL.load(Ordering::Relaxed);
        std::process::exit(if signal == 0 { 1 } else { 128 + signal });
    }
}

#[cfg(all(test, target_os = "linux"))]
mod output_tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static OUTPUT_TEST: Mutex<()> = Mutex::new(());
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = env::temp_dir().join(format!(
                "tos-ci-output-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            CANCEL.store(0, Ordering::Relaxed);
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn output_final_size_respects_existing_limit() {
        let _serial = OUTPUT_TEST.lock().unwrap_or_else(|p| p.into_inner());
        let root = Fixture::new();
        let selected = software_ci::select(Vec::new(), false).unwrap();
        let record = b"software_mode=full\nworker=true\nrust=true\n";
        let cap = 1024 * 1024;
        let output = root.0.join("output");
        fs::write(&output, b"prior=kept\n").unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&output)
            .unwrap()
            .set_len(cap - record.len() as u64)
            .unwrap();
        github_output(&output, &selected).unwrap();
        let exact = fs::read(&output).unwrap();
        assert_eq!(exact.len(), cap as usize);
        assert!(exact.starts_with(b"prior=kept\n"));
        assert!(exact.ends_with(record));
        for length in [cap - record.len() as u64 + 1, cap, cap + 1] {
            fs::OpenOptions::new()
                .write(true)
                .open(&output)
                .unwrap()
                .set_len(length)
                .unwrap();
            let before = fs::read(&output).unwrap();
            assert!(
                github_output(&output, &selected).is_err(),
                "final length {length}+record must refuse"
            );
            assert_eq!(
                fs::read(&output).unwrap(),
                before,
                "refusal must not append"
            );
        }
    }

    #[test]
    fn output_cancelled_after_selection_preserves_existing_and_absent_file() {
        let _serial = OUTPUT_TEST.lock().unwrap_or_else(|p| p.into_inner());
        let root = Fixture::new();
        let selected = software_ci::select(Vec::new(), false).unwrap();
        let output = root.0.join("output");
        fs::write(&output, b"prior=kept\n").unwrap();
        CANCEL.store(libc::SIGTERM, Ordering::Relaxed);
        let error = github_output(&output, &selected).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert_eq!(fs::read(&output).unwrap(), b"prior=kept\n");
        let absent = root.0.join("absent");
        assert_eq!(
            github_output(&absent, &selected).unwrap_err().kind(),
            io::ErrorKind::Interrupted
        );
        assert!(!absent.exists(), "cancelled output must not create a file");
    }
}
