//! One bounded local HTTP route for the first native query family.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};
use tos_query::{AbortProbe, AbortReason};

use crate::common::{
    AccessError, AccessErrorCode, AccessExecutor, AccessProfile, DisclosureFence, HTTP_PREFIX,
    IndexedSearchParams, Params, SEARCH_HTTP_PATH, error_json, validate_packet,
};

const MAX_HEAD: usize = 8 * 1024;
const MAX_CONCURRENT: usize = 32;

/// Poll the already-read request socket without blocking QRY's SQLite progress
/// callback on every VM instruction. The socket is restored to blocking mode
/// before the response write; this probe lives only through query execution.
struct HttpAbortProbe {
    client: TcpStream,
    deadline: Option<Instant>,
    checks: AtomicU64,
}

impl AbortProbe for HttpAbortProbe {
    fn reason(&self) -> Option<AbortReason> {
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Some(AbortReason::DeadlineExceeded);
        }
        if self.checks.fetch_add(1, Ordering::Relaxed) % 1024 != 0 {
            return None;
        }
        match self.client.peek(&mut [0]) {
            // A peer may half-close its request stream while still reading the
            // response. EOF alone is not a cancelled HTTP request.
            Ok(0) => None,
            Ok(_) => None,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => None,
            Err(_) => Some(AbortReason::Cancelled),
        }
    }
}

pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
    pub head_only: bool,
    fence: Option<Box<dyn DisclosureFence>>,
}

impl HttpResponse {
    fn error(status: u16, message: &'static str) -> Self {
        let code = match status {
            404 => AccessErrorCode::UnknownExactId,
            413 => AccessErrorCode::BudgetExceeded,
            503 => AccessErrorCode::Unavailable,
            _ => AccessErrorCode::InvalidRequest,
        };
        let error = AccessError::new(code, message);
        Self {
            status,
            body: error_json(&error),
            head_only: false,
            fence: None,
        }
    }

    fn error_for_method(status: u16, message: &'static str, method: &str) -> Self {
        let mut response = Self::error(status, message);
        response.head_only = method == "HEAD";
        response
    }
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn percent_decode(raw: &str, plus_space: bool) -> Result<String, AccessError> {
    let mut bytes = Vec::with_capacity(raw.len());
    let source = raw.as_bytes();
    let mut at = 0;
    while at < source.len() {
        match source[at] {
            b'%' if at + 2 < source.len() => {
                let high = hex(source[at + 1]).ok_or_else(|| {
                    AccessError::new(AccessErrorCode::InvalidRequest, "invalid percent escape")
                })?;
                let low = hex(source[at + 2]).ok_or_else(|| {
                    AccessError::new(AccessErrorCode::InvalidRequest, "invalid percent escape")
                })?;
                bytes.push(high * 16 + low);
                at += 3;
            }
            b'%' => {
                return Err(AccessError::new(
                    AccessErrorCode::InvalidRequest,
                    "invalid percent escape",
                ));
            }
            b'+' if plus_space => {
                bytes.push(b' ');
                at += 1;
            }
            value => {
                bytes.push(value);
                at += 1;
            }
        }
    }
    String::from_utf8(bytes).map_err(|_| {
        AccessError::new(
            AccessErrorCode::InvalidRequest,
            "request target is not UTF-8",
        )
    })
}

fn bounded_legacy_int(raw: Option<&str>, default: i64, low: i64, high: i64) -> i64 {
    let parsed = raw
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(default);
    parsed.clamp(low, high)
}

fn query_value<'a>(query: &'a str, name: &str) -> Option<String> {
    query
        .split('&')
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            if percent_decode(key, true).ok()?.as_str() == name {
                percent_decode(value, true).ok()
            } else {
                None
            }
        })
        .next()
}

fn query_list(query: &str, name: &str) -> Vec<String> {
    query_value(query, name)
        .unwrap_or_default()
        .split(',')
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

fn handle_indexed_search(
    executor: &dyn AccessExecutor,
    method: &str,
    query: &str,
    profile: AccessProfile,
    abort_probe: Arc<dyn AbortProbe>,
) -> HttpResponse {
    if query_value(query, "mode").as_deref() != Some("indexed") {
        return HttpResponse::error_for_method(
            503,
            "selected knowledge search mode unavailable",
            method,
        );
    }
    if !executor.knowledge_search_indexed_available() {
        return HttpResponse::error_for_method(503, "indexed knowledge search unavailable", method);
    }
    let offset = bounded_legacy_int(query_value(query, "offset").as_deref(), 0, 0, 100_000);
    if offset != 0 {
        return HttpResponse::error_for_method(
            400,
            "indexed search uses cursor, not offset",
            method,
        );
    }
    let limit = bounded_legacy_int(query_value(query, "limit").as_deref(), 40, 1, 100) as usize;
    let result = IndexedSearchParams::new(
        query_value(query, "query").unwrap_or_default(),
        query_list(query, "sources"),
        query_list(query, "kind_ids"),
        query_list(query, "predicate_ids"),
        query_value(query, "cursor").filter(|value| !value.is_empty()),
        limit,
    )
    .and_then(|params| executor.knowledge_search_indexed(params, abort_probe))
    .and_then(|packet| {
        if packet.body.len() > profile.max_response_bytes {
            return Err(AccessError::new(
                AccessErrorCode::BudgetExceeded,
                "indexed search response budget exceeded",
            ));
        }
        validate_packet(&packet.body, profile.max_response_bytes)?;
        Ok(packet)
    });
    match result {
        Ok(packet) => HttpResponse {
            status: 200,
            body: packet.body,
            head_only: method == "HEAD",
            fence: Some(packet.fence),
        },
        Err(error) => HttpResponse {
            status: error.http_status(),
            body: error_json(&error),
            head_only: method == "HEAD",
            fence: None,
        },
    }
}

pub fn handle_get(
    executor: &dyn AccessExecutor,
    method: &str,
    target: &str,
    profile: AccessProfile,
) -> HttpResponse {
    handle_get_with_probe(executor, method, target, profile, profile.deadline_probe())
}

fn handle_get_with_probe(
    executor: &dyn AccessExecutor,
    method: &str,
    target: &str,
    profile: AccessProfile,
    abort_probe: Arc<dyn AbortProbe>,
) -> HttpResponse {
    if method != "GET" && method != "HEAD" {
        return HttpResponse::error(405, "method not allowed");
    }
    if target.len() > profile.max_request_bytes {
        return HttpResponse::error_for_method(413, "request target too large", method);
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path == SEARCH_HTTP_PATH {
        return handle_indexed_search(executor, method, query, profile, abort_probe);
    }
    let Some(encoded_id) = path.strip_prefix(HTTP_PREFIX) else {
        return HttpResponse::error_for_method(404, "not found", method);
    };
    if !executor.source_descend_available() {
        return HttpResponse::error_for_method(503, "source descent unavailable", method);
    }
    let outcome = percent_decode(encoded_id, false)
        .and_then(|node_id| {
            let max_depth =
                bounded_legacy_int(query_value(query, "max_depth").as_deref(), 8, 1, 8) as u8;
            let limit =
                bounded_legacy_int(query_value(query, "limit").as_deref(), 300, 1, 300) as usize;
            Params::new(node_id, max_depth, limit)
        })
        .and_then(|params| executor.source_descend(params, abort_probe))
        .and_then(|packet| {
            if packet.body.len() > profile.max_response_bytes {
                return Err(AccessError::new(
                    AccessErrorCode::BudgetExceeded,
                    "source descent response budget exceeded",
                ));
            }
            validate_packet(&packet.body, profile.max_response_bytes)?;
            Ok(packet)
        });
    match outcome {
        Ok(packet) => HttpResponse {
            status: 200,
            body: packet.body,
            head_only: method == "HEAD",
            fence: Some(packet.fence),
        },
        Err(error) => HttpResponse {
            status: error.http_status(),
            body: error_json(&error),
            head_only: method == "HEAD",
            fence: None,
        },
    }
}

fn read_head(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    let deadline = Instant::now() + Duration::from_secs(5);
    while head.len() <= MAX_HEAD {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        stream.set_read_timeout(Some(remaining))?;
        if stream.read(&mut byte)? == 0 {
            break;
        }
        head.push(byte[0]);
        if head.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    Ok(head)
}

fn write_response(stream: &mut TcpStream, mut response: HttpResponse) -> std::io::Result<()> {
    if let Some(fence) = response.fence.as_mut() {
        if let Err(error) = fence.recheck() {
            response.status = error.http_status();
            response.body = error_json(&error);
            response.fence = None;
        }
    }
    let reason = match response.status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        409 => "Conflict",
        413 => "Content Too Large",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\n\r\n",
        response.status,
        reason,
        response.body.len()
    );
    stream.write_all(header.as_bytes())?;
    if !response.head_only {
        stream.write_all(&response.body)?;
    }
    Ok(())
}

/// Serve exactly one bounded HTTP/1.x request on an already accepted socket.
/// The listener owns admission and concurrency; this entry is useful for
/// actual socket-level conformance tests without a background server.
pub fn serve_connection(
    mut stream: TcpStream,
    executor: Arc<dyn AccessExecutor>,
    profile: AccessProfile,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let response = match read_head(&mut stream) {
        Ok(head) if head.len() <= MAX_HEAD && head.ends_with(b"\r\n\r\n") => {
            match std::str::from_utf8(&head) {
                Ok(text) => {
                    let first = text.split("\r\n").next().unwrap_or("");
                    match first.split_whitespace().collect::<Vec<_>>().as_slice() {
                        [method, target, "HTTP/1.1"] | [method, target, "HTTP/1.0"] => match stream
                            .try_clone()
                            .and_then(|client| {
                                client.set_nonblocking(true)?;
                                Ok(client)
                            }) {
                            Ok(client) => {
                                let now = Instant::now();
                                let deadline = profile
                                    .query_timeout
                                    .map(|timeout| now.checked_add(timeout).unwrap_or(now));
                                handle_get_with_probe(
                                    executor.as_ref(),
                                    method,
                                    target,
                                    profile,
                                    Arc::new(HttpAbortProbe {
                                        client,
                                        deadline,
                                        checks: AtomicU64::new(0),
                                    }),
                                )
                            }
                            Err(_) => {
                                HttpResponse::error(503, "client cancellation probe unavailable")
                            }
                        },
                        _ => HttpResponse::error(400, "invalid HTTP request line"),
                    }
                }
                Err(_) => HttpResponse::error(400, "HTTP header is not UTF-8"),
            }
        }
        _ => HttpResponse::error(400, "HTTP header incomplete or oversized"),
    };
    if stream.set_nonblocking(false).is_ok() {
        let _ = write_response(&mut stream, response);
    }
}

/// Loopback-only serving. At most 32 active connections; a full slot pool
/// refuses immediately rather than accumulating unbounded work.
pub fn serve(
    addr: &str,
    executor: Arc<dyn AccessExecutor>,
    profile: AccessProfile,
) -> std::io::Result<()> {
    let addresses = addr.to_socket_addrs()?.collect::<Vec<_>>();
    if addresses.is_empty() || addresses.iter().any(|address| !address.ip().is_loopback()) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "native access HTTP must bind loopback",
        ));
    }
    let listener = TcpListener::bind(addresses.as_slice())?;
    let active = Arc::new(AtomicUsize::new(0));
    for accepted in listener.incoming() {
        let mut stream = accepted?;
        if active.fetch_add(1, Ordering::AcqRel) >= MAX_CONCURRENT {
            active.fetch_sub(1, Ordering::AcqRel);
            let _ = write_response(&mut stream, HttpResponse::error(503, "server busy"));
            continue;
        }
        let active = Arc::clone(&active);
        let executor = Arc::clone(&executor);
        std::thread::spawn(move || {
            serve_connection(stream, executor, profile);
            active.fetch_sub(1, Ordering::AcqRel);
        });
    }
    Ok(())
}
