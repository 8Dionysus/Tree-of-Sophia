//! Candidate native transport binary. An owner-selected production binding
//! must be installed before it can serve a source operation.

use std::sync::Arc;
use tos_query::AbortProbe;

use tos_access::{
    AccessError, AccessErrorCode, AccessExecutor, AccessProfile, Params, PreparedPacket, cli, http,
    mcp,
};

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
    let mut args = std::env::args().skip(1);
    let profile = AccessProfile::new(65_536, 1_048_576, 65_536);
    let result = match args.next().as_deref() {
        Some("mcp") if args.next().is_none() => {
            mcp::run_stdio(&NoOwner, profile).map_err(|error| error.to_string())
        }
        Some("serve") => {
            let options: Vec<String> = args.collect();
            cli::parse_serve_address(&options).map_err(|error| error.message.to_owned())
                .and_then(|address| http::serve(&address, Arc::new(NoOwner), profile)
                    .map_err(|error| error.to_string()))
        }
        Some(route_name @ ("source" | "knowledge" | "lens")) => {
            let mut route = vec![route_name.to_owned()];
            route.extend(args);
            let code = cli::run_cli(
                &route,
                &NoOwner,
                profile,
                &mut std::io::stdout(),
                &mut std::io::stderr(),
            );
            std::process::exit(code);
        }
        _ => {
            Err("usage: tos-access mcp | serve [LOOPBACK:PORT] | source descend NODE_ID | knowledge search QUERY --mode indexed".to_owned())
        }
    };
    if let Err(message) = result {
        eprintln!("{message}");
        std::process::exit(2);
    }
}
