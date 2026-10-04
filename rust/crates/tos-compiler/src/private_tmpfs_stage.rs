//! Linux finite-validator guard paired with the abyss-machine private tmpfs launcher.
//! Install as tos-compiler::private_tmpfs_stage; host admission is a separate prerequisite.
use crate::{
    Error, Result,
    knowledge_stage::{StageIsolation, StageLimits, WritePhase},
};
use serde::Deserialize;
use std::{
    fs::{self, File},
    io::Read,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::{OsStrExt, OsStringExt},
            fs::{FileExt, MetadataExt},
        },
    },
    path::{Path, PathBuf},
};

const MAX_STATUS_BYTES: usize = 64 * 1024;
const MAX_PATH_BYTES: usize = 4096;
const MAX_STORE_COMPONENTS: usize = 128;

/// Logical admitted upper bounds, never measured IO or allocator/RSS. The
/// caller reserves workspace before the call, then debits read/retained upper
/// bounds on success. Verify retains no new guard state. Kernel stat/open work
/// and allocator overhead require the enclosing original working-RAM/CPU cap.
#[derive(Clone, Copy, Debug)]
pub struct PrivateTmpfsGuardCost {
    pub read_bytes: u64,
    pub workspace_bytes: usize,
    pub retained_bytes: usize,
}
// Parsing 8192B can grow String/Vec capacity (including malformed fallback
// arrays); reserve far more than the raw ticket, without constructing a DOM.
// After shape validation only <=3 fallback strings + two <=4096B paths remain.
pub const PRIVATE_TMPFS_SELECT_COST: PrivateTmpfsGuardCost = PrivateTmpfsGuardCost {
    read_bytes: (8192 + MAX_STATUS_BYTES + 1) as u64,
    workspace_bytes: 2 * 1024 * 1024,
    retained_bytes: 16 * 1024,
};
pub const PRIVATE_TMPFS_VERIFY_COST: PrivateTmpfsGuardCost = PrivateTmpfsGuardCost {
    read_bytes: (MAX_STATUS_BYTES + 1) as u64,
    workspace_bytes: 2 * 1024 * 1024,
    retained_bytes: 0,
};

fn bounded_env(name: &'static [u8], cap: usize) -> Result<std::ffi::OsString> {
    // Fixed NUL-terminated names below. Writer setup owns environment custody;
    // concurrent unsafe setenv/unsetenv is outside this constructor contract.
    let raw = unsafe { libc::getenv(name.as_ptr().cast()) };
    if raw.is_null() {
        return Err(Error::Invalid("private stage environment missing"));
    }
    for length in 0..=cap {
        if unsafe { *raw.add(length) } == 0 {
            let bytes = unsafe { std::slice::from_raw_parts(raw.cast::<u8>(), length) };
            return Ok(std::ffi::OsString::from_vec(bytes.to_vec()));
        }
    }
    Err(Error::Budget("private stage environment byte cap"))
}

fn open_store(path: &Path) -> Result<File> {
    let text = path
        .to_str()
        .ok_or(Error::Invalid("persistent store UTF8 path"))?;
    let parts = text.split('/').skip(1);
    if !text.starts_with('/')
        || text == "/"
        || text.len() > MAX_PATH_BYTES
        || text
            .chars()
            .any(|ch| ch.is_whitespace() || ch == '\\' || ch == '\0')
        || parts.clone().count() > MAX_STORE_COMPONENTS
        || parts
            .clone()
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(Error::Invalid("persistent store bounded canonical path"));
    }
    let root = std::ffi::CString::new("/").unwrap();
    let flags = libc::O_PATH | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    let raw = unsafe { libc::open(root.as_ptr(), flags) };
    if raw < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let mut held = unsafe { File::from_raw_fd(raw) };
    for part in parts {
        let name = std::ffi::CString::new(part)
            .map_err(|_| Error::Invalid("persistent store component"))?;
        let raw = unsafe { libc::openat(held.as_raw_fd(), name.as_ptr(), flags) };
        if raw < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        held = unsafe { File::from_raw_fd(raw) };
    }
    Ok(held)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PersistentStoreTicket {
    root: PathBuf,
    root_device: u64,
    root_inode: u64,
    quota_scope: String,
}
// Missing field is v1. An explicitly present null is not a capability shape.
fn deserialize_store<'de, D: serde::Deserializer<'de>>(
    reader: D,
) -> std::result::Result<Option<PersistentStoreTicket>, D::Error> {
    PersistentStoreTicket::deserialize(reader).map(Some)
}

fn bounded_text(path: &str, byte_cap: usize, row_cap: usize, line_cap: usize) -> Result<String> {
    let mut file = File::open(path)?;
    // Fixed allocation, including one oversize sentinel byte. EOF is required;
    // neither proc st_size nor a truncating read establishes the actual bound.
    let mut bytes = vec![0; byte_cap + 1];
    let mut used = 0;
    loop {
        match file.read(&mut bytes[used..]) {
            Ok(0) => break,
            Ok(count) => {
                used += count;
                if used > byte_cap {
                    return Err(Error::Budget("stage kernel evidence byte cap"));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        }
    }
    bytes.truncate(used);
    let text =
        String::from_utf8(bytes).map_err(|_| Error::Invalid("stage kernel evidence UTF8"))?;
    for (index, line) in text.lines().enumerate() {
        if index >= row_cap || line.len() > line_cap {
            return Err(Error::Budget("stage kernel evidence row or line cap"));
        }
    }
    Ok(text)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ticket {
    schema: String,
    quota_bytes: u64,
    inode_limit: u64,
    working_ram_bytes: u64,
    root: PathBuf,
    root_device: u64,
    root_inode: u64,
    mount_id: u64,
    mount_namespace_inode: u64,
    parent_mount_namespace_inode: u64,
    fallbacks: Vec<String>,
    lifetime: String,
    capabilities: String,
    write_confinement: String,
    consumer_requires_dumpable_zero: bool,
    #[serde(default, deserialize_with = "deserialize_store")]
    persistent_store: Option<PersistentStoreTicket>,
}

/// Current kernel-accounted allocation in the selected private tmpfs.
/// This observes retained blocks/inodes, not transient peak allocation or RSS.
#[derive(Clone, Copy, Debug)]
pub struct PrivateTmpfsUsage {
    pub used_bytes: u64,
    pub used_inodes: u64,
}

pub struct PrivateTmpfsStageIsolation {
    ticket: Ticket,
    custody: File,
    persistent_custody: Option<File>,
}
impl PrivateTmpfsStageIsolation {
    /// Call before source capture and before starting writer descendants. Each
    /// exec'd writer must likewise establish dumpable=0 before writing bytes.
    /// The admitted values come from the whole-run envelope, never this ticket.
    pub fn select_from_environment(
        quota_bytes: u64,
        inode_limit: u64,
        working_ram_bytes: u64,
    ) -> Result<Self> {
        if unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let raw: i32 = bounded_env(b"ABYSS_STAGE_TICKET_FD\0", 32)?
            .into_string()
            .ok()
            .and_then(|value| value.parse().ok())
            .filter(|value| *value > 2)
            .ok_or(Error::Invalid("private stage custody FD missing"))?;
        let duplicate = unsafe { libc::fcntl(raw, libc::F_DUPFD_CLOEXEC, 3) };
        if duplicate < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let custody = unsafe { File::from_raw_fd(duplicate) };
        let seals = unsafe { libc::fcntl(custody.as_raw_fd(), libc::F_GET_SEALS) };
        let required =
            libc::F_SEAL_SEAL | libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK;
        if seals < 0 || seals & required != required || custody.metadata()?.len() > 8192 {
            return Err(Error::Invalid("unsealed or oversized private stage ticket"));
        }
        // pread leaves the inherited open-file-description offset untouched,
        // so concurrently initialized descendants cannot race ticket reads.
        let mut bytes = vec![0; custody.metadata()?.len() as usize];
        custody.read_exact_at(&mut bytes, 0)?;
        let ticket: Ticket = serde_json::from_slice(&bytes)
            .map_err(|_| Error::Invalid("private stage ticket shape"))?;
        drop(bytes); // ticket read buffer never overlaps kernel-evidence buffers
        let root_text = ticket
            .root
            .to_str()
            .ok_or(Error::Invalid("non UTF8 stage root"))?;
        if root_text.len() > MAX_PATH_BYTES
            || !ticket.root.is_absolute()
            || root_text
                .bytes()
                .any(|byte| byte.is_ascii_whitespace() || byte == b'\\' || byte == 0)
            || ticket.root.components().any(|part| {
                matches!(
                    part,
                    std::path::Component::ParentDir | std::path::Component::CurDir
                )
            })
            || ticket.fallbacks.len() > 3
            || ticket.fallbacks.iter().enumerate().any(|(index, value)| {
                !["/var/tmp", "/usr/tmp", "/tmp"].contains(&value.as_str())
                    || ticket.fallbacks[..index].contains(value)
            })
        {
            return Err(Error::Invalid(
                "private stage ticket path or fallback bounds",
            ));
        }
        let version_valid = match (ticket.schema.as_str(), ticket.persistent_store.as_ref()) {
            ("abyss_machine_private_tmpfs_stage_v1", None) => true,
            ("abyss_machine_private_tmpfs_stage_v2", Some(store)) => {
                store.quota_scope == "outside-private-tmpfs"
                    && store.root_device != ticket.root_device
                    && !store.root.starts_with(&ticket.root)
                    && !ticket.root.starts_with(&store.root)
                    && ["/tmp", "/var/tmp", "/usr/tmp"].iter().all(|fallback| {
                        !store.root.starts_with(fallback)
                            && !Path::new(fallback).starts_with(&store.root)
                    })
            }
            _ => false,
        };
        if !version_valid
            || ticket.quota_bytes != quota_bytes
            || ticket.inode_limit != inode_limit
            || ticket.working_ram_bytes != working_ram_bytes
            || ticket.lifetime != "consumer-process-mount-namespace"
            || ticket.capabilities != "dropped-before-exec"
            || ticket.write_confinement != "landlock-v3"
            || !ticket.consumer_requires_dumpable_zero
            || bounded_env(b"ABYSS_STAGE_ROOT\0", MAX_PATH_BYTES)?.as_os_str()
                != ticket.root.as_os_str()
        {
            return Err(Error::Invalid(
                "private stage ticket differs from admitted envelope",
            ));
        }
        let persistent_custody = ticket
            .persistent_store
            .as_ref()
            .map(|store| open_store(&store.root))
            .transpose()?;
        let result = Self {
            ticket,
            custody,
            persistent_custody,
        };
        result.verify_kernel()?;
        Ok(result)
    }
    pub fn root(&self) -> &Path {
        &self.ticket.root
    }

    /// Optional ONE host-selected pre-existing store. Its writes are OUTSIDE
    /// the tmpfs byte/inode quota and require the caller's separate original
    /// persistent-write IO cap and fresh physical reservation. No root creation.
    pub fn persistent_store(&self) -> Option<&Path> {
        self.ticket
            .persistent_store
            .as_ref()
            .map(|store| store.root.as_path())
    }

    /// Call before any store write, before publication and before receipt.
    /// This rechecks the same sealed configuration/kernel boundary and exact
    /// held/named store identity; it creates no authority or new budget.
    pub fn verify_persistent_store(&self, requested: &Path) -> Result<()> {
        let store = self.ticket.persistent_store.as_ref().ok_or(Error::Invalid(
            "readonly stage has no persistent store capability",
        ))?;
        if requested.as_os_str() != store.root.as_os_str() {
            return Err(Error::Invalid(
                "requested persistent store differs from capability",
            ));
        }
        self.verify_kernel()
    }

    /// Borrow the exact originally held store after the existing capability
    /// and kernel checks. This adds no namespace or allocation authority.
    pub fn persistent_store_custody(&self, requested: &Path) -> Result<&File> {
        self.verify_persistent_store(requested)?;
        self.persistent_custody
            .as_ref()
            .ok_or(Error::Invalid("persistent store custody missing"))
    }

    fn verify_store_identity(&self) -> Result<()> {
        match (&self.ticket.persistent_store, &self.persistent_custody) {
            (None, None) => Ok(()),
            (Some(store), Some(held)) => {
                let current = open_store(&store.root)?;
                let named = current.metadata()?;
                let held = held.metadata()?;
                if !held.is_dir()
                    || !named.is_dir()
                    || (held.dev(), held.ino()) != (store.root_device, store.root_inode)
                    || (named.dev(), named.ino()) != (store.root_device, store.root_inode)
                {
                    return Err(Error::Invalid(
                        "persistent store held or named identity changed",
                    ));
                }
                Ok(())
            }
            _ => Err(Error::Invalid("persistent store custody missing")),
        }
    }

    /// Recheck the selected namespace and quota before observing its usage.
    /// Callers retain their original deadline/cancellation checks around this
    /// bounded guard read; no budget or admission authority is created here.
    pub fn quota_usage(&self) -> Result<PrivateTmpfsUsage> {
        self.verify_kernel()?;
        let root = File::open(&self.ticket.root)?;
        let identity = root.metadata()?;
        if !identity.is_dir()
            || identity.dev() != self.ticket.root_device
            || identity.ino() != self.ticket.root_inode
        {
            return Err(Error::Invalid("private stage usage root changed"));
        }
        let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstatvfs(root.as_raw_fd(), &mut stats) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        if stats.f_blocks.checked_mul(stats.f_frsize) != Some(self.ticket.quota_bytes)
            || stats.f_files != self.ticket.inode_limit
        {
            return Err(Error::Invalid("private stage usage quota changed"));
        }
        let used_bytes = stats
            .f_blocks
            .checked_sub(stats.f_bfree)
            .and_then(|blocks| blocks.checked_mul(stats.f_frsize))
            .ok_or(Error::Invalid("private stage block usage range"))?;
        let used_inodes = stats
            .f_files
            .checked_sub(stats.f_ffree)
            .ok_or(Error::Invalid("private stage inode usage range"))?;
        let named = fs::symlink_metadata(&self.ticket.root)?;
        if named.file_type().is_symlink()
            || named.dev() != identity.dev()
            || named.ino() != identity.ino()
        {
            return Err(Error::Invalid("private stage usage root rebound"));
        }
        Ok(PrivateTmpfsUsage {
            used_bytes,
            used_inodes,
        })
    }

    fn verify_kernel(&self) -> Result<()> {
        let ticket = &self.ticket;
        if unsafe { libc::prctl(libc::PR_GET_DUMPABLE, 0, 0, 0, 0) } != 0
            || unsafe { libc::prctl(libc::PR_GET_NO_NEW_PRIVS, 0, 0, 0, 0) } != 1
        {
            return Err(Error::Invalid("stage writer lacks dumpable/NNP boundary"));
        }
        let status = bounded_text("/proc/self/status", MAX_STATUS_BYTES, 256, 4096)?;
        for name in ["CapInh:", "CapPrm:", "CapEff:", "CapBnd:", "CapAmb:"] {
            let raw = status
                .lines()
                .find_map(|line| line.strip_prefix(name))
                .ok_or(Error::Invalid("capability field missing"))?
                .trim();
            if u64::from_str_radix(raw, 16).ok() != Some(0) {
                return Err(Error::Invalid("stage writer retains mount capabilities"));
            }
        }
        drop(status); // Remaining kernel observations use fixed-size syscall structs.
        let ns = fs::metadata("/proc/self/ns/mnt")?.ino();
        if ns != ticket.mount_namespace_inode || ns == ticket.parent_mount_namespace_inode {
            return Err(Error::Invalid("private stage mount lifetime changed"));
        }
        let root = fs::symlink_metadata(&ticket.root)?;
        if !root.is_dir()
            || root.dev() != ticket.root_device
            || root.ino() != ticket.root_inode
            || fs::canonicalize(&ticket.root)? != ticket.root
        {
            return Err(Error::Invalid("private stage root identity changed"));
        }
        // Observe the selected mount directly through this held directory.
        // STATX_MNT_ID is the same identity as mountinfo's first field, without
        // rereading and tokenizing every mount for each inserted SQLite row.
        // No lifetime cache: namespace, mount/type, quotas and custody remain
        // live checks at every original verification point.
        let file = File::open(&ticket.root)?;
        let held = file.metadata()?;
        if !held.is_dir() || held.dev() != ticket.root_device || held.ino() != ticket.root_inode {
            return Err(Error::Invalid("stage held root identity changed"));
        }
        let mut mount: libc::statx = unsafe { std::mem::zeroed() };
        if unsafe {
            libc::statx(
                file.as_raw_fd(),
                c"".as_ptr(),
                libc::AT_EMPTY_PATH,
                libc::STATX_MNT_ID,
                &mut mount,
            )
        } != 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        if mount.stx_mask & libc::STATX_MNT_ID == 0 || mount.stx_mnt_id != ticket.mount_id {
            return Err(Error::Invalid("stage quota mount changed or unavailable"));
        }
        let mut filesystem: libc::statfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstatfs(file.as_raw_fd(), &mut filesystem) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        if filesystem.f_type as u64 != libc::TMPFS_MAGIC as u64 {
            return Err(Error::Invalid("stage quota filesystem is not tmpfs"));
        }
        let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstatvfs(file.as_raw_fd(), &mut stats) } != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        if stats.f_blocks.checked_mul(stats.f_frsize) != Some(ticket.quota_bytes)
            || stats.f_files != ticket.inode_limit
            || stats.f_bavail == 0
            || stats.f_favail == 0
        {
            return Err(Error::Budget(
                "private stage kernel quota absent, changed or exhausted",
            ));
        }
        for name in [b"TMPDIR\0".as_slice(), b"SQLITE_TMPDIR\0".as_slice()] {
            if bounded_env(name, MAX_PATH_BYTES)?.as_os_str() != ticket.root.join("tmp").as_os_str()
            {
                return Err(Error::Invalid("SQLite temp environment changed"));
            }
        }
        for fallback in ["/var/tmp", "/usr/tmp", "/tmp"] {
            match fs::metadata(fallback) {
                Ok(meta)
                    if ticket.fallbacks.iter().any(|value| value == fallback)
                        && meta.dev() == ticket.root_device
                        && meta.ino() == fs::metadata(ticket.root.join("tmp"))?.ino() =>
                {
                    ()
                }
                Err(error)
                    if error.kind() == std::io::ErrorKind::NotFound
                        && !ticket.fallbacks.iter().any(|value| value == fallback) =>
                {
                    ()
                }
                _ => return Err(Error::Invalid("SQLite fallback escaped private quota")),
            }
        }
        // Retain custody; a sealed ticket is configuration from the selected
        // source launcher, not an independent admission or semantic proof.
        if self.custody.metadata()?.len() == 0 {
            return Err(Error::Invalid("stage custody lost"));
        }
        self.verify_store_identity()?;
        Ok(())
    }
}
impl StageIsolation for PrivateTmpfsStageIsolation {
    fn verify(&self, candidate: &Path, limits: StageLimits, _: WritePhase) -> Result<()> {
        self.verify_kernel()?;
        if limits.max_temp_bytes > self.ticket.quota_bytes {
            return Err(Error::Budget(
                "stage temp limit exceeds admitted aggregate quota",
            ));
        }
        if candidate.as_os_str().as_bytes().len() > MAX_PATH_BYTES {
            return Err(Error::Budget("stage candidate path cap"));
        }
        let absolute = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            std::env::current_dir()?.join(candidate)
        };
        if absolute.as_os_str().as_bytes().len() > MAX_PATH_BYTES
            || !absolute.starts_with(&self.ticket.root)
            || absolute
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(Error::Invalid("stage candidate outside private namespace"));
        }
        let mut cursor = self.ticket.root.clone();
        for component in absolute
            .strip_prefix(&self.ticket.root)
            .map_err(|_| Error::Invalid("candidate prefix"))?
            .components()
        {
            cursor.push(component);
            match fs::symlink_metadata(&cursor) {
                Ok(meta)
                    if !meta.file_type().is_symlink() && meta.dev() == self.ticket.root_device =>
                {
                    ()
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                _ => return Err(Error::Invalid("candidate symlink or foreign mount")),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod persistent_boundary_controls {
    use super::*;

    // Mechanical refusal fixtures only. No manufactured positive kernel,
    // admission, accepted source, or persistent-write authority is asserted.
    fn fixture_ticket() -> Ticket {
        serde_json::from_str(
            r#"{
            "schema":"abyss_machine_private_tmpfs_stage_v1",
            "quota_bytes":1048576,"inode_limit":128,"working_ram_bytes":268435456,
            "root":"/unselected-fixture-stage","root_device":7,"root_inode":8,
            "mount_id":9,"mount_namespace_inode":10,"parent_mount_namespace_inode":11,
            "fallbacks":[],"lifetime":"consumer-process-mount-namespace",
            "capabilities":"dropped-before-exec","write_confinement":"landlock-v3",
            "consumer_requires_dumpable_zero":true
        }"#,
        )
        .unwrap()
    }

    #[test]
    fn private_stage_v1_and_v2_ticket_and_store_refusal_controls() {
        let mut ticket = fixture_ticket();
        assert!(ticket.persistent_store.is_none());
        let mut encoded: serde_json::Value = serde_json::from_str(
            r#"{
            "schema":"abyss_machine_private_tmpfs_stage_v2",
            "quota_bytes":1048576,"inode_limit":128,"working_ram_bytes":268435456,
            "root":"/unselected-fixture-stage","root_device":7,"root_inode":8,
            "mount_id":9,"mount_namespace_inode":10,"parent_mount_namespace_inode":11,
            "fallbacks":[],"lifetime":"consumer-process-mount-namespace",
            "capabilities":"dropped-before-exec","write_confinement":"landlock-v3",
            "consumer_requires_dumpable_zero":true,"persistent_store":null
        }"#,
        )
        .unwrap();
        assert!(serde_json::from_value::<Ticket>(encoded.clone()).is_err());
        encoded["persistent_store"] = serde_json::json!({"root":"/selected-fixture-store",
            "root_device":12,"root_inode":13,"quota_scope":"outside-private-tmpfs"});
        assert!(
            serde_json::from_value::<Ticket>(encoded)
                .unwrap()
                .persistent_store
                .is_some()
        );
        let readonly = PrivateTmpfsStageIsolation {
            ticket,
            custody: File::open("/dev/null").unwrap(),
            persistent_custody: None,
        };
        assert!(matches!(
            readonly.verify_persistent_store(Path::new("/selected-fixture-store")),
            Err(Error::Invalid(
                "readonly stage has no persistent store capability"
            ))
        ));
        ticket = fixture_ticket();
        ticket.persistent_store = Some(PersistentStoreTicket {
            root: PathBuf::from("/selected-fixture-store"),
            root_device: 12,
            root_inode: 13,
            quota_scope: "outside-private-tmpfs".into(),
        });
        let unselected = PrivateTmpfsStageIsolation {
            ticket,
            custody: File::open("/dev/null").unwrap(),
            persistent_custody: None,
        };
        assert!(matches!(
            unselected.verify_persistent_store(Path::new("/adjacent-fixture-store")),
            Err(Error::Invalid(
                "requested persistent store differs from capability"
            ))
        ));
    }

    #[test]
    fn private_stage_store_path_and_named_identity_refusal_controls() {
        for path in [
            "relative",
            "/",
            "/fixture/../store",
            "/fixture//store",
            "/fixture/./store",
        ] {
            assert!(matches!(
                open_store(Path::new(path)),
                Err(Error::Invalid(_))
            ));
        }
        // Identity-only negative fixture: honor TMPDIR or the platform test temp.
        // No private tmpfs quota or positive kernel admission is claimed here.
        let root =
            std::env::temp_dir().join(format!("native-guard-store-control-{}", std::process::id()));
        assert!(!root.exists());
        fs::create_dir(&root).unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone()); // exact newly owned fixture only
        let path = root.join("selected");
        fs::create_dir(&path).unwrap();
        let held = open_store(&path).unwrap();
        let meta = held.metadata().unwrap();
        let mut ticket = fixture_ticket();
        ticket.persistent_store = Some(PersistentStoreTicket {
            root: path.clone(),
            root_device: meta.dev(),
            root_inode: meta.ino(),
            quota_scope: "outside-private-tmpfs".into(),
        });
        let guard = PrivateTmpfsStageIsolation {
            ticket,
            custody: File::open("/dev/null").unwrap(),
            persistent_custody: Some(held),
        };
        fs::rename(&path, root.join("retained")).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(matches!(
            guard.verify_store_identity(),
            Err(Error::Invalid(
                "persistent store held or named identity changed"
            ))
        ));
    }
}
