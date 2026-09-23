use std::io::{Cursor, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use tos_access::{
    AccessError, AccessExecutor, AccessProfile, DisclosureFence, Params, PreparedPacket, cli,
    http::{handle_get, serve_connection},
    mcp::run_io,
};

struct Synthetic {
    allowed: bool,
    calls: Mutex<Vec<Params>>,
}

struct Fence;
impl DisclosureFence for Fence {
    fn recheck(&mut self) -> Result<(), AccessError> {
        Ok(())
    }
}

impl AccessExecutor for Synthetic {
    fn source_descend_available(&self) -> bool {
        self.allowed
    }
    fn source_descend(&self, request: Params) -> Result<PreparedPacket, AccessError> {
        self.calls.lock().unwrap().push(request);
        Ok(PreparedPacket { body: br#"{"schema":"tos_source_descend_v1","authority_note":"synthetic source-owned boundary","nodes":[],"edges":[]}"#.to_vec(), fence: Box::new(Fence) })
    }
}

fn profile() -> AccessProfile {
    AccessProfile::new(65_536, 1_048_576, 65_536)
}

#[test]
fn http_maps_legacy_route_defaults_and_preserves_packet() {
    let executor = Synthetic {
        allowed: true,
        calls: Mutex::new(vec![]),
    };
    let response = handle_get(
        &executor,
        "GET",
        "/api/source/navigation/tos.%CE%B1?max_depth=bad&limit=400",
        profile(),
    );
    assert_eq!(response.status, 200);
    assert_eq!(
        executor.calls.lock().unwrap()[0],
        Params::new("tos.α".into(), 8, 300).unwrap()
    );
    assert!(
        std::str::from_utf8(&response.body)
            .unwrap()
            .contains("synthetic source-owned boundary")
    );
    let head = handle_get(&executor, "HEAD", "/api/source/navigation/tos.x", profile());
    assert_eq!(head.status, 200);
    assert!(head.head_only);
}

#[test]
fn capability_requires_real_owner_selection() {
    let executor = Synthetic {
        allowed: false,
        calls: Mutex::new(vec![]),
    };
    let response = handle_get(&executor, "GET", "/api/source/navigation/tos.x", profile());
    assert_eq!(response.status, 503);
    assert!(executor.calls.lock().unwrap().is_empty());
    let input = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
"#;
    let mut output = Vec::new();
    run_io(Cursor::new(input), &mut output, &executor, profile()).unwrap();
    let lines = std::str::from_utf8(&output)
        .unwrap()
        .lines()
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert!(lines[1].contains("\"tools\":[]"));
}

#[test]
fn unavailable_head_has_headers_without_an_error_body() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let executor: Arc<dyn AccessExecutor> = Arc::new(Synthetic {
        allowed: false,
        calls: Mutex::new(vec![]),
    });
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        serve_connection(stream, executor, profile());
    });
    let mut client = TcpStream::connect(addr).unwrap();
    client
        .write_all(b"HEAD /api/source/navigation/tos.x HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .unwrap();
    let mut response = Vec::new();
    client.read_to_end(&mut response).unwrap();
    server.join().unwrap();
    assert!(response.starts_with(b"HTTP/1.1 503 Service Unavailable\r\n"));
    assert!(response.ends_with(b"\r\n\r\n"));
}

#[test]
fn mcp_lifecycle_ids_notifications_and_tool_result() {
    let executor = Synthetic {
        allowed: true,
        calls: Mutex::new(vec![]),
    };
    let input = br#"{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":"early","method":"tools/list"}
{"jsonrpc":"2.0","id":"init","method":"initialize","params":{"protocolVersion":"2025-11-25"}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"tos_source_descend","arguments":{"node_id":"tos.\u03b1","max_depth":2,"limit":10}}}
"#;
    let mut output = Vec::new();
    run_io(Cursor::new(input), &mut output, &executor, profile()).unwrap();
    let lines = std::str::from_utf8(&output)
        .unwrap()
        .lines()
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 4);
    assert!(lines[0].contains("-32002"));
    assert!(lines[1].contains("\"id\":\"init\""));
    assert!(lines[2].contains("tos_source_descend"));
    assert!(lines[3].contains("\"id\":3"));
    assert!(lines[3].contains("\"structuredContent\":{\"schema\""));
    assert_eq!(
        executor.calls.lock().unwrap()[0],
        Params::new("tos.α".into(), 2, 10).unwrap()
    );
}

#[test]
fn malformed_mcp_frame_does_not_disclose_packet() {
    let executor = Synthetic {
        allowed: true,
        calls: Mutex::new(vec![]),
    };
    let mut output = Vec::new();
    run_io(Cursor::new(b"{oops\n"), &mut output, &executor, profile()).unwrap();
    assert!(std::str::from_utf8(&output).unwrap().contains("-32700"));
    assert!(executor.calls.lock().unwrap().is_empty());
}

#[test]
fn actual_mcp_stdio_bytes_have_one_frame_per_request_and_no_notification_output() {
    let executor = Synthetic {
        allowed: true,
        calls: Mutex::new(vec![]),
    };
    let input = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"tos_source_descend","arguments":{"node_id":"tos.x"}}}
"#;
    let mut output = Vec::new();
    run_io(Cursor::new(input), &mut output, &executor, profile()).unwrap();
    let lines = std::str::from_utf8(&output)
        .unwrap()
        .lines()
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert!(lines[1].contains("\"structuredContent\":{\"schema\""));
    assert_eq!(executor.calls.lock().unwrap().len(), 1);
}

#[test]
fn actual_loopback_http_bytes_match_selected_packet() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let executor = Arc::new(Synthetic {
        allowed: true,
        calls: Mutex::new(vec![]),
    });
    let server_executor: Arc<dyn AccessExecutor> = executor.clone();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        serve_connection(stream, server_executor, profile());
    });
    let mut client = TcpStream::connect(addr).unwrap();
    client
        .write_all(b"GET /api/source/navigation/tos.x HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .unwrap();
    let mut response = Vec::new();
    client.read_to_end(&mut response).unwrap();
    server.join().unwrap();
    let text = std::str::from_utf8(&response).unwrap();
    assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(text.contains("\r\n\r\n{\"schema\":\"tos_source_descend_v1\""));
    assert_eq!(executor.calls.lock().unwrap().len(), 1);
}

#[test]
fn additive_cli_writes_one_packet_to_stdout_and_diagnostics_to_stderr() {
    let executor = Synthetic {
        allowed: true,
        calls: Mutex::new(vec![]),
    };
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = cli::run_cli(
        &[
            "source".into(),
            "descend".into(),
            "tos.x".into(),
            "--limit".into(),
            "5".into(),
        ],
        &executor,
        profile(),
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    assert!(stdout.ends_with(b"\n"));
    assert!(
        std::str::from_utf8(&stdout)
            .unwrap()
            .contains("synthetic source-owned boundary")
    );
    assert_eq!(
        executor.calls.lock().unwrap()[0],
        Params::new("tos.x".into(), 8, 5).unwrap()
    );
}

struct Withdrawn;
impl DisclosureFence for Withdrawn {
    fn recheck(&mut self) -> Result<(), AccessError> {
        Err(AccessError::new(
            tos_access::AccessErrorCode::PolicyDenied,
            "current source policy denied",
        ))
    }
}
struct WithdrawnExecutor;
impl AccessExecutor for WithdrawnExecutor {
    fn source_descend_available(&self) -> bool {
        true
    }
    fn source_descend(&self, _: Params) -> Result<PreparedPacket, AccessError> {
        Ok(PreparedPacket {
            body: br#"{"secret":"would leak"}"#.to_vec(),
            fence: Box::new(Withdrawn),
        })
    }
}

#[test]
fn final_withdrawal_sends_no_packet_on_any_wire() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    assert_eq!(
        cli::run_cli(
            &["source".into(), "descend".into(), "tos.x".into()],
            &WithdrawnExecutor,
            profile(),
            &mut stdout,
            &mut stderr
        ),
        1
    );
    assert!(stdout.is_empty());
    assert!(
        std::str::from_utf8(&stderr)
            .unwrap()
            .contains("policy_denied")
    );

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        serve_connection(stream, Arc::new(WithdrawnExecutor), profile());
    });
    let mut client = TcpStream::connect(addr).unwrap();
    client
        .write_all(b"GET /api/source/navigation/tos.x HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .unwrap();
    let mut response = Vec::new();
    client.read_to_end(&mut response).unwrap();
    server.join().unwrap();
    let http = std::str::from_utf8(&response).unwrap();
    assert!(http.starts_with("HTTP/1.1 403 Forbidden\r\n"));
    assert!(!http.contains("would leak"));

    let input = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"tos_source_descend","arguments":{"node_id":"tos.x"}}}
"#;
    let mut output = Vec::new();
    run_io(
        Cursor::new(input),
        &mut output,
        &WithdrawnExecutor,
        profile(),
    )
    .unwrap();
    let text = std::str::from_utf8(&output).unwrap();
    assert!(text.contains("\"isError\":true"));
    assert!(!text.contains("would leak"));
}
