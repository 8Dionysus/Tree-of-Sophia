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
    MechanicsTopologyValidate,
    ActiveNamingValidate,
    SourceHome,
    PhilosophyTopology,
    SemanticRegistryTransition,
}

#[derive(Default)]
struct SemanticOptions {
    baseline_commit: Option<String>,
    allow_initial_introduction: bool,
    json_output: bool,
}

#[cfg(target_os = "linux")]
extern "C" fn cancelled(signal: i32) {
    CANCEL.store(signal, Ordering::Relaxed);
}

fn arguments() -> Result<(PathBuf, String, Action, Limits, SemanticOptions), String> {
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
    let mut mechanics_topology_validate = false;
    let mut active_naming_validate = false;
    let mut source_home = false;
    let mut philosophy_topology = false;
    let mut semantic_registry_transition = false;
    let mut semantic = SemanticOptions::default();
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
            "--mechanics-topology-validate" => mechanics_topology_validate = true,
            "--active-naming-validate" => active_naming_validate = true,
            "--source-home" => source_home = true,
            "--philosophy-topology" => philosophy_topology = true,
            "--semantic-registry-transition" => semantic_registry_transition = true,
            "--baseline-commit" => {
                let baseline = args.next().ok_or("missing baseline commit")?;
                if baseline.starts_with('-') || baseline.len() > 4096 {
                    return Err("baseline commit must be a bounded Git ref, not an option".into());
                }
                semantic.baseline_commit = Some(baseline)
            }
            "--allow-initial-introduction" => semantic.allow_initial_introduction = true,
            "--json" => semantic.json_output = true,
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
        + usize::from(mechanics_topology_validate)
        + usize::from(active_naming_validate)
        + usize::from(source_home)
        + usize::from(philosophy_topology)
        + usize::from(semantic_registry_transition)
        > 1
        || (check && !threshold_build)
        || (!semantic_registry_transition
            && (semantic.baseline_commit.is_some()
                || semantic.allow_initial_introduction
                || semantic.json_output))
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
    } else if active_naming_validate {
        Action::ActiveNamingValidate
    } else if semantic_registry_transition {
        Action::SemanticRegistryTransition
    } else if philosophy_topology {
        Action::PhilosophyTopology
    } else if source_home {
        Action::SourceHome
    } else if mechanics_topology_validate {
        Action::MechanicsTopologyValidate
    } else {
        Action::Plan
    };
    Ok((
        root.ok_or("--repo-root is required")?,
        python,
        action,
        limits,
        semantic,
    ))
}

fn main() {
    let (root, python, action, limits, semantic) = arguments().unwrap_or_else(|error| {
        eprintln!("{error}\nusage: tos-ops-mechanics-plan --repo-root PATH [--python COMMAND] [--execute | --threshold-registry-build [--check] | --threshold-registry-validate | --relation-pack-validate | --questbook-validate | --public-mirror-validate | --public-mirror-sync | --derived-kag-validate | --derived-kag-generate | --mechanics-topology-validate | --active-naming-validate | --source-home | --semantic-registry-transition [--baseline-commit REF] [--allow-initial-introduction] [--json]] [--command-timeout-ms N] [--lane-timeout-ms N] [--cleanup-grace-ms N] [--max-output-bytes N]");
        std::process::exit(2);
    });
    let result = match action {
        Action::SourceHome => {
            #[cfg(target_os = "linux")]
            unsafe {
                let mut handler: libc::sigaction = std::mem::zeroed();
                handler.sa_sigaction = cancelled as *const () as usize;
                libc::sigemptyset(&mut handler.sa_mask);
                for signal in [libc::SIGINT, libc::SIGTERM] {
                    if libc::sigaction(signal, &handler, std::ptr::null_mut()) != 0 {
                        eprintln!("[error] {}", std::io::Error::last_os_error());
                        std::process::exit(1);
                    }
                }
            }
            root.canonicalize()
                .and_then(|root| tos_ops_mechanics_plan::source_home::run(&root, &CANCEL))
        }

        Action::SemanticRegistryTransition => {
            #[cfg(target_os = "linux")]
            unsafe {
                let mut handler: libc::sigaction = std::mem::zeroed();
                handler.sa_sigaction = cancelled as *const () as usize;
                libc::sigemptyset(&mut handler.sa_mask);
                for signal in [libc::SIGINT, libc::SIGTERM] {
                    if libc::sigaction(signal, &handler, std::ptr::null_mut()) != 0 {
                        eprintln!(
                            "[error] semantic registry transition: {}",
                            std::io::Error::last_os_error()
                        );
                        std::process::exit(1);
                    }
                }
            }
            root.canonicalize().and_then(|root| {
                tos_ops_mechanics_plan::semantic_registry_transition::run(
                    &root,
                    semantic.baseline_commit.as_deref(),
                    semantic.allow_initial_introduction,
                    semantic.json_output,
                    limits,
                    &CANCEL,
                )
            })
        }
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
        Action::ActiveNamingValidate => {
            tos_ops_mechanics_plan::active_naming::validate(&root).map(|issues| {
                if issues.is_empty() {
                    println!("[ok] validated active naming");
                    0
                } else {
                    eprintln!("Active naming validation failed.");
                    for issue in issues {
                        eprintln!("- {issue}");
                    }
                    1
                }
            })
        }
        Action::PhilosophyTopology => {
            tos_ops_mechanics_plan::philosophy_topology::run(&root, &CANCEL)
        }
        Action::MechanicsTopologyValidate => {
            tos_ops_mechanics_plan::mechanics_topology::validate(&root).map(|issues| {
                if issues.is_empty() {
                    println!("[ok] validated ToS mechanics topology");
                    0
                } else {
                    eprintln!("Mechanics topology validation failed.");
                    for (location, message) in issues {
                        eprintln!("- {location}: {message}");
                    }
                    1
                }
            })
        }
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
                Action::QuestbookValidate | Action::DerivedKagValidate | Action::SourceHome
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
                Action::MechanicsTopologyValidate => "mechanics topology",
                Action::ActiveNamingValidate => "active naming",
                Action::SourceHome => "source home",
                Action::SemanticRegistryTransition => "semantic registry transition",
            };
            let diagnostic = if matches!(action, Action::SemanticRegistryTransition) {
                format!("[error] semantic registry transition: {error}\n")
            } else {
                format!("mechanics-local {route}: {error}\n")
            };
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
            if signal == 0 {
                1
            } else {
                128 + signal
            }
        }
    };
    std::process::exit(code);
}
