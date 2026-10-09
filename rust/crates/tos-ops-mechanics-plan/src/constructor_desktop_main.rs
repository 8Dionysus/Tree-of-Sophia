//! Loopback display, owned desktop launcher, and explicit inactive installation.
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    error::Error,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::{
        fs::{OpenOptionsExt, PermissionsExt},
        io::AsRawFd,
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tos_foundation::{Digest256, Digest256Hasher};
const APP: &str = "tos-sophia-demo";
const IDENTITY: &str = "/__tos_demo_identity";
type Result<T> = std::result::Result<T, Box<dyn Error>>;
fn bad(s: impl Into<String>) -> Box<dyn Error> {
    std::io::Error::other(s.into()).into()
}
fn text<'a>(v: &'a Value, k: &str) -> Result<&'a str> {
    v[k].as_str()
        .ok_or_else(|| bad(format!("missing configuration {k}")))
}
fn absolute(p: &str) -> bool {
    Path::new(p).is_absolute() && !p.contains(['\n', '\r', '\0'])
}
fn files(root: &Path) -> Result<BTreeMap<String, PathBuf>> {
    let root = fs::canonicalize(root)?;
    let assets = root.join("assets");
    if !fs::symlink_metadata(&assets)?.is_dir() {
        return Err(bad("release assets must be a real directory"));
    }
    let mut paths = vec![root.join("constructor.html"), root.join("library.json")];
    let mut dirs = vec![assets];
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            let meta = fs::symlink_metadata(&path)?;
            if meta.file_type().is_symlink() {
                return Err(bad("release contains a symlink"));
            }
            if meta.is_dir() {
                dirs.push(path)
            } else {
                paths.push(path)
            }
            if dirs.len() + paths.len() > 10000 {
                return Err(bad("release exceeds10000 entries"));
            }
        }
    }
    let mut out = BTreeMap::new();
    for path in paths {
        let m = fs::symlink_metadata(&path)?;
        if !m.is_file() || m.len() > 512 * 1024 * 1024 || fs::canonicalize(&path)? != path {
            return Err(bad("release file custody"));
        }
        let name = path
            .strip_prefix(&root)?
            .to_str()
            .ok_or_else(|| bad("non-UTF8 display path"))?
            .to_owned();
        out.insert(name, path);
    }
    Ok(out)
}
fn release_digest(root: &Path) -> Result<String> {
    let mut hash = Digest256Hasher::new();
    for (name, path) in files(root)? {
        hash.update(&(u32::try_from(name.len())?).to_be_bytes());
        hash.update(name.as_bytes());
        let mut f = File::open(path)?;
        let mut content = Digest256Hasher::new();
        let mut buf = [0u8; 65536];
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            content.update(&buf[..n]);
        }
        hash.update(content.finalize().as_bytes());
    }
    Ok(hash.finalize().to_hex())
}
fn mime(path: &Path) -> &'static str {
    match path.extension().and_then(|v| v.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "woff2" => "font/woff2",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}
fn decode(s: &str) -> Result<String> {
    let mut out = Vec::new();
    let mut iter = s.as_bytes().iter();
    while let Some(&x) = iter.next() {
        if x == b'%' {
            let a = *iter.next().ok_or_else(|| bad("percent encoding"))?;
            let b = *iter.next().ok_or_else(|| bad("percent encoding"))?;
            out.push(u8::from_str_radix(std::str::from_utf8(&[a, b])?, 16)?);
        } else {
            out.push(x)
        }
    }
    Ok(String::from_utf8(out)?)
}
fn response(s: &mut TcpStream, status: u16, kind: &str, raw: &[u8]) -> Result<()> {
    write!(
        s,
        "HTTP/1.1 {status} {}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        if status == 200 { "OK" } else { "Refused" },
        raw.len()
    )?;
    s.write_all(raw)?;
    Ok(())
}
fn request(
    mut stream: TcpStream,
    port: u16,
    allowed: &BTreeMap<String, PathBuf>,
    identity: &[u8],
) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut raw = Vec::new();
    let mut byte = [0u8; 1];
    while !raw.ends_with(b"\r\n\r\n") {
        if raw.len() >= 16384 || stream.read(&mut byte)? == 0 {
            return Err(bad("HTTP header limit"));
        }
        raw.push(byte[0]);
    }
    let headers = std::str::from_utf8(&raw)?;
    let mut lines = headers.split("\r\n");
    let first: Vec<_> = lines.next().unwrap_or("").split_whitespace().collect();
    if first.len() != 3 || first[0] != "GET" {
        return response(&mut stream, 405, "text/plain", b"GET required");
    }
    let hosts = lines
        .filter_map(|line| line.split_once(':'))
        .filter(|(key, _)| key.eq_ignore_ascii_case("host"))
        .map(|(_, v)| v.trim())
        .collect::<Vec<_>>();
    if hosts.len() != 1
        || !hosts
            .iter()
            .all(|v| *v == format!("127.0.0.1:{port}") || *v == format!("localhost:{port}"))
    {
        return response(&mut stream, 403, "text/plain", b"Host refused");
    }
    let route = match decode(first[1].split('?').next().unwrap_or("")) {
        Ok(s) => s,
        Err(_) => return response(&mut stream, 404, "text/plain", b"Not found"),
    };
    if route == IDENTITY {
        return response(
            &mut stream,
            200,
            "application/json; charset=utf-8",
            identity,
        );
    }
    let name = if route == "/" {
        "constructor.html"
    } else {
        route.trim_start_matches('/')
    };
    let Some(path) = allowed.get(name) else {
        return response(&mut stream, 404, "text/plain", b"Not found");
    };
    if fs::symlink_metadata(path).is_ok_and(|m| m.is_file())
        && fs::canonicalize(path).is_ok_and(|p| p == *path)
    {
        response(&mut stream, 200, mime(path), &fs::read(path)?)
    } else {
        response(&mut stream, 404, "text/plain", b"Not found")
    }
}
fn redirect(log: &Path) -> Result<()> {
    let input = File::open("/dev/null")?;
    let output = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(log)?;
    for (a, b) in [
        (input.as_raw_fd(), 0),
        (output.as_raw_fd(), 1),
        (output.as_raw_fd(), 2),
    ] {
        if unsafe { libc::dup2(a, b) } < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    Ok(())
}
fn serve(root: &Path, port: u16, digest: Option<&str>, log: Option<&Path>) -> Result<()> {
    let root = fs::canonicalize(root)?;
    let actual = release_digest(&root)?;
    if digest.is_some_and(|s| s != actual) {
        return Err(bad("release content differs from installed configuration"));
    }
    let allowed = Arc::new(files(&root)?);
    if let Some(log) = log {
        redirect(log)?;
    }
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let port = listener.local_addr()?.port();
    let identity = Arc::new(serde_json::to_vec(
        &json!({"schema":"tos_demo_server_identity_v1","app_id":APP,"release":root,"release_digest":actual,"pid":std::process::id()}),
    )?);
    println!(
        "{}",
        json!({"ready":true,"port":port,"pid":std::process::id()})
    );
    let active = Arc::new(AtomicUsize::new(0));
    for stream in listener.incoming() {
        let stream = stream?;
        if active.fetch_add(1, Ordering::SeqCst) >= 64 {
            active.fetch_sub(1, Ordering::SeqCst);
            drop(stream);
            continue;
        }
        let (a, i, n) = (allowed.clone(), identity.clone(), active.clone());
        std::thread::spawn(move || {
            let _ = request(stream, port, &a, &i);
            n.fetch_sub(1, Ordering::SeqCst);
        });
    }
    Ok(())
}
fn run(command: &[String], seconds: u64) -> Result<std::process::Output> {
    let mut child = Command::new(&command[0])
        .args(&command[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let until = Instant::now() + Duration::from_secs(seconds);
    loop {
        if child.try_wait()?.is_some() {
            return Ok(child.wait_with_output()?);
        }
        if Instant::now() >= until {
            child.kill()?;
            child.wait()?;
            return Err(bad("desktop command deadline"));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}
fn checked(command: &[String], seconds: u64) -> Result<()> {
    let out = run(command, seconds)?;
    if !out.status.success() {
        return Err(bad(String::from_utf8_lossy(&out.stderr).into_owned()));
    }
    Ok(())
}
fn properties(unit: &str) -> Result<BTreeMap<String, String>> {
    let out = run(
        &[
            "systemctl".into(),
            "--user".into(),
            "show".into(),
            unit.into(),
            "--property=ActiveState,SubState,MainPID,Result".into(),
        ],
        5,
    )?;
    if !out.status.success() {
        return Err(bad("cannot read owned user-service state"));
    }
    Ok(String::from_utf8(out.stdout)?
        .lines()
        .filter_map(|s| s.split_once('='))
        .map(|(k, v)| (k.into(), v.into()))
        .collect())
}
fn load(path: &Path) -> Result<Value> {
    let mut v: Value = serde_json::from_slice(&fs::read(path)?)?;
    let keys = [
        "schema",
        "release",
        "release_digest",
        "port",
        "browser",
        "profile",
        "cache",
        "runtime",
        "launcher",
        "server",
        "resource",
        "service_unit",
        "worker_unit",
    ];
    if v.as_object()
        .is_none_or(|m| m.len() != keys.len() || keys.iter().any(|k| !m.contains_key(*k)))
        || v["schema"] != "tos_sophia_demo_desktop_native_v1"
    {
        return Err(bad(
            "unsupported native desktop configuration; explicitly prepare the native install",
        ));
    }
    for k in [
        "release", "browser", "profile", "cache", "runtime", "launcher", "server", "resource",
    ] {
        if !absolute(text(&v, k)?) {
            return Err(bad(format!("{k} must be absolute")));
        }
    }
    if v["port"] != 44339
        || v["service_unit"] != format!("{APP}.service")
        || v["worker_unit"] != format!("{APP}-http.service")
    {
        return Err(bad("desktop service identity differs"));
    }
    let home = std::env::var_os("HOME").ok_or_else(|| bad("HOME absent"))?;
    if v["profile"] == v["cache"]
        || Path::new(text(&v, "profile")?).starts_with(PathBuf::from(home).join(".config/chromium"))
    {
        return Err(bad("browser requires a private profile and cache"));
    }
    let digest = text(&v, "release_digest")?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|x| x.is_ascii_hexdigit() && !x.is_ascii_uppercase())
    {
        return Err(bad("release digest invalid"));
    }
    v["config"] = json!(fs::canonicalize(path)?);
    Ok(v)
}
fn worker(v: &Value) -> Result<Vec<String>> {
    Ok(vec![
        text(v, "server")?.into(),
        "serve".into(),
        "--release".into(),
        text(v, "release")?.into(),
        "--port".into(),
        "44339".into(),
        "--release-digest".into(),
        text(v, "release_digest")?.into(),
        "--log".into(),
        Path::new(text(v, "runtime")?)
            .join("server.log")
            .to_string_lossy()
            .into_owned(),
    ])
}
fn owns(v: &Value, pid: Option<u32>) -> Result<bool> {
    let state = properties(text(v, "worker_unit")?)?;
    let actual = state
        .get("MainPID")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    if actual == 0 || pid.is_some_and(|p| p != actual) {
        return Ok(false);
    }
    let raw = match fs::read(format!("/proc/{actual}/cmdline")) {
        Ok(s) => s,
        Err(_) => return Ok(false),
    };
    let args = raw
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect::<Vec<_>>();
    Ok(args == worker(v)?)
}
fn identity(v: &Value) -> Result<Option<Value>> {
    let mut stream =
        match TcpStream::connect_timeout(&"127.0.0.1:44339".parse()?, Duration::from_secs(1)) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => return Ok(None),
            Err(e) => return Err(e.into()),
        };
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    write!(
        stream,
        "GET {IDENTITY} HTTP/1.1\r\nHost: 127.0.0.1:44339\r\nConnection: close\r\n\r\n"
    )?;
    let mut raw = Vec::new();
    stream.take(32769).read_to_end(&mut raw)?;
    if raw.len() > 32768 {
        return Err(bad("identity response exceeds limit"));
    }
    let bytes = String::from_utf8(raw)?;
    let (header, body) = bytes
        .split_once("\r\n\r\n")
        .ok_or_else(|| bad("invalid identity HTTP response"))?;
    if !header.starts_with("HTTP/1.1 200 ") && !header.starts_with("HTTP/1.0 200 ") {
        return Err(bad("port44339 serves another application"));
    }
    let response: Value = serde_json::from_str(body)?;
    if response["schema"] != "tos_demo_server_identity_v1"
        || response["app_id"] != APP
        || response["release"] != v["release"]
        || response["release_digest"] != v["release_digest"]
    {
        return Err(bad("port44339 serves another release"));
    }
    let pid = response["pid"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| bad("identity PID invalid"))?;
    if !owns(v, Some(pid))? {
        return Err(bad(
            "identity is not owned by the configured systemd worker",
        ));
    }
    Ok(Some(response))
}
fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    if !fs::symlink_metadata(path)?.is_dir() {
        return Err(bad("private directory is a symlink"));
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}
fn ensure(v: &Value) -> Result<Value> {
    if release_digest(Path::new(text(v, "release")?))? != text(v, "release_digest")? {
        return Err(bad("installed release content changed"));
    }
    let base = PathBuf::from(
        std::env::var_os("XDG_RUNTIME_DIR")
            .unwrap_or_else(|| format!("/run/user/{}", unsafe { libc::getuid() }).into()),
    )
    .join(APP);
    private_dir(&base)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(base.join("launch.lock"))?;
    let deadline = Instant::now() + Duration::from_secs(45);
    while unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        let e = std::io::Error::last_os_error();
        if e.kind() != std::io::ErrorKind::WouldBlock || Instant::now() >= deadline {
            return Err(e.into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if let Some(found) = identity(v)? {
        return Ok(found);
    }
    drop(
        TcpListener::bind("127.0.0.1:44339")
            .map_err(|_| bad("port44339 is occupied; existing process left untouched"))?,
    );
    checked(
        &[
            "systemctl".into(),
            "--user".into(),
            "start".into(),
            text(v, "service_unit")?.into(),
        ],
        8,
    )?;
    while Instant::now() < deadline {
        if let Some(found) = identity(v)? {
            return Ok(found);
        }
        let state = properties(text(v, "service_unit")?)?;
        if matches!(
            state.get("ActiveState").map(String::as_str),
            Some("failed" | "inactive")
        ) {
            return Err(bad(
                "owned desktop service failed to start; inspect its journal",
            ));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Err(bad(
        "no verified desktop response within45seconds; no process was killed",
    ))
}
fn resource(v: &Value, command: Vec<String>, browser: bool) -> Result<Vec<String>> {
    let unit = if browser {
        format!("{APP}-window-{}", std::process::id())
    } else {
        format!("{APP}-http")
    };
    let mut out = vec![
        text(v, "resource")?.into(),
        "resource".into(),
        "launch".into(),
        "--class".into(),
        if browser { "medium" } else { "light" }.into(),
        "--kind".into(),
        "generic".into(),
        "--activity".into(),
        "foreground".into(),
        "--latency".into(),
        "interactive".into(),
        "--unit".into(),
        unit,
        "--timeout".into(),
        "0".into(),
        "--memory-demand-mib".into(),
        if browser { "768" } else { "64" }.into(),
        "--demand-owner".into(),
        "tree-of-sophia".into(),
        "--demand-key".into(),
        if browser {
            "desktop-browser"
        } else {
            "desktop-http"
        }
        .into(),
        "--estimate-source".into(),
        "owner-declared-desktop-startup".into(),
        "--estimate-confidence".into(),
        "conservative".into(),
        "--no-same-dir".into(),
        "--".into(),
    ];
    out.extend(command);
    Ok(out)
}
fn browser(v: &Value) -> Result<Vec<String>> {
    Ok(vec![
        text(v, "browser")?.into(),
        "--app=http://127.0.0.1:44339/constructor.html".into(),
        format!("--user-data-dir={}", text(v, "profile")?),
        format!("--disk-cache-dir={}", text(v, "cache")?),
        "--disk-cache-size=33554432".into(),
        format!("--class={APP}"),
        "--no-first-run".into(),
    ])
}
fn launch(path: &Path, mode: &str) -> Result<i32> {
    let v = load(path)?;
    match mode {
        "--dry-run" => {
            println!(
                "{}",
                json!({"url":"http://127.0.0.1:44339/constructor.html","release":v["release"],"service":v["service_unit"],"resource_server_argv":resource(&v,worker(&v)?,false)?,"browser_argv":browser(&v)?,"installs":false,"starts":false})
            );
        }
        "--status" => {
            if release_digest(Path::new(text(&v, "release")?))? != text(&v, "release_digest")? {
                return Err(bad("release changed"));
            }
            let found = identity(&v)?;
            println!(
                "{}",
                json!({"ready":found.is_some(),"identity":found,"service":properties(text(&v,"service_unit")?)?})
            );
            return Ok(if found.is_some() { 0 } else { 1 });
        }
        "--stop-worker" => {
            if owns(&v, None)? {
                checked(
                    &[
                        "systemctl".into(),
                        "--user".into(),
                        "stop".into(),
                        text(&v, "worker_unit")?.into(),
                    ],
                    12,
                )?;
            } else if properties(text(&v, "worker_unit")?)?
                .get("ActiveState")
                .is_some_and(|s| matches!(s.as_str(), "active" | "activating"))
            {
                return Err(bad("refusing to stop an unowned active worker"));
            }
        }
        "--run-server" => {
            let runtime = Path::new(text(&v, "runtime")?);
            private_dir(runtime)?;
            redirect(&runtime.join("server.log"))?;
            let args = resource(&v, worker(&v)?, false)?;
            return Ok(Command::new(&args[0])
                .args(&args[1..])
                .stdin(Stdio::null())
                .status()?
                .code()
                .unwrap_or(1));
        }
        "--browser-worker" => {
            for key in ["profile", "cache", "runtime"] {
                private_dir(Path::new(text(&v, key)?))?;
            }
            redirect(&Path::new(text(&v, "runtime")?).join("browser.log"))?;
            let args = browser(&v)?;
            return Err(Command::new(&args[0]).args(&args[1..]).exec().into());
        }
        "--ensure-server" => {
            println!("{}", json!({"ready":true,"identity":ensure(&v)?}));
        }
        "" => {
            ensure(&v)?;
            let args = resource(
                &v,
                vec![
                    text(&v, "launcher")?.into(),
                    "launch".into(),
                    "--config".into(),
                    text(&v, "config")?.into(),
                    "--browser-worker".into(),
                ],
                true,
            )?;
            let code = Command::new(&args[0])
                .args(&args[1..])
                .stdin(Stdio::null())
                .status()?
                .code()
                .unwrap_or(1);
            if code != 0 {
                return Err(bad(format!("private browser launch failed: {code}")));
            }
        }
        _ => return Err(bad("unknown launch mode")),
    }
    Ok(0)
}
fn quoted(s: &str, desktop: bool) -> Result<String> {
    if s.contains(['\n', '\r', '\0']) {
        return Err(bad("multiline command path"));
    }
    let value = if desktop {
        s.replace('\\', "\\\\\\\\")
            .replace('"', "\\\\\"")
            .replace('`', "\\\\`")
            .replace('$', "\\\\$")
    } else {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('$', "$$")
    };
    Ok(format!("\"{}\"", value.replace('%', "%%")))
}
fn shell(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
fn program(name: &str) -> Result<PathBuf> {
    for base in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        let p = base.join(name);
        if fs::metadata(&p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0) {
            return Ok(fs::canonicalize(p)?);
        }
    }
    Err(bad(format!("required platform command missing: {name}")))
}
fn install(args: &BTreeMap<String, String>, commit: bool, replace: bool) -> Result<()> {
    let home = PathBuf::from(std::env::var_os("HOME").ok_or_else(|| bad("HOME absent"))?);
    let get = |key: &str, default: PathBuf| -> Result<PathBuf> {
        let p = args.get(key).map(PathBuf::from).unwrap_or(default);
        if !p.is_absolute() {
            return Err(bad(format!("{key} must be absolute")));
        }
        Ok(p)
    };
    let root = get(
        "application-root",
        PathBuf::from("/srv/abyss-machine/storage/artifacts/tos-sophia-demo"),
    )?;
    let release = fs::canonicalize(get("release", root.join("releases/meaning-v2"))?)?;
    let config = get("config", home.join(format!(".config/{APP}/config.json")))?;
    let bin = get("bin-dir", home.join(".local/bin"))?;
    let applications = get("applications-dir", home.join(".local/share/applications"))?;
    let units = get("systemd-dir", home.join(".config/systemd/user"))?;
    let runtime = get(
        "runtime-root",
        PathBuf::from("/srv/abyss-machine/runtimes/tos-sophia-demo"),
    )?;
    let cache = get(
        "cache-root",
        PathBuf::from("/srv/abyss-machine/cache/tos-sophia-demo"),
    )?;
    let browser = fs::canonicalize(get("browser", PathBuf::from("/usr/bin/chromium-browser"))?)?;
    let binary = root.join("desktop/tos-constructor-desktop");
    let icon = root.join("desktop/icon.svg");
    let wrapper = bin.join(APP);
    let desktop = applications.join(format!("{APP}.desktop"));
    let service = units.join(format!("{APP}.service"));
    let digest = release_digest(&release)?;
    let cfg = json!({"schema":"tos_sophia_demo_desktop_native_v1","release":release,"release_digest":digest,"port":44339,"browser":browser,"profile":runtime.join("profile"),"cache":cache,"runtime":runtime,"launcher":binary,"server":binary,"resource":program("abyss-machine")?,"service_unit":format!("{APP}.service"),"worker_unit":format!("{APP}-http.service")});
    let invoke = vec![
        binary.to_string_lossy().into_owned(),
        "launch".into(),
        "--config".into(),
        config.to_string_lossy().into_owned(),
    ];
    let command = |mode: &str| -> Result<String> {
        invoke
            .iter()
            .map(|s| quoted(s, false))
            .chain(std::iter::once(quoted(mode, false)))
            .collect::<Result<Vec<_>>>()
            .map(|x| x.join(" "))
    };
    let unit =
        include_str!("../../../../access/web/constructor/desktop/tos-sophia-demo.service.in")
            .replace("@START@", &command("--run-server")?)
            .replace("@STOP@", &command("--stop-worker")?)
            .replace("Environment=PYTHONUNBUFFERED=1\n", "");
    let entry =
        include_str!("../../../../access/web/constructor/desktop/tos-sophia-demo.desktop.in")
            .replace("@EXEC@", &quoted(&wrapper.to_string_lossy(), true)?)
            .replace("@BROWSER@", &browser.to_string_lossy())
            .replace("@ICON@", &icon.to_string_lossy());
    let wrapper_body = format!(
        "#!/bin/sh\nexec {} \"$@\"\n",
        invoke
            .iter()
            .map(|s| shell(s))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let outputs = vec![
        (binary, fs::read(std::env::current_exe()?)?, 0o700),
        (
            icon,
            include_bytes!("../../../../access/web/constructor/desktop/icon.svg").to_vec(),
            0o644,
        ),
        (config, serde_json::to_vec_pretty(&cfg)?, 0o600),
        (wrapper, wrapper_body.into_bytes(), 0o700),
        (desktop.clone(), entry.into_bytes(), 0o644),
        (service, unit.into_bytes(), 0o644),
    ];
    if commit {
        for (path, raw, _) in &outputs {
            if let Ok(meta) = fs::symlink_metadata(path) {
                if !meta.is_file() || (!replace && fs::read(path)? != *raw) {
                    return Err(bad(format!(
                        "existing installation differs or is a symlink: {}",
                        path.display()
                    )));
                }
            }
        }
        let state = properties(&format!("{APP}.service"))?;
        if state
            .get("ActiveState")
            .is_some_and(|s| matches!(s.as_str(), "active" | "activating" | "reloading"))
        {
            return Err(bad(
                "owned desktop service is active; explicit stop required before replacement",
            ));
        }
        let validator = program("desktop-file-validate")?;
        let validation =
            std::env::temp_dir().join(format!("tos-native-desktop-{}.desktop", std::process::id()));
        {
            let mut f = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&validation)?;
            f.write_all(&outputs.iter().find(|(p, _, _)| p == &desktop).unwrap().1)?;
        }
        let checked_result = checked(
            &[
                validator.to_string_lossy().into_owned(),
                validation.to_string_lossy().into_owned(),
            ],
            10,
        );
        fs::remove_file(validation)?;
        checked_result?;
        for (path, raw, mode) in &outputs {
            fs::create_dir_all(path.parent().ok_or_else(|| bad("installation parent"))?)?;
            let stage = path.with_extension("native-new");
            let mut f = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(*mode)
                .open(&stage)?;
            let result = (|| -> Result<()> {
                f.write_all(raw)?;
                f.sync_all()?;
                fs::rename(&stage, path)?;
                File::open(path.parent().unwrap())?.sync_all()?;
                Ok(())
            })();
            if stage.exists() {
                fs::remove_file(&stage)?;
            }
            result?;
        }
        checked(
            &["systemctl".into(), "--user".into(), "daemon-reload".into()],
            10,
        )?;
        if let Ok(program) = program("update-desktop-database") {
            checked(
                &[
                    program.to_string_lossy().into_owned(),
                    applications.to_string_lossy().into_owned(),
                ],
                10,
            )?;
        }
    }
    println!(
        "{}",
        json!({"mode":if commit{"installed"}else{"dry-run"},"release":release,"release_digest":digest,"files":outputs.iter().map(|(p,r,m)|json!({"path":p,"bytes":r.len(),"mode":format!("0o{m:o}")})).collect::<Vec<_>>(),"profile":cfg["profile"],"cache":cache,"starts_services":false,"opens_browser":false,"creates_profile_or_cache":false})
    );
    Ok(())
}
fn main_run() -> Result<i32> {
    let mut iter = std::env::args().skip(1);
    let op = iter.next().unwrap_or_else(|| "--help".into());
    let mut args = BTreeMap::new();
    let mut mode = String::new();
    let (mut commit, mut replace) = (false, false);
    while let Some(key) = iter.next() {
        match key.as_str() {
            "--install" => commit = true,
            "--replace" => replace = true,
            "--status" | "--dry-run" | "--ensure-server" | "--run-server" | "--stop-worker"
            | "--browser-worker" => {
                if !mode.is_empty() {
                    return Err(bad("one launch mode required"));
                }
                mode = key;
            }
            _ if key.starts_with("--") => {
                let value = iter.next().ok_or_else(|| bad("missing flag value"))?;
                if args.insert(key[2..].into(), value).is_some() {
                    return Err(bad("duplicate flag"));
                }
            }
            _ => return Err(bad("unexpected positional argument")),
        }
    }
    match op.as_str() {
        "serve" => {
            let root = Path::new(
                args.get("release")
                    .ok_or_else(|| bad("--release required"))?,
            );
            let port = args
                .get("port")
                .map(|v| v.parse())
                .transpose()?
                .unwrap_or(44336);
            serve(
                root,
                port,
                args.get("release-digest").map(String::as_str),
                args.get("log").map(Path::new),
            )?;
        }
        "digest" => println!(
            "{}",
            release_digest(Path::new(
                args.get("release")
                    .ok_or_else(|| bad("--release required"))?
            ))?
        ),
        "install" => install(&args, commit, replace)?,
        "launch" => {
            let path = args.get("config").map(PathBuf::from).unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                    .join(format!(".config/{APP}/config.json"))
            });
            return launch(&path, &mode);
        }
        "--help" | "-h" => println!(
            "usage: tos-constructor-desktop serve --release DIR [--port N] [--release-digest SHA256] [--log PATH]\n       tos-constructor-desktop digest --release DIR\n       tos-constructor-desktop install [--release DIR] [--install] [--replace] [--config PATH]\n       tos-constructor-desktop launch [--config PATH] [--status|--dry-run|--ensure-server]"
        ),
        _ => return Err(bad("unknown desktop operation")),
    }
    Ok(0)
}
fn main() {
    match main_run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("tos-constructor-desktop: {error}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let p = std::env::temp_dir().join(format!(
                "tos-native-desktop-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            fs::create_dir(p.join("assets")).unwrap();
            for (name, raw) in [
                ("constructor.html", b"<html>Sophia</html>".as_slice()),
                ("library.json", b"{}"),
                ("assets/app.js", b"document.title='Sophia'"),
                ("receipt.json", b"private"),
            ] {
                fs::write(p.join(name), raw).unwrap();
            }
            Self(p)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn get(root: &Path, route: &str, host: Option<&str>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let allowed = files(root).unwrap();
        let worker = std::thread::spawn(move || {
            let (s, _) = listener.accept().unwrap();
            request(s, port, &allowed, b"{\"identity\":true}").unwrap();
        });
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(
            stream,
            "GET {route} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            host.map(str::to_owned)
                .unwrap_or_else(|| format!("127.0.0.1:{port}"))
        )
        .unwrap();
        let mut raw = String::new();
        stream.read_to_string(&mut raw).unwrap();
        worker.join().unwrap();
        raw
    }
    #[test]
    fn native_http_preserves_display_allowlist_host_fence_and_identity() {
        let fixture = Fixture::new();
        for route in [
            "/",
            "/constructor.html",
            "/library.json",
            "/assets/app.js",
            IDENTITY,
        ] {
            let r = get(&fixture.0, route, None);
            assert!(r.starts_with("HTTP/1.1 200 "), "{route}");
            assert!(r.contains("X-Content-Type-Options: nosniff"));
        }
        for route in [
            "/receipt.json",
            "/assets/../receipt.json",
            "/assets/%2e%2e/receipt.json",
            "/assets/",
            "/assets/%00.js",
        ] {
            assert!(
                get(&fixture.0, route, None).starts_with("HTTP/1.1 404 "),
                "{route}"
            );
        }
        assert!(get(&fixture.0, IDENTITY, Some("unrelated.example")).starts_with("HTTP/1.1 403 "));
    }
    #[test]
    fn native_release_digest_excludes_private_neighbors_binds_assets_and_refuses_symlinks() {
        let f = Fixture::new();
        let before = release_digest(&f.0).unwrap();
        fs::write(f.0.join("receipt.json"), b"different private receipt").unwrap();
        assert_eq!(before, release_digest(&f.0).unwrap());
        fs::write(f.0.join("assets/app.js"), b"changed").unwrap();
        assert_ne!(before, release_digest(&f.0).unwrap());
        assert!(serve(&f.0, 0, Some(&before), None).is_err());
        symlink(f.0.join("receipt.json"), f.0.join("assets/secret")).unwrap();
        assert!(files(&f.0).is_err());
    }
    #[test]
    fn native_launch_configuration_and_dry_run_keep_profile_uncreated() {
        let f = Fixture::new();
        let runtime = f.0.join("runtime");
        let config = f.0.join("config.json");
        let exe = std::env::current_exe().unwrap();
        let value = json!({"schema":"tos_sophia_demo_desktop_native_v1","release":f.0,"release_digest":release_digest(&f.0).unwrap(),"port":44339,"browser":"/usr/bin/true","profile":runtime.join("profile"),"cache":f.0.join("cache"),"runtime":runtime,"launcher":exe,"server":exe,"resource":"/usr/bin/true","service_unit":"tos-sophia-demo.service","worker_unit":"tos-sophia-demo-http.service"});
        fs::write(&config, serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(launch(&config, "--dry-run").unwrap(), 0);
        assert!(!runtime.exists());
        let loaded = load(&config).unwrap();
        assert!(
            !browser(&loaded)
                .unwrap()
                .iter()
                .any(|arg| arg == "--no-sandbox")
        );
        let mut bad = value;
        bad["profile"] = bad["cache"].clone();
        fs::write(&config, serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(load(&config).is_err());
    }
}
