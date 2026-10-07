//! Loopback adapter over the selected native source owner. Authentication grants
//! transport access only; every dispatch enters the current native owner fence.
use crate::{
    source_command::{self as cmd, SourceCommandError},
    source_text_owner::{normalized_absolute, read_absolute},
};
use hmac::{Hmac, KeyInit, Mac};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const MAX_BODY: usize = 1_048_576;
const MAX_RESPONSE: usize = 4 * MAX_BODY;
const IDLE: Duration = Duration::from_secs(5);
const DEADLINE: Duration = Duration::from_secs(30);
const WINDOW: f64 = 30.0;
const NONCES: usize = 1024;
const HELP: &str = "usage: tos-native-owner-command http --owner-config ABSOLUTE_PATH --native-invocation ABSOLUTE_PATH --token-file ABSOLUTE_PATH --browser-origin http://127.0.0.1:PORT --port PORT\n";

type Error = (u16, &'static str);
fn hex(raw: &[u8]) -> String {
    raw.iter().map(|v| format!("{v:02x}")).collect()
}
fn is_hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn decode(s: &str) -> Option<Vec<u8>> {
    if !is_hex(s) {
        return None;
    }
    (0..32)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok())
        .collect()
}
fn digest(raw: &[u8]) -> String {
    hex(&Sha256::digest(raw))
}
// The wire contract uses Python ensure_ascii=True compact JSON.
fn signature_fields(fields: &Value) -> Vec<u8> {
    let json = serde_json::to_string(fields).expect("fixed JSON signature fields");
    let mut raw = Vec::with_capacity(json.len());
    for ch in json.chars() {
        if ch < '\u{007f}' {
            raw.push(ch as u8);
        } else {
            for unit in ch.encode_utf16(&mut [0; 2]).iter() {
                raw.extend_from_slice(format!("\\u{unit:04x}").as_bytes());
            }
        }
    }
    raw
}
fn signature(key: &[u8], fields: &Value) -> Vec<u8> {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key).expect("fixed HMAC key");
    mac.update(&signature_fields(fields));
    mac.finalize().into_bytes().to_vec()
}
fn verify(key: &[u8], fields: &Value, proof: &[u8]) -> bool {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key).expect("fixed HMAC key");
    mac.update(&signature_fields(fields));
    mac.verify_slice(proof).is_ok()
}
fn protected(path: &Path, max: usize) -> Result<Vec<u8>, SourceCommandError> {
    read_absolute(
        path,
        rustix::process::getuid().as_raw(),
        true,
        max,
        Instant::now() + IDLE,
        &AtomicBool::new(false),
    )
}
fn token(path: &Path) -> Result<Vec<u8>, Error> {
    use std::os::unix::fs::MetadataExt;
    let refused = (403, "transport-credential-unavailable");
    let uid = rustix::process::getuid().as_raw();
    crate::source_creation_store::protected_configuration_parents(path, uid)
        .map_err(|_| refused)?;
    let mut file = tos_fd_open::open_absolute_regular(path, 66).map_err(|_| refused)?;
    let before = file.metadata().map_err(|_| refused)?;
    if before.uid() != uid || before.mode() & 0o077 != 0 {
        return Err(refused);
    }
    let mut raw = Vec::new();
    Read::by_ref(&mut file)
        .take(67)
        .read_to_end(&mut raw)
        .map_err(|_| refused)?;
    let current = tos_fd_open::open_absolute_regular(path, 66).map_err(|_| refused)?;
    let stamp = |meta: &std::fs::Metadata| {
        (
            meta.dev(),
            meta.ino(),
            meta.mode(),
            meta.len(),
            meta.mtime(),
            meta.mtime_nsec(),
            meta.ctime(),
            meta.ctime_nsec(),
        )
    };
    if raw.len() > 66
        || stamp(&before) != stamp(&file.metadata().map_err(|_| refused)?)
        || stamp(&before) != stamp(&current.metadata().map_err(|_| refused)?)
    {
        return Err(refused);
    }
    let text = std::str::from_utf8(&raw).map_err(|_| refused)?;
    decode(text.strip_suffix('\n').unwrap_or(text)).ok_or(refused)
}
fn browser_origin(value: &str) -> bool {
    ["http://127.0.0.1:", "http://localhost:"]
        .iter()
        .any(|prefix| {
            value.strip_prefix(prefix).is_some_and(|port| {
                port.parse::<u16>()
                    .is_ok_and(|p| p > 0 && p.to_string() == port)
            })
        })
}
struct Selection {
    owner: PathBuf,
    invocation: PathBuf,
    credential: PathBuf,
    origin: String,
    host: String,
    recent: HashMap<String, f64>,
}
impl Selection {
    fn selected(&self) -> Result<(), SourceCommandError> {
        protected(&self.owner, MAX_BODY)?;
        let raw = protected(&self.invocation, MAX_BODY)?;
        let checked = cmd::parse(&raw)?;
        let value: Value = serde_json::from_slice(&cmd::canonical(&checked)?)
            .map_err(|_| SourceCommandError::Invalid("invocation JSON"))?;
        if value["owner_config"].as_str() != self.owner.to_str() {
            return Err(SourceCommandError::Denied(
                "invocation selects another owner",
            ));
        }
        Ok(())
    }
}
struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
}
impl Request {
    fn all(&self, name: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
            .collect()
    }
    fn one(&self, name: &str) -> Option<&str> {
        let all = self.all(name);
        if all.len() == 1 { Some(all[0]) } else { None }
    }
}
struct Reader<'a> {
    socket: &'a mut TcpStream,
    deadline: Instant,
}
impl Reader<'_> {
    fn read(&mut self, raw: &mut [u8]) -> io::Result<usize> {
        let left = self
            .deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "request deadline"))?;
        self.socket.set_read_timeout(Some(left.min(IDLE)))?;
        self.socket.read(raw)
    }
    fn line(&mut self) -> io::Result<Vec<u8>> {
        let mut raw = Vec::new();
        while raw.len() <= 65536 {
            let mut byte = [0];
            if self.read(&mut byte)? == 0 {
                break;
            }
            raw.push(byte[0]);
            if byte[0] == b'\n' {
                return Ok(raw);
            }
        }
        Err(io::Error::new(io::ErrorKind::InvalidData, "request line"))
    }
    fn request(&mut self) -> io::Result<Request> {
        let line = self.line()?;
        let text = std::str::from_utf8(&line)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "request encoding"))?;
        let fields = text.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 3 || !matches!(fields[2], "HTTP/1.0" | "HTTP/1.1") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "request grammar",
            ));
        }
        let mut req = Request {
            method: fields[0].to_owned(),
            path: fields[1].to_owned(),
            headers: Vec::new(),
        };
        for _ in 0..100 {
            let line = self.line()?;
            if line == b"\r\n" || line == b"\n" {
                return Ok(req);
            }
            let line = std::str::from_utf8(&line)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "header encoding"))?;
            let (name, value) = line
                .trim_end_matches(['\r', '\n'])
                .split_once(':')
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "header grammar"))?;
            if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "header name"));
            }
            req.headers.push((name.to_owned(), value.trim().to_owned()));
        }
        Err(io::Error::new(io::ErrorKind::InvalidData, "header count"))
    }
    fn body(&mut self, len: usize) -> io::Result<Vec<u8>> {
        let mut raw = vec![0; len];
        let mut read = 0;
        while read < len {
            let n = self.read(&mut raw[read..])?;
            if n == 0 {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "body"));
            }
            read += n;
        }
        Ok(raw)
    }
}
struct Auth {
    key: Vec<u8>,
    nonce: String,
    body_digest: String,
}
fn boundary(
    req: &Request,
    selection: &mut Selection,
    authenticate: bool,
) -> Result<Option<Auth>, Error> {
    if req.all("Host") != [selection.host.as_str()] {
        return Err((403, "host-not-allowed"));
    }
    let origins = req.all("Origin");
    if !origins.is_empty() && origins != [selection.origin.as_str()] {
        return Err((403, "origin-not-allowed"));
    }
    if !authenticate {
        return Ok(None);
    }
    let key = token(&selection.credential)?;
    let auth = req
        .one("Authorization")
        .and_then(|v| v.strip_prefix("ToS-HMAC-SHA256 "))
        .ok_or((401, "transport-authentication-required"))?;
    let parts = auth.split(':').collect::<Vec<_>>();
    if parts.len() != 4
        || !(10..=13).contains(&parts[0].len())
        || !parts[0].bytes().all(|b| b.is_ascii_digit())
        || !is_hex(parts[1])
        || !is_hex(parts[2])
        || !is_hex(parts[3])
    {
        return Err((401, "transport-authentication-required"));
    }
    let timestamp = parts[0]
        .parse::<u64>()
        .map_err(|_| (401, "transport-authentication-required"))?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| (401, "transport-authentication-required"))?
        .as_secs_f64();
    if (now - timestamp as f64).abs() > WINDOW
        || !verify(
            &key,
            &json!([
                "tos-request-v1",
                req.method,
                req.path,
                parts[0],
                parts[1],
                parts[2]
            ]),
            &decode(parts[3]).expect("checked hex"),
        )
    {
        return Err((401, "transport-authentication-required"));
    }
    // Authentication has succeeded: even a replay/capacity refusal is signed.
    Ok(Some(Auth {
        key,
        nonce: parts[1].to_owned(),
        body_digest: parts[2].to_owned(),
    }))
}
fn reserve_nonce(auth: &Auth, selection: &mut Selection) -> Result<(), Error> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| (401, "transport-authentication-required"))?
        .as_secs_f64();
    selection.recent.retain(|_, expiry| *expiry >= now);
    if selection.recent.contains_key(&auth.nonce) {
        return Err((409, "transport-nonce-replayed"));
    }
    if selection.recent.len() >= NONCES {
        return Err((429, "transport-authentication-capacity"));
    }
    // Set by caller to signed timestamp expiry, not arrival time.
    Ok(())
}
fn error(code: &str, dispatched: bool) -> Value {
    json!({"status":"error","code":code,"outcome":if dispatched {"unconfirmed"}else{"not-dispatched"}})
}
fn reply(
    socket: &mut TcpStream,
    req: &Request,
    selection: &Selection,
    auth: Option<&Auth>,
    mut status: u16,
    value: Value,
) -> io::Result<()> {
    socket.set_read_timeout(Some(IDLE))?;
    socket.set_write_timeout(Some(IDLE))?;
    let mut raw = serde_json::to_vec(&value).map_err(io::Error::other)?;
    if raw.len() > MAX_RESPONSE {
        status = 502;
        raw = serde_json::to_vec(&error("response-budget", true)).unwrap();
    }
    let mut headers = format!(
        "HTTP/1.1 {status} Response\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n",
        raw.len()
    );
    if let Some(auth) = auth {
        headers.push_str(&format!(
            "X-ToS-Response-Signature: {}\r\n",
            hex(&signature(
                &auth.key,
                &json!(["tos-response-v1", auth.nonce, status, digest(&raw)])
            ))
        ));
    }
    if req.all("Origin") == [selection.origin.as_str()] {
        headers.push_str(&format!("Access-Control-Allow-Origin: {}\r\nAccess-Control-Expose-Headers: X-ToS-Response-Signature\r\nVary: Origin\r\n",selection.origin));
    }
    headers.push_str("\r\n");
    socket.write_all(headers.as_bytes())?;
    socket.write_all(&raw)
}
fn process(socket: &mut TcpStream, selection: &mut Selection) -> io::Result<()> {
    let mut reader = Reader {
        socket,
        deadline: Instant::now() + DEADLINE,
    };
    let req = reader.request()?;
    let auth = match boundary(&req, selection, req.method != "OPTIONS") {
        Ok(v) => v,
        Err((status, code)) => {
            return reply(
                reader.socket,
                &req,
                selection,
                None,
                status,
                error(code, false),
            );
        }
    };
    if let Some(auth) = &auth {
        if let Err((status, code)) = reserve_nonce(auth, selection) {
            return reply(
                reader.socket,
                &req,
                selection,
                Some(auth),
                status,
                error(code, false),
            );
        }
        let timestamp = req
            .one("Authorization")
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .split(':')
            .next()
            .unwrap()
            .parse::<u64>()
            .unwrap();
        selection
            .recent
            .insert(auth.nonce.clone(), timestamp as f64 + WINDOW);
    }
    if req.method == "OPTIONS" {
        let method = match req.path.as_str() {
            "/commands" => "POST",
            "/commands/catalog" => "GET",
            _ => "",
        };
        if method.is_empty()
            || req.all("Origin") != [selection.origin.as_str()]
            || req.all("Access-Control-Request-Method") != [method]
        {
            return reply(
                reader.socket,
                &req,
                selection,
                None,
                403,
                error("preflight-not-allowed", false),
            );
        }
        let requested = req.one("Access-Control-Request-Headers");
        if requested.is_none_or(|v| {
            v.to_lowercase()
                .replace(' ', "")
                .split(',')
                .any(|h| !matches!(h, "authorization" | "content-type"))
        }) {
            return reply(
                reader.socket,
                &req,
                selection,
                None,
                403,
                error("preflight-headers-not-allowed", false),
            );
        }
        reader.socket.set_write_timeout(Some(IDLE))?;
        return reader.socket.write_all(format!("HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: {}\r\nAccess-Control-Allow-Methods: {method}\r\nAccess-Control-Allow-Headers: Authorization, Content-Type\r\nVary: Origin\r\nCache-Control: no-store\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",selection.origin).as_bytes());
    }
    let auth = auth.as_ref().expect("authenticated method");
    let deny = |socket: &mut TcpStream, status, code| {
        reply(
            socket,
            &req,
            selection,
            Some(auth),
            status,
            error(code, false),
        )
    };
    if req.method == "GET" {
        if req.path != "/commands/catalog" {
            return deny(reader.socket, 404, "route-not-found");
        }
        if !req.all("Transfer-Encoding").is_empty()
            || (!req.all("Content-Length").is_empty() && req.all("Content-Length") != ["0"])
            || auth.body_digest != digest(b"")
        {
            return deny(reader.socket, 400, "invalid-body-framing");
        }
        return reply(
            reader.socket,
            &req,
            selection,
            Some(auth),
            200,
            crate::source_native_cli::discover_commands(None).expect("packaged source catalog"),
        );
    }
    if req.method != "POST" {
        return deny(reader.socket, 501, "method-not-supported");
    }
    if req.path != "/commands" {
        return deny(reader.socket, 404, "route-not-found");
    }
    let length = req.one("Content-Length");
    if !req.all("Transfer-Encoding").is_empty()
        || length
            .is_none_or(|v| v.is_empty() || v.len() > 8 || !v.bytes().all(|b| b.is_ascii_digit()))
    {
        return deny(reader.socket, 400, "invalid-body-framing");
    }
    let length = length.unwrap().parse::<usize>().unwrap();
    if length > MAX_BODY {
        return deny(reader.socket, 413, "request-budget");
    }
    if req.one("Content-Type").is_none_or(|v| {
        !matches!(
            v.to_lowercase().as_str(),
            "application/json" | "application/json; charset=utf-8"
        )
    }) {
        return deny(reader.socket, 415, "json-required");
    }
    let raw = match reader.body(length) {
        Ok(v) => v,
        Err(_) => return deny(reader.socket, 400, "invalid-command-json"),
    };
    if digest(&raw) != auth.body_digest {
        return deny(reader.socket, 401, "request-digest-mismatch");
    }
    // Existing command parser rejects duplicate keys and non-object input.
    if cmd::parse(&raw).is_err()
        || !serde_json::from_slice::<Value>(&raw).is_ok_and(|v| v.is_object())
    {
        return deny(reader.socket, 400, "invalid-command-json");
    }
    reader.socket.set_read_timeout(Some(IDLE))?;
    let result = selection
        .selected()
        .and_then(|()| crate::source_native_cli::run(&selection.invocation, raw.as_slice()));
    let (status, value) = match result {
        Ok(v) => (200, v),
        Err(SourceCommandError::Denied(_)) => (403, error("owner-permission-denied", true)),
        Err(SourceCommandError::Conflict(_)) => (409, error("owner-conflict", true)),
        Err(SourceCommandError::Invalid(_)) => (422, error("owner-command-rejected", true)),
        Err(_) => (500, error("owner-outcome-unconfirmed", true)),
    };
    reply(reader.socket, &req, selection, Some(auth), status, value)
}
pub fn run(args: &[String]) -> Result<(), String> {
    if args == ["--help"] || args == ["-h"] {
        print!("{HELP}");
        return Ok(());
    }
    if rustix::process::geteuid() != rustix::process::getuid() {
        return Err("native source command HTTP setuid refused".into());
    }
    if args.len() != 10 {
        return Err(HELP.into());
    }
    let mut options = HashMap::new();
    for pair in args.chunks_exact(2) {
        if !matches!(
            pair[0].as_str(),
            "--owner-config"
                | "--native-invocation"
                | "--token-file"
                | "--browser-origin"
                | "--port"
        ) || options.insert(pair[0].as_str(), pair[1].as_str()).is_some()
        {
            return Err(HELP.into());
        }
    }
    let path = |key| {
        normalized_absolute(options[key]).map_err(|_| "absolute protected path required".to_owned())
    };
    let origin = options["--browser-origin"];
    if !browser_origin(origin) {
        return Err("exact loopback browser origin required".into());
    }
    let port = options["--port"]
        .parse::<u16>()
        .map_err(|_| "port must be in 1..65535")?;
    if port == 0 {
        return Err("port must be in 1..65535".into());
    }
    let mut selection = Selection {
        owner: path("--owner-config")?,
        invocation: path("--native-invocation")?,
        credential: path("--token-file")?,
        origin: origin.into(),
        host: format!("127.0.0.1:{port}"),
        recent: HashMap::new(),
    };
    selection
        .selected()
        .map_err(|_| "protected owner selection refused")?;
    token(&selection.credential).map_err(|_| "protected transport credential refused")?;
    let listener =
        TcpListener::bind(("127.0.0.1", port)).map_err(|_| "loopback listener unavailable")?;
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::AsRawFd;
        if unsafe { libc::listen(listener.as_raw_fd(), 4) } != 0 {
            return Err("listener queue unavailable".into());
        }
    }
    println!(
        "{}",
        json!({"schema_version":"tos_native_source_command_http_ready_v1","listen":format!("http://127.0.0.1:{port}")})
    );
    io::stdout()
        .flush()
        .map_err(|_| "readiness delivery failed")?;
    for socket in listener.incoming() {
        let mut socket = socket.map_err(|_| "listener failed")?;
        let _ = process(&mut socket, &mut selection);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    fn fixture() -> (tempfile::TempDir, Selection) {
        let home = tempfile::tempdir().unwrap();
        let credential = home.path().join("token");
        std::fs::write(&credential, "a".repeat(64)).unwrap();
        std::fs::set_permissions(&credential, std::fs::Permissions::from_mode(0o600)).unwrap();
        let selection = Selection {
            owner: home.path().join("owner"),
            invocation: home.path().join("invocation"),
            credential,
            origin: "http://127.0.0.1:44257".into(),
            host: "127.0.0.1:44259".into(),
            recent: HashMap::new(),
        };
        (home, selection)
    }
    fn auth(method: &str, path: &str, body: &[u8], nonce: &str, key: &[u8]) -> String {
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();
        let digest = digest(body);
        format!(
            "ToS-HMAC-SHA256 {time}:{nonce}:{digest}:{}",
            hex(&signature(
                key,
                &json!(["tos-request-v1", method, path, time, nonce, digest])
            ))
        )
    }
    fn exchange(
        selection: Selection,
        method: &str,
        path: &str,
        body: &[u8],
        extra: &str,
    ) -> (Selection, String) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let mut selection = selection;
            let (mut socket, _) = listener.accept().unwrap();
            process(&mut socket, &mut selection).unwrap();
            selection
        });
        let mut client = TcpStream::connect(address).unwrap();
        client.set_read_timeout(Some(IDLE)).unwrap();
        let wire = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:44259\r\n{extra}\r\n");
        let mut request = wire.into_bytes();
        request.extend_from_slice(body);
        client.write_all(&request).unwrap();
        // Invalid framing may leave unread request bytes and cause a TCP reset
        // after the complete response. Follow Content-Length, as real clients do.
        let mut raw = Vec::new();
        while !raw.ends_with(b"\r\n\r\n") {
            assert!(raw.len() < 65536);
            let mut byte = [0];
            client.read_exact(&mut byte).unwrap();
            raw.push(byte[0]);
        }
        let headers = std::str::from_utf8(&raw).unwrap();
        let length = headers
            .lines()
            .find_map(|line| line.strip_prefix("Content-Length: "))
            .unwrap()
            .parse::<usize>()
            .unwrap();
        assert!(length <= MAX_RESPONSE);
        let mut body = vec![0; length];
        client.read_exact(&mut body).unwrap();
        raw.extend_from_slice(&body);
        (worker.join().unwrap(), String::from_utf8(raw).unwrap())
    }
    #[test]
    fn packaged_catalog_preserves_discovery_authority_boundary() {
        let catalog = crate::source_native_cli::discover_commands(None).unwrap();
        assert_eq!(catalog["schema_version"], "tos_source_command_discovery_v1");
        assert_eq!(catalog["authorization_status"], "not_evaluated");
        for key in ["grants_admission", "reads_owner_configuration", "reads_source_targets"] {
            assert_eq!(catalog[key], false);
        }
        let handlers = catalog["handlers"].as_array().unwrap();
        assert_eq!(catalog["handler_count"], handlers.len());
        assert!(!handlers.is_empty());
        for handler in handlers {
            let name = handler["handler_id"].as_str().unwrap();
            let selected = crate::source_native_cli::discover_commands(Some(name)).unwrap();
            assert_eq!(selected["handler_count"], 1);
            assert_eq!(selected["handlers"][0], *handler);
            assert_eq!(handler["grants_admission"], false);
        }
        assert!(crate::source_native_cli::discover_commands(Some("unknown-handler")).is_err());
    }
    #[test]
    fn hmac_matches_maintained_python_wire_vector() {
        let key = decode(&"a".repeat(64)).unwrap();
        let fields = json!([
            "tos-request-v1",
            "POST",
            "/commands",
            "1791000000",
            "b".repeat(64),
            digest(b"{}")
        ]);
        assert_eq!(
            hex(&signature(&key, &fields)),
            "fd1b8736d79f0644fdc053042526d4c3f56a41144309def33b038414f6589724"
        );
        assert_eq!(
            signature_fields(&json!(["é", "😀"])),
            br#"["\u00e9","\ud83d\ude00"]"#
        );
    }
    #[test]
    fn authenticated_catalog_signed_response_nonce_and_rotation() {
        let (_home, mut selection) = fixture();
        let key = decode(&"a".repeat(64)).unwrap();
        let nonce = "b".repeat(64);
        let header = auth("GET", "/commands/catalog", b"", &nonce, &key);
        let extra = format!(
            "Authorization: {header}\r\nOrigin: {}\r\n",
            selection.origin
        );
        let (next, raw) = exchange(selection, "GET", "/commands/catalog", b"", &extra);
        selection = next;
        assert!(raw.starts_with("HTTP/1.1 200"));
        let (headers, body) = raw.split_once("\r\n\r\n").unwrap();
        let proof = headers
            .lines()
            .find_map(|line| line.strip_prefix("X-ToS-Response-Signature: "))
            .unwrap();
        assert!(verify(
            &key,
            &json!(["tos-response-v1", nonce, 200, digest(body.as_bytes())]),
            &decode(proof).unwrap()
        ));
        assert_eq!(
            serde_json::from_str::<Value>(body).unwrap()["grants_admission"],
            false
        );
        let (next, raw) = exchange(selection, "GET", "/commands/catalog", b"", &extra);
        selection = next;
        assert!(raw.starts_with("HTTP/1.1 409"));
        assert!(raw.contains("transport-nonce-replayed"));
        assert!(raw.contains("X-ToS-Response-Signature"));
        std::fs::write(&selection.credential, "c".repeat(64)).unwrap();
        let (next, raw) = exchange(selection, "GET", "/commands/catalog", b"", &extra);
        selection = next;
        assert!(raw.starts_with("HTTP/1.1 401"));
        assert!(!raw.contains("X-ToS-Response-Signature"));
        std::fs::set_permissions(
            &selection.credential,
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        let (_, raw) = exchange(selection, "GET", "/commands/catalog", b"", &extra);
        assert!(raw.starts_with("HTTP/1.1 403"));
    }
    #[test]
    fn framing_duplicate_json_digest_and_origin_refuse_without_dispatch() {
        let (_home, mut selection) = fixture();
        let key = decode(&"a".repeat(64)).unwrap();
        for (i, body, extra, path, status) in [
            (
                0,
                b"[]".as_slice(),
                "Content-Length: 2\r\nContent-Type: application/json\r\n",
                "/commands",
                400,
            ),
            (
                1,
                b"{\"x\":1,\"x\":2}".as_slice(),
                "Content-Length: 13\r\nContent-Type: application/json\r\n",
                "/commands",
                400,
            ),
            (
                2,
                b"{}".as_slice(),
                "Content-Length: 2\r\nContent-Length: 2\r\nContent-Type: application/json\r\n",
                "/commands",
                400,
            ),
            (
                3,
                b"{}".as_slice(),
                "Content-Length: 2\r\nContent-Type: text/plain\r\n",
                "/commands",
                415,
            ),
            (
                4,
                b"{}".as_slice(),
                "Content-Length: 2\r\nContent-Type: application/json\r\n",
                "/commands?owner=/etc/passwd",
                404,
            ),
            (
                5,
                b"{}".as_slice(),
                "Content-Length: 2\r\nContent-Type: application/json\r\nOrigin: null\r\n",
                "/commands",
                403,
            ),
        ] {
            let nonce = format!("{i:064x}");
            let header = auth("POST", path, body, &nonce, &key);
            let (next, raw) = exchange(
                selection,
                "POST",
                path,
                body,
                &format!("Authorization: {header}\r\n{extra}"),
            );
            selection = next;
            assert!(raw.starts_with(&format!("HTTP/1.1 {status}")), "{raw}");
            assert!(raw.contains("not-dispatched"));
        }
        let header = auth("POST", "/commands", b"ab", &"e".repeat(64), &key);
        let (_, raw) = exchange(
            selection,
            "POST",
            "/commands",
            b"{}",
            &format!(
                "Authorization: {header}\r\nContent-Length: 2\r\nContent-Type: application/json\r\n"
            ),
        );
        assert!(raw.starts_with("HTTP/1.1 401"));
        assert!(raw.contains("request-digest-mismatch"));
    }
    #[test]
    fn entire_reader_deadline_limits_trickle_across_lines() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let writer = std::thread::spawn(move || {
            let mut socket = TcpStream::connect(address).unwrap();
            for _ in 0..12 {
                if socket.write_all(b"x").is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });
        let (mut socket, _) = listener.accept().unwrap();
        let started = Instant::now();
        let mut reader = Reader {
            socket: &mut socket,
            deadline: started + Duration::from_millis(40),
        };
        assert!(reader.line().is_err());
        assert!(started.elapsed() < Duration::from_secs(1));
        drop(socket);
        writer.join().unwrap();
    }
}
