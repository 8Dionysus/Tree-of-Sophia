//! Native transports with an explicit managed-local ReleaseStore selection.
//! Without selection, no source or projection authority is invented.
use std::{path::Path, sync::Arc};
use tos_access::{
    AccessExecutor, AccessProfile, NoOwner, cli, http, managed_local::ManagedLocalExecutor, mcp,
};
fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // Software help/version never opens a selected release or grants readiness.
    let help = "usage: tos [--release-root ABSOLUTE_DIRECTORY | --root ABS | --prepared-read-model ABS --prepared-binding ABS [--root ABS] [--exploration-checkpoints ABS]] COMMAND\n\nCommands:\n  serve [LOOPBACK:PORT]     local HTTP and installed software site\n  mcp [--transport stdio|streamable-http] [--host HOST --port PORT]\n                            MCP JSONL or loopback /mcp JSON-response transport\n  knowledge | lens | source bounded read operations\n  reading-search --query Q   local Zarathustra reading data\n  word-analysis --query Q    private source-bound task or candidate validation\n  concept-search --query Q  private source-bound concept task\n  lexical-index build|validate|validate-legacy OPTIONS   exact-cut private lexical maintainer\n  structural-paragraph --source-root ABS --check|--validate-tracked\n  technical-markup --source-root ABS --build|--check|--validate-tracked\n  zarathustra-*-v1 --help    explicit source-selected research producers\n  doctor | verify           source-backed diagnostic report\n  software build|verify|extract|install OPTIONS\n  software build --native-command-products ABS_JSON optionally packages owner-command, schema-worker and three compiler-free ops commands; installed paths are absolute, without owner grants or data.\n  restore-source-cut --corpus-store ABS --source-revision sha256:HEX --output FRESH_ABS\n    --max-revisions N --max-directories N --max-members N --max-member-bytes N\n    --max-total-bytes N --max-metadata-bytes N --max-seconds N\n  capture-restore --capture ABS --output FRESH_ABS --source-commit SHA1 --source-tree SHA1\n    --manifest-sha256 HEX --max-archive-bytes N --max-decoded-bytes N\n    --max-source-bytes N --max-metadata-bytes N --max-members N --max-seconds N\n  prepare --source-root ROOT --output-dir FRESH_DIR --max-seconds N\n          [--max-bytes N --max-mutations N --attach-maintenance]\n  prepared-publication --max-seconds N  bounded framed stdin (not raw source)\n  evidence-projection build|check|validate --source-root ABS --staging FRESH_ABS --max-seconds N [--output FRESH_ABS]\n  edge-sql-stream --source SQL --directory EMPTY --maximum-bytes N --max-seconds N\n  edge-sql-chunk --source SQL --output FRESH --offset N --maximum-bytes N\n  edge-import-local --database EXISTING --sql SQL --base REVISION_OR_null --target REVISION\n  edge-offline-capture --request ABS.json\n             bounded private prepared-pair/navigation/integrity SQL capture\n  build-data --source-root ROOT --output DIST --runtime RUNTIME\n             [--max-build-seconds N]  disposable offline public D1 v9\n\nNative install: software install --archive ABS --prefix FRESH_ABS\nwith --max-total-bytes N --max-archive-bytes N --max-members N\nand --max-metadata-bytes N. Installation never selects data or edits PATH.\nExact-source reads: add --source-inputs ABS to --root ABS and the prepared pair; optional --source-local-text-selection ABS selects protected local conditions.\nCommands: source capabilities | contracts | discover REQUEST_JSON | read REQUEST_JSON (use - for stdin).\nData operations without a selected owner report unavailable.\nPublic build requires TOS_BUILD_MAX_SECONDS unless --max-build-seconds is supplied.\n";
    if args.len() == 1 && matches!(args[0].as_str(), "--version" | "-V") {
        println!("tos {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if (args.len() == 1 && matches!(args[0].as_str(), "--help" | "-h" | "help"))
        || (args.len() == 2
            && matches!(
                args[0].as_str(),
                "serve"
                    | "mcp"
                    | "software"
                    | "prepare"
                    | "capture-restore"
                    | "edge-offline-capture"
            )
            && matches!(args[1].as_str(), "--help" | "-h"))
    {
        print!("{help}");
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(code) = tos_access::research_builders_command::run_if_requested(
        &args,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ) {
        std::process::exit(code);
    }
    if let Some(code) = tos_access::lexical_index_command::run_if_requested(
        &args,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ) {
        std::process::exit(code);
    }
    if let Some(code) = tos_access::evidence_projection::run_if_requested(
        &args,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ) {
        std::process::exit(code);
    }
    if let Some(code) = tos_access::technical_markup_command::run_if_requested(
        &args,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ) {
        std::process::exit(code);
    }
    if let Some(code) = tos_access::structural_paragraph_command::run_if_requested(
        &args,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ) {
        std::process::exit(code);
    }
    if let Some(code) = tos_access::source_cut_restore::run_if_requested(
        &args,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ) {
        std::process::exit(code);
    }
    if let Some(code) = tos_access::capture_restore::run_if_requested(
        &args,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ) {
        std::process::exit(code);
    }
    if let Some(code) = tos_access::edge_offline_capture::run_if_requested(
        &args,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ) {
        std::process::exit(code);
    }
    if let Some(code) = tos_access::software_archive::run_if_requested(
        &args,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ) {
        std::process::exit(code);
    }
    if let Some(code) = tos_access::edge_sql::run_if_requested(
        &args,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ) {
        std::process::exit(code);
    }
    if let Some(code) = tos_access::public_d1_build::run_if_requested(
        &args,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ) {
        std::process::exit(code);
    }
    if let Some(code) = tos_access::native_prepare::run_if_requested(
        &args,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ) {
        std::process::exit(code);
    }
    if let Some(code) = tos_access::prepared_publication::run_if_requested(
        &args,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ) {
        std::process::exit(code);
    }
    if let Some(code) =
        tos_access::doctor::run_if_requested(&args, &mut std::io::stdout(), &mut std::io::stderr())
    {
        std::process::exit(code)
    }
    // This explicit native profile admits the complete duplicated MCP result
    // envelope from declared packet/request caps, including escaping and ID.
    let profile = AccessProfile::new(65_536, 1_048_576, 65_536);
    let frame =
        mcp::tool_result_frame_byte_bound(profile.max_response_bytes, profile.max_request_bytes)
            .expect("fixed native profile frame arithmetic");
    let profile = profile.with_mcp_frame_budget(frame);
    let mut release_root = std::env::var_os("TOS_RELEASE_ROOT");
    let mut explicit_release = false;
    let mut prepared_model = None;
    let mut prepared_binding = None;
    let mut exploration_checkpoints = None;
    let mut source_inputs = None;
    let mut source_local_text_selection = None;
    let mut prepared_root = std::env::var_os("TOS_DATA_ROOT");
    let mut explicit_data_root = false;
    while args.first().is_some_and(|arg| {
        [
            "--release-root",
            "--prepared-read-model",
            "--prepared-binding",
            "--exploration-checkpoints",
            "--source-inputs",
            "--source-local-text-selection",
            "--root",
        ]
        .iter()
        .any(|key| arg == key || arg.starts_with(&format!("{key}=")))
    }) {
        let option = args.remove(0);
        let (key, value) = if let Some((key, value)) = option.split_once('=') {
            (key.to_owned(), value.to_owned())
        } else if args.first().is_some_and(|value| !value.starts_with("--")) {
            (option, args.remove(0))
        } else {
            eprintln!("invalid_request: selection option requires an absolute path");
            std::process::exit(2)
        };
        let slot = match key.as_str() {
            "--release-root" => {
                explicit_release = true;
                &mut release_root
            }
            "--prepared-read-model" => &mut prepared_model,
            "--prepared-binding" => &mut prepared_binding,
            "--exploration-checkpoints" => &mut exploration_checkpoints,
            "--source-inputs" => &mut source_inputs,
            "--source-local-text-selection" => &mut source_local_text_selection,
            "--root" => {
                explicit_data_root = true;
                &mut prepared_root
            }
            _ => unreachable!(),
        };
        *slot = Some(value.into());
    }
    if prepared_model.is_some() != prepared_binding.is_some()
        || (prepared_model.is_some() && explicit_release)
        || (explicit_data_root && explicit_release)
    {
        eprintln!(
            "invalid_request: prepared paths require a pair; explicit root and release-root selections conflict"
        );
        std::process::exit(2)
    }
    if (source_inputs.is_some() && (prepared_model.is_none() || !explicit_data_root))
        || (source_local_text_selection.is_some() && source_inputs.is_none())
    {
        eprintln!(
            "invalid_request: source inputs require explicit root and prepared pair; local text requires source inputs"
        );
        std::process::exit(2)
    }
    if exploration_checkpoints.is_some() && prepared_model.is_none() {
        eprintln!(
            "invalid_request: --exploration-checkpoints requires an explicitly selected prepared reader"
        );
        std::process::exit(2)
    }
    // The scoped logical profile retains the maintained checked duplicated MCP
    // frame allowance. Explicit prepared selection takes precedence over env.
    let profile = if prepared_model.is_some() {
        tos_access::prepared_local::profile()
    } else if explicit_data_root || (prepared_root.is_some() && release_root.is_none()) {
        profile.with_query_timeout(std::time::Duration::from_secs(5))
    } else {
        profile
    };
    if args.first().is_none_or(|route| {
        !matches!(
            route.as_str(),
            "mcp"
                | "serve"
                | "source"
                | "knowledge"
                | "lens"
                | "reading-search"
                | "word-analysis"
                | "concept-search"
        )
    }) {
        eprintln!(
            "usage: tos-access [--release-root ABSOLUTE_DIRECTORY] mcp | serve [LOOPBACK:PORT] | source descend NODE_ID | knowledge search QUERY [--mode legacy|indexed|compressed]"
        );
        std::process::exit(2);
    }
    if args.first().is_some_and(|route| route == "serve") {
        if let Err(error) = cli::parse_serve_address(&args[1..]) {
            eprintln!("{}: {}", error.code_str(), error.message);
            std::process::exit(2);
        }
    }
    if args.first().is_some_and(|route| route == "mcp") {
        if let Err(message) = tos_access::mcp_http::parse_transport(&args[1..]) {
            eprintln!("invalid_request: {message}");
            std::process::exit(2);
        }
    }
    let selected: Result<Arc<dyn AccessExecutor>, tos_access::AccessError> =
        if let (Some(model), Some(binding)) = (prepared_model, prepared_binding) {
            let source_root = prepared_root.clone();
            tos_access::prepared_local::PreparedLocalExecutor::open_with_checkpoints(
                model.into(),
                binding.into(),
                prepared_root.map(Into::into),
                exploration_checkpoints.map(Into::into),
            )
            .and_then(|executor| match source_inputs {
                Some(inputs) => executor.with_source_reader(
                    Path::new(
                        source_root
                            .as_ref()
                            .expect("validated explicit source root"),
                    ),
                    Path::new(&inputs),
                    source_local_text_selection.as_deref().map(Path::new),
                    profile,
                ),
                None => Ok(executor),
            })
            .map(|executor| Arc::new(executor) as Arc<dyn AccessExecutor>)
        } else if explicit_data_root || (prepared_root.is_some() && release_root.is_none()) {
            tos_access::reading::ReadingLocalExecutor::open(
                prepared_root.expect("selected reading root").into(),
            )
            .map(|executor| Arc::new(executor) as Arc<dyn AccessExecutor>)
        } else {
            match release_root {
                Some(root) => ManagedLocalExecutor::open(Path::new(&root), profile)
                    .map(|executor| Arc::new(executor) as Arc<dyn AccessExecutor>),
                None => Ok(Arc::new(NoOwner)),
            }
        };
    let executor = selected.unwrap_or_else(|error| {
        eprintln!("{}: {}", error.code_str(), error.message);
        std::process::exit(3)
    });
    let mut args = args.into_iter();
    let result=match args.next().as_deref() {
        Some("mcp") => {
            // Reuse selected-source software admission; otherwise admit exactly
            // one installed companion under the existing absolute startup budget.
            let software = executor.installed_software().map(|site| Ok(Some(site))).unwrap_or_else(||
                tos_access::site::SoftwareSite::installed_for_mcp(profile.with_query_timeout(std::time::Duration::from_secs(30)).deadline_probe()));
            let software = software.unwrap_or_else(|error| { eprintln!("{}: {}", error.code_str(), error.message); std::process::exit(3) });
            let options: Vec<String> = args.collect();
            tos_access::mcp_http::parse_transport(&options).map_err(str::to_owned).and_then(|transport| match transport {
                tos_access::mcp_http::Transport::Stdio => mcp::run_stdio_with_software(executor.as_ref(),profile,software.as_ref().map(Arc::clone)).map_err(|error| error.to_string()),
                tos_access::mcp_http::Transport::StreamableHttp(address) =>
                    tos_access::mcp_http::serve_with_software(&address, Arc::clone(&executor), profile, software).map_err(|error| error.to_string()),
            })
        },
        Some("serve")=> {
            let options:Vec<String>=args.collect();
            cli::parse_serve_address(&options).map_err(|error| error.message.to_owned())
                .and_then(|address| http::serve(&address,Arc::clone(&executor),profile).map_err(|error|error.to_string()))
        },
        Some(route_name @ ("source"|"knowledge"|"lens"|"reading-search"|"word-analysis"|"concept-search"))=> {
            let mut route=vec![route_name.to_owned()];route.extend(args);
            let code=cli::run_cli(&route,executor.as_ref(),profile,&mut std::io::stdout(),&mut std::io::stderr());
            std::process::exit(code)
        },
        _=>Err("usage: tos-access [--release-root ABSOLUTE_DIRECTORY] mcp | serve [LOOPBACK:PORT] | source descend NODE_ID | knowledge search QUERY [--mode legacy|indexed|compressed]".to_owned()),
    };
    if let Err(message) = result {
        eprintln!("{message}");
        std::process::exit(2)
    }
}
