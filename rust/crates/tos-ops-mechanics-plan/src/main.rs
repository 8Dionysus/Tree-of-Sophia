use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;
use tos_ops_mechanics_plan::executor::Limits;

static CANCEL: AtomicI32 = AtomicI32::new(0);

#[cfg(target_os = "linux")]
extern "C" fn cancelled(signal: i32) {
    CANCEL.store(signal, Ordering::Relaxed);
}

fn arguments() -> Result<(PathBuf, String, bool, Limits), String> {
    let mut args = env::args().skip(1);
    let mut root = None;
    let mut python = "python".to_owned();
    let mut execute = false;
    let mut limits = Limits::default();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--execute" => execute = true,
            "--repo-root" => root = Some(PathBuf::from(args.next().ok_or("missing repo root")?)),
            "--python" => python = args.next().ok_or("missing Python adapter")?,
            "--command-timeout-ms"
            | "--lane-timeout-ms"
            | "--cleanup-grace-ms"
            | "--max-output-bytes" => {
                let value: u64 = args
                    .next()
                    .ok_or("missing limit")?
                    .parse()
                    .map_err(|_| "invalid numeric limit")?;
                match argument.as_str() {
                    "--command-timeout-ms" => limits.command_wall = Duration::from_millis(value),
                    "--lane-timeout-ms" => limits.lane_wall = Duration::from_millis(value),
                    "--cleanup-grace-ms" => limits.cleanup_grace = Duration::from_millis(value),
                    _ => {
                        limits.output_bytes =
                            usize::try_from(value).map_err(|_| "output limit overflow")?
                    }
                }
            }
            _ => return Err(format!("unknown argument: {argument}")),
        }
    }
    Ok((
        root.ok_or("--repo-root is required")?,
        python,
        execute,
        limits,
    ))
}

fn main() {
    let (root, python, execute, limits) = arguments().unwrap_or_else(|error| {
        eprintln!("{error}\nusage: tos-ops-mechanics-plan --repo-root PATH [--python COMMAND] [--execute] [--command-timeout-ms N] [--lane-timeout-ms N] [--cleanup-grace-ms N] [--max-output-bytes N]");
        std::process::exit(2);
    });
    let result = tos_ops_mechanics_plan::discover(&root, &python).and_then(|plan| {
        if !execute {
            let output = serde_json::to_string(&plan).map_err(std::io::Error::other)?;
            println!("{output}");
            return Ok(0);
        }
        #[cfg(target_os = "linux")]
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = cancelled as *const () as usize;
            libc::sigemptyset(&mut action.sa_mask);
            for signal in [libc::SIGINT, libc::SIGTERM] {
                if libc::sigaction(signal, &action, std::ptr::null_mut()) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
        }
        tos_ops_mechanics_plan::executor::run(&root, &plan, limits, &CANCEL)
    });
    let code = match result {
        Ok(code) => code,
        Err(error) => {
            let diagnostic = format!(
                "mechanics-local {}: {error}\n",
                if execute { "execution" } else { "plan" }
            );
            #[cfg(target_os = "linux")]
            if execute {
                // A stalled diagnostic sink must not undo bounded execution.
                unsafe {
                    let flags = libc::fcntl(2, libc::F_GETFL);
                    if flags >= 0 && libc::fcntl(2, libc::F_SETFL, flags | libc::O_NONBLOCK) == 0 {
                        libc::write(2, diagnostic.as_ptr().cast(), diagnostic.len());
                        libc::fcntl(2, libc::F_SETFL, flags);
                    }
                }
            } else {
                eprint!("{diagnostic}");
            }
            #[cfg(not(target_os = "linux"))]
            eprint!("{diagnostic}");
            let signal = CANCEL.load(Ordering::Relaxed);
            if signal == 0 { 1 } else { 128 + signal }
        }
    };
    std::process::exit(code);
}
