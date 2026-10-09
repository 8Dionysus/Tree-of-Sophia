#![cfg(not(target_arch = "wasm32"))]

//! Real installed-owner conformance across CLI, HTTP and MCP. The test is
//! intentionally ignored by default: running it requires a specifically
//! selected prepared source binding and owner-authored scenarios, never a
//! synthetic grant or guessed target.

use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Component, Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant},
};
use tos_foundation::Digest256;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Scenarios {
    cases: Vec<ReadCase>,
    #[serde(default)]
    stale_reads: Vec<StaleRead>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadCase {
    name: String,
    selector: Value,
    representation: String,
    expected_discovery_status: String,
    expected_read_status: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StaleRead {
    name: String,
    handle: Value,
    representation: String,
    expected_status: String,
}

struct Selection {
    binary: PathBuf,
    root: PathBuf,
    inputs: PathBuf,
    model: PathBuf,
    binding: PathBuf,
    local_text: Option<PathBuf>,
    scenarios: Scenarios,
}

impl Selection {
    fn read() -> Self {
        let binary = required_path("TOS_NATIVE_SOURCE_READ_BIN");
        let root = required_path("TOS_NATIVE_SOURCE_ROOT");
        let inputs = required_path("TOS_NATIVE_SOURCE_INPUTS");
        let model = required_path("TOS_NATIVE_PREPARED_READ_MODEL");
        let binding = required_path("TOS_NATIVE_PREPARED_BINDING");
        let scenario_path = required_path("TOS_NATIVE_SOURCE_READ_CASES");
        let local_text =
            std::env::var_os("TOS_NATIVE_SOURCE_LOCAL_TEXT_SELECTION").map(PathBuf::from);
        for (name, path, is_dir) in [
            ("TOS_NATIVE_SOURCE_READ_BIN", &binary, false),
            ("TOS_NATIVE_SOURCE_ROOT", &root, true),
            ("TOS_NATIVE_SOURCE_INPUTS", &inputs, false),
            ("TOS_NATIVE_PREPARED_READ_MODEL", &model, false),
            ("TOS_NATIVE_PREPARED_BINDING", &binding, false),
            ("TOS_NATIVE_SOURCE_READ_CASES", &scenario_path, false),
        ] {
            assert!(
                path.is_absolute(),
                "{name} must be absolute: {}",
                path.display()
            );
            assert!(path.exists(), "{name} does not exist: {}", path.display());
            assert_eq!(path.is_dir(), is_dir, "{name} has the wrong file kind");
        }
        if let Some(path) = &local_text {
            assert!(
                path.is_absolute(),
                "TOS_NATIVE_SOURCE_LOCAL_TEXT_SELECTION must be absolute"
            );
            assert!(path.is_file(), "local-text selection must be a file");
        }
        let scenarios: Scenarios = serde_json::from_slice(
            &fs::read(&scenario_path).expect("read owner-selected case file"),
        )
        .expect("parse owner-selected case file");
        assert!(
            scenarios.cases.iter().any(|case| {
                case.representation == "record"
                    && case.expected_discovery_status == "available"
                    && case.expected_read_status == "available"
            }),
            "case file must include one current exact metadata-record read"
        );
        assert!(
            scenarios.cases.iter().any(|case| {
                case.representation == "native_public_unit"
                    && case.expected_discovery_status == "available"
                    && ["available", "access-restricted"]
                        .contains(&case.expected_read_status.as_str())
            }),
            "case file must include one selected native-public-unit rights decision"
        );
        assert!(
            !scenarios.stale_reads.is_empty(),
            "case file must include a real previously issued handle from an older source epoch"
        );
        for case in &scenarios.cases {
            assert!(!case.name.is_empty(), "case name is required");
            assert!(
                case.selector.is_object(),
                "{} selector must be an object",
                case.name
            );
            assert!(
                [
                    "available",
                    "missing",
                    "stale",
                    "corrupt",
                    "access-restricted",
                    "over-budget",
                    "unsupported"
                ]
                .contains(&case.expected_discovery_status.as_str()),
                "{} has an unknown discovery status",
                case.name
            );
            assert!(
                [
                    "available",
                    "missing",
                    "stale",
                    "corrupt",
                    "access-restricted",
                    "over-budget",
                    "unsupported"
                ]
                .contains(&case.expected_read_status.as_str()),
                "{} has an unknown read status",
                case.name
            );
            assert!(
                matches!(
                    case.representation.as_str(),
                    "record" | "native_public_unit" | "native_local_unit"
                ),
                "{} has an unsupported representation",
                case.name
            );
            if case.representation == "native_local_unit" {
                assert!(
                    local_text.is_some(),
                    "{} requires TOS_NATIVE_SOURCE_LOCAL_TEXT_SELECTION",
                    case.name
                );
            }
        }
        for stale in &scenarios.stale_reads {
            assert!(!stale.name.is_empty(), "stale case name is required");
            assert!(
                stale.handle.is_object(),
                "{} handle must be an old real handle",
                stale.name
            );
            assert!(
                matches!(
                    stale.representation.as_str(),
                    "record" | "native_public_unit" | "native_local_unit"
                ),
                "{} has an unsupported representation",
                stale.name
            );
            assert_eq!(
                stale.expected_status, "stale",
                "stale cases must expect stale"
            );
        }
        Self {
            binary,
            root,
            inputs,
            model,
            binding,
            local_text,
            scenarios,
        }
    }

    fn global_args(&self, route: &str) -> Vec<String> {
        let mut args = vec![
            "--root".to_owned(),
            self.root.display().to_string(),
            "--prepared-read-model".to_owned(),
            self.model.display().to_string(),
            "--prepared-binding".to_owned(),
            self.binding.display().to_string(),
            "--source-inputs".to_owned(),
            self.inputs.display().to_string(),
        ];
        if let Some(path) = &self.local_text {
            args.push("--source-local-text-selection".to_owned());
            args.push(path.display().to_string());
        }
        args.push(route.to_owned());
        args
    }

    fn command(&self, route: &str) -> Command {
        let mut command = Command::new(&self.binary);
        command.args(self.global_args(route));
        // A test must run only the explicit prepared selection above.
        command
            .env_remove("TOS_RELEASE_ROOT")
            .env_remove("TOS_DATA_ROOT");
        command
    }
}

fn required_path(name: &str) -> PathBuf {
    let value = std::env::var_os(name).unwrap_or_else(|| panic!("{name} must be explicitly set"));
    PathBuf::from(value)
}

fn cli_request(selection: &Selection, operation: &str, request: &Value) -> Value {
    let mut child = selection
        .command("source")
        .arg(operation)
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start installed native CLI");
    let mut stdin = child.stdin.take().expect("CLI stdin");
    serde_json::to_writer(&mut stdin, request).expect("write CLI JSON request");
    stdin.write_all(b"\n").expect("terminate CLI JSON request");
    drop(stdin);
    let output = child
        .wait_with_output()
        .expect("wait for installed native CLI");
    assert!(
        output.status.success(),
        "CLI {operation} failed: {}",
        "stderr withheld"
    );
    serde_json::from_slice(&output.stdout).expect("parse native CLI response JSON")
}

fn loopback_address() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve loopback port");
    let address = listener.local_addr().expect("loopback address");
    drop(listener);
    address.to_string()
}

struct ServerChild(Child);
impl Drop for ServerChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_http(selection: &Selection) -> (ServerChild, String) {
    let address = loopback_address();
    let child = selection
        .command("serve")
        .arg(&address)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start native HTTP server");
    let mut server = ServerChild(child);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = server.0.try_wait().expect("poll native HTTP server") {
            panic!("native HTTP server exited before listening: {status}");
        }
        if TcpStream::connect(&address).is_ok() {
            return (server, address);
        }
        assert!(
            Instant::now() < deadline,
            "native HTTP server did not listen on {address}"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

fn http_request(address: &str, path: &str, request: &Value) -> Value {
    let body = serde_json::to_vec(request).expect("encode HTTP JSON request");
    let mut stream = TcpStream::connect(address).expect("connect to native HTTP server");
    stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .expect("set HTTP read timeout");
    write!(
        stream,
        "POST {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .expect("write HTTP headers");
    stream.write_all(&body).expect("write HTTP request body");
    stream.flush().expect("flush HTTP request");
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .expect("read HTTP response");
    let split = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("HTTP response headers");
    let header = std::str::from_utf8(&response[..split]).expect("HTTP response header UTF-8");
    let status = header
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .expect("HTTP status line");
    assert_eq!(status, "200", "HTTP {path} response: {header}");
    let content_length = header
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .expect("HTTP response Content-Length");
    let bytes = &response[split + 4..];
    assert_eq!(bytes.len(), content_length, "complete HTTP response body");
    serde_json::from_slice(bytes).expect("parse native HTTP response JSON")
}

struct McpClient {
    child: Child,
    input: ChildStdin,
    output: Receiver<String>,
    next_id: u64,
}
impl McpClient {
    fn start(selection: &Selection) -> Self {
        let mut child = selection
            .command("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start native MCP stdio process");
        let input = child.stdin.take().expect("MCP stdin");
        let stdout = child.stdout.take().expect("MCP stdout");
        let (response_tx, output) = mpsc::channel();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        if response_tx.send(line).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        let mut client = Self {
            child,
            input,
            output,
            next_id: 1,
        };
        let init = client.rpc(
            "initialize",
            json!({
                "protocolVersion":"2025-11-25",
                "capabilities":{},
                "clientInfo":{"name":"tos-source-read-native-test","version":"1"}
            }),
        );
        assert_eq!(init["result"]["protocolVersion"], "2025-11-25");
        client
            .input
            .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
            .expect("send MCP initialized notification");
        client
            .input
            .flush()
            .expect("flush MCP initialized notification");
        let tools = client.rpc("tools/list", json!({}));
        let names = tools["result"]["tools"]
            .as_array()
            .expect("MCP tools list")
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect::<Vec<_>>();
        assert!(
            names.contains(&"tos_source_handle_discover"),
            "MCP source-discover tool missing"
        );
        assert!(
            names.contains(&"tos_source_read"),
            "MCP source-read tool missing"
        );
        client
    }

    fn rpc(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let frame = json!({"jsonrpc":"2.0","id":id,"method":method,"params":params});
        serde_json::to_writer(&mut self.input, &frame).expect("write MCP request");
        self.input.write_all(b"\n").expect("end MCP request frame");
        self.input.flush().expect("flush MCP request");
        let line = self
            .output
            .recv_timeout(Duration::from_secs(15))
            .expect("MCP response deadline or closed stdout");
        let response: Value = serde_json::from_str(&line).expect("parse MCP JSON-RPC response");
        assert_eq!(response["id"], id, "MCP response id");
        assert!(response.get("error").is_none(), "MCP JSON-RPC error");
        response
    }

    fn tool(&mut self, name: &str, arguments: Value) -> Value {
        let response = self.rpc("tools/call", json!({"name":name,"arguments":arguments}));
        let result = &response["result"];
        assert_ne!(result["isError"], true, "MCP tool error");
        if let Some(value) = result.get("structuredContent") {
            return value.clone();
        }
        let text = result["content"][0]["text"]
            .as_str()
            .expect("MCP tool text result");
        serde_json::from_str(text).expect("parse MCP structured JSON text")
    }
}
impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_call(handle: &Value, representation: &str) -> Value {
    json!({"handle":handle,"representation":representation})
}

fn assert_flags(value: &Value) {
    assert_eq!(value["grants_current_use"], false);
    assert_eq!(value["performs_assessment"], false);
    assert_eq!(value["writes_to_source"], false);
}

fn assert_non_disclosure(value: &Value) {
    assert!(
        value["record"].is_null(),
        "denied result disclosed a record"
    );
    assert!(
        value.get("native_unit").is_none_or(Value::is_null),
        "denied result disclosed native text"
    );
    assert!(
        value.get("text_access").is_none_or(Value::is_null),
        "denied result disclosed text rights"
    );
}

fn assert_read_contract(value: &Value, handle: &Value, representation: &str, expected: &str) {
    assert!(
        value["status"] == expected,
        "unexpected exact-source status"
    );
    assert_flags(value);
    assert!(
        value["source_revision"].as_str().is_some(),
        "source revision missing"
    );
    if expected != "stale" {
        assert_eq!(value["source_revision"], handle["epoch"]["source_revision"]);
    }
    assert_eq!(
        value["content_revision"],
        handle["target"]["content_revision"]
    );
    if expected != "available" {
        assert_non_disclosure(value);
        return;
    }
    if representation == "record" {
        assert!(
            value["record"].is_object(),
            "available metadata result lacks record"
        );
        assert_eq!(value["access"]["rights_revalidated"], false);
        assert_eq!(value["access"]["rights_scope"], "metadata-disclosure-only");
    } else {
        assert!(
            value["record"].is_null(),
            "native-unit read returned metadata record"
        );
        assert_eq!(value["text_access"]["recorded_rights_verified"], true);
        assert_eq!(value["text_access"]["grants_current_use"], false);
        if representation == "native_local_unit" {
            assert_eq!(value["text_access"]["conditional_rights"], true);
            assert_eq!(
                value["text_access"]["external_publication_authorized"],
                false
            );
        } else {
            assert_eq!(value["text_access"]["scope"], "public-native-unit");
            assert_eq!(value["text_access"]["conditional_rights"], false);
            assert_ne!(
                value["text_access"]["external_publication_authorized"].as_bool(),
                Some(true)
            );
        }
        assert_eq!(value["native_unit"]["summary"]["content_verified"], true);
        assert_eq!(value["native_unit"]["summary"]["assessment_applied"], false);
    }
}

/// Confirm that the returned metadata witness is the exact source file, not a
/// reserialized record or a nearby projection. Other layers expose different
/// provenance owners and are checked by exact cross-transport equality.
fn assert_metadata_source_bytes(root: &Path, value: &Value) {
    if value["layer"] != "metadata_record" || value["status"] != "available" {
        return;
    }
    let source = &value["provenance"]["source"];
    let source_ref = source["source_ref"]
        .as_str()
        .expect("metadata provenance source_ref");
    let path = Path::new(source_ref);
    assert!(
        !path.is_absolute(),
        "metadata source ref must remain root-relative"
    );
    assert!(
        path.components()
            .all(|part| matches!(part, Component::Normal(_))),
        "metadata source ref must not traverse the selected root"
    );
    let raw = fs::read(root.join(path)).expect("read exact metadata source bytes");
    let digest = source["record_sha256"]
        .as_str()
        .expect("metadata record source digest");
    assert_eq!(
        Digest256::of_bytes(&raw).to_hex(),
        digest,
        "exact metadata source digest"
    );
    assert_eq!(source["record_bytes"].as_u64(), Some(raw.len() as u64));
}

#[test]
#[ignore = "requires an explicit installed binary, prepared owner binding, and owner-selected scenario file"]
fn selected_owner_source_read_crosses_cli_http_mcp_and_cold_processes() {
    let selection = Selection::read();
    let (_http_child, http_address) = start_http(&selection);
    let mut mcp = McpClient::start(&selection);

    for case in &selection.scenarios.cases {
        let discovery_request = json!({"selector":case.selector});
        let cli_discovery = cli_request(&selection, "discover", &discovery_request);
        let http_discovery = http_request(&http_address, "/api/source/handles", &discovery_request);
        let mcp_discovery = mcp.tool("tos_source_handle_discover", discovery_request.clone());
        for discovery in [&cli_discovery, &http_discovery, &mcp_discovery] {
            assert_eq!(
                discovery["status"], case.expected_discovery_status,
                "{} discovery status mismatch",
                case.name
            );
            assert_flags(discovery);
        }
        assert!(
            http_discovery == cli_discovery,
            "{} CLI/HTTP discovery differs",
            case.name
        );
        assert!(
            mcp_discovery == cli_discovery,
            "{} CLI/MCP discovery differs",
            case.name
        );
        let Some(handle) = cli_discovery["handle"]
            .as_object()
            .map(|_| &cli_discovery["handle"])
        else {
            assert_eq!(
                case.expected_read_status, case.expected_discovery_status,
                "{} without an issued handle cannot enter a separate read stage",
                case.name
            );
            if case.expected_discovery_status == "missing" {
                assert!(cli_discovery["target"].is_null());
                assert!(cli_discovery["content_revision"].is_null());
            } else {
                assert!(
                    cli_discovery["target"].is_object(),
                    "{} denied or unverified discovery should retain its typed target",
                    case.name
                );
            }
            continue;
        };
        assert_eq!(
            handle["epoch"]["source_revision"],
            cli_discovery["source_revision"]
        );
        assert_eq!(
            handle["target"]["content_revision"],
            cli_discovery["content_revision"]
        );

        let request = read_call(handle, &case.representation);
        let cli_read = cli_request(&selection, "read", &request);
        let http_read = http_request(&http_address, "/api/source/read", &request);
        let mcp_read = mcp.tool("tos_source_read", request.clone());
        for read in [&cli_read, &http_read, &mcp_read] {
            assert_read_contract(
                read,
                handle,
                &case.representation,
                &case.expected_read_status,
            );
        }
        assert!(http_read == cli_read, "{} CLI/HTTP read differs", case.name);
        assert!(mcp_read == cli_read, "{} CLI/MCP read differs", case.name);
        assert_metadata_source_bytes(&selection.root, &cli_read);

        // A new installed CLI process must replay the same exact owner handle
        // without a cache, latest fallback, or process-local authorization.
        for replay in 0..2 {
            let cold = cli_request(&selection, "read", &request);
            assert!(
                cold == cli_read,
                "{} cold replay {replay} differs",
                case.name
            );
        }
    }

    for stale in &selection.scenarios.stale_reads {
        let request = read_call(&stale.handle, &stale.representation);
        let cli = cli_request(&selection, "read", &request);
        let http = http_request(&http_address, "/api/source/read", &request);
        let mcp_result = mcp.tool("tos_source_read", request.clone());
        for read in [&cli, &http, &mcp_result] {
            assert_read_contract(
                read,
                &stale.handle,
                &stale.representation,
                &stale.expected_status,
            );
        }
        assert!(http == cli, "{} stale CLI/HTTP result differs", stale.name);
        assert!(
            mcp_result == cli,
            "{} stale CLI/MCP result differs",
            stale.name
        );
        for replay in 0..2 {
            let cold = cli_request(&selection, "read", &request);
            assert!(
                cold == cli,
                "{} stale cold replay {replay} differs",
                stale.name
            );
        }
    }
}
