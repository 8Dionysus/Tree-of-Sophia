//! Explicit candidate for software CI selection. Python and CI remain active.
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
    "usage: tos-software-ci plan --repo-root ABSOLUTE_PATH --base REF [--full] | tos-software-ci gate | tos-software-ci {executor-manifest|executor-bind|software-receipts|software-limits} --repo-root ABSOLUTE_PATH [operation paths]"
}

fn github_output(path: &Path, selection: &software_ci::Selection) -> io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.append(true).create(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
        options.mode(0o600);
    }
    let mut stream = options.open(path)?;
    let meta = stream.metadata()?;
    if !meta.is_file() || meta.len() > 1024 * 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "GITHUB_OUTPUT must be a bounded regular file",
        ));
    }
    write!(
        stream,
        "software_mode={}\nworker={}\nrust={}\n",
        selection.software_mode, selection.worker, selection.rust
    )
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
