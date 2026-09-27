//! Native transports with an explicit managed-local ReleaseStore selection.
//! Without selection, no source or projection authority is invented.
use std::{path::Path, sync::Arc};
use tos_access::{
    AccessError, AccessErrorCode, AccessExecutor, AccessProfile, Params, PreparedPacket, cli, http,
    managed_local::ManagedLocalExecutor, mcp,
};
use tos_query::AbortProbe;
struct NoOwner;
impl AccessExecutor for NoOwner {
    fn source_descend_available(&self) -> bool {
        false
    }
    fn source_descend(
        &self,
        _: Params,
        _: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket, AccessError> {
        Err(AccessError::new(
            AccessErrorCode::Unavailable,
            "source owner is not selected",
        ))
    }
}
fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = tos_access::software_archive::run_if_requested(
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
    while args
        .first()
        .is_some_and(|arg| arg == "--release-root" || arg.starts_with("--release-root="))
    {
        let option = args.remove(0);
        let value = if let Some((_, value)) = option.split_once('=') {
            value.to_owned()
        } else if !args.is_empty() {
            args.remove(0)
        } else {
            eprintln!("invalid_request: --release-root requires a directory");
            std::process::exit(2)
        };
        release_root = Some(value.into());
    }
    if args.first().is_none_or(|route| {
        !matches!(
            route.as_str(),
            "mcp" | "serve" | "source" | "knowledge" | "lens"
        )
    }) || (args.first().is_some_and(|route| route == "mcp") && args.len() != 1)
    {
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
    let executor: Arc<dyn AccessExecutor> = match release_root {
        Some(root) => match ManagedLocalExecutor::open(Path::new(&root), profile) {
            Ok(executor) => Arc::new(executor),
            Err(error) => {
                eprintln!("{}: {}", error.code_str(), error.message);
                std::process::exit(3)
            }
        },
        None => Arc::new(NoOwner),
    };
    let mut args = args.into_iter();
    let result=match args.next().as_deref() {
        Some("mcp") if args.next().is_none()=>mcp::run_stdio(executor.as_ref(),profile).map_err(|error| error.to_string()),
        Some("serve")=> {
            let options:Vec<String>=args.collect();
            cli::parse_serve_address(&options).map_err(|error| error.message.to_owned())
                .and_then(|address| http::serve(&address,Arc::clone(&executor),profile).map_err(|error|error.to_string()))
        },
        Some(route_name @ ("source"|"knowledge"|"lens"))=> {
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
