use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tos_ops_mechanics_plan::executor::Limits;

static CANCEL: AtomicI32 = AtomicI32::new(0);
static PRODUCT_CANCEL: OnceLock<Arc<AtomicBool>> = OnceLock::new();

#[derive(Clone)]
enum Action {
    Plan,
    GrowthNativePlan,
    Execute {
        native_contracts_only: bool,
        growth_python_oracle: bool,
    },
    LocalContracts {
        home: String,
    },
    ThresholdBuild {
        check: bool,
    },
    ThresholdValidate,
    RelationPackValidate,
    QuestbookValidate,
    PublicMirrorValidate,
    PublicMirrorSync,
    DerivedKagValidate,
    DerivedKagGenerate,
    MechanicsTopologyValidate,
    ActiveNamingValidate,
    AgentSurfaceBuild {
        check: bool,
    },
    AgentSurfaceValidate {
        fetch_budget_bases: bool,
    },
    AgentsRouteCurrentnessBuild {
        check: bool,
    },
    NestedAgentsValidate,
    AgentsRouteHarnessCheck,
    TinyEntryValidate,
    TreeNodeValidate,
    LivedWitnessValidate,
    IntakePackValidate,
    DocumentationFamilyBuild {
        check: bool,
    },
    DocumentationCrossCorpusValidate,
    DecisionRecordsValidate,
    DecisionIndexBuild {
        check: bool,
    },
    RootEntryMapBuild {
        check: bool,
    },
    RootEntryMapValidate,
    KagSourceExportBuild,
    KagSourceExportVerify,
    SourceHome,
    PhilosophyTopology,
    SemanticRegistryTransition,
    #[cfg(feature = "compiler-backed-validators")]
    PhilosophyGraphViews,
}

#[derive(Default)]
struct SemanticOptions {
    selected_interpreter: Option<PathBuf>,
    kag_export: Option<PathBuf>,
    source_store: Option<PathBuf>,
    source_revision: Option<String>,
    output: Option<PathBuf>,
    baseline_commit: Option<String>,
    allow_initial_introduction: bool,
    json_output: bool,
}

#[cfg(target_os = "linux")]
extern "C" fn cancelled(signal: i32) {
    CANCEL.store(signal, Ordering::Relaxed);
    tos_ops_mechanics_plan::kag_release::cancel(signal);
    if let Some(flag) = PRODUCT_CANCEL.get() {
        flag.store(true, Ordering::Relaxed);
    }
}

fn arguments() -> Result<(PathBuf, String, Action, Limits, SemanticOptions), String> {
    let mut args = env::args().skip(1);
    let mut root = None;
    let mut python = "python".to_owned();
    let mut execute = false;
    let mut native_contracts_only = false;
    let mut growth_python_oracle = false;
    let mut growth_native_plan = false;
    let mut local_contracts = None;
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
    let mut agent_surface_build = false;
    let mut agent_surface_validate = false;
    let mut agents_route_currentness_build = false;
    let mut nested_agents_validate = false;
    let mut agents_route_harness_check = false;
    let mut tiny_entry_validate = false;
    let mut tree_node_validate = false;
    let mut lived_witness_validate = false;
    let mut intake_pack_validate = false;
    let mut documentation_family_build = false;
    let mut documentation_cross_corpus_validate = false;
    let mut root_entry_map_build = false;
    let mut root_entry_map_validate = false;
    let mut kag_source_export_build = false;
    let mut kag_source_export_verify = false;
    let mut decision_records_validate = false;
    let mut decision_index_build = false;
    let mut fetch_budget_bases = false;
    #[cfg(feature = "compiler-backed-validators")]
    let mut philosophy_graph_views = false;
    #[cfg(not(feature = "compiler-backed-validators"))]
    let philosophy_graph_views = false;
    let mut semantic = SemanticOptions::default();
    let mut check = false;
    let mut limits = Limits::default();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--execute" => execute = true,
            "--native-contracts-only" => native_contracts_only = true,
            "--growth-python-oracle" => growth_python_oracle = true,
            "--growth-native-plan" => growth_native_plan = true,
            "--local-contracts" => {
                local_contracts = Some(args.next().ok_or("missing native assertion home")?)
            }
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
            "--agent-surface-build" => agent_surface_build = true,
            "--agent-surface-validate" => agent_surface_validate = true,
            "--agents-route-currentness-build" => agents_route_currentness_build = true,
            "--nested-agents-validate" => nested_agents_validate = true,
            "--agents-route-harness-check" => agents_route_harness_check = true,
            "--tiny-entry-validate" => tiny_entry_validate = true,
            "--tree-node-validate" => tree_node_validate = true,
            "--lived-witness-validate" => lived_witness_validate = true,
            "--intake-pack-validate" => intake_pack_validate = true,
            "--documentation-family-build" => documentation_family_build = true,
            "--documentation-cross-corpus-validate" => documentation_cross_corpus_validate = true,
            "--root-entry-map-build" => root_entry_map_build = true,
            "--root-entry-map-validate" => root_entry_map_validate = true,
            "--kag-source-export-build" => kag_source_export_build = true,
            "--kag-source-export-verify" => kag_source_export_verify = true,
            "--store" => {
                semantic.source_store =
                    Some(PathBuf::from(args.next().ok_or("missing source store")?))
            }
            "--revision" => {
                semantic.source_revision = Some(args.next().ok_or("missing corpus revision")?)
            }
            "--kag-export" => {
                semantic.kag_export =
                    Some(PathBuf::from(args.next().ok_or("missing KAG export path")?))
            }
            "--decision-records-validate" => decision_records_validate = true,
            "--decision-index-build" => decision_index_build = true,
            "--fetch-budget-bases" => fetch_budget_bases = true,
            "--output" => {
                semantic.output = Some(PathBuf::from(args.next().ok_or("missing output path")?))
            }
            #[cfg(feature = "compiler-backed-validators")]
            "--philosophy-graph-views-validate" => philosophy_graph_views = true,
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
            "--python" => {
                python = args.next().ok_or("missing Python adapter")?;
                semantic.selected_interpreter = Some(PathBuf::from(&python));
            }
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
    if usize::from(local_contracts.is_some())
        + usize::from(execute)
        + usize::from(growth_native_plan)
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
        + usize::from(agent_surface_build)
        + usize::from(agent_surface_validate)
        + usize::from(agents_route_currentness_build)
        + usize::from(nested_agents_validate)
        + usize::from(agents_route_harness_check)
        + usize::from(tiny_entry_validate)
        + usize::from(tree_node_validate)
        + usize::from(lived_witness_validate)
        + usize::from(intake_pack_validate)
        + usize::from(documentation_family_build)
        + usize::from(documentation_cross_corpus_validate)
        + usize::from(root_entry_map_build)
        + usize::from(root_entry_map_validate)
        + usize::from(kag_source_export_build)
        + usize::from(kag_source_export_verify)
        + usize::from(decision_records_validate)
        + usize::from(decision_index_build)
        + usize::from(philosophy_graph_views)
        > 1
        || (check
            && !(threshold_build
                || agent_surface_build
                || documentation_family_build
                || decision_index_build
                || root_entry_map_build
                || agents_route_currentness_build))
        || (fetch_budget_bases && !agent_surface_validate)
        || (semantic.output.is_some()
            && !(documentation_family_build
                || agents_route_currentness_build
                || kag_source_export_build))
        || (!kag_source_export_build
            && (semantic.source_store.is_some() || semantic.source_revision.is_some()))
        || (kag_source_export_build
            && (semantic.source_store.is_none()
                || semantic.source_revision.is_none()
                || semantic.output.is_none()))
        || (kag_source_export_verify && semantic.kag_export.is_none())
        || (semantic.kag_export.is_some()
            && !(documentation_cross_corpus_validate
                || root_entry_map_build
                || root_entry_map_validate
                || kag_source_export_verify))
        || (native_contracts_only && !execute)
        || (growth_python_oracle && !execute)
        || (native_contracts_only && growth_python_oracle)
        || (!semantic_registry_transition
            && (semantic.baseline_commit.is_some()
                || semantic.allow_initial_introduction
                || semantic.json_output))
    {
        return Err("incompatible mechanics modes".into());
    }
    let action = if let Some(home) = local_contracts {
        Action::LocalContracts { home }
    } else if growth_native_plan {
        Action::GrowthNativePlan
    } else if execute {
        Action::Execute {
            native_contracts_only,
            growth_python_oracle,
        }
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
    } else if philosophy_graph_views {
        #[cfg(feature = "compiler-backed-validators")]
        {
            Action::PhilosophyGraphViews
        }
        #[cfg(not(feature = "compiler-backed-validators"))]
        {
            return Err("compiler-backed validators are unavailable in this build".into());
        }
    } else if philosophy_topology {
        Action::PhilosophyTopology
    } else if agent_surface_build {
        Action::AgentSurfaceBuild { check }
    } else if agent_surface_validate {
        Action::AgentSurfaceValidate { fetch_budget_bases }
    } else if agents_route_currentness_build {
        Action::AgentsRouteCurrentnessBuild { check }
    } else if agents_route_harness_check {
        Action::AgentsRouteHarnessCheck
    } else if tree_node_validate {
        Action::TreeNodeValidate
    } else if lived_witness_validate {
        Action::LivedWitnessValidate
    } else if intake_pack_validate {
        Action::IntakePackValidate
    } else if tiny_entry_validate {
        Action::TinyEntryValidate
    } else if nested_agents_validate {
        Action::NestedAgentsValidate
    } else if documentation_family_build {
        Action::DocumentationFamilyBuild { check }
    } else if root_entry_map_build {
        Action::RootEntryMapBuild { check }
    } else if kag_source_export_build {
        Action::KagSourceExportBuild
    } else if kag_source_export_verify {
        Action::KagSourceExportVerify
    } else if root_entry_map_validate {
        Action::RootEntryMapValidate
    } else if documentation_cross_corpus_validate {
        Action::DocumentationCrossCorpusValidate
    } else if decision_records_validate {
        Action::DecisionRecordsValidate
    } else if decision_index_build {
        Action::DecisionIndexBuild { check }
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
    let product_started = std::time::Instant::now();
    let product_entry = env::args().nth(1);
    if matches!(
        product_entry.as_deref(),
        Some(
            "--philosophy-product"
                | "--philosophy-product-worker"
                | "--prepared-dossier"
                | "--prepared-dossier-worker"
        )
    ) {
        #[cfg(feature = "compiler-backed-validators")]
        {
            let flag = Arc::new(AtomicBool::new(false));
            PRODUCT_CANCEL
                .set(flag.clone())
                .expect("product cancellation initialized once");
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
            let mut args: Vec<String> = env::args().skip(1).collect();
            let prepared_dossier = matches!(
                product_entry.as_deref(),
                Some("--prepared-dossier" | "--prepared-dossier-worker")
            );
            if matches!(
                product_entry.as_deref(),
                Some("--philosophy-product" | "--prepared-dossier")
            ) {
                let result = if prepared_dossier {
                    {
                        #[cfg(target_os = "linux")]
                        {
                            tos_ops_mechanics_plan::prepared_dossier_entry::run_supervised(
                                &args,
                                &CANCEL,
                                product_started,
                            )
                        }
                        #[cfg(not(target_os = "linux"))]
                        {
                            Err(
                                "prepared dossiers require native Linux selected-directory custody"
                                    .into(),
                            )
                        }
                    }
                } else {
                    tos_ops_mechanics_plan::philosophy_products::run_supervised(
                        &args,
                        &CANCEL,
                        product_started,
                    )
                };
                match result {
                    Ok(code) => {
                        if code != 0 {
                            std::process::exit(code);
                        }
                    }
                    Err(_error) => {
                        // The custody scope restored stderr. A blocking
                        // diagnostic here could escape the original deadline.
                        // Worker diagnostics were already forwarded within it.
                        let signal = CANCEL.load(Ordering::Relaxed);
                        std::process::exit(if signal == 0 { 1 } else { 128 + signal });
                    }
                }
                return;
            }
            // Internal subprocess of the supervised entry; library and WASM
            // callers remain cooperative and do not inherit process custody.
            args[0] = if prepared_dossier {
                "--prepared-dossier"
            } else {
                "--philosophy-product"
            }
            .into();
            let result = if prepared_dossier {
                {
                    #[cfg(target_os = "linux")]
                    {
                        tos_ops_mechanics_plan::prepared_dossier_entry::run(&args, flag)
                    }
                    #[cfg(not(target_os = "linux"))]
                    {
                        Err(
                            "prepared dossiers require native Linux selected-directory custody"
                                .into(),
                        )
                    }
                }
            } else {
                tos_ops_mechanics_plan::philosophy_products::run(&args, flag)
            };
            match result {
                Ok(report) => {
                    if prepared_dossier {
                        #[cfg(target_os = "linux")]
                        let printed =
                            tos_ops_mechanics_plan::prepared_dossier_entry::report_bytes(&report)
                                .and_then(|bytes| {
                                    std::io::stdout()
                                        .lock()
                                        .write_all(&bytes)
                                        .map_err(|error| error.to_string())
                                });
                        #[cfg(not(target_os = "linux"))]
                        let printed: Result<(), String> = Err(
                            "prepared dossiers require native Linux selected-directory custody"
                                .into(),
                        );
                        if printed.is_err() {
                            std::process::exit(1);
                        }
                    } else {
                        println!("{report}");
                    }
                }
                Err(error) => {
                    eprintln!("[error] {error}");
                    let signal = CANCEL.load(Ordering::Relaxed);
                    std::process::exit(if signal == 0 { 1 } else { 128 + signal });
                }
            }
            return;
        }
        #[cfg(not(feature = "compiler-backed-validators"))]
        {
            eprintln!("[error] philosophy products require compiler-backed-validators");
            std::process::exit(1);
        }
    }
    let (root, python, action, limits, semantic) = arguments().unwrap_or_else(|error| {
        let compiler_flag = if cfg!(feature = "compiler-backed-validators") {
            " | --philosophy-graph-views-validate"
        } else { "" };
        eprintln!("{error}\nusage: tos-ops-mechanics-plan --repo-root PATH [--python COMMAND] [--execute [--growth-python-oracle | --native-contracts-only] | --growth-native-plan | --local-contracts HOME | --threshold-registry-build [--check] | --threshold-registry-validate | --relation-pack-validate | --questbook-validate | --public-mirror-validate | --public-mirror-sync | --derived-kag-validate | --derived-kag-generate | --mechanics-topology-validate | --active-naming-validate | --agent-surface-build [--check] | --agent-surface-validate [--fetch-budget-bases] | --agents-route-currentness-build [--check] [--output PATH] | --nested-agents-validate | --agents-route-harness-check | --tiny-entry-validate | --lived-witness-validate | --intake-pack-validate | --documentation-family-build [--check] [--output PATH] | --documentation-cross-corpus-validate | --decision-records-validate | --decision-index-build [--check] | --root-entry-map-build [--check] [--kag-export PATH] | --root-entry-map-validate [--kag-export PATH] | --kag-source-export-build --store PATH --revision SHA256 --output PATH | --kag-source-export-verify --kag-export PATH | --source-home | --philosophy-topology{compiler_flag} | --semantic-registry-transition [--baseline-commit REF] [--allow-initial-introduction] [--json]] [--command-timeout-ms N] [--lane-timeout-ms N] [--cleanup-grace-ms N] [--max-output-bytes N]");
        std::process::exit(2);
    });
    if matches!(
        action,
        Action::AgentSurfaceBuild { .. }
            | Action::AgentSurfaceValidate { .. }
            | Action::DocumentationFamilyBuild { .. }
            | Action::DocumentationCrossCorpusValidate
            | Action::DecisionRecordsValidate
            | Action::DecisionIndexBuild { .. }
            | Action::RootEntryMapBuild { .. }
            | Action::RootEntryMapValidate
            | Action::KagSourceExportBuild
            | Action::KagSourceExportVerify
            | Action::AgentsRouteCurrentnessBuild { .. }
            | Action::NestedAgentsValidate
            | Action::TreeNodeValidate
            | Action::LivedWitnessValidate
            | Action::IntakePackValidate
    ) {
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
    }
    let result = match action {
        Action::LocalContracts { ref home } => {
            tos_ops_mechanics_plan::local_contracts::run(&root, &home).map(|count| {
                println!("[ok] native mechanics assertions: {home}: {count} retained cases passed");
                0
            })
        }

        Action::DecisionRecordsValidate => root.canonicalize().and_then(|root| {
            let mut s = tos_ops_mechanics_plan::route_cards::RouteSources::new(&root)?;
            let issues =
                tos_ops_mechanics_plan::decision_records::run_validation(&root, &mut s, &CANCEL)?;
            if issues.is_empty() {
                println!("[ok] decision records validated");
                Ok(0)
            } else {
                println!("Decision record validation failed.");
                for (path, message) in issues {
                    println!("- {path}: {message}");
                }
                Ok(1)
            }
        }),
        Action::DecisionIndexBuild { check } => root.canonicalize().and_then(|root| {
            let mut s = tos_ops_mechanics_plan::route_cards::RouteSources::new(&root)?;
            let (outputs, issues) =
                tos_ops_mechanics_plan::decision_records::build_indexes(&mut s, &CANCEL)?;
            if !issues.is_empty() {
                for (path, message) in issues {
                    println!("- {path}: {message}");
                }
                return Ok(1);
            }
            let mut stale = Vec::new();
            for (path, text) in outputs {
                if check {
                    if tos_ops_mechanics_plan::route_cards::read_output(&root.join(&path))?
                        .is_none_or(|v| v.replace("\r\n", "\n").replace('\r', "\n") != text)
                    {
                        stale.push(path);
                    }
                } else {
                    tos_ops_mechanics_plan::route_cards::write_output(
                        &root,
                        std::path::Path::new(&path),
                        &text,
                    )?;
                }
            }
            if stale.is_empty() {
                Ok(0)
            } else {
                println!("Stale decision indexes:");
                for path in stale {
                    println!("- {path}");
                }
                Ok(1)
            }
        }),
        Action::RootEntryMapBuild { check } => root.canonicalize().and_then(|root| {
            let mut sources = tos_ops_mechanics_plan::route_cards::RouteSources::new(&root)?;
            tos_ops_mechanics_plan::root_entry_map::build_with_export(
                &root,
                &mut sources,
                semantic.kag_export.as_deref(),
                &CANCEL,
                check,
            )
            .map(|current| {
                if current {
                    println!("[ok] root-entry map is current");
                    0
                } else {
                    eprintln!("root-entry map is out of date");
                    1
                }
            })
        }),
        Action::RootEntryMapValidate => root.canonicalize().and_then(|root| {
            let mut sources = tos_ops_mechanics_plan::route_cards::RouteSources::new(&root)?;
            tos_ops_mechanics_plan::root_entry_map::validate_with_export(
                &root,
                &mut sources,
                semantic.kag_export.as_deref(),
                &CANCEL,
            )
            .map(|issues| {
                for (path, message) in &issues {
                    eprintln!("- {path}: {message}");
                }
                if issues.is_empty() {
                    println!("[ok] validated root-entry map");
                    0
                } else {
                    1
                }
            })
        }),
        Action::DocumentationFamilyBuild { check } => root.canonicalize().and_then(|root| {
            tos_ops_mechanics_plan::documentation_family::run(
                &root,
                semantic.output.as_deref(),
                check,
                &CANCEL,
            )
        }),
        Action::DocumentationCrossCorpusValidate => root.canonicalize().and_then(|root| {
            tos_ops_mechanics_plan::documentation_cross_corpus::run_validation_with_export(
                &root,
                semantic.selected_interpreter.as_deref(),
                semantic.kag_export.as_deref(),
                &CANCEL,
            )
            .map(|issues| {
                if issues.is_empty() {
                    println!(
                        "[ok] validated cross-corpus documentation currentness and context guards"
                    );
                    0
                } else {
                    println!("Cross-corpus documentation validation failed.");
                    for (location, message) in issues {
                        println!("- {location}: {message}");
                    }
                    1
                }
            })
        }),
        Action::KagSourceExportBuild => tos_ops_mechanics_plan::kag_corpus_export::build_export(
            &root,
            semantic
                .source_store
                .as_deref()
                .expect("source store required by arguments"),
            semantic
                .source_revision
                .as_deref()
                .expect("source revision required by arguments"),
            semantic
                .output
                .as_deref()
                .expect("export output required by arguments"),
        )
        .and_then(|value| {
            let bytes = tos_ops_mechanics_plan::kag_release::canonical(&value)?;
            std::io::stdout().lock().write_all(&bytes).map(|()| 0)
        }),
        Action::KagSourceExportVerify => tos_ops_mechanics_plan::kag_corpus_export::verify_export(
            semantic
                .kag_export
                .as_deref()
                .expect("KAG export required by arguments"),
        )
        .and_then(|value| {
            let bytes = tos_ops_mechanics_plan::kag_release::canonical(&value)?;
            std::io::stdout().lock().write_all(&bytes).map(|()| 0)
        }),
        Action::AgentSurfaceBuild { check } => root
            .canonicalize()
            .and_then(|root| tos_ops_mechanics_plan::agent_surface::run(&root, check, &CANCEL)),
        Action::AgentSurfaceValidate { fetch_budget_bases } => {
            root.canonicalize().and_then(|root| {
                tos_ops_mechanics_plan::agent_surface_validation::validate_manifest(
                    &root,
                    fetch_budget_bases,
                    semantic.selected_interpreter.as_deref(),
                    &CANCEL,
                )
                .map(|issues| {
                    if issues.is_empty() {
                        println!("[ok] validated ToS agent surface");
                        0
                    } else {
                        eprintln!("Agent surface validation failed.");
                        for (location, message) in issues {
                            eprintln!("- {location}: {message}");
                        }
                        1
                    }
                })
            })
        }

        Action::AgentsRouteCurrentnessBuild { check } => root.canonicalize().and_then(|root| {
            let value = tos_ops_mechanics_plan::route_cards::build_currentness(&root, &CANCEL)?;
            let rendered = tos_ops_mechanics_plan::route_cards::render_currentness(&value)?;
            let chosen = if let Some(output) = semantic.output.as_ref() {
                output.clone()
            } else {
                let inventory = tos_ops_mechanics_plan::route_cards::load_inventory(&root)?;
                PathBuf::from(
                    inventory["currentness"]
                        .as_str()
                        .ok_or_else(|| std::io::Error::other("missing currentness output path"))?,
                )
            };
            let path = if chosen.is_absolute() {
                chosen
            } else {
                root.join(chosen)
            };
            let display = path.strip_prefix(&root).unwrap_or(&path).display();
            if check {
                let actual = tos_ops_mechanics_plan::route_cards::read_output(&path)?;
                if actual.as_deref() != Some(rendered.as_str()) {
                    println!("AGENTS route currentness is stale or missing: {display}");
                    Ok(1)
                } else {
                    println!("AGENTS route currentness is current: {display}");
                    Ok(0)
                }
            } else {
                tos_ops_mechanics_plan::route_cards::write_output(&root, &path, &rendered)?;
                println!("wrote {display}");
                Ok(0)
            }
        }),
        Action::AgentsRouteHarnessCheck => root.canonicalize().and_then(|root| {
            let result =
                tos_ops_mechanics_plan::route_harness::build_result(&root, false, &CANCEL)?;
            let tasks = result["tasks"]
                .as_array()
                .ok_or_else(|| std::io::Error::other("invalid harness task result"))?;
            let failures = tasks
                .iter()
                .filter(|task| task["route_success"] != true)
                .count();
            if failures == 0 {
                println!(
                    "AGENTS route harness passed for {} task routes",
                    tasks.len()
                );
                Ok(0)
            } else {
                println!(
                    "AGENTS route harness failed for {failures}/{} task routes",
                    tasks.len()
                );
                Ok(1)
            }
        }),
        Action::TreeNodeValidate => {
            #[cfg(feature = "compiler-backed-validators")]
            {
                root.canonicalize().and_then(|root| {
                    let deadline = std::time::Instant::now().checked_add(limits.lane_wall)
                        .ok_or_else(|| std::io::Error::other("node validator deadline overflow"))?;
                    let mut sources = tos_ops_mechanics_plan::route_cards::RouteSources::new_until(&root, deadline)?;
                    let issues = tos_ops_mechanics_plan::tree_nodes::validate(&mut sources, &CANCEL)?;
                    for (path, message) in &issues { eprintln!("- {path}: {message}"); }
                    if issues.is_empty() { println!("[ok] canonical tree node contracts and consistency"); Ok(0) } else { Ok(1) }
                })
            }
            #[cfg(not(feature = "compiler-backed-validators"))]
            { Err(std::io::Error::other("tree node validation requires compiler-backed-validators")) }
        },
        Action::LivedWitnessValidate | Action::IntakePackValidate => root.canonicalize().and_then(|root| {
            let deadline = std::time::Instant::now().checked_add(limits.lane_wall)
                .ok_or_else(|| std::io::Error::other("validator deadline overflow"))?;
            let mut sources = tos_ops_mechanics_plan::route_cards::RouteSources::new_until(&root, deadline)?;
            let lived = matches!(action, Action::LivedWitnessValidate);
            let issues = if lived {
                tos_ops_mechanics_plan::lived_witness::validate(&root, &mut sources, limits, &CANCEL)?
            } else {
                tos_ops_mechanics_plan::intake_pack::validate(&mut sources, &CANCEL)?
            };
            for (path, message) in &issues { eprintln!("- {path}: {message}"); }
            if !issues.is_empty() { return Ok(1); }
            if lived {
                println!("Lived-witness route check passed for structure and private boundary only.");
                println!("Human authorship, consent, memory, context, and meaning remain unvalidated.");
            } else {
                println!("[ok] validated v6.1 tabular base intake pack");
                println!("[ok] validated tabular registries against the current edges.csv");
            }
            Ok(0)
        }),
        Action::TinyEntryValidate => root.canonicalize().and_then(|root| {
            let mut sources = tos_ops_mechanics_plan::route_cards::RouteSources::new(&root)?;
            let issues =
                tos_ops_mechanics_plan::tiny_entry::validate(&root, &mut sources, &CANCEL)?;
            for (path, message) in &issues {
                eprintln!("- {path}: {message}");
            }
            if issues.is_empty() {
                println!("[ok] validated tiny entry route");
                Ok(0)
            } else {
                Ok(1)
            }
        }),
        Action::NestedAgentsValidate => root.canonicalize().and_then(|root| {
            let issues = tos_ops_mechanics_plan::route_cards::run_validation(&root, &CANCEL)?;
            if !issues.is_empty() {
                println!("Nested AGENTS route-card check failed.");
                for (location, message) in issues {
                    println!("- {location}: {message}");
                }
                return Ok(1);
            }
            let inventory = tos_ops_mechanics_plan::route_cards::load_inventory(&root)?;
            let count =
                tos_ops_mechanics_plan::route_cards::discover_route_cards(&root, &inventory)?.len();
            println!("Nested AGENTS route-card check passed for {count} files.");
            Ok(0)
        }),
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
        #[cfg(feature = "compiler-backed-validators")]
        Action::PhilosophyGraphViews => {
            tos_ops_mechanics_plan::philosophy_graph_views::run(&root, &CANCEL)
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
        Action::GrowthNativePlan => tos_ops_mechanics_plan::growth_native_plan::discover(&root)
            .and_then(|plan| {
                println!(
                    "{}",
                    serde_json::to_string(&plan).map_err(std::io::Error::other)?
                );
                Ok(0)
            }),
        Action::Plan | Action::Execute { .. } => tos_ops_mechanics_plan::discover(&root, &python)
            .and_then(|mut plan| {
                let whole_steps = if matches!(
                    action,
                    Action::Execute {
                        growth_python_oracle: false,
                        native_contracts_only: false,
                    }
                )
                    && tos_ops_mechanics_plan::growth_coverage::uses_native_route(&root, &plan)?
                {
                    Some(tos_ops_mechanics_plan::growth_coverage::whole_steps(
                        &root, &python, &plan,
                    )?)
                } else {
                    None
                };
                if matches!(
                    action,
                    Action::Execute {
                        native_contracts_only: true,
                        ..
                    }
                ) {
                    plan.commands
                        .retain(|command| command.kind == "native_assertions");
                    plan.test_file_count = 5;
                    if plan.commands.len() != 3 {
                        return Err(std::io::Error::other(
                            "native mechanics contracts require all three supported homes",
                        ));
                    }
                }
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
                if let Some(steps) = whole_steps {
                    tos_ops_mechanics_plan::executor::run_validation_sequence(
                        &root, &python, &steps, limits, &CANCEL,
                    )
                } else {
                    tos_ops_mechanics_plan::executor::run(&root, &plan, limits, &CANCEL)
                }
            }),
    };
    let code = match result {
        Ok(code) => code,
        Err(error) => {
            if matches!(
                action,
                Action::QuestbookValidate
                    | Action::DerivedKagValidate
                    | Action::SourceHome
                    | Action::AgentsRouteCurrentnessBuild { .. }
                    | Action::NestedAgentsValidate
            ) {
                eprintln!("[error] {error}");
                std::process::exit(1);
            }
            let route = match action {
                Action::Execute { .. } => "execution",
                Action::LocalContracts { .. } => "native local contracts",
                Action::Plan => "plan",
                Action::GrowthNativePlan => "Growth native source plan",
                Action::ThresholdBuild { .. } | Action::ThresholdValidate => "threshold registry",
                Action::RelationPackValidate => "relation pack",
                Action::QuestbookValidate => "questbook",
                Action::PublicMirrorValidate | Action::PublicMirrorSync => "public mirror",
                Action::DerivedKagValidate | Action::DerivedKagGenerate => "derived KAG",
                Action::MechanicsTopologyValidate => "mechanics topology",
                Action::ActiveNamingValidate => "active naming",
                Action::AgentSurfaceBuild { .. } | Action::AgentSurfaceValidate { .. } => {
                    "agent surface"
                }
                Action::DocumentationFamilyBuild { .. }
                | Action::DocumentationCrossCorpusValidate => "documentation",
                Action::RootEntryMapBuild { .. } | Action::RootEntryMapValidate => "root-entry map",
                Action::KagSourceExportBuild | Action::KagSourceExportVerify => "KAG source export",
                Action::DecisionRecordsValidate | Action::DecisionIndexBuild { .. } => {
                    "decision records"
                }
                Action::SourceHome => "source home",
                Action::AgentsRouteCurrentnessBuild { .. } => "AGENTS route currentness",
                Action::NestedAgentsValidate => "nested AGENTS route cards",
                Action::AgentsRouteHarnessCheck => "AGENTS route harness",
                Action::TinyEntryValidate => "tiny entry route",
                Action::TreeNodeValidate => "tree node contracts",
                Action::LivedWitnessValidate => "lived-witness route",
                Action::IntakePackValidate => "intake pack",
                Action::PhilosophyTopology => "philosophy topology",
                #[cfg(feature = "compiler-backed-validators")]
                Action::PhilosophyGraphViews => "philosophy graph views",
                Action::SemanticRegistryTransition => "semantic registry transition",
            };
            let diagnostic = if matches!(action, Action::SemanticRegistryTransition) {
                format!("[error] semantic registry transition: {error}\n")
            } else {
                format!("mechanics-local {route}: {error}\n")
            };
            #[cfg(target_os = "linux")]
            if matches!(action, Action::Execute { .. }) {
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
