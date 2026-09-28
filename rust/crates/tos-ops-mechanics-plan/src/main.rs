use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;
use tos_ops_mechanics_plan::executor::Limits;

static CANCEL: AtomicI32 = AtomicI32::new(0);

#[derive(Clone, Copy)]
enum Action {
    Plan,
    Execute,
    ThresholdBuild { check: bool },
    ThresholdValidate,
    RelationPackValidate,
    QuestbookValidate,
    PublicMirrorValidate,
    PublicMirrorSync,
    DerivedKagValidate,
    DerivedKagGenerate,
}

#[cfg(target_os = "linux")]
extern "C" fn cancelled(signal: i32) {
    CANCEL.store(signal, Ordering::Relaxed);
}

fn arguments() -> Result<(PathBuf, String, Action, Limits), String> {
    let mut args = env::args().skip(1);
    let mut root = None;
    let mut python = "python".to_owned();
    let mut execute = false;
    let mut threshold_build = false;
    let mut threshold_validate = false;
    let mut relation_pack_validate = false;
    let mut questbook_validate = false;
    let mut public_mirror_validate = false;
    let mut public_mirror_sync = false;
    let mut derived_kag_validate = false;
    let mut derived_kag_generate = false;
    let mut check = false;
    let mut limits = Limits::default();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--execute" => execute = true,
            "--threshold-registry-build" => threshold_build = true,
            "--threshold-registry-validate" => threshold_validate = true,
            "--relation-pack-validate" => relation_pack_validate = true,
            "--questbook-validate" => questbook_validate = true,
            "--public-mirror-validate" => public_mirror_validate = true,
            "--public-mirror-sync" => public_mirror_sync = true,
            "--derived-kag-validate" => derived_kag_validate = true,
            "--derived-kag-generate" => derived_kag_generate = true,
            "--check" => check = true,
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
    if usize::from(execute)
        + usize::from(threshold_build)
        + usize::from(threshold_validate)
        + usize::from(relation_pack_validate)
        + usize::from(questbook_validate)
        + usize::from(public_mirror_validate)
        + usize::from(public_mirror_sync)
        + usize::from(derived_kag_validate)
        + usize::from(derived_kag_generate)
        > 1
        || (check && !threshold_build)
    {
        return Err("incompatible mechanics modes".into());
    }
    let action = if execute {
        Action::Execute
    } else if threshold_build {
        Action::ThresholdBuild { check }
    } else if threshold_validate {
        Action::ThresholdValidate
    } else if relation_pack_validate {
        Action::RelationPackValidate
    } else if questbook_validate {
        Action::QuestbookValidate
    } else if public_mirror_validate {
        Action::PublicMirrorValidate
    } else if public_mirror_sync {
        Action::PublicMirrorSync
    } else if derived_kag_validate {
        Action::DerivedKagValidate
    } else if derived_kag_generate {
        Action::DerivedKagGenerate
    } else {
        Action::Plan
    };
    Ok((
        root.ok_or("--repo-root is required")?,
        python,
        action,
        limits,
    ))
}

fn main() {
    let (root, python, action, limits) = arguments().unwrap_or_else(|error| {
        eprintln!("{error}\nusage: tos-ops-mechanics-plan --repo-root PATH [--python COMMAND] [--execute | --threshold-registry-build [--check] | --threshold-registry-validate | --relation-pack-validate | --questbook-validate | --public-mirror-validate | --public-mirror-sync | --derived-kag-validate | --derived-kag-generate] [--command-timeout-ms N] [--lane-timeout-ms N] [--cleanup-grace-ms N] [--max-output-bytes N]");
        std::process::exit(2);
    });
    let result = match action {
        Action::ThresholdBuild { check } => {
            tos_ops_mechanics_plan::threshold_registry::build(&root, check).map(|()| 0)
        }
        Action::ThresholdValidate => tos_ops_mechanics_plan::threshold_registry::validate(&root)
            .and_then(|report| {
                println!(
                    "{}",
                    serde_json::to_string(&report).map_err(std::io::Error::other)?
                );
                Ok(0)
            }),
        Action::RelationPackValidate => {
            let mut diagnostics = std::io::stderr().lock();
            let mut first_issue = true;
            tos_ops_mechanics_plan::relation_pack::validate(&root, |location, message| {
                if first_issue {
                    writeln!(diagnostics, "Tree relation-pack validation failed.")?;
                    first_issue = false;
                }
                writeln!(diagnostics, "- {location}: {message}")
            })
            .map(|valid| {
                if valid {
                    println!("[ok] validated route-local canonical relation pack");
                    println!("[ok] validated canonical relation predicates and endpoint classes against registries");
                    0
                } else {
                    1
                }
            })
        }
        Action::QuestbookValidate => tos_ops_mechanics_plan::questbook::validate_surface(&root)
            .map(|()| {
                println!("[ok] validated questbook boundary-runtime surfaces");
                0
            }),
        Action::PublicMirrorValidate => {
            tos_ops_mechanics_plan::public_mirror::validate(&root).map(|issues| {
                if issues.is_empty() {
                    println!("[ok] validated ToS/canon/example compatibility mirrors");
                    0
                } else {
                    eprintln!("Tree/example sync check failed.");
                    for (location, message) in issues {
                        eprintln!("- {location}: {message}");
                    }
                    1
                }
            })
        }
        Action::PublicMirrorSync => tos_ops_mechanics_plan::public_mirror::write_examples(&root)
            .map(|written| {
                for path in written {
                    println!("[ok] wrote {path}");
                }
                0
            }),
        Action::DerivedKagValidate => {
            tos_ops_mechanics_plan::derived_kag::validate(&root).map(|()| {
                println!("[ok] validated generated KAG export outputs are up to date");
                println!("[ok] validated generated KAG export structure");
                0
            })
        }
        Action::DerivedKagGenerate => tos_ops_mechanics_plan::derived_kag::write_outputs(&root)
            .map(|written| {
                for path in written {
                    println!("[ok] wrote {path}");
                }
                0
            }),
        Action::Plan | Action::Execute => tos_ops_mechanics_plan::discover(&root, &python)
            .and_then(|plan| {
                if matches!(action, Action::Plan) {
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
            }),
    };
    let code = match result {
        Ok(code) => code,
        Err(error) => {
            if matches!(
                action,
                Action::QuestbookValidate | Action::DerivedKagValidate
            ) {
                eprintln!("[error] {error}");
                std::process::exit(1);
            }
            let route = match action {
                Action::Execute => "execution",
                Action::Plan => "plan",
                Action::ThresholdBuild { .. } | Action::ThresholdValidate => "threshold registry",
                Action::RelationPackValidate => "relation pack",
                Action::QuestbookValidate => "questbook",
                Action::PublicMirrorValidate | Action::PublicMirrorSync => "public mirror",
                Action::DerivedKagValidate | Action::DerivedKagGenerate => "derived KAG",
            };
            let diagnostic = format!("mechanics-local {route}: {error}\n",);
            #[cfg(target_os = "linux")]
            if matches!(action, Action::Execute) {
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
