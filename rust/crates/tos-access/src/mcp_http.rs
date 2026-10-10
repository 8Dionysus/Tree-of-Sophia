//! Bounded local MCP 2025-11-25 Streamable HTTP, JSON-response profile.
//! Transport sessions select no data or rights: all operations retain the
//! installed executor, fresh request probe and shared final disclosure fence.
use crate::{AccessExecutor, AccessProfile, mcp::McpSession};
use std::{
    collections::BTreeMap,
    io::{self, Read, Write},
    net::{Shutdown, TcpListener, TcpStream, ToSocketAddrs},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tos_foundation::{JsonMode, JsonNumberKind, JsonValue, parse_json};
use tos_query::{AbortProbe, AbortReason};

const MAX_SESSIONS: usize = 32;
const MAX_CONNECTIONS: usize = 32;
const SESSION_IDLE: Duration = Duration::from_secs(900);
const PROTOCOL: &str = "2025-11-25";

struct ActiveRequest {
    id: Vec<u8>,
    cancelable: bool,
    cancelled: Arc<AtomicBool>,
}
struct Session {
    state: Mutex<McpSession>,
    active: Mutex<Option<ActiveRequest>>,
    last_used: Mutex<Instant>,
}

/// One listener's bounded protocol state; never an operation/authority registry.
pub struct HttpSessions {
    sessions: Mutex<BTreeMap<String, Arc<Session>>>,
    software: Option<Arc<crate::site::SoftwareSite>>,
}
impl Default for HttpSessions {
    fn default() -> Self {
        Self {
            sessions: Mutex::new(BTreeMap::new()),
            software: None,
        }
    }
}
impl HttpSessions {
    fn lookup(&self, id: &str) -> Option<Arc<Session>> {
        let mut sessions = self.sessions.lock().ok()?;
        let now = Instant::now();
        sessions.retain(|_, session| {
            let active = session.active.lock().ok().is_some_and(|v| v.is_some());
            active
                || session
                    .last_used
                    .lock()
                    .ok()
                    .is_some_and(|v| now.duration_since(*v) < SESSION_IDLE)
        });
        let session = Arc::clone(sessions.get(id)?);
        *session.last_used.lock().ok()? = now;
        Some(session)
    }
    fn create(&self, profile: AccessProfile) -> io::Result<(String, Arc<Session>)> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| io::Error::other("MCP sessions unavailable"))?;
        let now = Instant::now();
        sessions.retain(|_, s| {
            s.active.lock().ok().is_some_and(|v| v.is_some())
                || s.last_used
                    .lock()
                    .ok()
                    .is_some_and(|v| now.duration_since(*v) < SESSION_IDLE)
        });
        if sessions.len() >= MAX_SESSIONS {
            return Err(io::Error::other("MCP session budget exceeded"));
        }
        // IDs are not source capabilities. Unpredictability prevents another
        // local client from guessing a session to cancel or terminate it.
        let mut entropy = [0u8; 32];
        std::fs::File::open("/dev/urandom")?.read_exact(&mut entropy)?;
        let id = entropy
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        if sessions.contains_key(&id) {
            return Err(io::Error::other("MCP session identity collision"));
        }
        let session = Arc::new(Session {
            state: Mutex::new(McpSession::with_software(
                profile,
                self.software.as_ref().map(Arc::clone),
            )),
            active: Mutex::new(None),
            last_used: Mutex::new(now),
        });
        sessions.insert(id.clone(), Arc::clone(&session));
        Ok((id, session))
    }
    fn remove(&self, id: &str) {
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.remove(id);
        }
    }
}

struct RequestProbe {
    deadline: Option<Instant>,
    cancelled: Arc<AtomicBool>,
}
impl AbortProbe for RequestProbe {
    fn reason(&self) -> Option<AbortReason> {
        if self.cancelled.load(Ordering::Acquire) {
            Some(AbortReason::Cancelled)
        } else if self.deadline.is_some_and(|v| Instant::now() >= v) {
            Some(AbortReason::DeadlineExceeded)
        } else {
            None
        }
    }
}
impl RequestProbe {
    fn output_remaining(&self) -> io::Result<Duration> {
        match self.reason() {
            Some(AbortReason::Cancelled) => {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "MCP request cancelled",
                ));
            }
            Some(AbortReason::DeadlineExceeded) => {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "MCP request deadline exceeded",
                ));
            }
            None => {}
        }
        self.deadline
            .and_then(|v| v.checked_duration_since(Instant::now()))
            .filter(|v| !v.is_zero())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    "MCP output has no remaining request budget",
                )
            })
    }
}
struct ActiveHold(Arc<Session>);
impl Drop for ActiveHold {
    fn drop(&mut self) {
        if let Ok(mut active) = self.0.active.lock() {
            *active = None;
        }
    }
}
struct ConnectionHold(Arc<AtomicUsize>);
impl Drop for ConnectionHold {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
fn rpc_id(value: &JsonValue) -> Option<Vec<u8>> {
    match value {
        JsonValue::String(s) => s.as_str().map(crate::common::json_string),
        JsonValue::Number(n) if n.kind == JsonNumberKind::Int => Some(n.lexeme.as_bytes().to_vec()),
        _ => None,
    }
}
fn write_http(
    stream: &mut TcpStream,
    status: u16,
    body: &[u8],
    session: Option<&str>,
) -> io::Result<()> {
    write_http_with_probe(stream, status, body, session, None)
}
fn write_http_with_probe(
    stream: &mut TcpStream,
    status: u16,
    body: &[u8],
    session: Option<&str>,
    probe: Option<&RequestProbe>,
) -> io::Result<()> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        503 => "Service Unavailable",
        _ => "Error",
    };
    let identity = session
        .map(|id| format!("MCP-Session-Id: {id}\r\n"))
        .unwrap_or_default();
    let allow = if status == 405 {
        "Allow: POST, DELETE\r\n"
    } else {
        ""
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\n{identity}{allow}\r\n",
        body.len()
    );
    for mut bytes in [head.as_bytes(), body] {
        if let Some(probe) = probe {
            while !bytes.is_empty() {
                // Short polling is only for cancellation responsiveness. Each
                // syscall receives remaining absolute time, never a new budget.
                stream.set_write_timeout(Some(
                    probe.output_remaining()?.min(Duration::from_millis(50)),
                ))?;
                match stream.write(&bytes[..bytes.len().min(16_384)]) {
                    Ok(0) => {
                        return Err(io::Error::new(
                            io::ErrorKind::WriteZero,
                            "MCP output stalled",
                        ));
                    }
                    Ok(written) => bytes = &bytes[written..],
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock
                                | io::ErrorKind::TimedOut
                                | io::ErrorKind::Interrupted
                        ) => {}
                    Err(error) => return Err(error),
                }
                probe.output_remaining()?;
            }
        } else {
            stream.write_all(bytes)?;
        }
    }
    if let Some(probe) = probe {
        stream.set_write_timeout(Some(probe.output_remaining()?))?;
    }
    stream.flush()?;
    if let Some(probe) = probe {
        probe.output_remaining()?;
    }
    Ok(())
}
fn refuse(stream: &mut TcpStream, status: u16) -> io::Result<()> {
    write_http(stream, status, b"", None)?;
    // Publish a complete refusal before closing a socket with unread input.
    stream.shutdown(Shutdown::Write)?;
    stream.set_read_timeout(Some(Duration::from_millis(100)))?;
    let mut raw = [0; 1024];
    let _ = stream.read(&mut raw);
    Ok(())
}
fn accepts(value: &str, media: &str) -> bool {
    value.split(',').any(|part| {
        part.split(';')
            .next()
            .is_some_and(|v| v.trim().eq_ignore_ascii_case(media))
    })
}
// Match the maintained FastMCP loopback wildcard-port policy, while requiring
// an actual authority (no userinfo/path or malformed port suffix).
fn local_authority(value: &str) -> bool {
    let Some((host, port)) = value.rsplit_once(':') else {
        return false;
    };
    matches!(host, "127.0.0.1" | "localhost" | "[::1]")
        && !port.is_empty()
        && port.bytes().all(|v| v.is_ascii_digit())
        && port.parse::<u16>().is_ok()
}
fn serve_one(
    stream: &mut TcpStream,
    executor: &dyn AccessExecutor,
    profile: AccessProfile,
    sessions: &HttpSessions,
) -> io::Result<()> {
    let head = crate::http::read_head(stream)?;
    if head.len() > 8192 || !head.ends_with(b"\r\n\r\n") {
        return refuse(stream, 400);
    }
    let Ok(text) = std::str::from_utf8(&head) else {
        return refuse(stream, 400);
    };
    let mut lines = text.split("\r\n");
    let first = lines
        .next()
        .unwrap_or("")
        .split_whitespace()
        .collect::<Vec<_>>();
    let [method, target, version] = first.as_slice() else {
        return refuse(stream, 400);
    };
    if !matches!(*version, "HTTP/1.0" | "HTTP/1.1") {
        return refuse(stream, 400);
    }
    let mut headers = BTreeMap::new();
    for line in lines.filter(|v| !v.is_empty()) {
        let Some((name, value)) = line.split_once(':') else {
            return refuse(stream, 400);
        };
        if name.is_empty()
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || headers
                .insert(name.to_ascii_lowercase(), value.trim())
                .is_some()
        {
            return refuse(stream, 400);
        }
    }
    let Some(host) = headers.get("host") else {
        return refuse(stream, 400);
    };
    if !local_authority(host) {
        return refuse(stream, 403);
    }
    if headers
        .get("origin")
        .is_some_and(|origin| !origin.strip_prefix("http://").is_some_and(local_authority))
    {
        return refuse(stream, 403);
    }
    if *target != "/mcp" {
        return refuse(stream, 404);
    }
    if headers
        .get("mcp-protocol-version")
        .is_some_and(|v| *v != PROTOCOL)
    {
        return refuse(stream, 400);
    }
    let supplied = headers.get("mcp-session-id").copied();
    // JSON-response mode does not advertise server-initiated/resumable SSE.
    if *method == "GET" {
        if supplied.is_some_and(|id| sessions.lookup(id).is_none()) {
            return refuse(stream, 404);
        }
        return refuse(stream, 405);
    }
    if *method == "DELETE" {
        let Some(id) = supplied else {
            return refuse(stream, 400);
        };
        let Some(session) = sessions.lookup(id) else {
            return refuse(stream, 404);
        };
        if let Ok(active) = session.active.lock() {
            if let Some(active) = active.as_ref() {
                active.cancelled.store(true, Ordering::Release);
            }
        }
        sessions.remove(id);
        return write_http(stream, 200, b"", None);
    }
    if *method != "POST" {
        return refuse(stream, 405);
    }
    if !headers
        .get("accept")
        .is_some_and(|v| accepts(v, "application/json") && accepts(v, "text/event-stream"))
    {
        return refuse(stream, 400);
    }
    let body = match crate::http::post_body(stream, text, profile) {
        Ok(body) => body,
        Err(error) => return refuse(stream, error.http_status()),
    };
    let Ok(document) = parse_json(&body, JsonMode::RequestLastWins, profile.json_limits()) else {
        return refuse(stream, 400);
    };
    let value = document.root();
    if value.as_object().is_none()
        || value.object_get("jsonrpc").and_then(JsonValue::as_str) != Some("2.0")
    {
        return refuse(stream, 400);
    }
    let method = value.object_get("method").and_then(JsonValue::as_str);
    let initialize = method == Some("initialize");
    if initialize && supplied.is_some() {
        return refuse(stream, 400);
    }
    let (id, session) = if initialize {
        if value.object_get("id").and_then(rpc_id).is_none()
            || value
                .object_get("params")
                .and_then(|v| v.object_get("protocolVersion"))
                .and_then(JsonValue::as_str)
                != Some(PROTOCOL)
        {
            return refuse(stream, 400);
        }
        match sessions.create(profile) {
            Ok(v) => v,
            Err(_) => return refuse(stream, 503),
        }
    } else {
        let Some(id) = supplied else {
            return refuse(stream, 400);
        };
        let Some(session) = sessions.lookup(id) else {
            return refuse(stream, 404);
        };
        (id.to_owned(), session)
    };
    // Cancellation is independent of the serialized operation lock. A closed
    // HTTP connection alone is not an MCP cancellation notification.
    if method == Some("notifications/cancelled") && value.object_get("id").is_none() {
        let Some(request_id) = value
            .object_get("params")
            .and_then(|v| v.object_get("requestId"))
            .and_then(rpc_id)
        else {
            return write_http(stream, 202, b"", None);
        };
        if let Ok(active) = session.active.lock() {
            if let Some(active) = active
                .as_ref()
                .filter(|v| v.cancelable && v.id == request_id)
            {
                active.cancelled.store(true, Ordering::Release);
            }
        }
        return write_http(stream, 202, b"", None);
    }
    let Ok(mut state) = session.state.try_lock() else {
        return refuse(stream, 503);
    };
    let cancelled = Arc::new(AtomicBool::new(false));
    let request_id = value.object_get("id").and_then(rpc_id).unwrap_or_default();
    *session
        .active
        .lock()
        .map_err(|_| io::Error::other("MCP active request unavailable"))? = Some(ActiveRequest {
        id: request_id,
        cancelable: !initialize,
        cancelled: Arc::clone(&cancelled),
    });
    let _hold = ActiveHold(Arc::clone(&session));
    let now = Instant::now();
    let probe = Arc::new(RequestProbe {
        cancelled,
        deadline: profile
            .query_timeout
            .map(|v| now.checked_add(v).unwrap_or(now)),
    });
    let frame = state.handle_line_with_probe(executor, &body, probe.clone());
    let result = match frame {
        Some(frame) => state.write_reply(frame, b"", |frame| {
            write_http_with_probe(
                stream,
                200,
                frame,
                initialize.then_some(id.as_str()),
                Some(probe.as_ref()),
            )
        }),
        None => write_http_with_probe(stream, 202, b"", None, Some(probe.as_ref())),
    };
    if initialize && (!state.initializing() || result.is_err()) {
        sessions.remove(&id);
    }
    result
}

/// Actual socket path used by the listener and existing byte-level consumers.
pub fn serve_connection(
    mut stream: TcpStream,
    executor: Arc<dyn AccessExecutor>,
    profile: AccessProfile,
    sessions: Arc<HttpSessions>,
) {
    let profile =
        profile.with_query_timeout(profile.query_timeout.unwrap_or(Duration::from_secs(5)));
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
    let _ = serve_one(&mut stream, executor.as_ref(), profile, sessions.as_ref());
}

/// The listener admits at most32 connections and32 idle-bounded sessions.
pub fn serve(
    addr: &str,
    executor: Arc<dyn AccessExecutor>,
    profile: AccessProfile,
) -> io::Result<()> {
    serve_with_software(addr, executor, profile, None)
}
pub fn serve_with_software(
    addr: &str,
    executor: Arc<dyn AccessExecutor>,
    profile: AccessProfile,
    software: Option<Arc<crate::site::SoftwareSite>>,
) -> io::Result<()> {
    let addresses = addr.to_socket_addrs()?.collect::<Vec<_>>();
    if addresses.is_empty() || addresses.iter().any(|v| !v.ip().is_loopback()) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "native MCP HTTP must bind loopback",
        ));
    }
    let listener = TcpListener::bind(addresses.as_slice())?;
    let sessions = Arc::new(HttpSessions {
        sessions: Mutex::new(BTreeMap::new()),
        software,
    });
    let active = Arc::new(AtomicUsize::new(0));
    for accepted in listener.incoming() {
        let mut stream = accepted?;
        if active.fetch_add(1, Ordering::AcqRel) >= MAX_CONNECTIONS {
            active.fetch_sub(1, Ordering::AcqRel);
            if stream.set_nonblocking(true).is_ok() {
                let _ = write_http(&mut stream, 503, b"", None);
            }
            continue;
        }
        let hold = ConnectionHold(Arc::clone(&active));
        let executor = Arc::clone(&executor);
        let sessions = Arc::clone(&sessions);
        let _ = std::thread::Builder::new().spawn(move || {
            let _hold = hold;
            serve_connection(stream, executor, profile, sessions);
        });
    }
    Ok(())
}

pub enum Transport {
    Stdio,
    StreamableHttp(String),
}
/// Parse only transport options; software/data selection belongs to the caller.
pub fn parse_transport(args: &[String]) -> Result<Transport, &'static str> {
    if args.len() > 32 || args.iter().map(String::len).sum::<usize>() > 65_536 {
        return Err("MCP transport arguments exceed byte/count budget");
    }
    let mut transport = "stdio";
    let mut listener = Vec::new();
    let mut at = 0;
    while at < args.len() {
        let (key, inline) = args[at]
            .split_once('=')
            .map_or((args[at].as_str(), None), |(k, v)| (k, Some(v)));
        let value = if let Some(v) = inline {
            v
        } else {
            at += 1;
            args.get(at)
                .map(String::as_str)
                .ok_or("missing MCP transport value")?
        };
        match key {
            "--transport" if matches!(value, "stdio" | "streamable-http") => transport = value,
            "--host" | "--port" => {
                listener.push(key.to_owned());
                listener.push(value.to_owned());
            }
            _ => return Err("invalid MCP transport option"),
        }
        at += 1;
    }
    if transport == "stdio" {
        if !listener.is_empty() {
            return Err("stdio MCP has no listener options");
        }
        return Ok(Transport::Stdio);
    }
    if !listener.iter().any(|v| v == "--port") {
        listener.extend(["--port".into(), "5429".into()]);
    }
    crate::cli::parse_serve_address(&listener)
        .map(Transport::StreamableHttp)
        .map_err(|_| "invalid MCP listener address")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_streamable_http_session_capacity_and_idle_expiry_are_bounded() {
        let sessions = HttpSessions::default();
        let profile = AccessProfile::new(4096, 4096, 4096);
        let mut retained = Vec::new();
        for _ in 0..MAX_SESSIONS {
            retained.push(sessions.create(profile).unwrap());
        }
        assert!(sessions.create(profile).is_err());
        let (id, session) = &retained[0];
        *session.last_used.lock().unwrap() = Instant::now() - SESSION_IDLE;
        assert!(sessions.lookup(id).is_none());
        let (fresh, _) = sessions.create(profile).unwrap();
        assert!(retained.iter().all(|(id, _)| id != &fresh));
        assert_eq!(sessions.sessions.lock().unwrap().len(), MAX_SESSIONS);
    }
}
