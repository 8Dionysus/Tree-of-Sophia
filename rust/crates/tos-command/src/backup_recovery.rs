//! Bounded owner-selected PostgreSQL/STO backup transport.
//! The owner must quiesce its isolated database and byte store. These functions
//! preserve committed custody; they grant no source, rights or publication authority.
use crate::DurablePgCoordinator;
use postgres::{Client, NoTls};
use serde_json::{Value, json};
use std::os::{
    fd::AsRawFd,
    unix::{
        fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        process::CommandExt,
    },
};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};
use tos_foundation::Digest256Hasher;
use tos_segment_store::{SegmentLimits, SegmentStore};
const BYTES: u64 = 64 * 1024 * 1024;
const FILES: usize = 256;
const DIRS: [&str; 6] = [
    "staging",
    "segments",
    "pins",
    "attempts",
    "leaves",
    "generations",
];
pub struct PgTool<'a> {
    pub path: &'a Path,
    pub sha256: &'a str,
}
pub struct BackupSelection<'a> {
    pub pg_url: &'a str,
    pub domain: &'a str,
    pub store_root: &'a Path,
    pub backup_root: &'a Path,
    pub tool: PgTool<'a>,
    pub store_limits: SegmentLimits,
    pub quiescent_owner_confirmed: bool,
}
pub struct RestoreSelection<'a> {
    pub pg_url: &'a str,
    pub domain: &'a str,
    pub backup_root: &'a Path,
    pub receipt_sha256: &'a str,
    pub store_root: &'a Path,
    pub tool: PgTool<'a>,
    pub store_limits: SegmentLimits,
    pub fresh_target_owner_confirmed: bool,
}
fn error() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "owner-selected backup/restore integrity check failed",
    )
}
fn require(ok: bool) -> io::Result<()> {
    if ok { Ok(()) } else { Err(error()) }
}
fn active(d: Instant, c: &AtomicBool) -> io::Result<()> {
    if c.load(Ordering::Relaxed) {
        return Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "backup/restore cancelled",
        ));
    }
    if Instant::now() >= d {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "backup/restore deadline exceeded",
        ));
    }
    Ok(())
}

fn root(path: &Path) -> io::Result<File> {
    let f = tos_fd_open::open_absolute_directory(path).map_err(|_| error())?;
    let m = f.metadata()?;
    require(m.uid() == rustix::process::geteuid().as_raw() && m.mode() & 0o777 == 0o700)?;
    Ok(f)
}
fn anchored(f: &File) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", f.as_raw_fd()))
}
fn distinct(a: &File, b: &File) -> io::Result<()> {
    let x = a.metadata()?;
    let y = b.metadata()?;
    require((x.dev(), x.ino()) != (y.dev(), y.ino()))
}
fn hash(path: &Path, d: Instant, c: &AtomicBool) -> io::Result<(u64, String)> {
    active(d, c)?;
    let mut f = OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)?;
    let before = f.metadata()?;
    require(before.is_file() && before.nlink() == 1 && before.len() <= BYTES)?;
    let mut h = Digest256Hasher::new();
    let mut buf = [0u8; 65536];
    let mut n = 0u64;
    loop {
        active(d, c)?;
        let k = f.read(&mut buf)?;
        if k == 0 {
            break;
        }
        n = n.checked_add(k as u64).ok_or_else(error)?;
        require(n <= BYTES)?;
        h.update(&buf[..k]);
    }
    let after = f.metadata()?;
    let named = fs::symlink_metadata(path)?;
    require(
        n == before.len()
            && (
                before.dev(),
                before.ino(),
                before.len(),
                before.mtime(),
                before.mtime_nsec(),
                before.ctime(),
                before.ctime_nsec(),
            ) == (
                after.dev(),
                after.ino(),
                after.len(),
                after.mtime(),
                after.mtime_nsec(),
                after.ctime(),
                after.ctime_nsec(),
            )
            && (named.dev(), named.ino()) == (before.dev(), before.ino()),
    )?;
    Ok((n, h.finalize().to_hex()))
}
fn inventory(root: &Path, d: Instant, c: &AtomicBool) -> io::Result<Vec<Value>> {
    let mut out = Vec::new();
    let mut total = 0u64;
    let mut entries = 0usize;
    for entry in fs::read_dir(root)? {
        active(d, c)?;
        entries += 1;
        require(entries <= 512)?;
        let e = entry?;
        let p = e.path();
        let m = fs::symlink_metadata(&p)?;
        if m.is_dir() {
            let name = e.file_name().into_string().map_err(|_| error())?;
            require(DIRS.contains(&name.as_str()))?;
            for child in fs::read_dir(p)? {
                active(d, c)?;
                entries += 1;
                require(entries <= 512 && out.len() < FILES)?;
                let p = child?.path();
                let (bytes, sha) = hash(&p, d, c)?;
                total = total.checked_add(bytes).ok_or_else(error)?;
                require(total <= BYTES)?;
                out.push(json!({"path":format!("{}/{}",name,p.file_name().and_then(|n|n.to_str()).ok_or_else(error)?),"bytes":bytes,"sha256":sha}));
            }
        } else {
            require(m.is_file() && e.file_name() == "store.meta" && out.len() < FILES)?;
            let (bytes, sha) = hash(&p, d, c)?;
            total = total.checked_add(bytes).ok_or_else(error)?;
            require(total <= BYTES)?;
            out.push(json!({"path":"store.meta","bytes":bytes,"sha256":sha}));
        }
    }
    out.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    require(out.iter().any(|v| v["path"] == "store.meta"))?;
    Ok(out)
}
fn copy_store(
    source: &File,
    target: &File,
    members: &[Value],
    d: Instant,
    c: &AtomicBool,
) -> io::Result<()> {
    distinct(source, target)?;
    let from = anchored(source);
    let to = anchored(target);
    require(fs::read_dir(&to)?.next().is_none() && inventory(&from, d, c)? == members)?;
    for name in DIRS {
        if from.join(name).is_dir() {
            fs::create_dir(to.join(name))?;
            fs::set_permissions(to.join(name), fs::Permissions::from_mode(0o700))?;
        }
    }
    for member in members {
        active(d, c)?;
        let relative = Path::new(member["path"].as_str().ok_or_else(error)?);
        require(
            relative
                .components()
                .all(|p| matches!(p, std::path::Component::Normal(_))),
        )?;
        let mut input = OpenOptions::new()
            .read(true)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
            .open(from.join(relative))?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
            .open(to.join(relative))?;
        let mut count = 0u64;
        let mut buf = [0u8; 65536];
        loop {
            active(d, c)?;
            let n = input.read(&mut buf)?;
            if n == 0 {
                break;
            }
            count = count.checked_add(n as u64).ok_or_else(error)?;
            require(count <= member["bytes"].as_u64().ok_or_else(error)?)?;
            output.write_all(&buf[..n])?;
        }
        output.sync_all()?;
        let actual = hash(&to.join(relative), d, c)?;
        require(
            actual
                == (
                    member["bytes"].as_u64().ok_or_else(error)?,
                    member["sha256"].as_str().ok_or_else(error)?.to_owned(),
                ),
        )?;
    }
    require(inventory(&from, d, c)? == members && inventory(&to, d, c)? == members)?;
    for name in DIRS {
        if to.join(name).is_dir() {
            File::open(to.join(name))?.sync_all()?;
        }
    }
    target.sync_all()
}
fn client(url: &str, d: Instant, c: &AtomicBool) -> io::Result<Client> {
    active(d, c)?;
    let mut config = url.parse::<postgres::Config>().map_err(|_| error())?;
    config.connect_timeout(Duration::from_secs(5).min(d.saturating_duration_since(Instant::now())));
    let mut db = config.connect(NoTls).map_err(|_| error())?;
    active(d, c)?;
    db.batch_execute("SET statement_timeout='5s'; SET lock_timeout='5s'")
        .map_err(|_| error())?;
    let v: String = db
        .query_one("SHOW server_version_num", &[])
        .map_err(|_| error())?
        .get(0);
    require(v.parse::<u32>().map_err(|_| error())? / 10000 == 16)?;
    Ok(db)
}
fn tool(
    tool: &PgTool<'_>,
    args: &[&str],
    input: Option<File>,
    output: Option<File>,
    pg_url: &str,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Value> {
    active(d, c)?;
    let cap = rustix::process::getrlimit(rustix::process::Resource::Fsize);
    require(cap.current.is_some_and(|n| n <= BYTES) && cap.maximum.is_some_and(|n| n <= BYTES))?;
    let identity = hash(tool.path, d, c)?;
    require(identity.1 == tool.sha256)?;
    // Only standard streams cross a tool/container boundary. The host writer
    // inherits FSIZE; no directory or extra descriptor is exported.
    let mut command = Command::new(tool.path);
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("PG") {
            command.env_remove(name);
        }
    }
    // PGDATABASE is a database name, not a connection URI. Decode the same
    // configured target into private libpq environment without exposing URL argv.
    let selected = pg_url.parse::<postgres::Config>().map_err(|_| error())?;
    require(selected.get_hosts().len() == 1 && selected.get_ports().len() <= 1)?;
    let host = match &selected.get_hosts()[0] {
        postgres::config::Host::Tcp(host) => host.as_str(),
        _ => return Err(error()),
    };
    let user = selected.get_user().ok_or_else(error)?;
    let database = selected.get_dbname().ok_or_else(error)?;
    command
        .env("PGHOST", host)
        .env(
            "PGPORT",
            selected
                .get_ports()
                .first()
                .copied()
                .unwrap_or(5432)
                .to_string(),
        )
        .env("PGUSER", user)
        .env("PGDATABASE", database)
        .env("PGCONNECT_TIMEOUT", "5")
        .env("PGSSLMODE", "disable");
    if let Some(password) = selected.get_password() {
        command.env(
            "PGPASSWORD",
            std::str::from_utf8(password).map_err(|_| error())?,
        );
    }
    if let Some(options) = selected.get_options() {
        command.env("PGOPTIONS", options);
    }
    let spawned = command
        .args(args)
        .stdin(input.map(Stdio::from).unwrap_or_else(Stdio::null))
        .stdout(output.map(Stdio::from).unwrap_or_else(Stdio::null))
        .stderr(Stdio::null())
        .process_group(0)
        .spawn();
    let child = spawned?;
    let group = rustix::process::Pid::from_raw(child.id() as i32).ok_or_else(error)?;
    struct OwnedTool {
        child: std::process::Child,
        group: rustix::process::Pid,
    }
    impl Drop for OwnedTool {
        fn drop(&mut self) {
            let _ = rustix::process::kill_process_group(self.group, rustix::process::Signal::KILL);
            let _ = self.child.wait();
        }
    }
    let mut owned = OwnedTool { child, group };
    let limit = d.min(Instant::now() + Duration::from_secs(60));
    loop {
        if let Some(status) = owned.child.try_wait()? {
            require(status.success())?;
            break;
        }
        active(limit, c)?;
        thread::sleep(Duration::from_millis(10));
    }
    drop(owned);
    require(hash(tool.path, d, c)? == identity)?;
    Ok(json!({"path":tool.path,"bytes":identity.0,"sha256":identity.1,"max_seconds":60}))
}
fn frozen_domain(db: &mut Client, domain: &str) -> io::Result<()> {
    let rows = db
        .query("SELECT domain FROM cmd2_domain LIMIT 2", &[])
        .map_err(|_| error())?;
    require(rows.len() == 1 && rows[0].get::<_, String>(0) == domain)?;
    for table in ["cmd2_history", "cmd2_log"] {
        let query = format!("SELECT 1 FROM {table} LIMIT 257");
        require(db.query(&query, &[]).map_err(|_| error())?.len() <= 256)?;
    }
    Ok(())
}
fn cut(
    url: &str,
    domain: &str,
    store: &SegmentStore,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Value> {
    active(d, c)?;
    let bounded = client(url, d, c)?;
    let mut db = DurablePgCoordinator::from_configured_client(bounded);
    let cut = db
        .cold_verify_cut_with_budget(store, domain, Some((d, c)))
        .map_err(|_| error())?;
    active(d, c)?;
    Ok(
        json!({"digest":cut.log_digest().to_hex(),"through_commit_seq":cut.through_commit_seq(),"historical_members":cut.historical_members()}),
    )
}

pub fn backup_quiescent(s: &BackupSelection<'_>, d: Instant, c: &AtomicBool) -> io::Result<Value> {
    if !s.quiescent_owner_confirmed {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "quiescent owner confirmation required",
        ));
    }
    let source = root(s.store_root)?;
    let backup = root(s.backup_root)?;
    distinct(&source, &backup)?;
    let target = anchored(&backup);
    require(fs::read_dir(&target)?.next().is_none())?;
    let mut db = client(s.pg_url, d, c)?;
    frozen_domain(&mut db, s.domain)?;
    let database: String = db
        .query_one("SELECT current_database()", &[])
        .map_err(|_| error())?
        .get(0);
    let store = SegmentStore::open_existing(s.store_root, s.store_limits).map_err(|_| error())?;
    let original_cut = cut(s.pg_url, s.domain, &store, d, c)?;
    let members = inventory(&anchored(&source), d, c)?;
    let dump = target.join("metadata.dump");
    let dump_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(&dump)?;
    let tool_receipt = tool(
        &s.tool,
        &["--format=custom", "--no-owner", "--no-privileges"],
        None,
        Some(dump_file.try_clone()?),
        s.pg_url,
        d,
        c,
    )?;
    // Child close is not a durability fence.
    dump_file.sync_all()?;
    let dump_identity = hash(&dump, d, c)?;
    let mut magic = [0u8; 5];
    File::open(&dump)?.read_exact(&mut magic)?;
    require(&magic == b"PGDMP")?;
    fs::create_dir(target.join("store"))?;
    fs::set_permissions(target.join("store"), fs::Permissions::from_mode(0o700))?;
    let copied =
        tos_fd_open::open_directory_at(&backup, Path::new("store")).map_err(|_| error())?;
    copy_store(&source, &copied, &members, d, c)?;
    frozen_domain(&mut db, s.domain)?;
    require(cut(s.pg_url, s.domain, &store, d, c)? == original_cut)?;
    let receipt = json!({"schema":"tos_cmd2_quiescent_backup_v1","domain":s.domain,"database":database,"cut":original_cut,"store_members":members,"dump":{"bytes":dump_identity.0,"sha256":dump_identity.1},"tool":tool_receipt,"source_admission":false,"production_quiescence_verified":false});
    let raw = serde_json::to_vec(&receipt).map_err(|_| error())?;
    require(raw.len() <= 1024 * 1024)?;
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(target.join("receipt.json"))?;
    f.write_all(&raw)?;
    f.sync_all()?;
    backup.sync_all()?;
    active(d, c)?;
    Ok(
        json!({"status":"backup_complete","receipt":receipt,"receipt_sha256":hash(&target.join("receipt.json"),d,c)?.1}),
    )
}
pub fn restore_into_fresh(
    s: &RestoreSelection<'_>,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Value> {
    if !s.fresh_target_owner_confirmed {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "fresh target owner confirmation required",
        ));
    }
    let backup = root(s.backup_root)?;
    let restored = root(s.store_root)?;
    distinct(&backup, &restored)?;
    let base = anchored(&backup);
    let receipt_path = base.join("receipt.json");
    let (length, sha) = hash(&receipt_path, d, c)?;
    require(length <= 1024 * 1024 && sha == s.receipt_sha256)?;
    let receipt: Value = serde_json::from_slice(&fs::read(&receipt_path)?).map_err(|_| error())?;
    require(receipt["schema"] == "tos_cmd2_quiescent_backup_v1" && receipt["domain"] == s.domain)?;
    let mut db = client(s.pg_url, d, c)?;
    let database: String = db
        .query_one("SELECT current_database()", &[])
        .map_err(|_| error())?
        .get(0);
    require(Some(database.as_str()) != receipt["database"].as_str())?;
    require(
        db.query_one(
            "SELECT count(*) FROM pg_catalog.pg_tables WHERE schemaname='public'",
            &[],
        )
        .map_err(|_| error())?
        .get::<_, i64>(0)
            == 0,
    )?;
    let members = receipt["store_members"].as_array().ok_or_else(error)?;
    require(members.len() <= FILES)?;
    let source =
        tos_fd_open::open_directory_at(&backup, Path::new("store")).map_err(|_| error())?;
    copy_store(&source, &restored, members, d, c)?;
    let dump = base.join("metadata.dump");
    let expected = (
        receipt["dump"]["bytes"].as_u64().ok_or_else(error)?,
        receipt["dump"]["sha256"]
            .as_str()
            .ok_or_else(error)?
            .to_owned(),
    );
    require(hash(&dump, d, c)? == expected)?;
    let dump_file = OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(&dump)?;
    let tool_receipt = tool(
        &s.tool,
        &[
            "--exit-on-error",
            "--single-transaction",
            "--no-owner",
            "--no-privileges",
        ],
        Some(dump_file),
        None,
        s.pg_url,
        d,
        c,
    )?;
    require(hash(&dump, d, c)? == expected && hash(&receipt_path, d, c)?.1 == s.receipt_sha256)?;
    frozen_domain(&mut db, s.domain)?;
    let store = SegmentStore::open_existing(s.store_root, s.store_limits).map_err(|_| error())?;
    let recovered = cut(s.pg_url, s.domain, &store, d, c)?;
    require(recovered == receipt["cut"])?;
    active(d, c)?;
    Ok(
        json!({"status":"restore_verified","domain":s.domain,"cut":recovered,"tool":tool_receipt,"source_admission":false,"source_currentness_granted":false}),
    )
}
