//! Bounded owner-selected PostgreSQL/STO backup transport.
//! The owner must quiesce its isolated database and byte store. These functions
//! preserve committed custody; they grant no source, rights or publication authority.
use crate::{
    ColdWorkspaceLimits, DurableError, DurablePgCoordinator, PrivateGenerationWorkspace,
    StreamedGenerationProfile,
};
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
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};
use tos_foundation::Digest256Hasher;
use tos_segment_store::{GenerationReadLimits, GenerationShapeLimits, SegmentLimits, SegmentStore};
const BYTES: u64 = 64 * 1024 * 1024;
const BACKUP_SCHEMA_V1: &str = "tos_cmd2_quiescent_backup_v1";
const BACKUP_SCHEMA_V2: &str = "tos_cmd2_quiescent_backup_v2";
const BACKUP_SCHEMA_V3: &str = "tos_cmd2_quiescent_backup_v3";
const BACKUP_SCHEMA_V4: &str = "tos_cmd2_quiescent_backup_v4";
const BACKUP_SCHEMA_V5: &str = "tos_cmd2_quiescent_backup_v5";
const BACKUP_SCHEMA_V6: &str = "tos_cmd2_quiescent_backup_v6";
const BACKUP_SCHEMA_V7: &str = "tos_cmd2_quiescent_backup_v7";
const BACKUP_SCHEMA_V8: &str = "tos_cmd2_quiescent_backup_v8";
const BACKUP_SCHEMA_V9: &str = "tos_cmd2_quiescent_backup_v9";
const BACKUP_SCHEMA_V10: &str = "tos_cmd2_quiescent_backup_v10";
const BACKUP_SCHEMA_V11: &str = "tos_cmd2_quiescent_backup_v11";
const SOURCE_NAVIGATION_INITIAL_CENSUS_V1: &str = "tos_backup_source_navigation_initial_v1";
const SOURCE_NAVIGATION_INITIAL_TABLES: [(&str, &str); 3] = [
    (
        "cmd2_projection_navigation_initial",
        "consumer_id COLLATE \"C\"",
    ),
    (
        "cmd2_projection_navigation_initial_row",
        "consumer_id COLLATE \"C\",collection COLLATE \"C\",row_key COLLATE \"C\"",
    ),
    (
        "cmd2_projection_navigation_initial_contributor",
        "consumer_id COLLATE \"C\",node_id COLLATE \"C\",owner_source_path COLLATE \"C\",owner_record_id COLLATE \"C\"",
    ),
];
const SOURCE_NAVIGATION_COHORT_TREE_CENSUS_V1: &str = "tos_backup_source_navigation_cohort_tree_v1";
const SOURCE_NAVIGATION_COHORT_TREE_TABLES: [(&str, &str); 2] = [
    (
        "cmd2_projection_navigation_cohort_tree",
        "consumer_id COLLATE \"C\"",
    ),
    (
        "cmd2_projection_navigation_cohort_tree_node",
        "consumer_id COLLATE \"C\",node_sha256 COLLATE \"C\"",
    ),
];
const SOURCE_INVENTORY_CENSUS_V1: &str = "tos_backup_source_inventory_projection_v1";
const SOURCE_NAVIGATION_CENSUS_V1: &str = "tos_backup_source_navigation_projection_v1";
const SOURCE_NAVIGATION_CENSUS_V2: &str = "tos_backup_source_navigation_projection_v2";
const SOURCE_NAVIGATION_CENSUS_V3: &str = "tos_backup_source_navigation_projection_v3";
const SOURCE_NAVIGATION_POLICY_CENSUS_V1: &str = "tos_backup_source_navigation_policy_v1";
const PRIVATE_PACKAGE_CENSUS_V2: &str = "tos_backup_private_owner_packages_v2";
const SOURCE_PUBLICATION_CENSUS_V1: &str = "tos_backup_source_publication_v1";
const SOURCE_ITEM_ASSIGNMENT_CENSUS_V1: &str = "tos_backup_source_item_file_assignment_v1";
const SOURCE_ANNOTATION_IDENTITY_CENSUS_V1: &str = "tos_backup_source_annotation_identity_v1";
const SOURCE_CLAIM_NAVIGATION_REFERENCE_CENSUS_V1: &str =
    "tos_backup_source_claim_navigation_reference_v1";
const SOURCE_CLAIM_NAVIGATION_REFERENCE_CENSUS_V2: &str =
    "tos_backup_source_claim_navigation_reference_v2";
const DURABLE_SCHEMA_PROFILE_CENSUS_V1: &str = "tos_backup_durable_schema_profile_v1";

const SOURCE_NAVIGATION_POLICY_TABLES: [(&str, &str); 2] = [
    (
        "cmd2_source_navigation_policy_current",
        "consumer_id COLLATE \"C\"",
    ),
    (
        "cmd2_source_navigation_policy_history",
        "consumer_id COLLATE \"C\",policy_version",
    ),
];

const SOURCE_PUBLICATION_TABLES: [(&str, &str); 4] = [
    ("cmd2_attempt", "prepare_id"),
    ("cmd2_source_publication_current", "domain COLLATE \"C\""),
    (
        "cmd2_source_publication_history",
        "domain COLLATE \"C\",publication_generation",
    ),
    ("cmd2_source_publication_import", "domain COLLATE \"C\""),
];

const SOURCE_ITEM_ASSIGNMENT_TABLES: [(&str, &str); 1] = [(
    "cmd2_source_item_file_assignment",
    "edition_id COLLATE \"C\",manifest_path COLLATE \"C\",payload_ordinal",
)];

const SOURCE_ANNOTATION_IDENTITY_TABLES: [(&str, &str); 1] = [(
    "cmd2_source_annotation_identity",
    "token COLLATE \"C\",path COLLATE \"C\"",
)];

const SOURCE_CLAIM_NAVIGATION_REFERENCE_TABLES: [(&str, &str); 1] = [(
    "cmd2_source_claim_navigation_reference",
    "path COLLATE \"C\",claim_id COLLATE \"C\",endpoint_kind COLLATE \"C\",endpoint COLLATE \"C\",basis COLLATE \"C\"",
)];

const SOURCE_CLAIM_NAVIGATION_REFERENCE_TABLES_V2: [(&str, &str); 1] = [(
    "cmd2_source_claim_navigation_reference",
    "path COLLATE \"C\",claim_id COLLATE \"C\",endpoint_kind COLLATE \"C\",endpoint COLLATE \"C\",basis COLLATE \"C\",valid_from_seq NULLS LAST",
)];

const PRIVATE_PACKAGE_TABLES: [(&str, &str); 11] = [
    (
        "cmd2_private_owner_scope",
        "domain COLLATE \"C\",owner_scope COLLATE \"C\"",
    ),
    (
        "cmd2_private_owner_audit_fence",
        "domain COLLATE \"C\",owner_scope COLLATE \"C\"",
    ),
    (
        "cmd2_private_package_current",
        "domain COLLATE \"C\",owner_scope COLLATE \"C\",target_ref COLLATE \"C\"",
    ),
    (
        "cmd2_private_package_history",
        "domain COLLATE \"C\",owner_scope COLLATE \"C\",target_ref COLLATE \"C\",revision",
    ),
    (
        "cmd2_private_package_member",
        "domain COLLATE \"C\",owner_scope COLLATE \"C\",target_ref COLLATE \"C\",revision,member_name COLLATE \"C\"",
    ),
    (
        "cmd2_private_package_archive",
        "domain COLLATE \"C\",owner_scope COLLATE \"C\",target_ref COLLATE \"C\",archive_ref COLLATE \"C\"",
    ),
    (
        "cmd2_private_package_archive_member",
        "domain COLLATE \"C\",owner_scope COLLATE \"C\",target_ref COLLATE \"C\",archive_ref COLLATE \"C\",member_name COLLATE \"C\"",
    ),
    (
        "cmd2_private_command_receipt",
        "domain COLLATE \"C\",owner_scope COLLATE \"C\",command_id COLLATE \"C\"",
    ),
    (
        "cmd2_private_log",
        "domain COLLATE \"C\",owner_scope COLLATE \"C\",commit_seq",
    ),
    (
        "cmd2_private_outbox",
        "domain COLLATE \"C\",owner_scope COLLATE \"C\",commit_seq",
    ),
    (
        "cmd2_private_text_identity",
        "domain COLLATE \"C\",owner_scope COLLATE \"C\",identity_token COLLATE \"C\",target_ref COLLATE \"C\",revision,member_name COLLATE \"C\"",
    ),
];

const NAVIGATION_TABLES: [(&str, &str); 12] = [
    (
        "cmd2_projection_navigation_consumer",
        "consumer_id COLLATE \"C\"",
    ),
    (
        "cmd2_projection_navigation_bootstrap",
        "consumer_id COLLATE \"C\"",
    ),
    (
        "cmd2_projection_navigation_bootstrap_owner",
        "consumer_id COLLATE \"C\",owner_source_path COLLATE \"C\"",
    ),
    (
        "cmd2_projection_navigation_bootstrap_row",
        "consumer_id COLLATE \"C\",row_kind COLLATE \"C\",row_id COLLATE \"C\",owner_source_path COLLATE \"C\",owner_record_id COLLATE \"C\"",
    ),
    (
        "cmd2_projection_navigation_bootstrap_dependency",
        "consumer_id COLLATE \"C\",owner_source_path COLLATE \"C\",owner_record_id COLLATE \"C\",dependency_path COLLATE \"C\"",
    ),
    (
        "cmd2_projection_navigation_adjacency_certificate",
        "consumer_id COLLATE \"C\",node_id COLLATE \"C\"",
    ),
    (
        "cmd2_projection_navigation_coverage_event",
        "consumer_id COLLATE \"C\",event_sequence",
    ),
    (
        "cmd2_projection_navigation_cohort",
        "consumer_id COLLATE \"C\",source_path COLLATE \"C\",record_id COLLATE \"C\"",
    ),
    (
        "cmd2_projection_navigation_dependency",
        "consumer_id COLLATE \"C\",source_path COLLATE \"C\",record_id COLLATE \"C\",dependency_path COLLATE \"C\"",
    ),
    (
        "cmd2_projection_navigation_node",
        "consumer_id COLLATE \"C\",source_path COLLATE \"C\",record_id COLLATE \"C\",node_id COLLATE \"C\"",
    ),
    (
        "cmd2_projection_navigation_edge",
        "consumer_id COLLATE \"C\",source_path COLLATE \"C\",record_id COLLATE \"C\",edge_id COLLATE \"C\"",
    ),
    (
        "cmd2_projection_navigation_diagnostic",
        "consumer_id COLLATE \"C\",source_path COLLATE \"C\",record_id COLLATE \"C\",diagnostic_ordinal",
    ),
];

const NAVIGATION_TABLES_V2: [(&str, &str); 13] = [
    NAVIGATION_TABLES[0],
    NAVIGATION_TABLES[1],
    NAVIGATION_TABLES[2],
    NAVIGATION_TABLES[3],
    NAVIGATION_TABLES[4],
    NAVIGATION_TABLES[5],
    NAVIGATION_TABLES[6],
    (
        "cmd2_projection_navigation_coverage_range",
        "consumer_id COLLATE \"C\",through_seq",
    ),
    NAVIGATION_TABLES[7],
    NAVIGATION_TABLES[8],
    NAVIGATION_TABLES[9],
    NAVIGATION_TABLES[10],
    NAVIGATION_TABLES[11],
];

fn census_v2_for_schema(schema: &str) -> io::Result<bool> {
    if schema == BACKUP_SCHEMA_V1 {
        Ok(false)
    } else if schema == BACKUP_SCHEMA_V2
        || schema == BACKUP_SCHEMA_V3
        || schema == BACKUP_SCHEMA_V4
        || schema == BACKUP_SCHEMA_V5
        || schema == BACKUP_SCHEMA_V6
        || schema == BACKUP_SCHEMA_V7
        || schema == BACKUP_SCHEMA_V8
        || schema == BACKUP_SCHEMA_V9
        || schema == BACKUP_SCHEMA_V10
        || schema == BACKUP_SCHEMA_V11
    {
        Ok(true)
    } else {
        Err(error())
    }
}

fn census_v3_for_schema(schema: &str) -> io::Result<bool> {
    if schema == BACKUP_SCHEMA_V1 || schema == BACKUP_SCHEMA_V2 {
        Ok(false)
    } else if schema == BACKUP_SCHEMA_V3
        || schema == BACKUP_SCHEMA_V4
        || schema == BACKUP_SCHEMA_V5
        || schema == BACKUP_SCHEMA_V6
        || schema == BACKUP_SCHEMA_V7
        || schema == BACKUP_SCHEMA_V8
        || schema == BACKUP_SCHEMA_V9
        || schema == BACKUP_SCHEMA_V10
        || schema == BACKUP_SCHEMA_V11
    {
        Ok(true)
    } else {
        Err(error())
    }
}

fn census_v4_for_schema(schema: &str) -> io::Result<bool> {
    if schema == BACKUP_SCHEMA_V1 || schema == BACKUP_SCHEMA_V2 || schema == BACKUP_SCHEMA_V3 {
        Ok(false)
    } else if schema == BACKUP_SCHEMA_V4
        || schema == BACKUP_SCHEMA_V5
        || schema == BACKUP_SCHEMA_V6
        || schema == BACKUP_SCHEMA_V7
        || schema == BACKUP_SCHEMA_V8
        || schema == BACKUP_SCHEMA_V9
        || schema == BACKUP_SCHEMA_V10
        || schema == BACKUP_SCHEMA_V11
    {
        Ok(true)
    } else {
        Err(error())
    }
}

fn census_v5_for_schema(schema: &str) -> io::Result<bool> {
    if schema == BACKUP_SCHEMA_V1
        || schema == BACKUP_SCHEMA_V2
        || schema == BACKUP_SCHEMA_V3
        || schema == BACKUP_SCHEMA_V4
    {
        Ok(false)
    } else if schema == BACKUP_SCHEMA_V5
        || schema == BACKUP_SCHEMA_V6
        || schema == BACKUP_SCHEMA_V7
        || schema == BACKUP_SCHEMA_V8
        || schema == BACKUP_SCHEMA_V9
        || schema == BACKUP_SCHEMA_V10
        || schema == BACKUP_SCHEMA_V11
    {
        Ok(true)
    } else {
        Err(error())
    }
}

fn census_v6_for_schema(schema: &str) -> io::Result<bool> {
    if schema == BACKUP_SCHEMA_V1
        || schema == BACKUP_SCHEMA_V2
        || schema == BACKUP_SCHEMA_V3
        || schema == BACKUP_SCHEMA_V4
        || schema == BACKUP_SCHEMA_V5
    {
        Ok(false)
    } else if schema == BACKUP_SCHEMA_V6
        || schema == BACKUP_SCHEMA_V7
        || schema == BACKUP_SCHEMA_V8
        || schema == BACKUP_SCHEMA_V9
        || schema == BACKUP_SCHEMA_V10
        || schema == BACKUP_SCHEMA_V11
    {
        Ok(true)
    } else {
        Err(error())
    }
}

fn census_v7_for_schema(schema: &str) -> io::Result<bool> {
    if schema == BACKUP_SCHEMA_V1
        || schema == BACKUP_SCHEMA_V2
        || schema == BACKUP_SCHEMA_V3
        || schema == BACKUP_SCHEMA_V4
        || schema == BACKUP_SCHEMA_V5
        || schema == BACKUP_SCHEMA_V6
    {
        Ok(false)
    } else if schema == BACKUP_SCHEMA_V7
        || schema == BACKUP_SCHEMA_V8
        || schema == BACKUP_SCHEMA_V9
        || schema == BACKUP_SCHEMA_V10
        || schema == BACKUP_SCHEMA_V11
    {
        Ok(true)
    } else {
        Err(error())
    }
}

fn census_v9_for_schema(schema: &str) -> io::Result<bool> {
    match schema {
        BACKUP_SCHEMA_V9 | BACKUP_SCHEMA_V10 | BACKUP_SCHEMA_V11 => Ok(true),
        BACKUP_SCHEMA_V1 | BACKUP_SCHEMA_V2 | BACKUP_SCHEMA_V3 | BACKUP_SCHEMA_V4
        | BACKUP_SCHEMA_V5 | BACKUP_SCHEMA_V6 | BACKUP_SCHEMA_V7 | BACKUP_SCHEMA_V8 => Ok(false),
        _ => Err(error()),
    }
}

fn census_v8_for_schema(schema: &str) -> io::Result<bool> {
    if schema == BACKUP_SCHEMA_V1
        || schema == BACKUP_SCHEMA_V2
        || schema == BACKUP_SCHEMA_V3
        || schema == BACKUP_SCHEMA_V4
        || schema == BACKUP_SCHEMA_V5
        || schema == BACKUP_SCHEMA_V6
        || schema == BACKUP_SCHEMA_V7
        || schema == BACKUP_SCHEMA_V9
        || schema == BACKUP_SCHEMA_V10
        || schema == BACKUP_SCHEMA_V11
    {
        Ok(false)
    } else if schema == BACKUP_SCHEMA_V8 {
        Ok(true)
    } else {
        Err(error())
    }
}
fn census_v10_for_schema(schema: &str) -> io::Result<bool> {
    if schema == BACKUP_SCHEMA_V10 || schema == BACKUP_SCHEMA_V11 {
        Ok(true)
    } else {
        census_v9_for_schema(schema).map(|_| false)
    }
}
fn census_v11_for_schema(schema: &str) -> io::Result<bool> {
    if schema == BACKUP_SCHEMA_V11 {
        Ok(true)
    } else {
        census_v10_for_schema(schema).map(|_| false)
    }
}
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
#[track_caller]
fn error() -> io::Error {
    let predicate = std::panic::Location::caller().line();
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "owner-selected backup/restore integrity check failed at predicate line {predicate}"
        ),
    )
}
#[track_caller]
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
fn selected_local_host(config: &postgres::Config) -> io::Result<String> {
    match config.get_hosts().first().ok_or_else(error)? {
        postgres::config::Host::Tcp(host) if host == "127.0.0.1" || host == "::1" => {
            Ok(host.clone())
        }
        #[cfg(unix)]
        postgres::config::Host::Unix(path) => {
            require(path.is_absolute())?;
            let mut normalized = PathBuf::new();
            for component in path.components() {
                match component {
                    Component::RootDir if normalized.as_os_str().is_empty() => {
                        normalized.push("/");
                    }
                    Component::Normal(name) => normalized.push(name),
                    _ => return Err(error()),
                }
            }
            // `components` ignores repeated separators and `.`; preserve only
            // paths whose spelling is already the canonical absolute form.
            require(normalized.as_os_str() == path.as_os_str())?;
            let directory = tos_fd_open::open_absolute_directory(path).map_err(|_| error())?;
            require(directory.metadata()?.is_dir())?;
            path.to_str().map(str::to_owned).ok_or_else(error)
        }
        _ => Err(error()),
    }
}

pub(crate) fn selected_config(url: &str) -> io::Result<postgres::Config> {
    let config = url.parse::<postgres::Config>().map_err(|_| error())?;
    require(config.get_hosts().len() == 1 && config.get_ports().len() <= 1)?;
    require(config.get_hostaddrs().is_empty())?;
    require(config.get_target_session_attrs() == postgres::config::TargetSessionAttrs::Any)?;
    require(matches!(
        config.get_ssl_mode(),
        postgres::config::SslMode::Disable | postgres::config::SslMode::Prefer
    ))?;
    let _ = selected_local_host(&config)?;
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

fn require_current_cmd2_schema(url: &str, d: Instant, c: &AtomicBool) -> io::Result<()> {
    let bounded = client(url, d, c)?;
    let mut coordinator = DurablePgCoordinator::from_configured_client(bounded);
    coordinator
        .require_compatible_schema()
        .map_err(|_| error())?;
    active(d, c)
}

fn tool(
    tool: &PgTool<'_>,
    args: &[&str],
    input: Option<File>,
    output: Option<File>,
    pg_url: &str,
    mode: PgToolMode,
    max_dump_bytes: u64,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Value> {
    active(d, c)?;
    let cap = rustix::process::getrlimit(rustix::process::Resource::Fsize);
    require(
        cap.current.is_some_and(|n| n <= max_dump_bytes)
            && cap.maximum.is_some_and(|n| n <= max_dump_bytes),
    )?;
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
    // Offline archive inspection must have neither an argv target nor an
    // inherited libpq target. Database-backed dump/restore keeps the existing
    // selected connection behavior.
    let database = if mode == PgToolMode::Database {
        let selected = selected_config(pg_url)?;
        let host = selected_local_host(&selected)?;
        let user = selected.get_user().ok_or_else(error)?;
        let database = selected.get_dbname().ok_or_else(error)?.to_owned();
        command
            .env("PGHOST", &host)
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
            .env("PGDATABASE", &database)
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
        Some(database)
    } else {
        require(input.is_some() && output.is_some())?;
        None
    };
    command.args(args);
    if input.is_some() && mode == PgToolMode::Database {
        // pg_restore needs a database option to execute rather than emit SQL.
        // Only the decoded database name goes in argv, never the connection URL.
        let database = database.ok_or_else(error)?;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PgToolMode {
    Database,
    OfflineArchiveEmit,
}

fn snapshot_authenticated_dump(
    path: &Path,
    expected: (u64, String),
    cap: u64,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<File> {
    require(expected.0 <= cap)?;
    let mut source = OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(path)?;
    let before = source.metadata()?;
    require(before.is_file() && before.nlink() == 1 && before.len() == expected.0)?;
    let mut snapshot = tempfile::tempfile()?;
    let mut hasher = Digest256Hasher::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        active(d, c)?;
        let count = source.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total = total.checked_add(count as u64).ok_or_else(error)?;
        require(total <= cap && total <= expected.0)?;
        hasher.update(&buffer[..count]);
        snapshot.write_all(&buffer[..count])?;
    }
    let after = source.metadata()?;
    let named = fs::symlink_metadata(path)?;
    require(
        total == expected.0
            && hasher.finalize().to_hex() == expected.1
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
    snapshot.sync_all()?;
    snapshot.seek(SeekFrom::Start(0))?;
    Ok(snapshot)
}

fn emit_archive_sql(
    tool_spec: &PgTool<'_>,
    dump: &mut File,
    args: &[&str],
    pg_url: &str,
    cap: u64,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Vec<u8>> {
    let mut output = tempfile::tempfile()?;
    dump.seek(SeekFrom::Start(0))?;
    // PostgreSQL 16 requires pg_restore to select a database or script output.
    // Keep this inspection database-free; `--file=-` writes SQL to captured stdout.
    let mut output_args = args.to_vec();
    output_args.push("--file=-");
    tool(
        tool_spec,
        &output_args,
        Some(dump.try_clone()?),
        Some(output.try_clone()?),
        pg_url,
        PgToolMode::OfflineArchiveEmit,
        cap,
        d,
        c,
    )?;
    let size = output.metadata()?.len();
    require(size <= cap)?;
    output.seek(SeekFrom::Start(0))?;
    let mut raw = Vec::with_capacity(usize::try_from(size).map_err(|_| error())?);
    output
        .take(cap.checked_add(1).ok_or_else(error)?)
        .read_to_end(&mut raw)?;
    require(raw.len() as u64 == size)?;
    Ok(raw)
}

fn verify_current_archive_abi_v1(
    tool_spec: &PgTool<'_>,
    dump: &mut File,
    domain: &str,
    pg_url: &str,
    cap: u64,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<()> {
    // Receipt schema versions describe the backup envelope, not its SQL ABI.
    // Require the current physical family markers and the domain's exact
    // compiled ABI digest before either caller-owned target is written.
    let schema = emit_archive_sql(
        tool_spec,
        dump,
        &["--schema-only", "--no-owner", "--no-privileges"],
        pg_url,
        cap,
        d,
        c,
    )?;
    let schema = std::str::from_utf8(&schema).map_err(|_| error())?;
    for marker in [
        "CREATE TABLE public.cmd2_source_inventory_projection (",
        "source_configuration_raw_digest",
        "source_configuration_canonical_digest",
        "CREATE TABLE public.cmd2_source_annotation_identity (",
        "CREATE TABLE public.cmd2_source_claim_navigation_reference (",
        "source_claim_navigation_reference_baseline_seq",
        "valid_from_seq",
        "valid_to_seq",
        "CREATE TABLE public.cmd2_source_publication_import (",
        "CREATE TABLE public.cmd2_projection_navigation_consumer (",
    ] {
        require(schema.contains(marker))?;
    }
    let private_schema_markers = PRIVATE_PACKAGE_TABLES
        .iter()
        .map(|(table, _)| format!("CREATE TABLE public.{table} ("))
        .chain(std::iter::once(
            "CREATE TABLE public.cmd2_private_schema_profile (".to_owned(),
        ));
    let mut private_schema_table_count = 0usize;
    for marker in private_schema_markers {
        if schema.contains(&marker) {
            private_schema_table_count += 1;
        }
    }
    require(
        private_schema_table_count == 0
            || private_schema_table_count == PRIVATE_PACKAGE_TABLES.len() + 1,
    )?;
    if private_schema_table_count > 0 {
        let private_profile_data = emit_archive_sql(
            tool_spec,
            dump,
            &[
                "--data-only",
                "--table=cmd2_private_schema_profile",
                "--no-owner",
                "--no-privileges",
            ],
            pg_url,
            cap,
            d,
            c,
        )?;
        let observed_private_profile = private_schema_profile_from_copy(&private_profile_data)?;
        require(
            observed_private_profile
                == crate::durable_adapter::durable_private_packages::PRIVATE_SCHEMA_PROFILE,
        )?;
    }
    let domain_data = emit_archive_sql(
        tool_spec,
        dump,
        &[
            "--data-only",
            "--table=cmd2_domain",
            "--no-owner",
            "--no-privileges",
        ],
        pg_url,
        cap,
        d,
        c,
    )?;
    let (observed, baseline_sequence, source_head_sequence) =
        domain_schema_profile_from_copy(&domain_data, domain)?;
    require(observed == crate::durable_adapter::schema_profile_digest().to_hex())?;
    require(baseline_sequence <= source_head_sequence)
}

fn domain_schema_profile_from_copy(
    raw: &[u8],
    expected_domain: &str,
) -> io::Result<(String, u64, u64)> {
    let sql = std::str::from_utf8(raw).map_err(|_| error())?;
    let mut lines = sql.lines();
    let mut observed_profile = None;
    let mut header_count = 0usize;
    while let Some(line) = lines.next() {
        let line = line.trim_end();
        if !line.starts_with("COPY ")
            || !line.contains("cmd2_domain")
            || !line.ends_with("FROM stdin;")
        {
            continue;
        }
        header_count += 1;
        let open = line.find('(').ok_or_else(error)?;
        let close = line.rfind(") FROM stdin;").ok_or_else(error)?;
        require(close > open)?;
        let mut column_count = 0usize;
        let mut domain_index = None;
        let mut profile_index = None;
        let mut baseline_index = None;
        let mut head_index = None;
        for column in line[open + 1..close].split(',') {
            let column = column.trim().trim_matches('"');
            if column == "domain" {
                require(domain_index.replace(column_count).is_none())?;
            } else if column == "schema_profile_digest" {
                require(profile_index.replace(column_count).is_none())?;
            } else if column == "source_claim_navigation_reference_baseline_seq" {
                require(baseline_index.replace(column_count).is_none())?;
            } else if column == "head_seq" {
                require(head_index.replace(column_count).is_none())?;
            }
            column_count = column_count.checked_add(1).ok_or_else(error)?;
            require(column_count <= 1024)?;
        }
        let domain_index = domain_index.ok_or_else(error)?;
        let profile_index = profile_index.ok_or_else(error)?;
        let baseline_index = baseline_index.ok_or_else(error)?;
        let head_index = head_index.ok_or_else(error)?;
        let mut row_count = 0usize;
        let mut copy_ended = false;
        while let Some(data) = lines.next() {
            if data == "\\." {
                copy_ended = true;
                break;
            }
            row_count = row_count.checked_add(1).ok_or_else(error)?;
            require(row_count == 1)?;
            let mut observed_domain = None;
            let mut profile = None;
            let mut baseline_sequence = None;
            let mut source_head_sequence = None;
            let mut field_count = 0usize;
            for field in data.split('\t') {
                if field_count == domain_index {
                    observed_domain = Some(decode_pg_copy_text_field(field)?.ok_or_else(error)?);
                } else if field_count == profile_index {
                    profile = Some(decode_pg_copy_text_field(field)?.ok_or_else(error)?);
                } else if field_count == baseline_index {
                    baseline_sequence = decode_pg_copy_text_field(field)?;
                } else if field_count == head_index {
                    source_head_sequence =
                        Some(decode_pg_copy_text_field(field)?.ok_or_else(error)?);
                }
                field_count = field_count.checked_add(1).ok_or_else(error)?;
                require(field_count <= 1024)?;
            }
            require(field_count == column_count)?;
            let domain = observed_domain.ok_or_else(error)?;
            let profile = profile.ok_or_else(error)?;
            let baseline_sequence = baseline_sequence
                .ok_or_else(error)?
                .parse::<u64>()
                .map_err(|_| error())?;
            let source_head_sequence = source_head_sequence
                .ok_or_else(error)?
                .parse::<u64>()
                .map_err(|_| error())?;
            require(domain == expected_domain && profile.len() == 64)?;
            require(profile.bytes().all(|byte| byte.is_ascii_hexdigit()))?;
            require(baseline_sequence <= source_head_sequence)?;
            observed_profile = Some((
                profile.to_ascii_lowercase(),
                baseline_sequence,
                source_head_sequence,
            ));
        }
        require(copy_ended && row_count == 1)?;
    }
    require(header_count == 1)?;
    observed_profile.ok_or_else(error)
}

fn decode_pg_copy_text_field(field: &str) -> io::Result<Option<String>> {
    let bytes = field.as_bytes();
    if bytes == b"\\N" {
        return Ok(None);
    }

    let mut decoded = Vec::with_capacity(bytes.len());
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        cursor += 1;
        if byte != b'\\' {
            decoded.push(byte);
            continue;
        }

        let escaped = *bytes.get(cursor).ok_or_else(error)?;
        cursor += 1;
        match escaped {
            b'\\' => decoded.push(b'\\'),
            b'b' => decoded.push(0x08),
            b'f' => decoded.push(0x0c),
            b'n' => decoded.push(b'\n'),
            b'r' => decoded.push(b'\r'),
            b't' => decoded.push(b'\t'),
            b'v' => decoded.push(0x0b),
            b'0'..=b'7' => {
                let mut value = u16::from(escaped - b'0');
                for _ in 0..2 {
                    let Some(next @ b'0'..=b'7') = bytes.get(cursor).copied() else {
                        break;
                    };
                    cursor += 1;
                    value = value * 8 + u16::from(next - b'0');
                }
                decoded.push(u8::try_from(value).map_err(|_| error())?);
            }
            b'x' => {
                let mut value = 0u8;
                let mut digits = 0usize;
                while digits < 2 {
                    let Some(next) = bytes.get(cursor).copied() else {
                        break;
                    };
                    let Some(digit) = (next as char).to_digit(16) else {
                        break;
                    };
                    cursor += 1;
                    value = value * 16 + u8::try_from(digit).map_err(|_| error())?;
                    digits += 1;
                }
                require(digits > 0)?;
                decoded.push(value);
            }
            _ => return Err(error()),
        }
    }
    require(!decoded.contains(&0))?;
    String::from_utf8(decoded).map(Some).map_err(|_| error())
}

fn private_schema_profile_from_copy(raw: &[u8]) -> io::Result<String> {
    let sql = std::str::from_utf8(raw).map_err(|_| error())?;
    let mut lines = sql.lines();
    let mut header_count = 0usize;
    let mut observed_profile = None;
    while let Some(line) = lines.next() {
        let line = line.trim_end();
        if !line.starts_with("COPY ")
            || !line.contains("cmd2_private_schema_profile")
            || !line.ends_with("FROM stdin;")
        {
            continue;
        }
        header_count += 1;
        let open = line.find('(').ok_or_else(error)?;
        let close = line.rfind(") FROM stdin;").ok_or_else(error)?;
        require(close > open)?;
        let mut column_count = 0usize;
        let mut profile_index = None;
        for column in line[open + 1..close].split(',') {
            if column.trim().trim_matches('"') == "profile_id" {
                require(profile_index.replace(column_count).is_none())?;
            }
            column_count = column_count.checked_add(1).ok_or_else(error)?;
            require(column_count <= 16)?;
        }
        let profile_index = profile_index.ok_or_else(error)?;
        let data = lines.next().ok_or_else(error)?;
        require(data != "\\.")?;
        let mut field_count = 0usize;
        let mut profile = None;
        for field in data.split('\t') {
            if field_count == profile_index {
                profile = decode_pg_copy_text_field(field)?;
            }
            field_count = field_count.checked_add(1).ok_or_else(error)?;
            require(field_count <= 16)?;
        }
        require(field_count == column_count)?;
        observed_profile = Some(profile.ok_or_else(error)?);
        require(lines.next().is_some_and(|line| line == "\\."))?;
    }
    require(header_count == 1)?;
    observed_profile.ok_or_else(error)
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
    // `row_to_json` includes timestamptz values using the session timezone.
    // Bind the outbox enqueue timestamp to UTC in this backup census so a
    // restore compares the same bytes regardless of the client environment.
    db.batch_execute("SET TIME ZONE 'UTC'")
        .map_err(|_| error())?;
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
        "cmd2_source_text_identity",
        "cmd2_source_claim_navigation_reference",
        "cmd2_predicate",
    ];
    let work_revision_presence = db.query_one(
        "SELECT to_regclass(format('%I.%I',current_schema(),'cmd2_work_revision_history_v1')) IS NOT NULL,
                EXISTS(SELECT 1 FROM cmd2_predicate WHERE domain=$1 AND scope='pg-work-revision-v1'),
                coalesce(to_regclass('cmd2_predicate')=to_regclass(format('%I.%I',current_schema(),'cmd2_predicate')),false),
                coalesce(to_regclass('cmd2_work_revision_history_v1')=to_regclass(format('%I.%I',current_schema(),'cmd2_work_revision_history_v1')),false)",
        &[&domain],
    ).map_err(|_| error())?;
    let work_revision_table: bool = work_revision_presence.get(0);
    let work_revision_predicate: bool = work_revision_presence.get(1);
    require(
        work_revision_presence.get::<_, bool>(2)
            && (!work_revision_table || work_revision_presence.get::<_, bool>(3)),
    )?;
    require(work_revision_table || !work_revision_predicate)?;
    let work_revision_metadata = if work_revision_table {
        let mut tx = db.transaction().map_err(|_| error())?;
        crate::source_pg_work_revision::require_schema_v1(&mut tx, d, c).map_err(|_| error())?;
        tx.commit().map_err(|_| error())?;
        tables.push("cmd2_work_revision_history_v1");
        let cap = i64::try_from(profile.max_metadata_rows.checked_add(1).ok_or_else(error)?)
            .map_err(|_| error())?;
        let rows = db
            .query(
                "SELECT row_to_json(t)::text FROM (
                 SELECT * FROM cmd2_work_revision_history_v1 WHERE domain=$1
                 ORDER BY source_path COLLATE \"C\",record_id COLLATE \"C\",generation
                 LIMIT $2
             ) t",
                &[&domain, &cap],
            )
            .map_err(|_| error())?;
        require(rows.len() as u64 <= profile.max_metadata_rows)?;
        let mut root = Digest256Hasher::new();
        root.update(b"tos-backup-pg-work-revision-history-v1\0");
        let mut bytes = 0u64;
        for row in &rows {
            let raw: String = row.get(0);
            let raw = raw.as_bytes();
            bytes = bytes.checked_add(raw.len() as u64).ok_or_else(error)?;
            require(bytes <= profile.max_metadata_bytes && raw.len() <= 131_072)?;
            root.update(&(raw.len() as u64).to_be_bytes());
            root.update(raw);
        }
        Some(json!({
            "schema_sha256":tos_foundation::Digest256::of_bytes(include_bytes!("source_pg_work_revision_v1.sql")).to_hex(),
            "rows":rows.len(),
            "bytes":bytes,
            "row_root":root.finalize().to_hex(),
        }))
    } else {
        None
    };
    if addressed {
        let marker = db.query_opt("SELECT p.profile_digest,p.baseline_generation,f.generation FROM cmd2_audit_delta_v1_domain p JOIN cmd2_audit_fence f USING(domain) WHERE p.domain=$1", &[&domain]).map_err(|_| error())?;
        if let Some(marker) = marker {
            require(
                crate::durable_adapter::audit_delta::profile_hex_is_supported(
                    marker.get::<_, String>(0).trim_end(),
                ) && marker.get::<_, i64>(1) >= 0
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
    if let Some(identity) = work_revision_metadata {
        result["pg_work_revision_v1"] = identity;
    }
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

/// Add the complete retained source-inventory overlay to the legacy bounded
/// census. Cold-verify the cut first, then record this census from the same
/// quiescent state. Cold verification does not write overlay rows.
fn frozen_domain_v2(
    db: &mut Client,
    domain: &str,
    store: &SegmentStore,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Value> {
    let mut result = frozen_domain(db, domain, store, profile, d, c)?;
    let mut total_rows = result["rows"].as_u64().ok_or_else(error)?;
    let mut total_bytes = result["bytes"].as_u64().ok_or_else(error)?;
    let scan_rows = i64::try_from(
        profile
            .max_metadata_rows
            .checked_sub(total_rows)
            .and_then(|n| n.checked_add(1))
            .ok_or_else(error)?,
    )
    .map_err(|_| error())?;
    let query = "SELECT octet_length(row_to_json(t)::text),
                        sha256(convert_to(row_to_json(t)::text,'UTF8'))
         FROM (
             SELECT * FROM cmd2_source_inventory_projection WHERE domain=$1
             ORDER BY subject COLLATE \"C\",effective_seq LIMIT $2
         ) t";
    let params: [&(dyn postgres::types::ToSql + Sync); 2] = [&domain, &scan_rows];
    let mut rows = db.query_raw(query, params).map_err(|_| error())?;
    use postgres::fallible_iterator::FallibleIterator;
    let mut count = 0u64;
    let mut bytes = 0u64;
    let mut root = Digest256Hasher::new();
    root.update(b"tos-backup-source-inventory-projection-v1\0");
    while let Some(row) = rows.next().map_err(|_| error())? {
        active(d, c)?;
        let row_bytes =
            u64::try_from(row.try_get::<_, i32>(0).map_err(|_| error())?).map_err(|_| error())?;
        let digest: Vec<u8> = row.try_get(1).map_err(|_| error())?;
        require(digest.len() == 32)?;
        count = count.checked_add(1).ok_or_else(error)?;
        bytes = bytes.checked_add(row_bytes).ok_or_else(error)?;
        total_rows = total_rows.checked_add(1).ok_or_else(error)?;
        total_bytes = total_bytes.checked_add(row_bytes).ok_or_else(error)?;
        require(
            count <= profile.max_metadata_rows
                && total_rows <= profile.max_metadata_rows
                && bytes <= profile.max_metadata_bytes
                && total_bytes <= profile.max_metadata_bytes,
        )?;
        root.update(&row_bytes.to_be_bytes());
        root.update(&digest);
    }
    active(d, c)?;
    {
        let counts = result["counts"].as_object_mut().ok_or_else(error)?;
        require(!counts.contains_key("cmd2_source_inventory_projection"))?;
        counts.insert("cmd2_source_inventory_projection".into(), json!(count));
    }
    result["rows"] = json!(total_rows);
    result["bytes"] = json!(total_bytes);
    result["source_inventory_projection_v1"] = json!({
        "schema":SOURCE_INVENTORY_CENSUS_V1,
        "rows":count,
        "bytes":bytes,
        "row_root":root.finalize().to_hex(),
    });
    Ok(result)
}

/// V3-V7 retain their historical navigation census formats.
fn frozen_domain_v3(
    db: &mut Client,
    domain: &str,
    store: &SegmentStore,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Value> {
    frozen_domain_v3_inner(db, domain, store, profile, d, c, false, false)
}

fn frozen_domain_v3_inner(
    db: &mut Client,
    domain: &str,
    store: &SegmentStore,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
    navigation_census_v2: bool,
    navigation_census_v3: bool,
) -> io::Result<Value> {
    let mut result = frozen_domain_v2(db, domain, store, profile, d, c)?;
    let (tables, census_key, census_schema) = if navigation_census_v3 {
        (
            &NAVIGATION_TABLES_V2[..],
            "source_navigation_projection_v3",
            SOURCE_NAVIGATION_CENSUS_V3,
        )
    } else if navigation_census_v2 {
        (
            &NAVIGATION_TABLES_V2[..],
            "source_navigation_projection_v2",
            SOURCE_NAVIGATION_CENSUS_V2,
        )
    } else {
        (
            &NAVIGATION_TABLES[..],
            "source_navigation_projection_v1",
            SOURCE_NAVIGATION_CENSUS_V1,
        )
    };
    append_navigation_census(
        db,
        domain,
        profile,
        d,
        c,
        &mut result,
        tables,
        census_key,
        census_schema,
    )?;
    Ok(result)
}

fn frozen_domain_v8(
    db: &mut Client,
    domain: &str,
    store: &SegmentStore,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Value> {
    frozen_domain_v6_inner(db, domain, store, profile, d, c, true, true, false)
}

fn frozen_domain_v9(
    db: &mut Client,
    domain: &str,
    store: &SegmentStore,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Value> {
    frozen_domain_v6_inner(db, domain, store, profile, d, c, true, false, true)
}

fn frozen_domain_v10(
    db: &mut Client,
    domain: &str,
    store: &SegmentStore,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Value> {
    let mut result = frozen_domain_v9(db, domain, store, profile, d, c)?;
    append_relation_census_v1(
        db,
        domain,
        profile,
        d,
        c,
        "source_navigation_cohort_tree_v1",
        SOURCE_NAVIGATION_COHORT_TREE_CENSUS_V1,
        &SOURCE_NAVIGATION_COHORT_TREE_TABLES,
        &mut result,
    )?;
    Ok(result)
}

fn frozen_domain_v11(
    db: &mut Client,
    domain: &str,
    store: &SegmentStore,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Value> {
    let mut result = frozen_domain_v10(db, domain, store, profile, d, c)?;
    append_relation_census_v1(
        db,
        domain,
        profile,
        d,
        c,
        "source_navigation_initial_v1",
        SOURCE_NAVIGATION_INITIAL_CENSUS_V1,
        &SOURCE_NAVIGATION_INITIAL_TABLES,
        &mut result,
    )?;
    Ok(result)
}

fn append_navigation_census(
    db: &mut Client,
    domain: &str,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
    result: &mut Value,
    tables: &[(&str, &str)],
    census_key: &str,
    census_schema: &str,
) -> io::Result<()> {
    require(!tables.is_empty())?;
    let mut total_rows = result["rows"].as_u64().ok_or_else(error)?;
    let mut total_bytes = result["bytes"].as_u64().ok_or_else(error)?;
    let names = tables
        .iter()
        .map(|(table, _)| format!("'public.{table}'"))
        .collect::<Vec<_>>();
    let presence_sql = format!(
        "SELECT {}",
        names
            .iter()
            .map(|name| format!("to_regclass({name}) IS NOT NULL"))
            .collect::<Vec<_>>()
            .join(",")
    );
    let presence = db.query_one(&presence_sql, &[]).map_err(|_| error())?;
    let present_count = (0..tables.len())
        .filter(|index| presence.get::<_, bool>(*index))
        .count();
    require(present_count == 0 || present_count == tables.len())?;
    let installed = present_count == tables.len();
    let scan_rows = i64::try_from(
        profile
            .max_metadata_rows
            .checked_sub(total_rows)
            .and_then(|n| n.checked_add(1))
            .ok_or_else(error)?,
    )
    .map_err(|_| error())?;
    let mut table_counts = serde_json::Map::new();
    let mut census_rows = 0u64;
    let mut census_bytes = 0u64;
    let mut root = Digest256Hasher::new();
    let root_domain = match census_schema {
        SOURCE_NAVIGATION_CENSUS_V1 => b"tos-backup-source-navigation-projection-v1\0".as_slice(),
        SOURCE_NAVIGATION_CENSUS_V2 => b"tos-backup-source-navigation-projection-v2\0".as_slice(),
        SOURCE_NAVIGATION_CENSUS_V3 => b"tos-backup-source-navigation-projection-v3\0".as_slice(),
        _ => return Err(error()),
    };
    root.update(root_domain);
    root.update(if installed {
        &b"present\0"[..]
    } else {
        &b"absent\0"[..]
    });
    if installed {
        for (table, order) in tables.iter().copied() {
            active(d, c)?;
            let row_json = if census_schema == SOURCE_NAVIGATION_CENSUS_V3 {
                "to_jsonb(t)::text"
            } else {
                "row_to_json(t)::text"
            };
            let query = format!(
                "SELECT octet_length({row_json}),
                        sha256(convert_to({row_json},'UTF8'))
                 FROM (SELECT * FROM {table} WHERE domain=$1 ORDER BY {order} LIMIT $2) t"
            );
            let params: [&(dyn postgres::types::ToSql + Sync); 2] = [&domain, &scan_rows];
            let mut rows = db.query_raw(&query, params).map_err(|_| error())?;
            use postgres::fallible_iterator::FallibleIterator;
            let mut count = 0u64;
            root.update(table.as_bytes());
            root.update(&[0]);
            while let Some(row) = rows.next().map_err(|_| error())? {
                active(d, c)?;
                let row_bytes = u64::try_from(row.try_get::<_, i32>(0).map_err(|_| error())?)
                    .map_err(|_| error())?;
                let digest: Vec<u8> = row.try_get(1).map_err(|_| error())?;
                require(digest.len() == 32)?;
                count = count.checked_add(1).ok_or_else(error)?;
                census_rows = census_rows.checked_add(1).ok_or_else(error)?;
                census_bytes = census_bytes.checked_add(row_bytes).ok_or_else(error)?;
                total_rows = total_rows.checked_add(1).ok_or_else(error)?;
                total_bytes = total_bytes.checked_add(row_bytes).ok_or_else(error)?;
                require(
                    count <= profile.max_metadata_rows
                        && census_rows <= profile.max_metadata_rows
                        && total_rows <= profile.max_metadata_rows
                        && census_bytes <= profile.max_metadata_bytes
                        && total_bytes <= profile.max_metadata_bytes,
                )?;
                root.update(&row_bytes.to_be_bytes());
                root.update(&digest);
            }
            root.update(&count.to_be_bytes());
            table_counts.insert(table.into(), json!(count));
        }
    } else {
        for (table, _) in tables.iter().copied() {
            table_counts.insert(table.into(), json!(0));
        }
    }
    active(d, c)?;
    {
        let counts = result["counts"].as_object_mut().ok_or_else(error)?;
        for (table, _) in tables.iter().copied() {
            require(!counts.contains_key(table))?;
            counts.insert(
                table.into(),
                table_counts.get(table).cloned().ok_or_else(error)?,
            );
        }
    }
    result["rows"] = json!(total_rows);
    result["bytes"] = json!(total_bytes);
    result[census_key] = json!({
        "schema":census_schema,
        "state":if installed { "present" } else { "absent" },
        "tables":table_counts,
        "rows":census_rows,
        "bytes":census_bytes,
        "row_root":if installed { json!(root.finalize().to_hex()) } else { Value::Null },
    });
    Ok(())
}

fn validate_navigation_census(metadata: &Value, profile: BackupProfile) -> io::Result<()> {
    let census = &metadata["source_navigation_projection_v1"];
    let tables = census["tables"].as_object().ok_or_else(error)?;
    require(census["schema"] == SOURCE_NAVIGATION_CENSUS_V1)?;
    require(tables.len() == NAVIGATION_TABLES.len())?;
    let state = census["state"].as_str().ok_or_else(error)?;
    require(state == "present")?;
    let mut total_rows = 0u64;
    for (table, _) in NAVIGATION_TABLES {
        let count = tables[table].as_u64().ok_or_else(error)?;
        require(count <= profile.max_metadata_rows)?;
        require(metadata["counts"][table].as_u64() == Some(count))?;
        total_rows = total_rows.checked_add(count).ok_or_else(error)?;
    }
    let rows = census["rows"].as_u64().ok_or_else(error)?;
    let bytes = census["bytes"].as_u64().ok_or_else(error)?;
    require(
        rows == total_rows
            && rows <= profile.max_metadata_rows
            && bytes <= profile.max_metadata_bytes,
    )?;
    let root = census["row_root"].as_str().ok_or_else(error)?;
    require(tos_foundation::Digest256::from_hex(root).is_ok())?;
    Ok(())
}

fn validate_navigation_census_v2(metadata: &Value, profile: BackupProfile) -> io::Result<()> {
    let census = &metadata["source_navigation_projection_v2"];
    let tables = census["tables"].as_object().ok_or_else(error)?;
    require(census["schema"] == SOURCE_NAVIGATION_CENSUS_V2)?;
    require(tables.len() == NAVIGATION_TABLES_V2.len())?;
    let state = census["state"].as_str().ok_or_else(error)?;
    require(state == "present" || state == "absent")?;
    let mut total_rows = 0u64;
    for (table, _) in NAVIGATION_TABLES_V2 {
        let count = tables[table].as_u64().ok_or_else(error)?;
        require(count <= profile.max_metadata_rows)?;
        require(metadata["counts"][table].as_u64() == Some(count))?;
        total_rows = total_rows.checked_add(count).ok_or_else(error)?;
    }
    let rows = census["rows"].as_u64().ok_or_else(error)?;
    let bytes = census["bytes"].as_u64().ok_or_else(error)?;
    require(
        rows == total_rows
            && rows <= profile.max_metadata_rows
            && bytes <= profile.max_metadata_bytes,
    )?;
    if state == "absent" {
        require(rows == 0 && bytes == 0 && census["row_root"].is_null())?;
    } else {
        let root = census["row_root"].as_str().ok_or_else(error)?;
        require(tos_foundation::Digest256::from_hex(root).is_ok())?;
    }
    Ok(())
}

fn validate_navigation_census_v3(metadata: &Value, profile: BackupProfile) -> io::Result<()> {
    let census = &metadata["source_navigation_projection_v3"];
    let tables = census["tables"].as_object().ok_or_else(error)?;
    require(census["schema"] == SOURCE_NAVIGATION_CENSUS_V3)?;
    require(tables.len() == NAVIGATION_TABLES_V2.len())?;
    let state = census["state"].as_str().ok_or_else(error)?;
    require(state == "present" || state == "absent")?;
    let mut total_rows = 0u64;
    for (table, _) in NAVIGATION_TABLES_V2 {
        let count = tables[table].as_u64().ok_or_else(error)?;
        require(count <= profile.max_metadata_rows)?;
        require(metadata["counts"][table].as_u64() == Some(count))?;
        total_rows = total_rows.checked_add(count).ok_or_else(error)?;
    }
    let rows = census["rows"].as_u64().ok_or_else(error)?;
    let bytes = census["bytes"].as_u64().ok_or_else(error)?;
    require(
        rows == total_rows
            && rows <= profile.max_metadata_rows
            && bytes <= profile.max_metadata_bytes,
    )?;
    if state == "absent" {
        require(rows == 0 && bytes == 0 && census["row_root"].is_null())?;
    } else {
        let root = census["row_root"].as_str().ok_or_else(error)?;
        require(tos_foundation::Digest256::from_hex(root).is_ok())?;
    }
    Ok(())
}

/// Add a bounded digest census for the owner-private package namespace. The
/// receipt carries only table counts and an opaque row root; private target
/// references and member bytes remain inside the encrypted/owner-selected dump.
/// A partial install is corruption because it cannot be restored as the same
/// private package ABI.
fn frozen_domain_v4(
    db: &mut Client,
    domain: &str,
    store: &SegmentStore,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Value> {
    frozen_domain_v4_inner(db, domain, store, profile, d, c, false, false)
}

fn frozen_domain_v4_inner(
    db: &mut Client,
    domain: &str,
    store: &SegmentStore,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
    navigation_census_v2: bool,
    navigation_census_v3: bool,
) -> io::Result<Value> {
    let mut result = frozen_domain_v3_inner(
        db,
        domain,
        store,
        profile,
        d,
        c,
        navigation_census_v2,
        navigation_census_v3,
    )?;
    let mut total_rows = result["rows"].as_u64().ok_or_else(error)?;
    let mut total_bytes = result["bytes"].as_u64().ok_or_else(error)?;
    let mut names = PRIVATE_PACKAGE_TABLES
        .iter()
        .map(|(table, _)| format!("'public.{table}'"))
        .collect::<Vec<_>>();
    names.push("'public.cmd2_private_schema_profile'".to_owned());
    let presence_sql = format!(
        "SELECT {}",
        names
            .iter()
            .map(|name| format!("to_regclass({name}) IS NOT NULL"))
            .collect::<Vec<_>>()
            .join(",")
    );
    let presence = db.query_one(&presence_sql, &[]).map_err(|_| error())?;
    let present_count = (0..names.len())
        .filter(|index| presence.get::<_, bool>(*index))
        .count();
    require(present_count == 0 || present_count == names.len())?;
    let installed = present_count == names.len();
    let schema_profile = if installed {
        crate::durable_adapter::durable_private_packages::require_compatible_private_package_schema(
            db,
        )
        .map_err(|_| error())?;
        let rows = db
            .query(
                "SELECT profile_id FROM cmd2_private_schema_profile WHERE singleton",
                &[],
            )
            .map_err(|_| error())?;
        require(rows.len() == 1)?;
        let profile_id: String = rows[0].get(0);
        require(
            profile_id == crate::durable_adapter::durable_private_packages::PRIVATE_SCHEMA_PROFILE,
        )?;
        Value::String(profile_id)
    } else {
        Value::Null
    };
    let scan_rows = i64::try_from(
        profile
            .max_metadata_rows
            .checked_sub(total_rows)
            .and_then(|n| n.checked_add(1))
            .ok_or_else(error)?,
    )
    .map_err(|_| error())?;
    let mut table_counts = serde_json::Map::new();
    let mut census_rows = 0u64;
    let mut census_bytes = 0u64;
    let mut root = Digest256Hasher::new();
    root.update(b"tos-backup-private-owner-packages-v2\0");
    root.update(if installed {
        &b"present\0"[..]
    } else {
        &b"absent\0"[..]
    });
    if installed {
        root.update(schema_profile.as_str().ok_or_else(error)?.as_bytes());
        for (table, order) in PRIVATE_PACKAGE_TABLES {
            active(d, c)?;
            let query = format!(
                "SELECT octet_length(row_to_json(t)::text),
                        sha256(convert_to(row_to_json(t)::text,'UTF8'))
                 FROM (SELECT * FROM {table} WHERE domain=$1 ORDER BY {order} LIMIT $2) t"
            );
            let params: [&(dyn postgres::types::ToSql + Sync); 2] = [&domain, &scan_rows];
            let mut rows = db.query_raw(&query, params).map_err(|_| error())?;
            use postgres::fallible_iterator::FallibleIterator;
            let mut count = 0u64;
            root.update(table.as_bytes());
            root.update(&[0]);
            while let Some(row) = rows.next().map_err(|_| error())? {
                active(d, c)?;
                let row_bytes = u64::try_from(row.try_get::<_, i32>(0).map_err(|_| error())?)
                    .map_err(|_| error())?;
                let digest: Vec<u8> = row.try_get(1).map_err(|_| error())?;
                require(digest.len() == 32)?;
                count = count.checked_add(1).ok_or_else(error)?;
                census_rows = census_rows.checked_add(1).ok_or_else(error)?;
                census_bytes = census_bytes.checked_add(row_bytes).ok_or_else(error)?;
                total_rows = total_rows.checked_add(1).ok_or_else(error)?;
                total_bytes = total_bytes.checked_add(row_bytes).ok_or_else(error)?;
                require(
                    count <= profile.max_metadata_rows
                        && census_rows <= profile.max_metadata_rows
                        && total_rows <= profile.max_metadata_rows
                        && census_bytes <= profile.max_metadata_bytes
                        && total_bytes <= profile.max_metadata_bytes,
                )?;
                root.update(&row_bytes.to_be_bytes());
                root.update(&digest);
            }
            root.update(&count.to_be_bytes());
            table_counts.insert(table.into(), json!(count));
        }
    } else {
        for (table, _) in PRIVATE_PACKAGE_TABLES {
            table_counts.insert(table.into(), json!(0));
        }
    }
    active(d, c)?;
    {
        let counts = result["counts"].as_object_mut().ok_or_else(error)?;
        for (table, _) in PRIVATE_PACKAGE_TABLES {
            require(!counts.contains_key(table))?;
            counts.insert(
                table.into(),
                table_counts.get(table).cloned().ok_or_else(error)?,
            );
        }
    }
    result["rows"] = json!(total_rows);
    result["bytes"] = json!(total_bytes);
    result["private_owner_packages_v2"] = json!({
        "schema":PRIVATE_PACKAGE_CENSUS_V2,
        "state":if installed { "present" } else { "absent" },
        "profile_id":schema_profile,
        "tables":table_counts,
        "rows":census_rows,
        "bytes":census_bytes,
        "row_root":if installed { json!(root.finalize().to_hex()) } else { Value::Null },
    });
    Ok(result)
}

fn validate_private_package_census(metadata: &Value, profile: BackupProfile) -> io::Result<()> {
    let census = &metadata["private_owner_packages_v2"];
    let tables = census["tables"].as_object().ok_or_else(error)?;
    require(census["schema"] == PRIVATE_PACKAGE_CENSUS_V2)?;
    require(tables.len() == PRIVATE_PACKAGE_TABLES.len())?;
    let state = census["state"].as_str().ok_or_else(error)?;
    require(state == "absent" || state == "present")?;
    let mut total_rows = 0u64;
    for (table, _) in PRIVATE_PACKAGE_TABLES {
        let count = tables[table].as_u64().ok_or_else(error)?;
        require(count <= profile.max_metadata_rows)?;
        require(metadata["counts"][table].as_u64() == Some(count))?;
        total_rows = total_rows.checked_add(count).ok_or_else(error)?;
    }
    let rows = census["rows"].as_u64().ok_or_else(error)?;
    let bytes = census["bytes"].as_u64().ok_or_else(error)?;
    require(
        rows == total_rows
            && rows <= profile.max_metadata_rows
            && bytes <= profile.max_metadata_bytes,
    )?;
    if state == "absent" {
        require(
            rows == 0
                && bytes == 0
                && census["row_root"].is_null()
                && census["profile_id"].is_null(),
        )?;
    } else {
        require(
            census["profile_id"].as_str()
                == Some(crate::durable_adapter::durable_private_packages::PRIVATE_SCHEMA_PROFILE),
        )?;
        let root = census["row_root"].as_str().ok_or_else(error)?;
        require(tos_foundation::Digest256::from_hex(root).is_ok())?;
    }
    Ok(())
}

/// Add a bounded digest census for explicit source-navigation current-use
/// policy and withdrawal history. An absent pair is an older schema with no
/// navigation policy store; a partial pair is corruption.
fn frozen_domain_v5(
    db: &mut Client,
    domain: &str,
    store: &SegmentStore,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Value> {
    frozen_domain_v5_inner(db, domain, store, profile, d, c, false, false)
}

fn frozen_domain_v5_inner(
    db: &mut Client,
    domain: &str,
    store: &SegmentStore,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
    navigation_census_v2: bool,
    navigation_census_v3: bool,
) -> io::Result<Value> {
    let mut result = frozen_domain_v4_inner(
        db,
        domain,
        store,
        profile,
        d,
        c,
        navigation_census_v2,
        navigation_census_v3,
    )?;
    let mut total_rows = result["rows"].as_u64().ok_or_else(error)?;
    let mut total_bytes = result["bytes"].as_u64().ok_or_else(error)?;
    let names = SOURCE_NAVIGATION_POLICY_TABLES
        .iter()
        .map(|(table, _)| format!("'public.{table}'"))
        .collect::<Vec<_>>();
    let presence_sql = format!(
        "SELECT {}",
        names
            .iter()
            .map(|name| format!("to_regclass({name}) IS NOT NULL"))
            .collect::<Vec<_>>()
            .join(",")
    );
    let presence = db.query_one(&presence_sql, &[]).map_err(|_| error())?;
    let present_count = (0..names.len())
        .filter(|index| presence.get::<_, bool>(*index))
        .count();
    require(present_count == 0 || present_count == names.len())?;
    let installed = present_count == names.len();
    let scan_rows = i64::try_from(
        profile
            .max_metadata_rows
            .checked_sub(total_rows)
            .and_then(|n| n.checked_add(1))
            .ok_or_else(error)?,
    )
    .map_err(|_| error())?;
    let mut table_counts = serde_json::Map::new();
    let mut census_rows = 0u64;
    let mut census_bytes = 0u64;
    let mut root = Digest256Hasher::new();
    root.update(b"tos-backup-source-navigation-policy-v1\0");
    root.update(if installed {
        &b"present\0"[..]
    } else {
        &b"absent\0"[..]
    });
    if installed {
        for (table, order) in SOURCE_NAVIGATION_POLICY_TABLES {
            active(d, c)?;
            let query = format!(
                "SELECT octet_length(row_to_json(t)::text),
                        sha256(convert_to(row_to_json(t)::text,'UTF8'))
                 FROM (SELECT * FROM {table} WHERE domain=$1 ORDER BY {order} LIMIT $2) t"
            );
            let params: [&(dyn postgres::types::ToSql + Sync); 2] = [&domain, &scan_rows];
            let mut rows = db.query_raw(&query, params).map_err(|_| error())?;
            use postgres::fallible_iterator::FallibleIterator;
            let mut count = 0u64;
            root.update(table.as_bytes());
            root.update(&[0]);
            while let Some(row) = rows.next().map_err(|_| error())? {
                active(d, c)?;
                let row_bytes = u64::try_from(row.try_get::<_, i32>(0).map_err(|_| error())?)
                    .map_err(|_| error())?;
                let digest: Vec<u8> = row.try_get(1).map_err(|_| error())?;
                require(digest.len() == 32)?;
                count = count.checked_add(1).ok_or_else(error)?;
                census_rows = census_rows.checked_add(1).ok_or_else(error)?;
                census_bytes = census_bytes.checked_add(row_bytes).ok_or_else(error)?;
                total_rows = total_rows.checked_add(1).ok_or_else(error)?;
                total_bytes = total_bytes.checked_add(row_bytes).ok_or_else(error)?;
                require(
                    count <= profile.max_metadata_rows
                        && census_rows <= profile.max_metadata_rows
                        && total_rows <= profile.max_metadata_rows
                        && census_bytes <= profile.max_metadata_bytes
                        && total_bytes <= profile.max_metadata_bytes,
                )?;
                root.update(&row_bytes.to_be_bytes());
                root.update(&digest);
            }
            root.update(&count.to_be_bytes());
            table_counts.insert(table.into(), json!(count));
        }
    } else {
        for (table, _) in SOURCE_NAVIGATION_POLICY_TABLES {
            table_counts.insert(table.into(), json!(0));
        }
    }
    active(d, c)?;
    {
        let counts = result["counts"].as_object_mut().ok_or_else(error)?;
        for (table, _) in SOURCE_NAVIGATION_POLICY_TABLES {
            require(!counts.contains_key(table))?;
            counts.insert(
                table.into(),
                table_counts.get(table).cloned().ok_or_else(error)?,
            );
        }
    }
    result["rows"] = json!(total_rows);
    result["bytes"] = json!(total_bytes);
    result["source_navigation_policy_v1"] = json!({
        "schema":SOURCE_NAVIGATION_POLICY_CENSUS_V1,
        "state":if installed { "present" } else { "absent" },
        "tables":table_counts,
        "rows":census_rows,
        "bytes":census_bytes,
        "row_root":if installed { json!(root.finalize().to_hex()) } else { Value::Null },
    });
    Ok(result)
}

/// V6 adds current durable-schema identity and bounded row roots for the
/// publication/import lineage and current source indexes. Keep V5's output
/// shape frozen for receipts that already use that profile.
fn frozen_domain_v6(
    db: &mut Client,
    domain: &str,
    store: &SegmentStore,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Value> {
    frozen_domain_v6_inner(db, domain, store, profile, d, c, false, false, false)
}

fn frozen_domain_v7(
    db: &mut Client,
    domain: &str,
    store: &SegmentStore,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Value> {
    frozen_domain_v6_inner(db, domain, store, profile, d, c, true, false, false)
}

fn frozen_domain_v6_inner(
    db: &mut Client,
    domain: &str,
    store: &SegmentStore,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
    claim_navigation_v2: bool,
    navigation_census_v2: bool,
    navigation_census_v3: bool,
) -> io::Result<Value> {
    let mut result = frozen_domain_v5_inner(
        db,
        domain,
        store,
        profile,
        d,
        c,
        navigation_census_v2,
        navigation_census_v3,
    )?;
    let expected_schema_profile = crate::durable_adapter::schema_profile_digest().to_hex();
    let schema_profile = db
        .query_opt(
            "SELECT schema_profile_digest FROM cmd2_domain WHERE domain=$1",
            &[&domain],
        )
        .map_err(|_| error())?;
    require(
        schema_profile
            .as_ref()
            .and_then(|row| row.get::<_, Option<String>>(0))
            .as_deref()
            == Some(expected_schema_profile.as_str()),
    )?;
    result["durable_schema_profile_v1"] = json!({
        "schema":DURABLE_SCHEMA_PROFILE_CENSUS_V1,
        "sha256":expected_schema_profile,
    });
    for (census_key, census_schema, tables) in [
        (
            "source_publication_v1",
            SOURCE_PUBLICATION_CENSUS_V1,
            &SOURCE_PUBLICATION_TABLES[..],
        ),
        (
            "source_item_file_assignment_v1",
            SOURCE_ITEM_ASSIGNMENT_CENSUS_V1,
            &SOURCE_ITEM_ASSIGNMENT_TABLES[..],
        ),
        (
            "source_annotation_identity_v1",
            SOURCE_ANNOTATION_IDENTITY_CENSUS_V1,
            &SOURCE_ANNOTATION_IDENTITY_TABLES[..],
        ),
    ] {
        append_relation_census_v1(
            db,
            domain,
            profile,
            d,
            c,
            census_key,
            census_schema,
            tables,
            &mut result,
        )?;
        require(result[census_key]["state"] == "present")?;
    }
    let (claim_census_key, claim_census_schema, claim_tables) = if claim_navigation_v2 {
        (
            "source_claim_navigation_reference_v2",
            SOURCE_CLAIM_NAVIGATION_REFERENCE_CENSUS_V2,
            &SOURCE_CLAIM_NAVIGATION_REFERENCE_TABLES_V2[..],
        )
    } else {
        (
            "source_claim_navigation_reference_v1",
            SOURCE_CLAIM_NAVIGATION_REFERENCE_CENSUS_V1,
            &SOURCE_CLAIM_NAVIGATION_REFERENCE_TABLES[..],
        )
    };
    append_relation_census_v1(
        db,
        domain,
        profile,
        d,
        c,
        claim_census_key,
        claim_census_schema,
        claim_tables,
        &mut result,
    )?;
    require(result[claim_census_key]["state"] == "present")?;
    if claim_navigation_v2 {
        let baseline_sequence = db
            .query_one(
                "SELECT source_claim_navigation_reference_baseline_seq FROM cmd2_domain WHERE domain=$1",
                &[&domain],
            )
            .map_err(|_| error())?
            .get::<_, Option<i64>>(0)
            .ok_or_else(error)?;
        let baseline_sequence = u64::try_from(baseline_sequence).map_err(|_| error())?;
        let relation_root = result[claim_census_key]["row_root"]
            .as_str()
            .ok_or_else(error)?
            .to_owned();
        let mut root = Digest256Hasher::new();
        root.update(b"tos-backup-source-claim-navigation-reference-v2-baseline\0");
        root.update(&baseline_sequence.to_be_bytes());
        root.update(relation_root.as_bytes());
        result[claim_census_key]["baseline_sequence"] = json!(baseline_sequence);
        result[claim_census_key]["row_root"] = json!(root.finalize().to_hex());
    }
    Ok(result)
}

fn validate_relation_census_v1(
    metadata: &Value,
    profile: BackupProfile,
    census_key: &str,
    schema: &str,
    tables: &[(&str, &str)],
) -> io::Result<()> {
    let census = &metadata[census_key];
    let counts = census["tables"].as_object().ok_or_else(error)?;
    require(census["schema"] == schema && counts.len() == tables.len())?;
    let state = census["state"].as_str().ok_or_else(error)?;
    require(state == "absent" || state == "present")?;
    let mut total_rows = 0u64;
    for (table, _) in tables {
        let count = counts
            .get(*table)
            .and_then(Value::as_u64)
            .ok_or_else(error)?;
        require(count <= profile.max_metadata_rows)?;
        require(metadata["counts"][*table].as_u64() == Some(count))?;
        total_rows = total_rows.checked_add(count).ok_or_else(error)?;
    }
    let rows = census["rows"].as_u64().ok_or_else(error)?;
    let bytes = census["bytes"].as_u64().ok_or_else(error)?;
    require(
        rows == total_rows
            && rows <= profile.max_metadata_rows
            && bytes <= profile.max_metadata_bytes,
    )?;
    if state == "absent" {
        require(rows == 0 && bytes == 0 && census["row_root"].is_null())?;
    } else {
        let root = census["row_root"].as_str().ok_or_else(error)?;
        require(tos_foundation::Digest256::from_hex(root).is_ok())?;
    }
    Ok(())
}

fn validate_durable_schema_profile_census(metadata: &Value) -> io::Result<()> {
    let expected = crate::durable_adapter::schema_profile_digest().to_hex();
    let census = &metadata["durable_schema_profile_v1"];
    require(
        census["schema"] == DURABLE_SCHEMA_PROFILE_CENSUS_V1
            && census["sha256"].as_str() == Some(expected.as_str()),
    )
}

fn validate_source_claim_navigation_reference_census_v2(
    metadata: &Value,
    profile: BackupProfile,
) -> io::Result<()> {
    validate_relation_census_v1(
        metadata,
        profile,
        "source_claim_navigation_reference_v2",
        SOURCE_CLAIM_NAVIGATION_REFERENCE_CENSUS_V2,
        &SOURCE_CLAIM_NAVIGATION_REFERENCE_TABLES_V2,
    )?;
    require(
        metadata["source_claim_navigation_reference_v2"]["baseline_sequence"]
            .as_u64()
            .is_some(),
    )
}

fn validate_source_navigation_policy_census(
    metadata: &Value,
    profile: BackupProfile,
) -> io::Result<()> {
    let census = &metadata["source_navigation_policy_v1"];
    let tables = census["tables"].as_object().ok_or_else(error)?;
    require(census["schema"] == SOURCE_NAVIGATION_POLICY_CENSUS_V1)?;
    require(tables.len() == SOURCE_NAVIGATION_POLICY_TABLES.len())?;
    let state = census["state"].as_str().ok_or_else(error)?;
    require(state == "absent" || state == "present")?;
    let mut total_rows = 0u64;
    for (table, _) in SOURCE_NAVIGATION_POLICY_TABLES {
        let count = tables[table].as_u64().ok_or_else(error)?;
        require(count <= profile.max_metadata_rows)?;
        require(metadata["counts"][table].as_u64() == Some(count))?;
        total_rows = total_rows.checked_add(count).ok_or_else(error)?;
    }
    let rows = census["rows"].as_u64().ok_or_else(error)?;
    let bytes = census["bytes"].as_u64().ok_or_else(error)?;
    require(
        rows == total_rows
            && rows <= profile.max_metadata_rows
            && bytes <= profile.max_metadata_bytes,
    )?;
    if state == "absent" {
        require(rows == 0 && bytes == 0 && census["row_root"].is_null())?;
    } else {
        let root = census["row_root"].as_str().ok_or_else(error)?;
        require(tos_foundation::Digest256::from_hex(root).is_ok())?;
    }
    Ok(())
}

/// Add a bounded digest census for one required relation set. The receipt
/// records only row counts, byte counts, and a root; table payloads remain in
/// the database dump. A missing or partial V6 relation set is corruption.
fn append_relation_census_v1(
    db: &mut Client,
    domain: &str,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
    census_key: &str,
    schema: &str,
    tables: &[(&str, &str)],
    result: &mut Value,
) -> io::Result<()> {
    require(!tables.is_empty())?;
    let names = tables
        .iter()
        .map(|(table, _)| format!("'public.{table}'"))
        .collect::<Vec<_>>();
    let presence_sql = format!(
        "SELECT {}",
        names
            .iter()
            .map(|name| format!("to_regclass({name}) IS NOT NULL"))
            .collect::<Vec<_>>()
            .join(",")
    );
    let presence = db.query_one(&presence_sql, &[]).map_err(|_| error())?;
    let present_count = (0..tables.len())
        .filter(|index| presence.get::<_, bool>(*index))
        .count();
    require(present_count == tables.len())?;
    let mut total_rows = result["rows"].as_u64().ok_or_else(error)?;
    let mut total_bytes = result["bytes"].as_u64().ok_or_else(error)?;
    // A V6-only relation census may include a table that the historical base
    // census already counted. Bound the rescan itself by the full metadata
    // budget; below, add only previously uncounted rows to the aggregate.
    let scan_rows = i64::try_from(profile.max_metadata_rows.checked_add(1).ok_or_else(error)?)
        .map_err(|_| error())?;
    let mut table_counts = serde_json::Map::new();
    let mut census_rows = 0u64;
    let mut census_bytes = 0u64;
    let mut root = Digest256Hasher::new();
    root.update(schema.as_bytes());
    root.update(&[0]);
    root.update(b"present\0");
    for (table, order) in tables {
        active(d, c)?;
        let query = format!(
            "SELECT octet_length(row_to_json(t)::text),
                    sha256(convert_to(row_to_json(t)::text,'UTF8'))
             FROM (SELECT * FROM {table} WHERE domain=$1 ORDER BY {order} LIMIT $2) t"
        );
        let params: [&(dyn postgres::types::ToSql + Sync); 2] = [&domain, &scan_rows];
        let mut rows = db.query_raw(&query, params).map_err(|_| error())?;
        use postgres::fallible_iterator::FallibleIterator;
        let mut count = 0u64;
        let already_counted = result["counts"].get(*table).and_then(Value::as_u64);
        root.update(table.as_bytes());
        root.update(&[0]);
        while let Some(row) = rows.next().map_err(|_| error())? {
            active(d, c)?;
            let row_bytes = u64::try_from(row.try_get::<_, i32>(0).map_err(|_| error())?)
                .map_err(|_| error())?;
            let digest: Vec<u8> = row.try_get(1).map_err(|_| error())?;
            require(digest.len() == 32)?;
            count = count.checked_add(1).ok_or_else(error)?;
            census_rows = census_rows.checked_add(1).ok_or_else(error)?;
            census_bytes = census_bytes.checked_add(row_bytes).ok_or_else(error)?;
            if already_counted.is_none() {
                total_rows = total_rows.checked_add(1).ok_or_else(error)?;
                total_bytes = total_bytes.checked_add(row_bytes).ok_or_else(error)?;
            }
            require(
                count <= profile.max_metadata_rows
                    && census_rows <= profile.max_metadata_rows
                    && total_rows <= profile.max_metadata_rows
                    && census_bytes <= profile.max_metadata_bytes
                    && total_bytes <= profile.max_metadata_bytes,
            )?;
            root.update(&row_bytes.to_be_bytes());
            root.update(&digest);
        }
        root.update(&count.to_be_bytes());
        table_counts.insert((*table).to_owned(), json!(count));
        if let Some(already_counted) = already_counted {
            require(already_counted == count)?;
        }
    }
    active(d, c)?;
    let counts = result["counts"].as_object_mut().ok_or_else(error)?;
    for (table, _) in tables {
        let table_count = table_counts.get(*table).cloned().ok_or_else(error)?;
        if let Some(existing) = counts.get(*table) {
            require(existing == &table_count)?;
        } else {
            counts.insert((*table).to_owned(), table_count);
        }
    }
    result["rows"] = json!(total_rows);
    result["bytes"] = json!(total_bytes);
    result[census_key] = json!({
        "schema":schema,
        "state":"present",
        "tables":table_counts,
        "rows":census_rows,
        "bytes":census_bytes,
        "row_root":root.finalize().to_hex(),
    });
    Ok(())
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
    workspace_root: &Path,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Value> {
    active(d, c)?;
    // Reuse the complete cold audit, including its chunked metadata reader.
    // The transport's selected bounds apply to the cut as well as the census.
    // Unnamed scratch files live only in the selected writable destination;
    // a restore never needs write access to its authenticated backup.
    let workspace = PrivateGenerationWorkspace::open(
        workspace_root,
        ColdWorkspaceLimits {
            max_scratch_written_bytes: profile.max_metadata_bytes,
            max_run_bytes: 64 * 1024,
            max_runs: usize::try_from(profile.max_history_rows).map_err(|_| error())?,
            merge_fan_in: 16,
            max_rows: profile.max_history_rows,
            max_key_bytes: 4096,
        },
    )
    .map_err(cut_error)?;
    let audit = StreamedGenerationProfile {
        max_commit_seq: profile.max_log_rows,
        max_members: profile.max_history_rows,
        max_pins: usize::try_from(profile.max_log_rows).map_err(|_| error())?,
        max_segment_bytes: profile.max_store_bytes,
        max_membership_key_bytes: profile.max_metadata_bytes,
        max_metadata_rows: usize::try_from(profile.max_metadata_rows).map_err(|_| error())?,
        max_metadata_bytes: usize::try_from(profile.max_metadata_bytes).map_err(|_| error())?,
        max_elapsed: d.saturating_duration_since(Instant::now()),
        max_pg_temp_bytes: profile.max_metadata_bytes,
        max_sql_statement_ms: 60_000,
        // The cut does not publish a generation. Supply the finite minimum
        // shape accepted by the shared profile; membership stays disk-backed.
        generation: GenerationReadLimits {
            max_descriptor_bytes: 4096,
            shape: GenerationShapeLimits {
                max_partitions: 1,
                max_rows_per_partition: 1,
                max_key_bytes: 4096,
                max_leaf_bytes: 8192,
            },
            max_stream_rows: profile.max_history_rows,
            max_stream_key_bytes: profile.max_metadata_bytes,
        },
        rows_per_leaf: 1,
    };
    let bounded = client(url, d, c)?;
    let mut db = DurablePgCoordinator::from_configured_client(bounded);
    let cut = db
        .cold_verify_cut_streamed(store, domain, &workspace, audit, d, c)
        .map_err(cut_error)?;
    active(d, c)?;
    Ok(
        json!({"digest":cut.log_digest().to_hex(),"through_commit_seq":cut.through_commit_seq(),"historical_members":cut.historical_members()}),
    )
}

fn cut_error(cause: DurableError) -> io::Error {
    match cause {
        DurableError::Refused(reason)
        | DurableError::Corrupt(reason)
        | DurableError::Conflict(reason)
        | DurableError::Invalid(reason) => {
            io::Error::new(io::ErrorKind::InvalidData, format!("backup cut: {reason}"))
        }
        // Do not expose database payloads, paths or connection credentials.
        _ => error(),
    }
}

pub fn backup_quiescent(s: &BackupSelection<'_>, d: Instant, c: &AtomicBool) -> io::Result<Value> {
    backup_quiescent_with_profile(s, BackupProfile::default(), d, c)
}

/// Execute backup under the exact finite profile selected by the owner.
pub fn backup_quiescent_with_profile(
    s: &BackupSelection<'_>,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Value> {
    let profile = profile.validate()?;
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
    require_current_cmd2_schema(s.pg_url, d, c)?;
    let store = SegmentStore::open_existing(s.store_root, s.store_limits).map_err(|_| error())?;
    let original_cut = cut(s.pg_url, s.domain, &store, s.backup_root, profile, d, c)?;
    let metadata = frozen_domain_v11(&mut db, s.domain, &store, profile, d, c)?;
    let database: String = db
        .query_one("SELECT current_database()", &[])
        .map_err(|_| error())?
        .get(0);
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
        PgToolMode::Database,
        profile.max_dump_bytes,
        d,
        c,
    )?;
    // Child close is not a durability fence.
    dump_file.sync_all()?;
    let dump_identity = hash_bounded(&dump, profile.max_dump_bytes, d, c)?;
    let mut magic = [0u8; 5];
    File::open(&dump)?.read_exact(&mut magic)?;
    require(&magic == b"PGDMP")?;
    fs::create_dir(target.join("store"))?;
    fs::set_permissions(target.join("store"), fs::Permissions::from_mode(0o700))?;
    let copied =
        tos_fd_open::open_directory_at(&backup, Path::new("store")).map_err(|_| error())?;
    copy_store(&source, &copied, &members, profile, d, c)?;
    require(cut(s.pg_url, s.domain, &store, s.backup_root, profile, d, c)? == original_cut)?;
    require(frozen_domain_v11(&mut db, s.domain, &store, profile, d, c)? == metadata)?;
    let mut receipt = json!({"schema":BACKUP_SCHEMA_V11,"domain":s.domain,"database":database,"cut":original_cut,"profile":profile,"metadata":metadata,"store_members":members,"dump":{"bytes":dump_identity.0,"sha256":dump_identity.1},"tool":tool_receipt,"source_admission":false,"production_quiescence_verified":false});
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
        json!({"status":"backup_complete","receipt":receipt,"receipt_sha256":hash_bounded(&target.join("receipt.json"),profile.max_receipt_bytes,d,c)?.1}),
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
    restore_into_fresh_with_profile(s, BackupProfile::default(), d, c)
}

/// Restore under the exact finite profile selected for the backup.
pub fn restore_into_fresh_with_profile(
    s: &RestoreSelection<'_>,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<Value> {
    restore_into_fresh_with_managed_binding_profile(s, profile, d, c).map(|(result, _)| result)
}
/// Additive route for the existing whole recovery operation. The witness
/// cannot cross a process boundary as JSON or stand for a current source cap.
pub fn restore_into_fresh_with_managed_binding(
    s: &RestoreSelection<'_>,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<(Value, Option<VerifiedManagedModelRestoreV2>)> {
    restore_into_fresh_with_managed_binding_profile(s, BackupProfile::default(), d, c)
}

fn restore_into_fresh_with_managed_binding_profile(
    s: &RestoreSelection<'_>,
    profile: BackupProfile,
    d: Instant,
    c: &AtomicBool,
) -> io::Result<(Value, Option<VerifiedManagedModelRestoreV2>)> {
    let profile = profile.validate()?;
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
    let schema = receipt["schema"].as_str().ok_or_else(error)?;
    let census_v2 = census_v2_for_schema(schema)?;
    let census_v3 = census_v3_for_schema(schema)?;
    let census_v4 = census_v4_for_schema(schema)?;
    let census_v5 = census_v5_for_schema(schema)?;
    let census_v6 = census_v6_for_schema(schema)?;
    let census_v7 = census_v7_for_schema(schema)?;
    let census_v8 = census_v8_for_schema(schema)?;
    let census_v9 = census_v9_for_schema(schema)?;
    let census_v10 = census_v10_for_schema(schema)?;
    let census_v11 = census_v11_for_schema(schema)?;
    require(receipt["domain"] == s.domain)?;
    require(receipt.get("profile").is_some() == receipt.get("metadata").is_some())?;
    if census_v2 {
        require(receipt.get("profile").is_some() && receipt.get("metadata").is_some())?;
    }
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
        if census_v2 {
            let census = &metadata["source_inventory_projection_v1"];
            let rows = metadata["counts"]["cmd2_source_inventory_projection"]
                .as_u64()
                .ok_or_else(error)?;
            let root = census["row_root"].as_str().ok_or_else(error)?;
            require(
                census["schema"] == SOURCE_INVENTORY_CENSUS_V1
                    && census["rows"].as_u64() == Some(rows)
                    && census["bytes"]
                        .as_u64()
                        .is_some_and(|n| n <= profile.max_metadata_bytes)
                    && tos_foundation::Digest256::from_hex(root).is_ok()
                    && rows <= profile.max_metadata_rows,
            )?;
        }
        if census_v3 && !census_v8 && !census_v9 {
            validate_navigation_census(metadata, profile)?;
        }
        if census_v9 {
            validate_navigation_census_v3(metadata, profile)?;
        } else if census_v8 {
            validate_navigation_census_v2(metadata, profile)?;
        }
        if census_v11 {
            validate_relation_census_v1(
                metadata,
                profile,
                "source_navigation_initial_v1",
                SOURCE_NAVIGATION_INITIAL_CENSUS_V1,
                &SOURCE_NAVIGATION_INITIAL_TABLES,
            )?;
        }
        if census_v10 {
            validate_relation_census_v1(
                metadata,
                profile,
                "source_navigation_cohort_tree_v1",
                SOURCE_NAVIGATION_COHORT_TREE_CENSUS_V1,
                &SOURCE_NAVIGATION_COHORT_TREE_TABLES,
            )?;
        }
        if census_v4 {
            validate_private_package_census(metadata, profile)?;
        }
        if census_v5 {
            validate_source_navigation_policy_census(metadata, profile)?;
        }
        if census_v6 {
            validate_durable_schema_profile_census(metadata)?;
            validate_relation_census_v1(
                metadata,
                profile,
                "source_publication_v1",
                SOURCE_PUBLICATION_CENSUS_V1,
                &SOURCE_PUBLICATION_TABLES,
            )?;
            validate_relation_census_v1(
                metadata,
                profile,
                "source_item_file_assignment_v1",
                SOURCE_ITEM_ASSIGNMENT_CENSUS_V1,
                &SOURCE_ITEM_ASSIGNMENT_TABLES,
            )?;
            validate_relation_census_v1(
                metadata,
                profile,
                "source_annotation_identity_v1",
                SOURCE_ANNOTATION_IDENTITY_CENSUS_V1,
                &SOURCE_ANNOTATION_IDENTITY_TABLES,
            )?;
            if !census_v7 {
                validate_relation_census_v1(
                    metadata,
                    profile,
                    "source_claim_navigation_reference_v1",
                    SOURCE_CLAIM_NAVIGATION_REFERENCE_CENSUS_V1,
                    &SOURCE_CLAIM_NAVIGATION_REFERENCE_TABLES,
                )?;
            }
        }
        if census_v7 {
            validate_source_claim_navigation_reference_census_v2(metadata, profile)?;
        }
    }
    let mut authenticated_dump =
        snapshot_authenticated_dump(&dump, expected.clone(), profile.max_dump_bytes, d, c)?;
    verify_current_archive_abi_v1(
        &s.tool,
        &mut authenticated_dump,
        s.domain,
        s.pg_url,
        profile.max_dump_bytes,
        d,
        c,
    )?;
    copy_store(&source, &restored, members, profile, d, c)?;
    authenticated_dump.seek(SeekFrom::Start(0))?;
    let tool_receipt = tool(
        &s.tool,
        &[
            "--exit-on-error",
            "--single-transaction",
            "--no-owner",
            "--no-privileges",
        ],
        Some(authenticated_dump.try_clone()?),
        None,
        s.pg_url,
        PgToolMode::Database,
        profile.max_dump_bytes,
        d,
        c,
    )?;
    require(
        hash_bounded(&dump, profile.max_dump_bytes, d, c)? == expected
            && hash_bounded(&receipt_path, profile.max_receipt_bytes, d, c)?.1 == s.receipt_sha256,
    )?;
    if census_v6 {
        require_current_cmd2_schema(s.pg_url, d, c)?;
    }
    let store = SegmentStore::open_existing(s.store_root, s.store_limits).map_err(|_| error())?;
    let (recovered_metadata, recovered) = if census_v11 {
        let recovered_cut = cut(s.pg_url, s.domain, &store, s.store_root, profile, d, c)?;
        let recovered_metadata = frozen_domain_v11(&mut db, s.domain, &store, profile, d, c)?;
        if let Some(expected) = receipt.get("metadata") {
            require(&recovered_metadata == expected)?;
        }
        (recovered_metadata, recovered_cut)
    } else if census_v10 {
        let recovered_cut = cut(s.pg_url, s.domain, &store, s.store_root, profile, d, c)?;
        let recovered_metadata = frozen_domain_v10(&mut db, s.domain, &store, profile, d, c)?;
        if let Some(expected) = receipt.get("metadata") {
            require(&recovered_metadata == expected)?;
        }
        (recovered_metadata, recovered_cut)
    } else if census_v9 {
        let recovered_cut = cut(s.pg_url, s.domain, &store, s.store_root, profile, d, c)?;
        let recovered_metadata = frozen_domain_v9(&mut db, s.domain, &store, profile, d, c)?;
        if let Some(expected) = receipt.get("metadata") {
            require(&recovered_metadata == expected)?;
        }
        (recovered_metadata, recovered_cut)
    } else if census_v8 {
        let recovered_cut = cut(s.pg_url, s.domain, &store, s.store_root, profile, d, c)?;
        let recovered_metadata = frozen_domain_v8(&mut db, s.domain, &store, profile, d, c)?;
        if let Some(expected) = receipt.get("metadata") {
            require(&recovered_metadata == expected)?;
        }
        (recovered_metadata, recovered_cut)
    } else if census_v7 {
        let recovered_cut = cut(s.pg_url, s.domain, &store, s.store_root, profile, d, c)?;
        let recovered_metadata = frozen_domain_v7(&mut db, s.domain, &store, profile, d, c)?;
        if let Some(expected) = receipt.get("metadata") {
            require(&recovered_metadata == expected)?;
        }
        (recovered_metadata, recovered_cut)
    } else if census_v6 {
        let recovered_cut = cut(s.pg_url, s.domain, &store, s.store_root, profile, d, c)?;
        let recovered_metadata = frozen_domain_v6(&mut db, s.domain, &store, profile, d, c)?;
        if let Some(expected) = receipt.get("metadata") {
            require(&recovered_metadata == expected)?;
        }
        (recovered_metadata, recovered_cut)
    } else if census_v5 {
        let recovered_cut = cut(s.pg_url, s.domain, &store, s.store_root, profile, d, c)?;
        let recovered_metadata = frozen_domain_v5(&mut db, s.domain, &store, profile, d, c)?;
        if let Some(expected) = receipt.get("metadata") {
            require(&recovered_metadata == expected)?;
        }
        (recovered_metadata, recovered_cut)
    } else if census_v4 {
        let recovered_cut = cut(s.pg_url, s.domain, &store, s.store_root, profile, d, c)?;
        let recovered_metadata = frozen_domain_v4(&mut db, s.domain, &store, profile, d, c)?;
        if let Some(expected) = receipt.get("metadata") {
            require(&recovered_metadata == expected)?;
        }
        (recovered_metadata, recovered_cut)
    } else if census_v3 {
        let recovered_cut = cut(s.pg_url, s.domain, &store, s.store_root, profile, d, c)?;
        let recovered_metadata = frozen_domain_v3(&mut db, s.domain, &store, profile, d, c)?;
        if let Some(expected) = receipt.get("metadata") {
            require(&recovered_metadata == expected)?;
        }
        (recovered_metadata, recovered_cut)
    } else if census_v2 {
        let recovered_cut = cut(s.pg_url, s.domain, &store, s.store_root, profile, d, c)?;
        let recovered_metadata = frozen_domain_v2(&mut db, s.domain, &store, profile, d, c)?;
        if let Some(expected) = receipt.get("metadata") {
            require(&recovered_metadata == expected)?;
        }
        (recovered_metadata, recovered_cut)
    } else {
        // Preserve the v1 receipt's exact historical census and verification
        // order; it never claimed a source-inventory overlay row commitment.
        let recovered_metadata = frozen_domain(&mut db, s.domain, &store, profile, d, c)?;
        if let Some(expected) = receipt.get("metadata") {
            require(&recovered_metadata == expected)?;
        }
        let recovered_cut = cut(s.pg_url, s.domain, &store, s.store_root, profile, d, c)?;
        (recovered_metadata, recovered_cut)
    };
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

#[cfg(test)]
mod tests {
    use super::{
        BACKUP_SCHEMA_V1, BACKUP_SCHEMA_V2, BACKUP_SCHEMA_V3, BACKUP_SCHEMA_V4, BACKUP_SCHEMA_V5,
        BACKUP_SCHEMA_V6, BACKUP_SCHEMA_V7, BACKUP_SCHEMA_V8, BACKUP_SCHEMA_V9, BACKUP_SCHEMA_V10,
        census_v2_for_schema, census_v3_for_schema, census_v4_for_schema, census_v5_for_schema,
        census_v6_for_schema, census_v7_for_schema, census_v8_for_schema, census_v9_for_schema,
        selected_config,
    };
    use std::path::{Path, PathBuf};

    #[test]
    #[ignore = "requires isolated PostgreSQL via TOS_METADATA_TEST_PG_URL"]
    fn backup_cut_streams_large_metadata_and_keeps_selected_bounds() {
        use super::*;
        let url = std::env::var("TOS_METADATA_TEST_PG_URL").unwrap();
        let mut db = DurablePgCoordinator::connect(&url).unwrap();
        db.init_lab_schema().unwrap();
        let domain = "backup-streamed-cut-test";
        db.create_domain(domain, tos_foundation::Digest256::of_bytes(b"backup-test"))
            .unwrap();
        let source = tempfile::tempdir().unwrap();
        let scratch = tempfile::tempdir().unwrap();
        let store = SegmentStore::initialize_empty(
            source.path(),
            domain.as_bytes(),
            SegmentLimits {
                max_segment_bytes: 8 * 1024 * 1024,
                max_frame_bytes: 1024 * 1024,
                max_frames: 64,
                max_journal_bytes: 1024 * 1024,
            },
        )
        .unwrap();
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(60);
        let profile = BackupProfile::default();
        let finite = db.cold_verify_cut(&store, domain).unwrap();
        let expected = json!({"digest":finite.log_digest().to_hex(),"through_commit_seq":finite.through_commit_seq(),"historical_members":finite.historical_members()});
        assert_eq!(
            cut(
                &url,
                domain,
                &store,
                scratch.path(),
                profile,
                deadline,
                &cancelled
            )
            .unwrap(),
            expected
        );
        let mut sql = Client::connect(&url, NoTls).unwrap();
        sql.execute("INSERT INTO cmd2_source_index(domain,kind,token,path,definition_digest,inventory_projection) VALUES($1,'path','ToS/large','ToS/large',repeat('a',64),$2)", &[&domain,&vec![0xab_u8;1_048_576]]).unwrap();
        assert!(matches!(
            db.cold_verify_cut(&store, domain),
            Err(DurableError::Refused(
                "cold metadata preadmission whole-row budget exceeded"
            ))
        ));
        // The log cut is unchanged; its full metadata audit must still finish.
        assert_eq!(
            cut(
                &url,
                domain,
                &store,
                scratch.path(),
                profile,
                deadline,
                &cancelled
            )
            .unwrap(),
            expected
        );
        for short in [
            BackupProfile {
                max_metadata_bytes: 1_048_576,
                ..profile
            },
            BackupProfile {
                max_metadata_rows: 1,
                ..profile
            },
        ] {
            let refused = cut(
                &url,
                domain,
                &store,
                scratch.path(),
                short,
                deadline,
                &cancelled,
            )
            .unwrap_err();
            assert!(
                refused.to_string().contains("metadata preadmission"),
                "{refused}"
            );
        }
        cancelled.store(true, Ordering::Relaxed);
        assert!(
            cut(
                &url,
                domain,
                &store,
                scratch.path(),
                profile,
                deadline,
                &cancelled
            )
            .is_err()
        );
        assert_eq!(fs::read_dir(scratch.path()).unwrap().count(), 0);
    }

    #[test]
    fn receipt_version_dispatch_is_explicit() {
        assert!(!census_v2_for_schema(BACKUP_SCHEMA_V1).unwrap());
        assert!(census_v2_for_schema(BACKUP_SCHEMA_V2).unwrap());
        assert!(census_v2_for_schema(BACKUP_SCHEMA_V3).unwrap());
        assert!(census_v2_for_schema(BACKUP_SCHEMA_V4).unwrap());
        assert!(census_v2_for_schema(BACKUP_SCHEMA_V5).unwrap());
        assert!(census_v2_for_schema(BACKUP_SCHEMA_V6).unwrap());
        assert!(census_v2_for_schema(BACKUP_SCHEMA_V7).unwrap());
        assert!(census_v2_for_schema(BACKUP_SCHEMA_V8).unwrap());
        assert!(!census_v3_for_schema(BACKUP_SCHEMA_V2).unwrap());
        assert!(census_v3_for_schema(BACKUP_SCHEMA_V3).unwrap());
        assert!(census_v3_for_schema(BACKUP_SCHEMA_V4).unwrap());
        assert!(census_v3_for_schema(BACKUP_SCHEMA_V5).unwrap());
        assert!(census_v3_for_schema(BACKUP_SCHEMA_V6).unwrap());
        assert!(census_v3_for_schema(BACKUP_SCHEMA_V7).unwrap());
        assert!(census_v3_for_schema(BACKUP_SCHEMA_V8).unwrap());
        assert!(!census_v4_for_schema(BACKUP_SCHEMA_V3).unwrap());
        assert!(census_v4_for_schema(BACKUP_SCHEMA_V4).unwrap());
        assert!(census_v4_for_schema(BACKUP_SCHEMA_V5).unwrap());
        assert!(census_v4_for_schema(BACKUP_SCHEMA_V6).unwrap());
        assert!(census_v4_for_schema(BACKUP_SCHEMA_V7).unwrap());
        assert!(census_v4_for_schema(BACKUP_SCHEMA_V8).unwrap());
        assert!(!census_v5_for_schema(BACKUP_SCHEMA_V4).unwrap());
        assert!(census_v5_for_schema(BACKUP_SCHEMA_V5).unwrap());
        assert!(census_v5_for_schema(BACKUP_SCHEMA_V6).unwrap());
        assert!(census_v5_for_schema(BACKUP_SCHEMA_V7).unwrap());
        assert!(census_v5_for_schema(BACKUP_SCHEMA_V8).unwrap());
        assert!(!census_v6_for_schema(BACKUP_SCHEMA_V5).unwrap());
        assert!(census_v6_for_schema(BACKUP_SCHEMA_V6).unwrap());
        assert!(census_v6_for_schema(BACKUP_SCHEMA_V7).unwrap());
        assert!(census_v6_for_schema(BACKUP_SCHEMA_V8).unwrap());
        assert!(!census_v7_for_schema(BACKUP_SCHEMA_V6).unwrap());
        assert!(census_v7_for_schema(BACKUP_SCHEMA_V7).unwrap());
        assert!(census_v7_for_schema(BACKUP_SCHEMA_V8).unwrap());
        assert!(!census_v8_for_schema(BACKUP_SCHEMA_V7).unwrap());
        assert!(census_v8_for_schema(BACKUP_SCHEMA_V8).unwrap());
        assert!(!census_v8_for_schema(BACKUP_SCHEMA_V9).unwrap());
        assert!(!census_v9_for_schema(BACKUP_SCHEMA_V8).unwrap());
        assert!(census_v9_for_schema(BACKUP_SCHEMA_V9).unwrap());
        assert!(census_v9_for_schema(BACKUP_SCHEMA_V10).unwrap());
        assert!(!super::census_v10_for_schema(BACKUP_SCHEMA_V9).unwrap());
        assert!(super::census_v10_for_schema(BACKUP_SCHEMA_V10).unwrap());
        assert!(!super::census_v11_for_schema(BACKUP_SCHEMA_V10).unwrap());
        assert!(super::census_v11_for_schema(super::BACKUP_SCHEMA_V11).unwrap());
        assert!(super::census_v10_for_schema(super::BACKUP_SCHEMA_V11).unwrap());
        assert!(census_v9_for_schema(super::BACKUP_SCHEMA_V11).unwrap());
        assert!(census_v6_for_schema(super::BACKUP_SCHEMA_V11).unwrap());
        assert!(!census_v8_for_schema(super::BACKUP_SCHEMA_V11).unwrap());
        assert_eq!(
            census_v4_for_schema("unknown").unwrap_err().kind(),
            std::io::ErrorKind::InvalidData
        );
        assert_eq!(
            census_v3_for_schema("unknown").unwrap_err().kind(),
            std::io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn selected_config_accepts_only_canonical_no_follow_unix_socket_directories() {
        let temporary = tempfile::tempdir().unwrap();
        let directory = temporary.path().canonicalize().unwrap();
        let connection = |host: &Path| {
            let host = host
                .to_str()
                .unwrap()
                .replace('\\', "\\\\")
                .replace('\'', "\\'");
            format!("host='{host}' user=tos dbname=tos sslmode=disable")
        };
        assert!(selected_config(&connection(&directory)).is_ok());

        let noncanonical = PathBuf::from(format!("{}/.", directory.display()));
        assert!(selected_config(&connection(&noncanonical)).is_err());

        let alias = directory.join("socket-alias");
        std::os::unix::fs::symlink(&directory, &alias).unwrap();
        assert!(selected_config(&connection(&alias)).is_err());
        assert!(selected_config("host=relative user=tos dbname=tos sslmode=disable").is_err());
        assert!(selected_config("host=@socket user=tos dbname=tos sslmode=disable").is_err());
    }
}
