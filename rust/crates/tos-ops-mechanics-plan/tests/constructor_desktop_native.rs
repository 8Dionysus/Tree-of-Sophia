//! Native desktop lifecycle against a private fake service manager. Only the
//! selected HTTP process is started; the user's systemd and browser are unused.
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
struct Sandbox {
    root: PathBuf,
    children: Vec<Child>,
    managed: Option<(PathBuf, Vec<String>)>,
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        for child in &mut self.children {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some((state, worker)) = &self.managed {
            if let Ok(pid) = fs::read_to_string(state)
                .and_then(|s| s.parse::<u32>().map_err(std::io::Error::other))
            {
                if let Ok(raw) = fs::read(format!("/proc/{pid}/cmdline")) {
                    let actual = raw
                        .split(|b| *b == 0)
                        .filter(|b| !b.is_empty())
                        .map(|b| String::from_utf8_lossy(b).into_owned())
                        .collect::<Vec<_>>();
                    if actual == *worker {
                        unsafe {
                            libc::kill(pid as i32, libc::SIGTERM);
                        }
                    }
                }
            }
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn exec(path: &Path, raw: &str) {
    fs::write(path, raw).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}
fn quote(path: &Path) -> String {
    format!("'{}'", path.to_str().unwrap().replace('\'', "'\\''"))
}
fn response(port: u16) -> std::io::Result<Value> {
    let mut socket = TcpStream::connect_timeout(
        &format!("127.0.0.1:{port}").parse().unwrap(),
        Duration::from_millis(200),
    )?;
    socket.set_read_timeout(Some(Duration::from_secs(1)))?;
    write!(
        socket,
        "GET /__tos_demo_identity HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    )?;
    let mut raw = String::new();
    socket.take(32769).read_to_string(&mut raw)?;
    assert!(raw.len() <= 32768);
    let (head, body) = raw.split_once("\r\n\r\n").unwrap();
    assert!(head.starts_with("HTTP/1.1 200"));
    Ok(serde_json::from_str(body).unwrap())
}
fn ready(child: &mut Child, port: u16) -> Value {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(v) = response(port) {
            return v;
        }
        assert!(child.try_wait().unwrap().is_none(), "server exited");
        assert!(Instant::now() < until, "server readiness deadline");
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn output(mut command: Command) -> std::process::Output {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    let until = Instant::now() + Duration::from_secs(12);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= until {
            let _ = child.kill();
            let _ = child.wait();
            panic!("native desktop step deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let r = child.wait_with_output().unwrap();
    assert!(r.stdout.len() + r.stderr.len() <= 1_048_576);
    r
}
#[test]
fn native_desktop_preserves_owned_startup_concurrency_pipe_lifetime_and_install_plan() {
    let binary = std::env::var_os("TOS_NATIVE_DESKTOP_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_tos-constructor-desktop")));
    assert!(binary.is_absolute() && binary.is_file());
    // Do not exercise the fixed desktop endpoint if another owner holds it.
    let probe = TcpListener::bind("127.0.0.1:44339")
        .expect("selected desktop test port already occupied; no process was stopped");
    drop(probe);
    let root = std::env::temp_dir().join(format!(
        "tos-native-desktop-lifecycle-{}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    let mut guard = Sandbox {
        root: root.clone(),
        children: vec![],
        managed: None,
    };
    let release = root.join("release");
    fs::create_dir(&release).unwrap();
    fs::create_dir(release.join("assets")).unwrap();
    fs::write(release.join("constructor.html"), b"<html>Sophia</html>").unwrap();
    fs::write(release.join("library.json"), b"{}").unwrap();
    fs::write(release.join("assets/app.js"), b"test").unwrap();
    let mut c = Command::new(&binary);
    c.args(["digest", "--release"]).arg(&release);
    let r = output(c);
    assert!(r.status.success());
    let digest = String::from_utf8(r.stdout).unwrap().trim().to_owned();
    let runtime = root.join("runtime");
    fs::create_dir(&runtime).unwrap();
    let tools = root.join("tools");
    fs::create_dir(&tools).unwrap();
    let state = root.join("pid");
    let calls = root.join("calls");
    let config = root.join("config.json");
    let xdg = root.join("xdg");
    fs::create_dir(&xdg).unwrap();
    let worker = vec![
        binary.to_string_lossy().to_string(),
        "serve".into(),
        "--release".into(),
        release.to_string_lossy().to_string(),
        "--port".into(),
        "44339".into(),
        "--release-digest".into(),
        digest.clone(),
        "--log".into(),
        runtime.join("server.log").to_string_lossy().to_string(),
    ];
    guard.managed = Some((state.clone(), worker.clone()));
    let value = json!({"schema":"tos_sophia_demo_desktop_native_v1","release":release,"release_digest":digest,"port":44339,"browser":"/usr/bin/true","profile":runtime.join("profile"),"cache":root.join("cache"),"runtime":runtime,"launcher":binary,"server":binary,"resource":"/usr/bin/true","service_unit":"tos-sophia-demo.service","worker_unit":"tos-sophia-demo-http.service"});
    fs::write(&config, serde_json::to_vec(&value).unwrap()).unwrap();
    let command_text = worker
        .iter()
        .map(|v| quote(Path::new(v)))
        .collect::<Vec<_>>()
        .join(" ");
    exec(
        &tools.join("systemctl"),
        &format!(
            "#!/bin/sh\nset -eu\ncase \"$2\" in\n show) pid=0; [ ! -f {state} ] || pid=$(/usr/bin/cat {state}); printf 'MainPID=%s\\nActiveState=active\\nSubState=running\\nResult=success\\n' \"$pid\";;\n start) printf 'start\\n' >> {calls}; {command} </dev/null >/dev/null 2>/dev/null & printf '%s' \"$!\" > {state};;\n stop) printf 'stop\\n' >> {calls};;\n *) printf '%s\\n' \"$2\" >> {calls};;\nesac\n",
            state = quote(&state),
            calls = quote(&calls),
            command = command_text
        ),
    );
    let launch = |mode: &str| {
        let mut c = Command::new(&binary);
        c.args(["launch", "--config"])
            .arg(&config)
            .arg(mode)
            .env("PATH", &tools)
            .env("XDG_RUNTIME_DIR", &xdg);
        c
    };
    let mut server = Command::new(&binary);
    server
        .args(&worker[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = server.spawn().unwrap();
    drop(child.stdout.take());
    drop(child.stderr.take());
    let first = ready(&mut child, 44339);
    assert_eq!(first["pid"], child.id());
    assert_eq!(first["release"], json!(release));
    guard.children.push(child);
    // A plausible HTTP identity alone cannot authorize reuse or stopping.
    let refused = output(launch("--ensure-server"));
    assert!(!refused.status.success());
    assert!(!calls.exists());
    let refused = output(launch("--stop-worker"));
    assert!(!refused.status.success());
    assert!(!calls.exists());
    assert!(guard.children[0].try_wait().unwrap().is_none());
    fs::write(&state, guard.children[0].id().to_string()).unwrap();
    let reused = output(launch("--ensure-server"));
    assert!(
        reused.status.success(),
        "{}",
        String::from_utf8_lossy(&reused.stderr)
    );
    assert!(!calls.exists());
    // Changed release content is refused before any service-manager action.
    fs::write(release.join("assets/app.js"), b"changed").unwrap();
    let changed = output(launch("--ensure-server"));
    assert!(!changed.status.success());
    assert!(!calls.exists());
    fs::write(release.join("assets/app.js"), b"test").unwrap();
    guard.children[0].kill().unwrap();
    guard.children[0].wait().unwrap();
    fs::remove_file(&state).unwrap();
    let mut left = launch("--ensure-server")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut right = launch("--ensure-server")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let until = Instant::now() + Duration::from_secs(8);
    while left.try_wait().unwrap().is_none() || right.try_wait().unwrap().is_none() {
        if Instant::now() >= until {
            let _ = left.kill();
            let _ = right.kill();
            panic!("concurrent ensure deadline");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let left = left.wait_with_output().unwrap();
    let right = right.wait_with_output().unwrap();
    assert!(
        left.status.success(),
        "{}",
        String::from_utf8_lossy(&left.stderr)
    );
    assert!(
        right.status.success(),
        "{}",
        String::from_utf8_lossy(&right.stderr)
    );
    assert_eq!(fs::read_to_string(&calls).unwrap(), "start\n");
    let pid = fs::read_to_string(&state).unwrap().parse::<u32>().unwrap();
    assert_eq!(response(44339).unwrap()["pid"], pid);
    // Only our fake manager's exact native argv child can be stopped by cleanup.
    let raw = fs::read(format!("/proc/{pid}/cmdline")).unwrap();
    let actual = raw
        .split(|b| *b == 0)
        .filter(|b| !b.is_empty())
        .map(|b| String::from_utf8(b.to_vec()).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(actual, worker);
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }
    exec(&tools.join("abyss-machine"), "#!/bin/sh\nexit 0\n");
    exec(&tools.join("chromium"), "#!/bin/sh\nexit 0\n");
    let installed = root.join("installed");
    let mut install = Command::new(&binary);
    install
        .arg("install")
        .arg("--release")
        .arg(&release)
        .arg("--application-root")
        .arg(&installed)
        .arg("--config")
        .arg(root.join("install-config.json"))
        .arg("--bin-dir")
        .arg(root.join("bin"))
        .arg("--applications-dir")
        .arg(root.join("applications"))
        .arg("--systemd-dir")
        .arg(root.join("units"))
        .arg("--runtime-root")
        .arg(root.join("install-runtime"))
        .arg("--cache-root")
        .arg(root.join("install-cache"))
        .env("PATH", &tools);
    let r = output(install);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let plan: Value = serde_json::from_slice(&r.stdout).unwrap();
    assert_eq!(plan["mode"], "dry-run");
    for key in [
        "starts_services",
        "opens_browser",
        "creates_profile_or_cache",
    ] {
        assert_eq!(plan[key], false);
    }
    assert_ne!(plan["profile"], plan["cache"]);
    assert!(!installed.exists() && !root.join("install-runtime").exists());
    assert_eq!(fs::read_to_string(&calls).unwrap(), "start\n");
    let service =
        include_str!("../../../../access/web/constructor/desktop/tos-sophia-demo.service.in");
    assert!(!service.contains("WantedBy="));
    assert!(service.contains("@START@") && service.contains("@STOP@"));
}
