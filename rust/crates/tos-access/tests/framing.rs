use std::io::{Cursor, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use tos_query::{AbortProbe, AbortReason};

use tos_access::{
    AccessError, AccessExecutor, AccessProfile, DisclosureFence, IndexedSearchParams, Params,
    PreparedPacket, cli,
    http::{handle_get, serve, serve_connection},
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
    fn source_descend(
        &self,
        request: Params,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket, AccessError> {
        if let Some(reason) = probe.reason() {
            let code = match reason {
                AbortReason::Cancelled => tos_access::AccessErrorCode::Cancelled,
                AbortReason::DeadlineExceeded => tos_access::AccessErrorCode::DeadlineExceeded,
            };
            return Err(AccessError::new(code, "synthetic request aborted"));
        }
        self.calls.lock().unwrap().push(request);
        Ok(PreparedPacket { body: br#"{"schema":"tos_source_descend_v1","authority_note":"synthetic source-owned boundary","nodes":[],"edges":[]}"#.to_vec(), fence: Box::new(Fence) })
    }
}

#[test]
fn http_half_closed_request_can_receive_its_response() {
    let executor: Arc<dyn AccessExecutor> = Arc::new(Synthetic {
        allowed: true,
        calls: Mutex::new(vec![]),
    });
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        serve_connection(stream, executor, profile());
    });
    let mut client = TcpStream::connect(addr).unwrap();
    client
        .write_all(b"GET /api/source/navigation/tos.x HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .unwrap();
    client.shutdown(Shutdown::Write).unwrap();
    let mut response = Vec::new();
    client.read_to_end(&mut response).unwrap();
    server.join().unwrap();
    assert!(response.starts_with(b"HTTP/1.1 200 OK\r\n"));
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
fn http_refuses_non_loopback_before_listening() {
    let executor: Arc<dyn AccessExecutor> = Arc::new(Synthetic {
        allowed: false,
        calls: Mutex::new(vec![]),
    });
    let error = serve("0.0.0.0:0", executor, profile()).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
}

struct SearchSynthetic {
    available: bool,
    calls: Mutex<Vec<IndexedSearchParams>>,
}
impl AccessExecutor for SearchSynthetic {
    fn source_descend_available(&self) -> bool {
        false
    }
    fn source_descend(
        &self,
        _: Params,
        _: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket, AccessError> {
        unreachable!()
    }
    fn knowledge_search_indexed_available(&self) -> bool {
        self.available
    }
    fn knowledge_search_indexed(
        &self,
        request: IndexedSearchParams,
        _: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket, AccessError> {
        self.calls.lock().unwrap().push(request);
        Ok(PreparedPacket {
            body: br#"{"schema":"tos_knowledge_search_indexed_v2","nodes":[],"relations":[]}"#
                .to_vec(),
            fence: Box::new(Fence),
        })
    }
}

#[test]
fn indexed_search_requires_explicit_mode_and_routes_all_three_wires() {
    let executor = SearchSynthetic {
        available: true,
        calls: Mutex::new(vec![]),
    };
    let unavailable_default = handle_get(
        &executor,
        "GET",
        "/api/knowledge/search?query=abc",
        profile(),
    );
    assert_eq!(unavailable_default.status, 503);
    let http = handle_get(
        &executor,
        "GET",
        "/api/knowledge/search?mode=indexed&query=abc&sources=source.a,source.b&limit=5",
        profile(),
    );
    assert_eq!(http.status, 200);
    assert_eq!(
        executor.calls.lock().unwrap()[0].sources,
        vec!["source.a", "source.b"]
    );
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = cli::run_cli(
        &[
            "knowledge".into(),
            "search".into(),
            "abc".into(),
            "--mode".into(),
            "indexed".into(),
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
    assert!(stdout.starts_with(b"{\"schema\":\"tos_knowledge_search_indexed_v2\""));
    let input = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"tos_knowledge_search","arguments":{"mode":"indexed","query":"abc","limit":5}}}
"#;
    let mut output = Vec::new();
    run_io(Cursor::new(input), &mut output, &executor, profile()).unwrap();
    let frames: Vec<_> = std::str::from_utf8(&output).unwrap().lines().collect();
    assert_eq!(frames.len(), 3);
    assert!(frames[1].contains("tos_knowledge_search"));
    assert!(!frames[1].contains("tos_source_descend"));
    assert!(
        frames[2].contains("\"structuredContent\":{\"schema\":\"tos_knowledge_search_indexed_v2\"")
    );
    assert_eq!(executor.calls.lock().unwrap().len(), 3);
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

struct LargePacket;
impl AccessExecutor for LargePacket {
    fn source_descend_available(&self) -> bool {
        true
    }
    fn source_descend(
        &self,
        _: Params,
        _: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket, AccessError> {
        let mut body = b"{\"schema\":\"synthetic\",\"value\":\"".to_vec();
        body.extend(vec![b'a'; 600]);
        body.extend_from_slice(b"\"}");
        Ok(PreparedPacket {
            body,
            fence: Box::new(Fence),
        })
    }
}

#[test]
fn mcp_rejects_duplicated_packet_over_frame_cap_without_disclosure() {
    let input = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"tos_source_descend","arguments":{"node_id":"tos.x"}}}
"#;
    let mut output = Vec::new();
    run_io(
        Cursor::new(input),
        &mut output,
        &LargePacket,
        AccessProfile::new(65_536, 1024, 65_536),
    )
    .unwrap();
    let frames: Vec<_> = std::str::from_utf8(&output).unwrap().lines().collect();
    assert_eq!(frames.len(), 2);
    assert!(frames[1].contains("MCP response frame exceeds byte budget"));
    assert!(!frames[1].contains("structuredContent"));
    assert!(!frames[1].contains("aaaa"));
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
    fn source_descend(
        &self,
        _: Params,
        _: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket, AccessError> {
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

struct KnowledgeSynthetic {
    calls: Mutex<Vec<tos_access::KnowledgeRequest>>,
    allowed: bool,
}
impl AccessExecutor for KnowledgeSynthetic {
    fn source_descend_available(&self) -> bool {
        false
    }
    fn source_descend(
        &self,
        _: Params,
        _: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket, AccessError> {
        unreachable!()
    }
    fn knowledge_available(&self, _: tos_access::KnowledgeOperation) -> bool {
        self.allowed
    }
    fn knowledge(
        &self,
        request: tos_access::KnowledgeRequest,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket, AccessError> {
        assert!(probe.reason().is_none());
        self.calls.lock().unwrap().push(request);
        Ok(PreparedPacket{body:br#"{"source_revision":"fixture","source_refs":[],"authority_note":"synthetic; no rights grant"}"#.to_vec(),fence:Box::new(Fence)})
    }
}
#[test]
fn selected_knowledge_transport_shapes_use_existing_contracts() {
    use tos_access::{KnowledgeOperation as O, KnowledgeRequest as R};
    let executor = KnowledgeSynthetic {
        calls: Mutex::new(Vec::new()),
        allowed: true,
    };
    for target in [
        "/api/knowledge/catalog",
        "/api/knowledge/nodes/fixture%2Fnode?relation_limit=0",
        "/api/knowledge/relations/fixture%2Frelation",
    ] {
        assert_eq!(handle_get(&executor, "HEAD", target, profile()).status, 200);
    }
    let calls = executor.calls.lock().unwrap();
    assert!(matches!(calls[0], R::Catalog));
    assert!(matches!(&calls[1],R::Node{node_id,relation_limit:0} if node_id=="fixture/node"));
    assert!(matches!(&calls[2],R::Relation{relation_id} if relation_id=="fixture/relation"));
    drop(calls);
    let vectors = [
        (
            O::Temporal,
            "/api/knowledge/temporal/compare",
            "tos_knowledge_temporal_compare",
            "request",
        ),
        (
            O::Lens,
            "/api/knowledge/lenses/compile",
            "tos_knowledge_lens_compile",
            "spec",
        ),
        (
            O::Explore,
            "/api/knowledge/explore",
            "tos_knowledge_explore",
            "request",
        ),
    ];
    for (op, path, tool, field) in vectors {
        assert_eq!(
            tos_access::http::handle_post(&executor, path, br#"{"selection":"raw"}"#, profile())
                .status,
            200
        );
        assert_eq!(
            executor.calls.lock().unwrap().last().unwrap().operation(),
            op
        );
        let frames = format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"protocolVersion\":\"2025-11-25\"}}}}\n{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}}\n{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{{\"name\":\"{tool}\",\"arguments\":{{\"{field}\":{{\"selection\":\"raw\"}}}}}}}}\n"
        );
        let mut out = Vec::new();
        run_io(Cursor::new(frames), &mut out, &executor, profile()).unwrap();
        assert!(
            String::from_utf8(out)
                .unwrap()
                .contains("structuredContent")
        );
        assert_eq!(
            executor.calls.lock().unwrap().last().unwrap().operation(),
            op
        );
    }
    for (args, input) in [
        (vec!["knowledge", "catalog"], ""),
        (
            vec!["knowledge", "node", "fixture/node", "--relation-limit", "0"],
            "",
        ),
        (vec!["knowledge", "relation", "fixture/relation"], ""),
        (vec!["knowledge", "temporal-compare", "-"], "{}"),
        (vec!["lens", "compile", "-"], "{}"),
    ] {
        let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();
        let mut out = Vec::new();
        let mut err = Vec::new();
        assert_eq!(
            cli::run_cli_with_input(
                &args,
                &executor,
                profile(),
                &mut Cursor::new(input),
                &mut out,
                &mut err
            ),
            0
        );
        assert!(err.is_empty());
        assert!(out.ends_with(b"\n"));
    }
    assert_eq!(
        tos_access::http::handle_post(&executor, "/api/knowledge/explore", b"[]", profile()).status,
        400
    );
    let unavailable = KnowledgeSynthetic {
        calls: Mutex::new(Vec::new()),
        allowed: false,
    };
    assert_eq!(
        handle_get(&unavailable, "HEAD", "/api/knowledge/catalog", profile()).status,
        503
    );
    assert_eq!(
        tos_access::http::handle_post(&unavailable, "/api/knowledge/explore", b"{}", profile())
            .status,
        503
    );
    assert!(unavailable.calls.lock().unwrap().is_empty());
}
#[test]
fn post_socket_requires_one_bounded_complete_json_body() {
    let cases = [
        (
            "Content-Type: application/json\r\nContent-Length: 2\r\n",
            "{}",
            200,
        ),
        (
            "Content-Type: application/json\r\nContent-Length: 2\r\nContent-Length: 2\r\n",
            "{}",
            400,
        ),
        (
            "Content-Type: application/json\r\nContent-Length: 2\r\nTransfer-Encoding: chunked\r\n",
            "{}",
            400,
        ),
        (
            "Content-Type: application/json\r\nContent-Length: 3\r\n",
            "{}",
            400,
        ),
        (
            "Content-Type: application/json\r\nContent-Length: 65537\r\n",
            "",
            413,
        ),
    ];
    for (headers, body, status) in cases {
        let executor: Arc<dyn AccessExecutor> = Arc::new(KnowledgeSynthetic {
            calls: Mutex::new(Vec::new()),
            allowed: true,
        });
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            serve_connection(stream, executor, profile());
        });
        let mut client = TcpStream::connect(address).unwrap();
        client
            .write_all(
                format!(
                    "POST /api/knowledge/explore HTTP/1.1\r\nHost: 127.0.0.1\r\n{headers}\r\n{body}"
                )
                .as_bytes(),
            )
            .unwrap();
        client.shutdown(Shutdown::Write).unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).unwrap_or_else(|error| {
            panic!(
                "rejection status {status}, headers {headers:?}: {error}; retained response {}",
                String::from_utf8_lossy(&response)
            )
        });
        server.join().unwrap();
        let split = response
            .windows(4)
            .position(|bytes| bytes == b"\r\n\r\n")
            .expect("complete refusal HTTP header")
            + 4;
        let header = std::str::from_utf8(&response[..split]).unwrap();
        assert!(header.starts_with(&format!("HTTP/1.1 {status} ")));
        let length = header
            .split("\r\n")
            .find_map(|line| line.strip_prefix("Content-Length: "))
            .unwrap()
            .parse::<usize>()
            .unwrap();
        assert_eq!(
            response[split..].len(),
            length,
            "complete HTTP body on status {status}"
        );
        let body = tos_foundation::parse_json(
            &response[split..],
            tos_foundation::JsonMode::PublishedStrict,
            profile().json_limits(),
        )
        .unwrap();
        assert!(body.root().as_object().is_some());
        if status >= 400 {
            assert!(
                body.root()
                    .object_get("error")
                    .and_then(tos_foundation::JsonValue::as_str)
                    .is_some()
            );
        }
    }
}
