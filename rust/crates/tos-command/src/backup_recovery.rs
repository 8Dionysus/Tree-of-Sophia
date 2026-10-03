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
/// Owner-selected finite transport/admission budget, not corpus topology.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupProfile {
    pub max_store_bytes: u64,
    pub max_dump_bytes: u64,
    pub max_files: usize,
    pub max_entries: usize,
    pub max_history_rows: u64,
    pub max_log_rows: u64,
    pub max_member_rows: u64,
    pub max_metadata_rows: u64,
    pub max_metadata_bytes: u64,
    pub max_receipt_bytes: u64,
}
impl Default for BackupProfile {
    fn default() -> Self {
        Self {
            max_store_bytes: BYTES,
            max_dump_bytes: BYTES,
            max_files: 256,
            max_entries: 512,
            max_history_rows: 256,
            max_log_rows: 256,
            max_member_rows: 256,
            max_metadata_rows: 100_000,
            max_metadata_bytes: BYTES,
            max_receipt_bytes: 1024 * 1024,
        }
    }
}
impl BackupProfile {
    pub fn validate(self) -> io::Result<Self> {
        for n in [
            self.max_store_bytes,
            self.max_dump_bytes,
            self.max_history_rows,
            self.max_log_rows,
            self.max_member_rows,
            self.max_metadata_rows,
            self.max_metadata_bytes,
            self.max_receipt_bytes,
        ] {
            if n == 0 || n > i64::MAX as u64 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid backup profile",
                ));
            }
        }
        if self.max_files == 0
            || self.max_entries < self.max_files
            || self.max_entries > i64::MAX as usize
            || self.max_receipt_bytes > usize::MAX as u64
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid backup profile",
            ));
        }
        Ok(self)
    }
}
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
    hash_bounded(path, BYTES, d, c)
}
fn hash_bounded(path: &Path, cap: u64, d: Instant, c: &AtomicBool) -> io::Result<(u64, String)> {
    active(d, c)?;
    let mut f = OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)?;
    let before = f.metadata()?;
    require(before.is_file() && before.nlink() == 1 && before.len() <= cap)?;
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
        require(n <= cap)?;
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
fn inventory(
    root: &Path,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Vec<Value>> {
    let mut out = Vec::new();
    let mut total = 0u64;
    let mut entries = 0usize;
    for entry in fs::read_dir(root)? {
        active(d, c)?;
        entries += 1;
        require(entries <= profile.max_entries)?;
        let e = entry?;
        let p = e.path();
        let m = fs::symlink_metadata(&p)?;
        if m.is_dir() {
            let name = e.file_name().into_string().map_err(|_| error())?;
            require(DIRS.contains(&name.as_str()))?;
            for child in fs::read_dir(p)? {
                active(d, c)?;
                entries += 1;
                if entries > profile.max_entries || out.len() >= profile.max_files {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "backup store inventory non-fit: observed entries={entries}, completed_files={}, completed_bytes={total}, limits entries={}, files={}",
                            out.len(),
                            profile.max_entries,
                            profile.max_files,
                        ),
                    ));
                }
                let p = child?.path();
                let (bytes, sha) = hash_bounded(&p, profile.max_store_bytes, d, c)?;
                total = total.checked_add(bytes).ok_or_else(error)?;
                require(total <= profile.max_store_bytes)?;
                out.push(json!({"path":format!("{}/{}",name,p.file_name().and_then(|n|n.to_str()).ok_or_else(error)?),"bytes":bytes,"sha256":sha}));
            }
        } else {
            require(m.is_file() && e.file_name() == "store.meta" && out.len() < profile.max_files)?;
            let (bytes, sha) = hash_bounded(&p, profile.max_store_bytes, d, c)?;
            total = total.checked_add(bytes).ok_or_else(error)?;
            require(total <= profile.max_store_bytes)?;
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
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<()> {
    distinct(source, target)?;
    let from = anchored(source);
    let to = anchored(target);
    require(fs::read_dir(&to)?.next().is_none() && inventory(&from, profile, d, c)? == members)?;
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
        let actual = hash_bounded(&to.join(relative), profile.max_store_bytes, d, c)?;
        require(
            actual
                == (
                    member["bytes"].as_u64().ok_or_else(error)?,
                    member["sha256"].as_str().ok_or_else(error)?.to_owned(),
                ),
        )?;
    }
    require(
        inventory(&from, profile, d, c)? == members && inventory(&to, profile, d, c)? == members,
    )?;
    for name in DIRS {
        if to.join(name).is_dir() {
            File::open(to.join(name))?.sync_all()?;
        }
    }
    target.sync_all()
}
// Both Rust and native tools accept only this same non-TLS target subset.
// Reject routing differences before opening a database or writing backup bytes.
fn selected_config(url: &str) -> io::Result<postgres::Config> {
    let config = url.parse::<postgres::Config>().map_err(|_| error())?;
    require(config.get_hosts().len() == 1 && config.get_ports().len() <= 1)?;
    require(config.get_hostaddrs().is_empty())?;
    require(config.get_target_session_attrs() == postgres::config::TargetSessionAttrs::Any)?;
    require(matches!(
        config.get_ssl_mode(),
        postgres::config::SslMode::Disable | postgres::config::SslMode::Prefer
    ))?;
    require(
        matches!(&config.get_hosts()[0], postgres::config::Host::Tcp(host) if host == "127.0.0.1" || host == "::1"),
    )?;
    require(config.get_user().is_some())?;
    let database = config.get_dbname().ok_or_else(error)?;
    require(
        !database.is_empty()
            && !database.contains('=')
            && !database.starts_with("postgres://")
            && !database.starts_with("postgresql://"),
    )?;
    Ok(config)
}
fn client(url: &str, d: Instant, c: &AtomicBool) -> io::Result<Client> {
    active(d, c)?;
    let mut config = selected_config(url)?;
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
    let selected = selected_config(pg_url)?;
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
    command.args(args);
    if input.is_some() {
        // pg_restore needs a database option to execute rather than emit SQL.
        // Only the decoded database name goes in argv, never the connection URL.
        require(
            !database.contains('=')
                && !database.starts_with("postgres://")
                && !database.starts_with("postgresql://"),
        )?;
        command.arg("--dbname").arg(database);
    }
    let spawned = command
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
fn frozen_domain(
    db: &mut Client,
    domain: &str,
    store: &SegmentStore,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Value> {
    active(d, c)?;
    let rows = db
        .query("SELECT domain FROM cmd2_domain LIMIT 2", &[])
        .map_err(|_| error())?;
    require(rows.len() == 1 && rows[0].get::<_, String>(0) == domain)?;
    let mut total_rows = 0u64;
    let mut total_bytes = 0u64;
    let mut counts = serde_json::Map::new();
    let presence = db.query_one("SELECT to_regclass('public.cmd2_audit_delta_v1') IS NOT NULL, to_regclass('public.cmd2_audit_delta_v1_domain') IS NOT NULL", &[]).map_err(|_| error())?;
    let addressed: bool = presence.get(0);
    require(addressed == presence.get::<_, bool>(1))?;
    let mut tables = vec![
        "cmd2_domain",
        "cmd2_audit_fence",
        "cmd2_job",
        "cmd2_history",
        "cmd2_log",
        "cmd2_member",
        "cmd2_current",
        "cmd2_attempt",
        "cmd2_receipt",
        "cmd2_outbox",
        "cmd2_source_index",
        "cmd2_predicate",
    ];
    if addressed {
        let marker = db.query_opt("SELECT p.profile_digest,p.baseline_generation,f.generation FROM cmd2_audit_delta_v1_domain p JOIN cmd2_audit_fence f USING(domain) WHERE p.domain=$1", &[&domain]).map_err(|_| error())?;
        if let Some(marker) = marker {
            require(
                marker.get::<_, String>(0).trim_end()
                    == crate::durable_adapter::audit_delta::audit_delta_schema_digest().to_hex()
                    && marker.get::<_, i64>(1) >= 0
                    && marker.get::<_, i64>(1) <= marker.get::<_, i64>(2),
            )?;
        } else {
            require(
                db.query_one(
                    "SELECT NOT EXISTS(SELECT 1 FROM cmd2_audit_delta_v1 WHERE domain=$1 LIMIT 1)",
                    &[&domain],
                )
                .map_err(|_| error())?
                .get::<_, bool>(0),
            )?;
        }
        tables.extend(["cmd2_audit_delta_v1_domain", "cmd2_audit_delta_v1"]);
    }
    let model_presence = db.query_one("SELECT to_regclass('public.cmd2_model_selection_v2') IS NOT NULL, to_regclass('public.cmd2_model_manifest_history_v2') IS NOT NULL", &[]).map_err(|_| error())?;
    let models: bool = model_presence.get(0);
    require(models == model_presence.get::<_, bool>(1))?;
    if models {
        let mut tx = db.transaction().map_err(|_| error())?;
        crate::source_cohort::require_managed_model_schema_v2(&mut tx, d, c)
            .map_err(|_| error())?;
        require(tx.query_one("SELECT NOT EXISTS(SELECT 1 FROM cmd2_model_selection_v2 s LEFT JOIN cmd2_model_manifest_history_v2 h USING(domain,selected_generation_digest,selected_audit_generation,model_version) WHERE s.domain=$1 AND (h.manifest_digest IS NULL OR h.manifest_digest<>s.manifest_digest) LIMIT 1)", &[&domain]).map_err(|_| error())?.get::<_, bool>(0))?;
        tx.commit().map_err(|_| error())?;
        tables.extend(["cmd2_model_selection_v2", "cmd2_model_manifest_history_v2"]);
    }
    let mut journal_roots = serde_json::Map::new();
    let mut model_roots = serde_json::Map::new();
    // Scalar lengths and optional 32-byte journal row commitments cross the
    // cursor. Original metadata payloads are never materialized here.
    for table in tables {
        active(d, c)?;
        let model = matches!(
            table,
            "cmd2_model_selection_v2" | "cmd2_model_manifest_history_v2"
        );
        let journal =
            model || matches!(table, "cmd2_audit_delta_v1_domain" | "cmd2_audit_delta_v1");
        let scan_rows = i64::try_from(
            profile
                .max_metadata_rows
                .checked_sub(total_rows)
                .and_then(|n| n.checked_add(1))
                .ok_or_else(error)?,
        )
        .map_err(|_| error())?;
        let query = if journal {
            let order = if model {
                "selected_generation_digest,selected_audit_generation,model_version"
            } else if table == "cmd2_audit_delta_v1" {
                "generation"
            } else {
                "domain"
            };
            format!(
                "SELECT octet_length(row_to_json(t)::text), sha256(convert_to(row_to_json(t)::text,'UTF8')) FROM (SELECT * FROM {table} WHERE domain=$1 ORDER BY {order} LIMIT $2) t"
            )
        } else {
            format!(
                "SELECT octet_length(row_to_json(t)::text), NULL::bytea FROM (SELECT * FROM {table} WHERE domain=$1 LIMIT $2) t"
            )
        };
        let mut journal_hash = Digest256Hasher::new();
        journal_hash.update(b"tos-backup-addressed-audit-v1");
        journal_hash.update(table.as_bytes());
        let params: [&(dyn postgres::types::ToSql + Sync); 2] = [&domain, &scan_rows];
        let mut rows = db.query_raw(&query, params).map_err(|_| error())?;
        use postgres::fallible_iterator::FallibleIterator;
        let mut count = 0u64;
        while let Some(row) = rows.next().map_err(|_| error())? {
            active(d, c)?;
            count = count.checked_add(1).ok_or_else(error)?;
            total_rows = total_rows.checked_add(1).ok_or_else(error)?;
            let bytes = u64::try_from(row.get::<_, i32>(0)).map_err(|_| error())?;
            total_bytes = total_bytes.checked_add(bytes).ok_or_else(error)?;
            let cap = match table {
                "cmd2_history" => profile.max_history_rows,
                "cmd2_log" => profile.max_log_rows,
                "cmd2_member" | "cmd2_current" => profile.max_member_rows,
                _ => profile.max_metadata_rows,
            };
            require(
                count <= cap
                    && total_rows <= profile.max_metadata_rows
                    && total_bytes <= profile.max_metadata_bytes,
            )?;
            if journal {
                let digest: Vec<u8> = row.get(1);
                require(digest.len() == 32)?;
                journal_hash.update(&digest);
            }
        }
        if model {
            model_roots.insert(table.into(), json!(journal_hash.finalize().to_hex()));
        } else if journal {
            journal_roots.insert(table.into(), json!(journal_hash.finalize().to_hex()));
        }
        counts.insert(table.into(), json!(count));
    }
    active(d, c)?;
    let mut result = json!({"rows":total_rows,"bytes":total_bytes,"counts":counts});
    if addressed {
        result["addressed_audit_v1"] = Value::Object(journal_roots);
    }
    if models {
        result["managed_model_v2"] = json!({
            "schema_sha256": tos_foundation::Digest256::of_bytes(include_bytes!("managed_model_selection_v2.sql")).to_hex(),
            "row_roots": model_roots,
            "retained_closure": managed_model_cold_closure(db, domain, store, profile, d, c)?,
        });
    }
    Ok(result)
}
/// Global cold backup work is explicit; it never runs on a warm successor.
fn managed_model_cold_closure(
    db: &mut Client,
    domain: &str,
    store: &SegmentStore,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Value> {
    let cap = i64::try_from(profile.max_metadata_rows.checked_add(1).ok_or_else(error)?)
        .map_err(|_| error())?;
    let rows = db.query("SELECT selected_generation_digest,selected_audit_generation,model_version,manifest_digest FROM cmd2_model_manifest_history_v2 WHERE domain=$1 ORDER BY selected_generation_digest,selected_audit_generation,model_version LIMIT $2", &[&domain,&cap]).map_err(|_| error())?;
    require(rows.len() as u64 <= profile.max_metadata_rows)?;
    let history = rows
        .iter()
        .map(|r| r.get::<_, String>(3).trim_end().to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    let audited = store.hold_audit_root().map_err(|_| error())?;
    let limits = tos_compiler::ManagedManifestLimitsV2 {
        max_manifest_bytes: 1_048_576,
        max_retained_generations: profile.max_metadata_rows.min(256),
        max_retained_logical_bound_bytes: profile.max_store_bytes,
    };
    let tree_limits = tos_segment_store::AuthenticatedTreeLimitsV1 {
        max_key_bytes: 4096,
        max_value_bytes: 1_048_576,
        max_kind_bytes: 4096,
        max_node_bytes: 1_048_576 + 65_536,
        max_children: 16,
        max_nodes: profile.max_entries as u64,
        max_total_bytes: profile.max_store_bytes,
        max_rows: profile.max_metadata_rows,
    };
    let digests = rows
        .iter()
        .map(|row| {
            tos_foundation::Digest256::from_hex(row.get::<_, String>(3).trim_end())
                .map_err(|_| error())
        })
        .collect::<io::Result<Vec<_>>>()?;
    let (admitted, work) = tos_compiler::ManagedManifestV2::read_retained_cold(
        store,
        &audited,
        &digests,
        limits,
        tree_limits,
        d,
        c,
    )
    .map_err(|_| error())?;
    require(admitted.len() == rows.len())?;
    let mut refs = Vec::new();
    for (row, (manifest, chain)) in rows.into_iter().zip(admitted) {
        active(d, c)?;
        let generation: String = row.get(0);
        let audit = u64::try_from(row.get::<_, i64>(1)).map_err(|_| error())?;
        let version = u64::try_from(row.get::<_, i64>(2)).map_err(|_| error())?;
        let digest: String = row.get(3);
        let binding = manifest.source_binding();
        require(
            binding.domain == domain
                && binding.selected_generation_digest == generation.trim_end()
                && binding.selected_audit_generation == audit
                && version > 0
                && chain
                    .iter()
                    .all(|digest| history.contains(&digest.to_hex())),
        )?;
        refs.push(json!({"manifest_digest":digest.trim_end(),"model_version":version,"generation":generation.trim_end(),"audit":audit}));
    }
    let read_nodes = work.read_nodes;
    let read_bytes = work.read_bytes;
    Ok(json!({"manifests":refs,"read_nodes":read_nodes,"read_bytes":read_bytes}))
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
    let profile = BackupProfile::default().validate()?;
    active(d, c)?;
    if !s.quiescent_owner_confirmed {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "quiescent owner confirmation required",
        ));
    }
    let cap = rustix::process::getrlimit(rustix::process::Resource::Fsize);
    require(
        cap.current.is_some_and(|n| n <= profile.max_dump_bytes)
            && cap.maximum.is_some_and(|n| n <= profile.max_dump_bytes),
    )?;
    let source = root(s.store_root)?;
    let backup = root(s.backup_root)?;
    distinct(&source, &backup)?;
    let target = anchored(&backup);
    require(fs::read_dir(&target)?.next().is_none())?;
    let mut db = client(s.pg_url, d, c)?;
    let store = SegmentStore::open_existing(s.store_root, s.store_limits).map_err(|_| error())?;
    let metadata = frozen_domain(&mut db, s.domain, &store, profile, d, c)?;
    let database: String = db
        .query_one("SELECT current_database()", &[])
        .map_err(|_| error())?
        .get(0);
    let original_cut = cut(s.pg_url, s.domain, &store, d, c)?;
    let members = inventory(&anchored(&source), profile, d, c)?;
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
    copy_store(&source, &copied, &members, profile, d, c)?;
    require(frozen_domain(&mut db, s.domain, &store, profile, d, c)? == metadata)?;
    require(cut(s.pg_url, s.domain, &store, d, c)? == original_cut)?;
    let mut receipt = json!({"schema":"tos_cmd2_quiescent_backup_v1","domain":s.domain,"database":database,"cut":original_cut,"profile":profile,"metadata":metadata,"store_members":members,"dump":{"bytes":dump_identity.0,"sha256":dump_identity.1},"tool":tool_receipt,"source_admission":false,"production_quiescence_verified":false});
    if metadata.get("managed_model_v2").is_some() {
        let oid = db
            .query_one(
                "SELECT oid::bigint FROM pg_database WHERE datname=current_database()",
                &[],
            )
            .map_err(|_| error())?
            .get::<_, i64>(0);
        require(oid > 0)?;
        receipt["managed_model_restore_binding_v2"] = json!({
            "schema":"tos_managed_model_backup_binding_v2",
            "database_oid":oid,
            "metadata_sha256":backup_binding_digest(&metadata,profile.max_receipt_bytes)?,
            "store_inventory_sha256":backup_binding_digest(&receipt["store_members"],profile.max_receipt_bytes)?,
        });
    }
    struct ReceiptWriter {
        raw: Vec<u8>,
        cap: u64,
    }
    impl Write for ReceiptWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() as u64 > self.cap - self.raw.len() as u64 {
                return Err(error());
            }
            self.raw.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut writer = ReceiptWriter {
        raw: Vec::new(),
        cap: profile.max_receipt_bytes,
    };
    serde_json::to_writer(&mut writer, &receipt).map_err(|_| error())?;
    let raw = writer.raw;
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
/// An in-process witness issued only after the real dump restore, exact
/// metadata/cut/store checks and protected backup receipt checks succeed.
/// No serde constructor: a descriptive JSON status cannot manufacture it.
pub struct VerifiedManagedModelRestoreV2 {
    pub(crate) domain: String,
    pub(crate) backup_receipt_sha256: String,
    pub(crate) old_database_oid: u64,
    pub(crate) new_database_oid: u64,
    pub(crate) metadata_sha256: String,
    pub(crate) store_inventory_sha256: String,
    pub(crate) cut_digest: String,
    pub(crate) manifests: std::collections::BTreeSet<String>,
}

pub fn restore_into_fresh(
    s: &RestoreSelection<'_>,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Value> {
    restore_into_fresh_with_managed_binding(s, d, c).map(|(result, _)| result)
}
/// Additive route for the existing whole recovery operation. The witness
/// cannot cross a process boundary as JSON or stand for a current source cap.
pub fn restore_into_fresh_with_managed_binding(
    s: &RestoreSelection<'_>,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<(Value, Option<VerifiedManagedModelRestoreV2>)> {
    let profile = BackupProfile::default().validate()?;
    active(d, c)?;
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
    let (length, sha) = hash_bounded(&receipt_path, profile.max_receipt_bytes, d, c)?;
    require(length <= profile.max_receipt_bytes && sha == s.receipt_sha256)?;
    let receipt: Value = serde_json::from_slice(&{
        let mut raw = Vec::new();
        OpenOptions::new()
            .read(true)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
            .open(&receipt_path)?
            .take(profile.max_receipt_bytes + 1)
            .read_to_end(&mut raw)?;
        require(raw.len() as u64 <= profile.max_receipt_bytes)?;
        raw
    })
    .map_err(|_| error())?;
    require(receipt["schema"] == "tos_cmd2_quiescent_backup_v1" && receipt["domain"] == s.domain)?;
    require(receipt.get("profile").is_some() == receipt.get("metadata").is_some())?;
    let stored_profile: BackupProfile = receipt
        .get("profile")
        .map(|v| serde_json::from_value(v.clone()))
        .transpose()
        .map_err(|_| error())?
        .unwrap_or_default();
    require(stored_profile.validate()? == profile)?;
    require(
        receipt["cut"]["historical_members"]
            .as_u64()
            .is_some_and(|n| n <= profile.max_history_rows)
            && receipt["cut"]["through_commit_seq"]
                .as_u64()
                .is_some_and(|n| n <= profile.max_log_rows),
    )?;
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
    require(members.len() <= profile.max_files)?;
    let source =
        tos_fd_open::open_directory_at(&backup, Path::new("store")).map_err(|_| error())?;
    let dump = base.join("metadata.dump");
    let expected = (
        receipt["dump"]["bytes"].as_u64().ok_or_else(error)?,
        receipt["dump"]["sha256"]
            .as_str()
            .ok_or_else(error)?
            .to_owned(),
    );
    require(hash_bounded(&dump, profile.max_dump_bytes, d, c)? == expected)?;
    if let Some(metadata) = receipt.get("metadata") {
        require(
            metadata["rows"]
                .as_u64()
                .is_some_and(|n| n <= profile.max_metadata_rows)
                && metadata["bytes"]
                    .as_u64()
                    .is_some_and(|n| n <= profile.max_metadata_bytes),
        )?;
        for (table, cap) in [
            ("cmd2_history", profile.max_history_rows),
            ("cmd2_log", profile.max_log_rows),
            ("cmd2_member", profile.max_member_rows),
            ("cmd2_current", profile.max_member_rows),
        ] {
            require(metadata["counts"][table].as_u64().is_some_and(|n| n <= cap))?;
        }
        // The authenticated cold cut binds linked history and a contiguous
        // log from 1 through commit_seq; independent ceilings do not bind
        // those facts to this receipt's metadata census.
        require(
            metadata["counts"]["cmd2_history"].as_u64()
                == receipt["cut"]["historical_members"].as_u64()
                && metadata["counts"]["cmd2_log"].as_u64()
                    == receipt["cut"]["through_commit_seq"].as_u64(),
        )?;
    }
    copy_store(&source, &restored, members, profile, d, c)?;
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
    require(
        hash_bounded(&dump, profile.max_dump_bytes, d, c)? == expected
            && hash_bounded(&receipt_path, profile.max_receipt_bytes, d, c)?.1 == s.receipt_sha256,
    )?;
    let store = SegmentStore::open_existing(s.store_root, s.store_limits).map_err(|_| error())?;
    let recovered_metadata = frozen_domain(&mut db, s.domain, &store, profile, d, c)?;
    if let Some(expected) = receipt.get("metadata") {
        require(&recovered_metadata == expected)?;
    }
    let recovered = cut(s.pg_url, s.domain, &store, d, c)?;
    require(recovered == receipt["cut"])?;
    active(d, c)?;
    let mut result = json!({"status":"restore_verified","domain":s.domain,"cut":recovered,"tool":tool_receipt,"source_admission":false,"source_currentness_granted":false});
    let mut verified_model_restore = None;
    if recovered_metadata.get("managed_model_v2").is_some() {
        let binding = receipt
            .get("managed_model_restore_binding_v2")
            .ok_or_else(error)?;
        let old_oid = binding["database_oid"].as_u64().ok_or_else(error)?;
        let new_oid = u64::try_from(
            db.query_one(
                "SELECT oid::bigint FROM pg_database WHERE datname=current_database()",
                &[],
            )
            .map_err(|_| error())?
            .get::<_, i64>(0),
        )
        .map_err(|_| error())?;
        let metadata_sha256 =
            backup_binding_digest(&recovered_metadata, profile.max_receipt_bytes)?;
        let store_inventory_sha256 =
            backup_binding_digest(&receipt["store_members"], profile.max_receipt_bytes)?;
        require(
            binding["schema"] == "tos_managed_model_backup_binding_v2"
                && old_oid > 0
                && new_oid > 0
                && old_oid != new_oid
                && binding["metadata_sha256"] == metadata_sha256
                && binding["store_inventory_sha256"] == store_inventory_sha256,
        )?;
        // This descriptive receipt names the successful existing restore.
        // Current model rebind must independently check it at the actual
        // source-fenced publication; JSON alone issues no authority.
        result["managed_model_restore_binding_v2"] = json!({
            "schema":"tos_managed_model_restore_binding_v2",
            "backup_receipt_sha256":s.receipt_sha256,
            "old_database_oid":old_oid,"new_database_oid":new_oid,
            "metadata_sha256":metadata_sha256,"store_inventory_sha256":store_inventory_sha256,
            "cut_digest":recovered["digest"],
        });
        let manifests = recovered_metadata["managed_model_v2"]["retained_closure"]["manifests"]
            .as_array()
            .ok_or_else(error)?
            .iter()
            .map(|entry| {
                let digest = entry["manifest_digest"].as_str().ok_or_else(error)?;
                tos_foundation::Digest256::from_hex(digest).map_err(|_| error())?;
                Ok(digest.to_owned())
            })
            .collect::<io::Result<std::collections::BTreeSet<_>>>()?;
        verified_model_restore = Some(VerifiedManagedModelRestoreV2 {
            domain: s.domain.to_owned(),
            backup_receipt_sha256: s.receipt_sha256.to_owned(),
            old_database_oid: old_oid,
            new_database_oid: new_oid,
            metadata_sha256,
            store_inventory_sha256,
            cut_digest: recovered["digest"].as_str().ok_or_else(error)?.to_owned(),
            manifests,
        });
    }
    Ok((result, verified_model_restore))
}

fn backup_binding_digest(value: &Value, max_bytes: u64) -> io::Result<String> {
    use tos_foundation::{CanonicalProfile, JsonLimits, JsonMode, canonical_bytes_v1, parse_json};
    let raw = serde_json::to_vec(value).map_err(|_| error())?;
    require(raw.len() as u64 <= max_bytes)?;
    let mut limits = JsonLimits::default();
    limits.max_bytes = usize::try_from(max_bytes).map_err(|_| error())?;
    let parsed = parse_json(&raw, JsonMode::PublishedStrict, limits).map_err(|_| error())?;
    let canonical = canonical_bytes_v1(
        parsed.root(),
        CanonicalProfile::SourceRecordDigestV1,
        limits,
    )
    .map_err(|_| error())?;
    Ok(tos_foundation::Digest256::of_bytes(&canonical).to_hex())
}
