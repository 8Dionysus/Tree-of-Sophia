use std::io::{Cursor, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use tos_foundation::{JsonLimits, JsonMode, JsonValue, parse_json};
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
    ) -> Result<PreparedPacket<'static>, AccessError> {
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

/// One actual HTTP connection over the production MCP socket path, using the
/// existing transport fixture executor and shared session state.
fn mcp_http_roundtrip(
    executor: Arc<dyn AccessExecutor>,
    sessions: Arc<tos_access::mcp_http::HttpSessions>,
    method: &str,
    body: &[u8],
    extra: &str,
) -> (u16, String, Vec<u8>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        tos_access::mcp_http::serve_connection(stream, executor, profile(), sessions);
    });
    let mut client = TcpStream::connect(address).unwrap();
    client
        .set_read_timeout(Some(std::time::Duration::from_secs(6)))
        .unwrap();
    client
        .set_write_timeout(Some(std::time::Duration::from_secs(2)))
        .unwrap();
    write!(client, "{method} /mcp HTTP/1.1\r\nHost: {address}\r\nAccept: application/json, text/event-stream\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{extra}\r\n", body.len()).unwrap();
    client.write_all(body).unwrap();
    client.shutdown(Shutdown::Write).unwrap();
    let mut raw = Vec::new();
    client.read_to_end(&mut raw).unwrap();
    server.join().unwrap();
    assert!(raw.len() <= 2_097_152);
    if raw.is_empty() {
        return (0, String::new(), raw);
    }
    let split = raw.windows(4).position(|v| v == b"\r\n\r\n").unwrap();
    let head = std::str::from_utf8(&raw[..split]).unwrap().to_owned();
    let code = head.split_whitespace().nth(1).unwrap().parse().unwrap();
    (code, head, raw[split + 4..].to_vec())
}

#[test]
fn mcp_streamable_http_real_session_packets_and_transport_boundaries() {
    let concrete = Arc::new(Synthetic {
        allowed: true,
        calls: Mutex::new(vec![]),
    });
    let executor: Arc<dyn AccessExecutor> = concrete.clone();
    let sessions = Arc::new(tos_access::mcp_http::HttpSessions::default());
    let initialize = br#"{"jsonrpc":"2.0","id":"init","method":"initialize","params":{"protocolVersion":"2025-11-25"}}"#;
    let (code, head, raw) =
        mcp_http_roundtrip(executor.clone(), sessions.clone(), "POST", initialize, "");
    assert_eq!(code, 200);
    let parsed = parse_json(&raw, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    assert_eq!(
        parsed
            .root()
            .object_get("result")
            .unwrap()
            .object_get("protocolVersion")
            .unwrap()
            .as_str(),
        Some("2025-11-25")
    );
    let id = head
        .lines()
        .find_map(|line| line.strip_prefix("MCP-Session-Id: "))
        .unwrap();
    assert_eq!(id.len(), 64);
    assert!(id.bytes().all(|v| v.is_ascii_hexdigit()));
    let headers = format!("MCP-Session-Id: {id}\r\nMCP-Protocol-Version: 2025-11-25\r\n");
    let notification = br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
    let (code, _, body) = mcp_http_roundtrip(
        executor.clone(),
        sessions.clone(),
        "POST",
        notification,
        &headers,
    );
    assert_eq!(code, 202);
    assert!(body.is_empty());
    let list = br#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
    let (code, _, raw) =
        mcp_http_roundtrip(executor.clone(), sessions.clone(), "POST", list, &headers);
    assert_eq!(code, 200);
    assert!(
        std::str::from_utf8(&raw)
            .unwrap()
            .contains("tos_source_descend")
    );
    let request = br#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"tos_source_descend","arguments":{"node_id":"tos.literal"}}}"#;
    let (code, _, http_packet) = mcp_http_roundtrip(
        executor.clone(),
        sessions.clone(),
        "POST",
        request,
        &headers,
    );
    assert_eq!(code, 200);
    let mut stdio = Vec::new();
    let mut input = initialize.to_vec();
    input.push(b'\n');
    input.extend(notification);
    input.push(b'\n');
    input.extend(request);
    input.push(b'\n');
    run_io(Cursor::new(input), &mut stdio, executor.as_ref(), profile()).unwrap();
    assert_eq!(
        http_packet,
        stdio
            .split(|v| *v == b'\n')
            .filter(|v| !v.is_empty())
            .last()
            .unwrap()
    );
    assert_eq!(concrete.calls.lock().unwrap().len(), 2);
    assert_eq!(
        mcp_http_roundtrip(executor.clone(), sessions.clone(), "GET", b"", &headers).0,
        405
    );
    assert_eq!(
        mcp_http_roundtrip(executor.clone(), sessions.clone(), "POST", list, "").0,
        400
    );
    assert_eq!(
        mcp_http_roundtrip(
            executor.clone(),
            sessions.clone(),
            "POST",
            list,
            "MCP-Session-Id: unknown\r\n"
        )
        .0,
        404
    );
    assert_eq!(
        mcp_http_roundtrip(
            executor.clone(),
            sessions.clone(),
            "POST",
            list,
            &format!("MCP-Session-Id: {id}\r\nMCP-Protocol-Version: invalid\r\n")
        )
        .0,
        400
    );
    assert_eq!(
        mcp_http_roundtrip(
            executor.clone(),
            sessions.clone(),
            "POST",
            list,
            &format!("{headers}Origin: https://foreign.example\r\n")
        )
        .0,
        403
    );
    assert_eq!(
        mcp_http_roundtrip(executor.clone(), sessions.clone(), "DELETE", b"", &headers).0,
        200
    );
    assert_eq!(
        mcp_http_roundtrip(executor.clone(), sessions.clone(), "POST", list, &headers).0,
        404
    );
    assert_eq!(
        mcp_http_roundtrip(executor, sessions, "GET", b"", &headers).0,
        404
    );
}

#[test]
fn mcp_streamable_http_cancellation_reaches_the_held_operation() {
    use std::sync::atomic::{AtomicBool, Ordering};
    struct Blocking {
        started: AtomicBool,
    }
    impl AccessExecutor for Blocking {
        fn source_descend_available(&self) -> bool {
            true
        }
        fn source_descend(
            &self,
            _: Params,
            probe: Arc<dyn AbortProbe>,
        ) -> Result<PreparedPacket<'static>, AccessError> {
            self.started.store(true, Ordering::Release);
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            while std::time::Instant::now() < deadline {
                if probe.reason() == Some(AbortReason::Cancelled) {
                    return Err(AccessError::new(
                        tos_access::AccessErrorCode::Cancelled,
                        "cancelled actual held operation",
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            panic!("MCP cancellation did not reach active request");
        }
    }
    let blocking = Arc::new(Blocking {
        started: AtomicBool::new(false),
    });
    let executor: Arc<dyn AccessExecutor> = blocking.clone();
    let sessions = Arc::new(tos_access::mcp_http::HttpSessions::default());
    let (_,head,_)=mcp_http_roundtrip(executor.clone(),sessions.clone(),"POST",br#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}"#,"");
    let id = head
        .lines()
        .find_map(|v| v.strip_prefix("MCP-Session-Id: "))
        .unwrap();
    let headers = format!("MCP-Session-Id: {id}\r\nMCP-Protocol-Version: 2025-11-25\r\n");
    assert_eq!(
        mcp_http_roundtrip(
            executor.clone(),
            sessions.clone(),
            "POST",
            br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            &headers
        )
        .0,
        202
    );
    let e = executor.clone();
    let s = sessions.clone();
    let h = headers.clone();
    let pending = std::thread::spawn(move || {
        mcp_http_roundtrip(e,s,"POST",br#"{"jsonrpc":"2.0","id":"held","method":"tools/call","params":{"name":"tos_source_descend","arguments":{"node_id":"tos.literal"}}}"#,&h)
    });
    let wait = std::time::Instant::now() + std::time::Duration::from_secs(1);
    while !blocking.started.load(Ordering::Acquire) {
        assert!(std::time::Instant::now() < wait);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let (code, _, raw) = mcp_http_roundtrip(
        executor,
        sessions,
        "POST",
        br#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"held"}}"#,
        &headers,
    );
    assert_eq!(code, 202);
    assert!(raw.is_empty());
    let (code, _, raw) = pending.join().unwrap();
    // The active executor observed cancellation; the same output probe now
    // prevents publishing a response after the cancelled request boundary.
    assert_eq!(code, 0);
    assert!(raw.is_empty());
}

#[test]
fn mcp_streamable_http_delayed_packet_and_stalled_output_share_one_deadline() {
    use std::os::fd::AsRawFd;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};
    struct Held(Arc<AtomicBool>);
    impl DisclosureFence for Held {
        fn recheck(&mut self) -> Result<(), AccessError> {
            Ok(())
        }
    }
    impl Drop for Held {
        fn drop(&mut self) {
            self.0.store(false, Ordering::Release);
        }
    }
    struct Delayed(Arc<AtomicBool>);
    impl AccessExecutor for Delayed {
        fn source_descend_available(&self) -> bool {
            true
        }
        fn source_descend(
            &self,
            _: Params,
            _: Arc<dyn AbortProbe>,
        ) -> Result<PreparedPacket<'static>, AccessError> {
            std::thread::sleep(Duration::from_millis(200));
            let mut body = br#"{"padding":""#.to_vec();
            body.extend(std::iter::repeat_n(b'x', 262_144));
            body.extend_from_slice(br#""}"#);
            self.0.store(true, Ordering::Release);
            Ok(PreparedPacket {
                body,
                fence: Box::new(Held(self.0.clone())),
            })
        }
    }
    fn small_buffer(stream: &TcpStream, option: libc::c_int) {
        let bytes: libc::c_int = 4096;
        // Test-owned TCP descriptor only; shrink buffers to force real output
        // backpressure without changing the production listener or packet cap.
        assert_eq!(
            unsafe {
                libc::setsockopt(
                    stream.as_raw_fd(),
                    libc::SOL_SOCKET,
                    option,
                    (&bytes as *const libc::c_int).cast(),
                    std::mem::size_of_val(&bytes) as libc::socklen_t,
                )
            },
            0
        );
    }
    let held = Arc::new(AtomicBool::new(false));
    let executor: Arc<dyn AccessExecutor> = Arc::new(Delayed(held.clone()));
    let sessions = Arc::new(tos_access::mcp_http::HttpSessions::default());
    let (_,head,_)=mcp_http_roundtrip(executor.clone(),sessions.clone(),"POST",br#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}"#,"");
    let id = head
        .lines()
        .find_map(|v| v.strip_prefix("MCP-Session-Id: "))
        .unwrap();
    let headers = format!("MCP-Session-Id: {id}\r\nMCP-Protocol-Version: 2025-11-25\r\n");
    assert_eq!(
        mcp_http_roundtrip(
            executor.clone(),
            sessions.clone(),
            "POST",
            br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            &headers
        )
        .0,
        202
    );
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (done, finished) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        small_buffer(&stream, libc::SO_SNDBUF);
        let started = Instant::now();
        tos_access::mcp_http::serve_connection(
            stream,
            executor,
            profile().with_query_timeout(Duration::from_millis(400)),
            sessions,
        );
        done.send(started.elapsed()).unwrap();
    });
    let mut client = TcpStream::connect(address).unwrap();
    small_buffer(&client, libc::SO_RCVBUF);
    client
        .set_write_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let body=br#"{"jsonrpc":"2.0","id":"delayed","method":"tools/call","params":{"name":"tos_source_descend","arguments":{"node_id":"tos.literal"}}}"#;
    write!(client,"POST /mcp HTTP/1.1\r\nHost: {address}\r\nAccept: application/json, text/event-stream\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{headers}\r\n",body.len()).unwrap();
    client.write_all(body).unwrap();
    client.shutdown(Shutdown::Write).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    // Consume exactly the header, leaving the packet unread. This witnesses
    // actual output while the disclosure fence must remain held.
    let mut raw = Vec::new();
    while !raw.ends_with(b"\r\n\r\n") {
        assert!(raw.len() < 8192, "HTTP header exceeded control budget");
        let mut byte = [0];
        client.read_exact(&mut byte).unwrap();
        raw.push(byte[0]);
    }
    assert!(
        raw.starts_with(b"HTTP/1.1 200 OK\r\n"),
        "control never reached actual packet output"
    );
    let split = raw.len();
    let declared: usize = std::str::from_utf8(&raw)
        .unwrap()
        .lines()
        .find_map(|v| v.strip_prefix("Content-Length: "))
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        declared > 262_144,
        "control emitted a refusal instead of the large packet"
    );
    assert!(
        held.load(Ordering::Acquire),
        "fence released while output is active"
    );
    assert!(
        matches!(
            finished.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ),
        "output completed without the intended backpressure"
    );
    // Do not consume the body: granting output a fresh5s cannot finish this
    // blocked connection within the same400ms request lifetime.
    let elapsed = finished
        .recv_timeout(Duration::from_millis(700))
        .expect("HTTP output acquired a fresh deadline");
    assert!(elapsed < Duration::from_millis(700));
    assert!(
        !held.load(Ordering::Acquire),
        "fence not released after bounded output termination"
    );
    server.join().unwrap();
    let _ = client.read_to_end(&mut raw);
    let delimiter = raw.windows(4).position(|v| v == b"\r\n\r\n").unwrap() + 4;
    assert_eq!(delimiter, split);
    assert!(
        raw.len() - delimiter < declared,
        "stalled output delivered the complete declared HTTP body"
    );
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
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"tos_zarathustra_reading_search","arguments":{"query":"x"}}}
"#;
    let mut output = Vec::new();
    run_io(Cursor::new(input), &mut output, &executor, profile()).unwrap();
    let lines = std::str::from_utf8(&output)
        .unwrap()
        .lines()
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 3);
    let document = parse_json(
        lines[1].as_bytes(),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap();
    let tools = document
        .root()
        .object_get("result")
        .unwrap()
        .object_get("tools")
        .unwrap()
        .as_array()
        .unwrap();
    let names = tools
        .iter()
        .map(|tool| {
            tool.object_get("name")
                .and_then(tos_foundation::JsonValue::as_str)
                .unwrap()
                .to_owned()
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        names,
        std::collections::BTreeSet::from([
            tos_access::exploration_contracts::OPERATION.to_owned(),
            tos_access::reading::MCP_TOOL.to_owned(),
            "tos_zarathustra_reading_public_capability".to_owned(),
            "tos_zarathustra_word_analysis_public_capability".to_owned(),
            "tos_source_read_capabilities".to_owned(),
            "tos_source_read_contract".to_owned(),
        ]),
        "only packaged contracts and unavailable Reading/Word capability metadata need no data owner"
    );
    let reading = parse_json(
        lines[2].as_bytes(),
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap();
    let capability = reading
        .root()
        .object_get("result")
        .unwrap()
        .object_get("structuredContent")
        .unwrap();
    assert_eq!(
        capability.object_get("available"),
        Some(&tos_foundation::JsonValue::Bool(false))
    );
    assert!(matches!(
        capability.object_get("result"),
        Some(tos_foundation::JsonValue::Null)
    ));
    assert!(executor.calls.lock().unwrap().is_empty());
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
    ) -> Result<PreparedPacket<'static>, AccessError> {
        unreachable!()
    }
    fn knowledge_search_indexed_available(&self) -> bool {
        self.available
    }
    fn knowledge_search_indexed(
        &self,
        request: IndexedSearchParams,
        _: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
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
    for invalid in [
        "/api/knowledge/search?mode=indexed&query=abc&cursor=",
        "/api/knowledge/search?mode=indexed&query=abc&cursor=%ZZ",
        "/api/knowledge/search?mode=indexed&query=abc&cursor=one&cursor=two",
    ] {
        assert_eq!(handle_get(&executor, "GET", invalid, profile()).status, 400);
    }
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
    ) -> Result<PreparedPacket<'static>, AccessError> {
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
    ) -> Result<PreparedPacket<'static>, AccessError> {
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
    ) -> Result<PreparedPacket<'static>, AccessError> {
        unreachable!()
    }
    fn knowledge_available(&self, _: tos_access::KnowledgeOperation) -> bool {
        self.allowed
    }
    fn knowledge(
        &self,
        request: tos_access::KnowledgeRequest,
        probe: Arc<dyn AbortProbe>,
    ) -> Result<PreparedPacket<'static>, AccessError> {
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
            "Content-Type: text/plain\r\nContent-Length: 2\r\n",
            "{}",
            415,
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

#[test]
fn mcp_bounds_metadata_and_refusal_frames_before_output() {
    let input = br#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":"list\n","method":"tools/list"}
"#;
    let executor = Synthetic {
        allowed: true,
        calls: Mutex::new(vec![]),
    };
    let profile = profile().with_mcp_frame_budget(256);
    let mut output = Vec::new();
    run_io(Cursor::new(input), &mut output, &executor, profile).unwrap();
    let lines: Vec<_> = output
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .collect();
    assert_eq!(lines.len(), 2);
    for line in &lines {
        assert!(line.len() + 1 <= profile.max_mcp_frame_bytes);
    }
    let result = parse_json(lines[1], JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    assert_eq!(
        result.root().object_get("id").unwrap().as_str(),
        Some("list\n")
    );
    assert!(result.root().object_get("error").is_some());
    assert!(!String::from_utf8_lossy(lines[1]).contains("inputSchema"));
    let mut output = Vec::new();
    assert!(
        run_io(
            Cursor::new(input),
            &mut output,
            &executor,
            profile.with_mcp_frame_budget(1)
        )
        .is_err()
    );
    assert!(
        output.is_empty(),
        "unrepresentable refusal must emit no oversized frame"
    );
}

#[test]
fn exploration_software_contracts_survive_unrelated_data_selection_and_all_native_wires() {
    use tos_foundation::{CanonicalProfile, JsonValue, canonical_bytes_v1};
    let executor = Synthetic {
        allowed: false,
        calls: Mutex::new(vec![]),
    };
    let profile = profile();
    let route = tos_access::registered_operations()
        .unwrap()
        .iter()
        .find(|op| op.operation_id == tos_access::exploration_contracts::OPERATION)
        .unwrap();
    assert!(route.cli_command.is_none());
    let response = handle_get(&executor, "GET", &route.http_path, profile);
    assert_eq!(response.status, 200);
    let expected = response.body.clone();
    let document = parse_json(&expected, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
    let capabilities = document.root().object_get("capabilities").unwrap();
    assert_eq!(
        capabilities.object_get("available"),
        Some(&JsonValue::Bool(false))
    );
    assert_eq!(
        capabilities.object_get("restart_survival"),
        Some(&JsonValue::Bool(false))
    );
    assert_eq!(
        capabilities
            .object_get("storage")
            .and_then(JsonValue::as_str),
        Some("unavailable")
    );
    assert_eq!(
        capabilities
            .object_get("max_checkpoints")
            .and_then(JsonValue::as_u64),
        Some(0)
    );
    let capability_bytes = canonical_bytes_v1(
        capabilities,
        CanonicalProfile::SourceRecordDigestV1,
        JsonLimits::default(),
    )
    .unwrap();
    let capability_route = "/api/knowledge/explore/capabilities";
    let capability_get = handle_get(&executor, "GET", capability_route, profile);
    assert_eq!(capability_get.status, 200);
    assert_eq!(capability_get.body, capability_bytes);
    assert_eq!(
        handle_get(
            &executor,
            "GET",
            capability_route,
            profile.with_query_timeout(std::time::Duration::ZERO),
        )
        .status,
        408
    );
    let capability_head = handle_get(&executor, "HEAD", capability_route, profile);
    assert_eq!(capability_head.status, 200);
    let mut capability_output = vec![];
    tos_access::http::write_response(&mut capability_output, capability_head).unwrap();
    assert!(capability_output.ends_with(b"\r\n\r\n"));
    assert!(
        String::from_utf8_lossy(&capability_output)
            .contains(&format!("Content-Length: {}", capability_bytes.len()))
    );
    let socket_capability = |method: &str| {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            serve_connection(stream, Arc::new(tos_access::NoOwner), profile);
        });
        let mut client = TcpStream::connect(address).unwrap();
        client
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        client
            .write_all(
                format!("{method} {capability_route} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
                    .as_bytes(),
            )
            .unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).unwrap();
        server.join().unwrap();
        response
    };
    for method in ["GET", "HEAD"] {
        let response = socket_capability(method);
        let split = response
            .windows(4)
            .position(|part| part == b"\r\n\r\n")
            .unwrap();
        let headers = std::str::from_utf8(&response[..split]).unwrap();
        assert!(headers.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(headers.lines().any(|line| line.trim_end_matches('\r')
            == format!("Content-Length: {}", capability_bytes.len())));
        assert_eq!(
            &response[split + 4..],
            if method == "GET" {
                capability_bytes.as_slice()
            } else {
                b""
            },
        );
    }
    for (key, raw) in tos_access::exploration_contracts::CONTRACTS {
        let original = parse_json(raw, JsonMode::PublishedStrict, JsonLimits::default()).unwrap();
        let canonical = |value| {
            canonical_bytes_v1(
                value,
                CanonicalProfile::CorpusSnapshotV1,
                JsonLimits::default(),
            )
            .unwrap()
        };
        assert_eq!(
            canonical(document.root().object_get(key).unwrap()),
            canonical(original.root())
        );
    }
    // Runtime request/data selection cannot substitute software schema bytes.
    let attempted = handle_get(
        &executor,
        "GET",
        &format!("{}?root=/missing-data&request=overridden", route.http_path),
        profile,
    );
    assert_eq!(attempted.status, 200);
    assert_eq!(attempted.body, expected);
    let head = handle_get(&executor, "HEAD", &route.http_path, profile);
    assert_eq!(head.status, 200);
    let mut output = vec![];
    tos_access::http::write_response(&mut output, head).unwrap();
    assert!(output.ends_with(b"\r\n\r\n"));
    assert!(
        String::from_utf8_lossy(&output).contains(&format!("Content-Length: {}", expected.len()))
    );
    let input = format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"protocolVersion\":\"2025-11-25\"}}}}\n{{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}}\n{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}}\n{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{{\"name\":\"{}\",\"arguments\":{{}}}}}}\n",
        route.mcp_tool
    );
    let mcp_profile = profile.with_mcp_frame_budget(
        tos_access::mcp::tool_result_frame_byte_bound(
            profile.max_response_bytes,
            profile.max_request_bytes,
        )
        .unwrap(),
    );
    // A valid unrelated data selection cannot replace software-owned schemas.
    let data_root = std::env::temp_dir().join(format!(
        "tos-unrelated-empty-data-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&data_root).unwrap();
    let reading_executor =
        tos_access::reading::ReadingLocalExecutor::open(data_root.clone()).unwrap();
    output.clear();
    run_io(
        Cursor::new(input.as_bytes()),
        &mut output,
        &reading_executor,
        mcp_profile,
    )
    .unwrap();
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_tos-access"))
        .arg("mcp")
        .env_remove("TOS_RELEASE_ROOT")
        .env("TOS_DATA_ROOT", &data_root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let installed = child.wait_with_output().unwrap();
    std::fs::remove_dir(&data_root).unwrap();
    assert!(
        installed.status.success(),
        "{}",
        String::from_utf8_lossy(&installed.stderr)
    );
    assert_eq!(installed.stdout, output);
    let frames = output
        .split(|b| *b == b'\n')
        .filter(|frame| !frame.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(frames.len(), 3, "one complete JSON value per MCP line");
    let last = parse_json(
        frames.last().unwrap(),
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: mcp_profile.max_mcp_frame_bytes,
            ..JsonLimits::default()
        },
    )
    .unwrap();
    let result = last.root().object_get("result").unwrap();
    assert_eq!(
        result.object_get("content").unwrap().as_array().unwrap()[0]
            .object_get("text")
            .unwrap()
            .as_str()
            .unwrap()
            .as_bytes(),
        expected
    );
    assert_eq!(
        canonical_bytes_v1(
            result.object_get("structuredContent").unwrap(),
            CanonicalProfile::SourceRecordDigestV1,
            JsonLimits::default()
        )
        .unwrap(),
        expected
    );
    assert!(executor.calls.lock().unwrap().is_empty());
    assert!(tos_access::exploration_contracts::execute(&executor, 32).is_err());
    let deadline = profile.with_query_timeout(std::time::Duration::ZERO);
    assert_eq!(
        handle_get(&executor, "GET", &route.http_path, deadline).status,
        408
    );
}

#[test]
fn source_backed_doctor_verify_binary_preserves_diagnostic_boundaries() {
    use std::{
        fs,
        path::Path,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let directory = std::env::temp_dir().join(format!(
        "tos-doctor-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    // Reuse the maintained source-shaped fixture; Python is fixture setup only,
    // never a diagnostic runtime/backend or packet oracle.
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap();
    let setup=Command::new("python3").args(["-c","import sys;from pathlib import Path;sys.path.insert(0,sys.argv[1]);from fixture_support import write_fixture;write_fixture(Path(sys.argv[2]))"])
        .arg(repository.join("access/tests")).arg(&directory).output().unwrap();
    assert!(
        setup.status.success(),
        "{}",
        String::from_utf8_lossy(&setup.stderr)
    );
    let run = |args: &[&str]| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tos-access"));
        command.arg("--root").arg(&directory).args(args);
        for name in [
            "TOS_RELEASE_ROOT",
            "TOS_DATA_ROOT",
            "TOS_QUERY_STORE_PATH",
            "TOS_CORPUS_INDEX_PATH",
            "TOS_PHILOSOPHY_GRAPH_PROJECTION_PATH",
            "TOS_EVIDENCE_PROJECTION_PATH",
            "TOS_BIBLIOGRAPHIC_GRAPH_PATH",
            "TOS_ENTITY_TYPE_REGISTRY_PATH",
            "TOS_RELATION_TYPE_REGISTRY_PATH",
            "TOS_ABYSSOS_ROOT",
        ] {
            command.env_remove(name);
        }
        command.output().unwrap()
    };
    let report = run(&["doctor", "--json"]);
    assert!(matches!(report.status.code(), Some(0) | Some(1)));
    let report_exit = report.status.code();
    assert!(
        report.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&report.stderr)
    );
    let report = parse_json(
        &report.stdout,
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap();
    assert_eq!(
        report.root().object_get("schema_version").unwrap().as_str(),
        Some("tos_access_doctor_report_v1")
    );
    let checks = report
        .root()
        .object_get("checks")
        .unwrap()
        .as_array()
        .unwrap();
    let check = |id: &str| {
        checks
            .iter()
            .find(|row| row.object_get("check_id").unwrap().as_str() == Some(id))
            .unwrap()
    };
    for id in [
        "corpus-index-schema",
        "philosophy-graph-schema",
        "graph-view-materialization",
        "evidence-projection-schema",
        "runtime-contracts",
        "native-mcp-dependency",
    ] {
        assert_eq!(
            check(id).object_get("ok"),
            Some(&tos_foundation::JsonValue::Bool(true)),
            "{id}"
        );
    }
    assert_eq!(
        check("runtime-contracts")
            .object_get("path")
            .unwrap()
            .as_str(),
        Some("embedded:access/contracts")
    );
    let program_web = Path::new(env!("CARGO_BIN_EXE_tos-access"))
        .parent()
        .unwrap()
        .join("web_dist");
    if let Some(path) = check("web-assets").object_get("path").unwrap().as_str() {
        assert_eq!(
            Path::new(path),
            program_web,
            "data-root software markers cannot supply executable code"
        );
    } else {
        assert_eq!(
            report_exit,
            Some(1),
            "missing installed web companion is a required failure"
        );
    }
    assert!(
        !checks
            .iter()
            .any(|row| row.object_get("check_id").unwrap().as_str() == Some("query-store"))
    );
    let rendered = run(&["doctor"]);
    assert_eq!(rendered.status.code(), report_exit);
    let state = if report_exit == Some(0) {
        "ready"
    } else {
        "not ready"
    };
    assert!(
        String::from_utf8_lossy(&rendered.stdout).starts_with(&format!(
            "Tree of Sophia access: {state} (standalone)\n[ok] corpus-index-present\n"
        ))
    );
    let verify = run(&["verify", "--json"]);
    assert_eq!(verify.status.code(), report_exit);
    let verify = parse_json(
        &verify.stdout,
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap();
    assert_eq!(
        verify
            .root()
            .object_get("checks")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(
                |row| row.object_get("check_id").unwrap().as_str() == Some("native-mcp-dependency")
            )
            .unwrap()
            .object_get("required"),
        Some(&tos_foundation::JsonValue::Bool(true))
    );
    let prepared = Command::new(env!("CARGO_BIN_EXE_tos-access"))
        .args([
            "--prepared-read-model",
            "unopened.sqlite",
            "--prepared-binding",
            "unopened.json",
            "doctor",
        ])
        .output()
        .unwrap();
    assert_eq!(prepared.status.code(), Some(2));
    assert!(prepared.stdout.is_empty());
    assert!(String::from_utf8_lossy(&prepared.stderr).contains("source-backed profile"));
    let abyss = directory.join("AbyssOS");
    fs::create_dir_all(abyss.join("abyss-stack")).unwrap();
    let abyss = Command::new(env!("CARGO_BIN_EXE_tos-access"))
        .arg("--root")
        .arg(&directory)
        .args(["verify", "--profile=abyssos", "--json"])
        .env("TOS_ABYSSOS_ROOT", format!(" {} ", abyss.display()))
        .env_remove("TOS_RELEASE_ROOT")
        .env_remove("TOS_QUERY_STORE_PATH")
        .env_remove("TOS_DATA_ROOT")
        .env_remove("TOS_CORPUS_INDEX_PATH")
        .env_remove("TOS_PHILOSOPHY_GRAPH_PROJECTION_PATH")
        .env_remove("TOS_EVIDENCE_PROJECTION_PATH")
        .env_remove("TOS_BIBLIOGRAPHIC_GRAPH_PATH")
        .output()
        .unwrap();
    assert_eq!(abyss.status.code(), Some(1));
    let abyss = parse_json(
        &abyss.stdout,
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap();
    assert_eq!(
        abyss
            .root()
            .object_get("checks")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row.object_get("check_id").unwrap().as_str() == Some("abyssos-integration"))
            .unwrap()
            .object_get("ok"),
        Some(&tos_foundation::JsonValue::Bool(true)),
        "maintained profile trims configured root and freeze remains a separate check"
    );
    assert!(
        abyss
            .root()
            .object_get("required_failures")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id.as_str() == Some("abyssos-integration-freeze")),
        "selected data cannot unpause packaged software integration"
    );
    let invalid = run(&["verify", "--profile=unknown"]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(invalid.stdout.is_empty());
    let store = directory.join("ToS/derived-exports/runtime/knowledge.sqlite3");
    fs::create_dir_all(store.parent().unwrap()).unwrap();
    fs::write(&store, []).unwrap();
    let unsupported = run(&["doctor", "--json"]);
    assert_eq!(unsupported.status.code(), Some(1));
    let unsupported = parse_json(
        &unsupported.stdout,
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap();
    assert!(
        unsupported
            .root()
            .object_get("required_failures")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id.as_str() == Some("query-store")),
        "a store file marker cannot establish native readiness"
    );
    fs::remove_file(&store).unwrap();
    // A real completed legacy store and partitioned source remain supported
    // inputs during replacement. Compile the maintained fixture once; runtime
    // below is exclusively the native CLI, including stale/journal refusals.
    let setup = Command::new("python3")
        .args([
            "-c",
            r#"
import sys
from pathlib import Path
sys.path[:0] = [str(Path(sys.argv[1]) / 'access/src'), sys.argv[1]]
from scripts.partitioned_projection_common import write_partitioned_payload
from tos_access.knowledge_compile import compile_knowledge_store
import json
root = Path(sys.argv[2])
for relative in (
    'ToS/derived-exports/tos_corpus_index.min.json',
    'ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json',
    'ToS/derived-exports/philosophy_graph_projection.min.json',
):
    path = root / relative
    value = json.loads(path.read_text())
    if value.get('schema_version') == 'tos_source_witness_bibliographic_graph_v1':
        value['input_digests'] = {}
    write_partitioned_payload(path, value)
compile_knowledge_store(root, root / 'ToS/derived-exports/runtime/knowledge.sqlite3')
"#,
        ])
        .arg(repository)
        .arg(&directory)
        .output()
        .unwrap();
    assert!(
        setup.status.success(),
        "{}",
        String::from_utf8_lossy(&setup.stderr)
    );
    let ready = run(&["verify", "--json"]);
    let ready = parse_json(
        &ready.stdout,
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap();
    let rows = ready
        .root()
        .object_get("checks")
        .unwrap()
        .as_array()
        .unwrap();
    for id in [
        "query-store",
        "corpus-index-schema",
        "philosophy-graph-schema",
        "graph-view-materialization",
    ] {
        let row = rows
            .iter()
            .find(|row| row.object_get("check_id").unwrap().as_str() == Some(id))
            .unwrap();
        assert_eq!(
            row.object_get("ok"),
            Some(&tos_foundation::JsonValue::Bool(true)),
            "{id}: {:?}",
            row
        );
    }
    let rejected_store = |output: std::process::Output| {
        assert_eq!(output.status.code(), Some(1));
        let report = parse_json(
            &output.stdout,
            JsonMode::PublishedStrict,
            JsonLimits::default(),
        )
        .unwrap();
        assert!(
            report
                .root()
                .object_get("required_failures")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .any(|id| id.as_str() == Some("query-store"))
        );
    };
    let registry = directory.join("ToS/doctrine/semantic-interchange/entity-types.v1.json");
    let original = fs::read(&registry).unwrap();
    let mut changed = original.clone();
    changed.push(b'\n');
    fs::write(&registry, changed).unwrap();
    rejected_store(run(&["doctor", "--json"]));
    fs::write(&registry, original).unwrap();
    let journal = store.with_file_name("knowledge.sqlite3-journal");
    fs::write(&journal, []).unwrap();
    rejected_store(run(&["doctor", "--json"]));
    fs::remove_file(journal).unwrap();
    fs::remove_file(&store).unwrap();
    rejected_store(run(&["doctor", "--json"]));
    let graph = directory.join("ToS/derived-exports/philosophy_graph_projection.min.json");
    fs::File::create(&graph)
        .unwrap()
        .set_len(4 * 1024 * 1024 + 1)
        .unwrap();
    let oversized = run(&["doctor", "--json"]);
    assert_eq!(oversized.status.code(), Some(1));
    let oversized = parse_json(
        &oversized.stdout,
        JsonMode::PublishedStrict,
        JsonLimits::default(),
    )
    .unwrap();
    assert!(
        oversized
            .root()
            .object_get("required_failures")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id.as_str() == Some("philosophy-graph-schema"))
    );
    fs::remove_dir_all(directory).unwrap();
}

// Association control only: genuine packet semantics are compared in selected_lens.
#[test]
fn mcp_maintained_resources_prompts_and_packet_associations() {
    use tos_access::KnowledgeRequest as R;
    use tos_query::philosophy_read::PhilosophyReadRequest as P;
    let executor = KnowledgeSynthetic {
        calls: Mutex::new(vec![]),
        allowed: true,
    };
    let mut requests = String::from(
        "{\"jsonrpc\":\"2.0\",\"id\":0,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\"}}\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n",
    );
    for (id, method, params) in [
        (1, "resources/list", "{}"),
        (2, "resources/templates/list", "{}"),
        (3, "prompts/list", "{}"),
        (4, "prompts/get", r#"{"name":"tos-corpus-review"}"#),
        (
            5,
            "prompts/get",
            r#"{"name":"tos-philosophy-graph-review","arguments":{"query":"\ud800\n'","view_id":"chronology"}}"#,
        ),
        (
            6,
            "prompts/get",
            r#"{"name":"tos-zarathustra-word-analysis","arguments":{"query":"Wille","rank":"+002.00"}}"#,
        ),
        (
            7,
            "resources/read",
            r#"{"uri":"tos-philosophy://contracts"}"#,
        ),
        (8, "resources/read", r#"{"uri":"tos-philosophy://audit"}"#),
        (
            9,
            "resources/read",
            r#"{"uri":"tos-philosophy://lens/chronology"}"#,
        ),
        (
            10,
            "tools/call",
            r#"{"name":"tos_philosophy_epistemic_packet","arguments":{"item_id":"literal%2Fid"}}"#,
        ),
        (
            11,
            "tools/call",
            r#"{"name":"tos_philosophy_graph_packet","arguments":{"query":"q","view_id":"chronology","limit":2}}"#,
        ),
        (
            12,
            "tools/call",
            r#"{"name":"tos_philosophy_graph_chronology_packet","arguments":{"limit":3}}"#,
        ),
        (
            13,
            "tools/call",
            r#"{"name":"tos_evidence_lens","arguments":{"mode":"philosophy","item_id":"literal"}}"#,
        ),
        (
            14,
            "resources/read",
            r#"{"uri":"tos-philosophy://lens/bad/segment"}"#,
        ),
        (
            15,
            "prompts/get",
            r#"{"name":"tos-zarathustra-word-analysis","arguments":{"query":"q","rank":"1.5"}}"#,
        ),
        (
            16,
            "resources/read",
            r#"{"uri":"tos-corpus://graph-views"}"#,
        ),
        (
            17,
            "tools/call",
            r#"{"name":"tos_source_handle_discover","arguments":{}}"#,
        ),
        (
            18,
            "tools/call",
            r#"{"name":"tos_source_read","arguments":{}}"#,
        ),
        (
            19,
            "tools/call",
            r#"{"name":"tos_not_a_registered_tool","arguments":{}}"#,
        ),
    ] {
        requests.push_str(&format!(
            "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"{method}\",\"params\":{params}}}\n"
        ));
    }
    let mut output = vec![];
    run_io(
        Cursor::new(requests),
        &mut output,
        &executor,
        profile().with_query_timeout(std::time::Duration::from_secs(5)),
    )
    .unwrap();
    let rows: Vec<_> = output
        .split(|b| *b == b'\n')
        .filter(|row| !row.is_empty())
        .map(|raw| {
            parse_json(raw, JsonMode::PublishedStrict, JsonLimits::default())
                .unwrap()
                .into_root()
        })
        .collect();
    assert_eq!(rows.len(), 20);
    let result = |id: usize| rows[id].object_get("result").unwrap();
    assert_eq!(
        result(1)
            .object_get("resources")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        12
    );
    assert_eq!(
        result(2)
            .object_get("resourceTemplates")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        5
    );
    assert_eq!(
        result(3)
            .object_get("prompts")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        3
    );
    // FastMCP keeps an empty description for functions without docstrings;
    // absence (None) changes the complete imported Resource packet.
    for (id, field) in [(1, "resources"), (2, "resourceTemplates")] {
        for resource in result(id).object_get(field).unwrap().as_array().unwrap() {
            assert_eq!(
                resource.object_get("description").and_then(|v| v.as_str()),
                Some("")
            );
        }
    }
    let prompt = |id| {
        result(id)
            .object_get("messages")
            .unwrap()
            .as_array()
            .unwrap()[0]
            .object_get("content")
            .unwrap()
            .object_get("text")
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert_eq!(
        prompt(4),
        "Use tos_corpus_status(), then tos_corpus_packet(query='', view_id='corpus-topology'). Treat Tree-of-Sophia source_refs returned by the packet as authority; treat native MCP and standalone runtime as read-only access surfaces."
    );
    assert!(prompt(5).contains("query=\"\\ud800\\n'\""));
    assert!(prompt(6).contains("query='Wille', language='ru', rank=2"));
    for id in 7..=13 {
        assert!(rows[id].object_get("error").is_none(), "{id}");
    }
    for id in [14, 15] {
        assert!(rows[id].object_get("error").is_some());
    }
    assert_eq!(
        result(7)
            .object_get("contents")
            .unwrap()
            .as_array()
            .unwrap()[0]
            .object_get("mimeType")
            .unwrap()
            .as_str(),
        Some("text/plain")
    );
    for id in [17, 18] {
        let packet = result(id);
        assert_eq!(
            packet.object_get("isError").and_then(JsonValue::as_bool),
            Some(true)
        );
        let content = packet.object_get("content").unwrap().as_array().unwrap();
        assert_eq!(
            content[0].object_get("text").and_then(JsonValue::as_str),
            Some("exact source reader unavailable: no selected owner")
        );
        assert!(packet.object_get("structuredContent").is_none());
    }
    assert_eq!(
        rows[19]
            .object_get("error")
            .unwrap()
            .object_get("message")
            .and_then(JsonValue::as_str),
        Some("Unknown tool")
    );
    let calls = executor.calls.lock().unwrap();
    assert_eq!(calls.len(), 8);
    assert!(matches!(&calls[0], R::Philosophy(P::Contracts)));
    assert!(matches!(&calls[1], R::PhilosophyAudit));
    assert!(
        matches!(&calls[2], R::Philosophy(P::LensPacket{view_id,limit:20}) if view_id=="chronology")
    );
    assert!(
        matches!(&calls[3], R::Philosophy(P::Epistemic{item_id,view_id:None,limit:80}) if item_id=="literal%2Fid")
    );
    assert!(
        matches!(&calls[4], R::Philosophy(P::Packet{query,view_id:Some(view),limit:2}) if query=="q" && view=="chronology")
    );
    assert!(
        matches!(&calls[5], R::Philosophy(P::LensPacket{view_id,limit:3}) if view_id=="chronology")
    );
    assert!(matches!(&calls[6], R::EvidenceLens(_)));
    assert!(matches!(
        &calls[7],
        R::Corpus(tos_query::corpus_read::CorpusReadRequest::GraphViews)
    ));
}
