//! Disposable native HTTP fixture host for the Node Playwright browser suite.
//! Domain query packets are produced through the existing Rust access/QRY seams.
#[path = "tos-e2e-fixture/browser_fixture.rs"]
mod browser_fixture;

use std::{
    io::{self, Write},
    net::TcpListener,
    path::PathBuf,
    sync::Arc,
    thread,
};
use tos_access::{AccessProfile, http};

fn main() {
    let mut args = std::env::args().skip(1);
    let result = match args.next().as_deref() {
        Some("serve") => run_serve(args.collect()),
        _ => Err("usage: tos-e2e-fixture serve --site-executable ABS --scenario NAME".to_owned()),
    };
    if let Err(message) = result {
        eprintln!("{message}");
        std::process::exit(2);
    }
}

fn option(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}

fn run_serve(args: Vec<String>) -> Result<(), String> {
    let executable = option(&args, "--site-executable")
        .or_else(|| std::env::var("TOS_E2E_SITE_EXECUTABLE").ok())
        .map(PathBuf::from)
        .ok_or_else(|| "serve requires --site-executable or TOS_E2E_SITE_EXECUTABLE".to_owned())?;
    let scenario = option(&args, "--scenario").unwrap_or_else(|| "default".to_owned());
    if !matches!(
        scenario.as_str(),
        "default" | "source" | "source-native-metadata" | "prepared"
    ) {
        return Err("unsupported browser fixture scenario".to_owned());
    }
    let profile = AccessProfile::new(65_536, 1_048_576, 65_536);
    let site = tos_access::site::SoftwareSite::open(&executable, profile.deadline_probe())
        .map_err(|error| format!("open native software site: {}", error.message))?;
    let executor = Arc::new(browser_fixture::BrowserFixtureExecutor::new(&scenario));
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|error| format!("bind fixture listener: {error}"))?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("read fixture listener address: {error}"))?;
    println!("ready http://{address}");
    io::stdout()
        .flush()
        .map_err(|error| format!("flush fixture readiness: {error}"))?;
    for accepted in listener.incoming() {
        let stream = accepted.map_err(|error| format!("accept fixture connection: {error}"))?;
        let executor = Arc::clone(&executor);
        let site = Arc::clone(&site);
        let _ = thread::Builder::new()
            .name("tos-e2e-http".to_owned())
            .spawn(move || {
                let _ = http::serve_connection_scoped_with_site(
                    stream,
                    executor.as_ref(),
                    &site,
                    profile,
                );
            });
    }
    Ok(())
}
