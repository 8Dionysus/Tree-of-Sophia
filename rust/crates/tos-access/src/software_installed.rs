//! Explicit installed software role association; integrity is neither admission nor execution.
//! This is a child of `software_archive`, whose native proof law remains authoritative.
use super::{
    COMMANDS, Checked, MANIFEST, MANIFEST_BYTES, PROGRAM, Result, SCHEMA, command_closure,
    command_member, field, proof, string, toolchain, uint,
};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};
use tos_foundation::{
    Digest256, Digest256Hasher, JsonLimits, JsonMode, JsonValue, parse_json_with_state_budget,
};

const IMAGE_BYTES: u64 = 512 * 1024 * 1024;
const LOCK_BYTES: usize = 1_048_576;
const TOOLCHAIN_BYTES: usize = 8192;
const PATH_BYTES: usize = 4096;
const BUFFER_BYTES: usize = 65536;
static OPERATION_IDS: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectedRole {
    Access,
    Command(&'static str),
}
impl SelectedRole {
    fn validate(self) -> Result<()> {
        match self {
            Self::Access => Ok(()),
            Self::Command(name) if COMMANDS.contains(&name) => Ok(()),
            _ => Err("unsupported installed software role".into()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InstalledAccessLimits {
    /// Caller-selected image ceiling; the installed dispatch upper bound is 512 MiB.
    pub max_image_bytes: u64,
    pub max_metadata_bytes: usize,
    pub max_state_bytes: usize,
    pub max_io_bytes: u64,
    /// Includes two transient descriptors used by component-safe currentness
    /// opens; running-image association also holds a third kernel witness.
    pub max_held_fds: usize,
}

/// Cumulative structural accounting, not an allocator/RSS or kernel-memory guarantee.
/// Rechecks consume the same operation budget; they do not renew any allowance.
pub struct InstalledAccessBudget {
    operation_id: u64,
    original_deadline: Option<Instant>,
    limits: InstalledAccessLimits,
    metadata_bytes: usize,
    state_bytes: usize,
    io_bytes: u64,
    held_fds: usize,
}
impl InstalledAccessBudget {
    pub fn limits(&self) -> InstalledAccessLimits {
        self.limits
    }
    pub fn metadata_bytes(&self) -> usize {
        self.metadata_bytes
    }
    pub fn state_bytes(&self) -> usize {
        self.state_bytes
    }
    pub fn io_bytes(&self) -> u64 {
        self.io_bytes
    }
    pub fn held_fds(&self) -> usize {
        self.held_fds
    }
    pub fn new(limits: InstalledAccessLimits) -> Result<Self> {
        if limits.max_image_bytes == 0
            || limits.max_image_bytes > IMAGE_BYTES
            || limits.max_metadata_bytes == 0
            || limits.max_state_bytes == 0
            || limits.max_io_bytes == 0
            || limits.max_held_fds < 3
        {
            return Err("finite installed Access budgets required".into());
        }
        Ok(Self {
            operation_id: OPERATION_IDS
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
                .map_err(|_| "installed Access operation identities exhausted")?,
            original_deadline: None,
            limits,
            metadata_bytes: 0,
            state_bytes: 0,
            io_bytes: 0,
            held_fds: 0,
        })
    }
    fn state(&mut self, amount: usize) -> Result<()> {
        self.state_bytes = self
            .state_bytes
            .checked_add(amount)
            .filter(|v| *v <= self.limits.max_state_bytes)
            .ok_or("installed Access state budget")?;
        Ok(())
    }
    fn bind_deadline(&mut self, deadline: Instant) -> Result<()> {
        match self.original_deadline {
            Some(original) if original != deadline => {
                Err("installed Access operation deadline cannot renew".into())
            }
            Some(_) => Ok(()),
            None => {
                self.original_deadline = Some(deadline);
                Ok(())
            }
        }
    }
    fn metadata(&mut self, amount: usize) -> Result<()> {
        self.metadata_bytes = self
            .metadata_bytes
            .checked_add(amount)
            .filter(|v| *v <= self.limits.max_metadata_bytes)
            .ok_or("installed Access metadata budget")?;
        Ok(())
    }
    fn io(&mut self, amount: u64) -> Result<()> {
        self.io_bytes = self
            .io_bytes
            .checked_add(amount)
            .filter(|v| *v <= self.limits.max_io_bytes)
            .ok_or("installed Access read budget")?;
        Ok(())
    }
    fn require_io(&self, amount: u64) -> Result<()> {
        if self
            .io_bytes
            .checked_add(amount)
            .is_none_or(|v| v > self.limits.max_io_bytes)
        {
            return Err("installed software complete read plan exceeds operation budget".into());
        }
        Ok(())
    }
    fn require_metadata_state(&self, metadata: usize, state: usize) -> Result<()> {
        if self
            .metadata_bytes
            .checked_add(metadata)
            .is_none_or(|v| v > self.limits.max_metadata_bytes)
            || self
                .state_bytes
                .checked_add(state)
                .is_none_or(|v| v > self.limits.max_state_bytes)
        {
            return Err(
                "installed software complete metadata plan exceeds operation budget".into(),
            );
        }
        Ok(())
    }
    fn descriptors(&self) -> Result<()> {
        if self
            .held_fds
            .checked_add(2)
            .is_none_or(|v| v > self.limits.max_held_fds)
        {
            return Err("installed Access descriptor budget".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstalledIdentity {
    pub device: u64,
    pub inode: u64,
    pub bytes: u64,
    pub mode: u32,
    pub mtime_seconds: i64,
    pub mtime_nanoseconds: i64,
    pub ctime_seconds: i64,
    pub ctime_nanoseconds: i64,
}
fn identity(file: &File) -> Result<InstalledIdentity> {
    let m = file.metadata().checked()?;
    Ok(InstalledIdentity {
        device: m.dev(),
        inode: m.ino(),
        bytes: m.len(),
        mode: m.mode(),
        mtime_seconds: m.mtime(),
        mtime_nanoseconds: m.mtime_nsec(),
        ctime_seconds: m.ctime(),
        ctime_nanoseconds: m.ctime_nsec(),
    })
}
fn check(deadline: Instant, cancel: &mut dyn FnMut() -> Result<()>) -> Result<()> {
    cancel()?;
    if Instant::now() >= deadline {
        return Err("installed Access original deadline elapsed".into());
    }
    Ok(())
}
struct Held {
    file: File,
    path: PathBuf,
    identity: InstalledIdentity,
    directory: bool,
    digest: Option<Digest256>,
}
/// Owns the selected software inode and all association pins until the caller drops it.
/// It supplies no source/data selection, product admission, or process execution.
pub struct VerifiedInstalledAccess {
    held: Vec<Held>,
    manifest: JsonValue,
    image_index: usize,
    operation_id: u64,
    deadline: Instant,
    role: SelectedRole,
}
impl VerifiedInstalledAccess {
    pub fn image(&self) -> &File {
        &self.held[self.image_index].file
    }
    pub fn identity(&self) -> InstalledIdentity {
        self.held[self.image_index].identity
    }
    pub fn proof(&self) -> &JsonValue {
        match self.role {
            SelectedRole::Access => self
                .manifest
                .object_get("native_access")
                .expect("verified native proof"),
            SelectedRole::Command(name) => self
                .manifest
                .object_get("native_commands")
                .and_then(|roles| roles.object_get(name))
                .expect("verified command proof"),
        }
    }
    pub fn role(&self) -> SelectedRole {
        self.role
    }
    pub fn software_ref(&self) -> &str {
        string(&self.manifest, "software_ref").expect("verified software reference")
    }
    pub fn manifest(&self) -> &JsonValue {
        &self.manifest
    }

    /// Associate the explicitly selected role with the actual running image.
    /// `/proc/self/exe` is the kernel witness; a supplied pathname cannot
    /// substitute another valid installed ELF. Only this role is rehashed.
    pub fn verify_running_image(
        &mut self,
        deadline: Instant,
        cancel: &mut dyn FnMut() -> Result<()>,
        budget: &mut InstalledAccessBudget,
    ) -> Result<()> {
        budget.bind_deadline(deadline)?;
        if deadline != self.deadline || budget.operation_id != self.operation_id {
            return Err("running role requires its original installed operation".into());
        }
        check(deadline, cancel)?;
        // The kernel image stays held while component-safe currentness opens
        // can own two further descriptors (anchor/directory or directory/file).
        if budget
            .held_fds
            .checked_add(3)
            .is_none_or(|count| count > budget.limits.max_held_fds)
        {
            return Err("running role requires three transient descriptors".into());
        }
        budget.metadata(1024)?;
        // This deliberate kernel symlink follows the same running-image law
        // as SoftwareSite::open_running. All installed names still use the
        // owner component-safe no-follow opens in verify_current.
        let running = File::open("/proc/self/exe").checked()?;
        let actual = identity(&running)?;
        if actual != self.identity() || identity(self.image())? != actual {
            return Err("selected installed role is not the running image".into());
        }
        self.verify_current(deadline, cancel, budget)?;
        check(deadline, cancel)?;
        if identity(&running)? != actual || identity(self.image())? != actual {
            return Err("running installed role changed during verification".into());
        }
        check(deadline, cancel)
    }

    /// Check held inodes and the explicitly selected names, then rehash every file.
    fn check_original_operation(
        &self,
        deadline: Instant,
        budget: &mut InstalledAccessBudget,
    ) -> Result<()> {
        budget.bind_deadline(deadline)?;
        if deadline != self.deadline
            || budget.operation_id != self.operation_id
            || budget.held_fds < self.held.len()
        {
            return Err("installed Access recheck requires its original operation budget".into());
        }
        Ok(())
    }
    /// Retained identity fence for the short publication critical section.
    /// This neither rehashes software nor creates admission: the private holder
    /// can only be issued by the complete installed software verifier. Its
    /// caller retains full verification outside the exclusive release lock.
    pub(crate) fn verify_current_identity(
        &self,
        deadline: Instant,
        cancel: &mut dyn FnMut() -> Result<()>,
        budget: &mut InstalledAccessBudget,
    ) -> Result<()> {
        self.check_original_operation(deadline, budget)?;
        for member in &self.held {
            check(deadline, cancel)?;
            budget.descriptors()?;
            let named = if member.directory {
                tos_fd_open::open_absolute_directory(&member.path).checked()?
            } else {
                tos_fd_open::open_absolute_regular(&member.path, member.identity.bytes).checked()?
            };
            if identity(&named)? != member.identity || identity(&member.file)? != member.identity {
                return Err("installed Access association changed".into());
            }
        }
        check(deadline, cancel)
    }
    pub fn verify_current(
        &mut self,
        deadline: Instant,
        cancel: &mut dyn FnMut() -> Result<()>,
        budget: &mut InstalledAccessBudget,
    ) -> Result<()> {
        self.check_original_operation(deadline, budget)?;
        let required = self
            .held
            .iter()
            .filter(|member| !member.directory)
            .try_fold(0u64, |sum, member| {
                sum.checked_add(member.identity.bytes)
                    .and_then(|v| v.checked_add(1))
                    .ok_or("installed software read plan overflow")
            })?;
        budget.require_io(required)?;
        budget.state(BUFFER_BYTES)?;
        for member in &mut self.held {
            check(deadline, cancel)?;
            budget.descriptors()?;
            let named = if member.directory {
                tos_fd_open::open_absolute_directory(&member.path).checked()?
            } else {
                tos_fd_open::open_absolute_regular(&member.path, member.identity.bytes).checked()?
            };
            if identity(&named)? != member.identity || identity(&member.file)? != member.identity {
                return Err("installed Access association changed".into());
            }
            drop(named);
            if let Some(expected) = member.digest {
                if hash(
                    &mut member.file,
                    member.identity.bytes,
                    deadline,
                    cancel,
                    budget,
                )? != expected
                {
                    return Err("installed Access member digest changed".into());
                }
            }
            if identity(&member.file)? != member.identity {
                return Err("installed Access held inode changed".into());
            }
        }
        // Hashing a later member must not hide changes to an earlier pin.
        for member in &self.held {
            check(deadline, cancel)?;
            budget.descriptors()?;
            let named = if member.directory {
                tos_fd_open::open_absolute_directory(&member.path).checked()?
            } else {
                tos_fd_open::open_absolute_regular(&member.path, member.identity.bytes).checked()?
            };
            if identity(&named)? != member.identity || identity(&member.file)? != member.identity {
                return Err("installed Access association changed after hashing".into());
            }
        }
        check(deadline, cancel)
    }
}

fn retain(
    held: &mut Vec<Held>,
    file: File,
    path: PathBuf,
    directory: bool,
    budget: &mut InstalledAccessBudget,
) -> Result<usize> {
    let stamp = identity(&file)?;
    budget.held_fds = budget
        .held_fds
        .checked_add(1)
        .ok_or("installed Access descriptor count overflow")?;
    held.push(Held {
        file,
        path,
        identity: stamp,
        directory,
        digest: None,
    });
    Ok(held.len() - 1)
}
fn path_charge(prefix: &Path, relative: &str, budget: &mut InstalledAccessBudget) -> Result<()> {
    let bytes = prefix
        .as_os_str()
        .len()
        .checked_add(relative.len())
        .and_then(|v| v.checked_add(1))
        .filter(|v| *v <= PATH_BYTES)
        .ok_or("installed Access path budget")?;
    budget.state(bytes)
}
fn child(
    held: &mut Vec<Held>,
    parent: usize,
    leaf: &str,
    directory: bool,
    cap: u64,
    deadline: Instant,
    cancel: &mut dyn FnMut() -> Result<()>,
    budget: &mut InstalledAccessBudget,
) -> Result<usize> {
    check(deadline, cancel)?;
    budget.descriptors()?;
    path_charge(&held[parent].path, leaf, budget)?;
    let path = held[parent].path.join(leaf);
    let file = if directory {
        tos_fd_open::open_directory_at(&held[parent].file, Path::new(leaf)).checked()?
    } else {
        tos_fd_open::open_regular_at(&held[parent].file, Path::new(leaf)).checked()?
    };
    if !directory && file.metadata().checked()?.len() > cap {
        return Err("installed Access member byte bound".into());
    }
    retain(held, file, path, directory, budget)
}
fn read_metadata(
    member: &mut Held,
    cap: usize,
    deadline: Instant,
    cancel: &mut dyn FnMut() -> Result<()>,
    budget: &mut InstalledAccessBudget,
) -> Result<Vec<u8>> {
    check(deadline, cancel)?;
    let bytes =
        usize::try_from(member.identity.bytes).map_err(|_| "installed metadata size overflow")?;
    if bytes > cap {
        return Err("installed Access metadata member bound".into());
    }
    budget.require_io(
        member
            .identity
            .bytes
            .checked_add(1)
            .ok_or("installed metadata read plan overflow")?,
    )?;
    budget.require_metadata_state(bytes, bytes)?;
    budget.metadata(bytes)?;
    budget.state(bytes)?;
    member.file.seek(SeekFrom::Start(0)).checked()?;
    let mut raw = Vec::new();
    raw.try_reserve_exact(bytes).checked()?;
    raw.resize(bytes, 0);
    for chunk in raw.chunks_mut(BUFFER_BYTES) {
        check(deadline, cancel)?;
        budget.io(chunk.len() as u64)?;
        member.file.read_exact(chunk).checked()?;
    }
    let mut eof = [0u8; 1];
    budget.io(1)?;
    if member.file.read(&mut eof).checked()? != 0 || identity(&member.file)? != member.identity {
        return Err("installed Access metadata changed".into());
    }
    member.digest = Some(Digest256::of_bytes(&raw));
    Ok(raw)
}
fn hash(
    file: &mut File,
    bytes: u64,
    deadline: Instant,
    cancel: &mut dyn FnMut() -> Result<()>,
    budget: &mut InstalledAccessBudget,
) -> Result<Digest256> {
    file.seek(SeekFrom::Start(0)).checked()?;
    let mut digest = Digest256Hasher::new();
    let mut remaining = bytes;
    let mut buffer = [0u8; BUFFER_BYTES];
    while remaining > 0 {
        check(deadline, cancel)?;
        let count = remaining.min(BUFFER_BYTES as u64) as usize;
        budget.io(count as u64)?;
        file.read_exact(&mut buffer[..count]).checked()?;
        digest.update(&buffer[..count]);
        remaining -= count as u64;
    }
    check(deadline, cancel)?;
    budget.io(1)?;
    if file.read(&mut buffer[..1]).checked()? != 0 {
        return Err("installed Access image grew".into());
    }
    Ok(digest.finalize())
}

pub fn verify_installed_access(
    prefix: &Path,
    deadline: Instant,
    cancel: &mut dyn FnMut() -> Result<()>,
    budget: &mut InstalledAccessBudget,
) -> Result<VerifiedInstalledAccess> {
    verify_installed_role(prefix, SelectedRole::Access, deadline, cancel, budget)
}

/// Select one supported installed role. A command role is an association, not
/// an owner grant; no image other than the selected role is opened or hashed.
pub fn verify_installed_role(
    prefix: &Path,
    role: SelectedRole,
    deadline: Instant,
    cancel: &mut dyn FnMut() -> Result<()>,
    budget: &mut InstalledAccessBudget,
) -> Result<VerifiedInstalledAccess> {
    budget.bind_deadline(deadline)?;
    check(deadline, cancel)?;
    role.validate()?;
    let name = prefix
        .to_str()
        .ok_or("installed Access prefix must be UTF-8")?;
    if !prefix.is_absolute()
        || name.len() > PATH_BYTES
        || name
            .split('/')
            .skip(1)
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err("explicit normalized installed Access prefix required".into());
    }
    // Nine retained descriptors: prefix/software, four files, three executable parents.
    budget.state(
        9 * std::mem::size_of::<Held>()
            + std::mem::size_of::<VerifiedInstalledAccess>()
            + BUFFER_BYTES
            + 8192,
    )?;
    budget.descriptors()?;
    path_charge(prefix, "", budget)?;
    let prefix_file = tos_fd_open::open_absolute_directory(prefix).checked()?;
    let mut held = Vec::new();
    held.try_reserve_exact(9).checked()?;
    let prefix_index = retain(&mut held, prefix_file, prefix.to_owned(), true, budget)?;
    let software = child(
        &mut held,
        prefix_index,
        "software",
        true,
        0,
        deadline,
        cancel,
        budget,
    )?;
    let manifest_index = child(
        &mut held,
        software,
        MANIFEST,
        false,
        MANIFEST_BYTES as u64,
        deadline,
        cancel,
        budget,
    )?;
    let manifest_bytes = usize::try_from(held[manifest_index].identity.bytes)
        .map_err(|_| "installed metadata size overflow")?;
    let parser_state = manifest_bytes
        .checked_mul(64)
        .and_then(|v| v.checked_add(16384))
        .ok_or("installed Access parser state overflow")?;
    budget.require_metadata_state(
        manifest_bytes,
        manifest_bytes
            .checked_add(parser_state)
            .ok_or("installed Access parser state overflow")?,
    )?;
    let raw = read_metadata(
        &mut held[manifest_index],
        MANIFEST_BYTES,
        deadline,
        cancel,
        budget,
    )?;
    // Reserve parser workspace before creating any typed state. The Foundation
    // parser enforces this reservation using the archive owner's exact grammar.
    budget.state(parser_state)?;
    check(deadline, cancel)?;
    let manifest = parse_json_with_state_budget(
        &raw,
        JsonMode::PublishedStrict,
        JsonLimits {
            max_bytes: MANIFEST_BYTES,
            max_depth: 32,
            max_visits: 100_000,
            max_integer_digits: 20,
        },
        parser_state,
    )
    .checked()?
    .into_root();
    drop(raw);
    check(deadline, cancel)?;
    if manifest.as_object().is_none()
        || string(&manifest, "schema_version")? != SCHEMA
        || field(&manifest, "data_included")? != &JsonValue::Bool(false)
        || field(&manifest, "source_dirty")? != &JsonValue::Bool(false)
    {
        return Err("explicit prefix lacks native software-only manifest".into());
    }
    let access = field(&manifest, "native_access")?;
    proof(access, string(&manifest, "software_ref")?)?;
    if uint(access, "size_bytes")? > IMAGE_BYTES {
        return Err("installed Access proof exceeds 512 MiB".into());
    }
    let commands = match role {
        SelectedRole::Access => None,
        SelectedRole::Command(_) => Some(
            command_closure(&manifest, access)?.ok_or("installed command role closure absent")?,
        ),
    };
    let native = match role {
        SelectedRole::Access => access,
        SelectedRole::Command(name) => field(commands.expect("selected command closure"), name)?,
    };
    let bytes = uint(native, "size_bytes")?;
    if bytes > budget.limits.max_image_bytes {
        return Err("selected installed ELF exceeds caller image ceiling".into());
    }
    let expected_image = Digest256::from_hex(string(native, "sha256")?).checked()?;
    let expected_lock = Digest256::from_hex(string(native, "lock_sha256")?).checked()?;
    let members = field(&manifest, "members")?
        .as_array()
        .ok_or("installed Access members must be array")?;
    if members.len() > 1024 {
        return Err("installed Access member declaration bound".into());
    }
    let check_member =
        |path: &str, receipt: &JsonValue, cancel: &mut dyn FnMut() -> Result<()>| -> Result<()> {
            let mut selected = None;
            for member in members {
                check(deadline, cancel)?;
                if member.object_get("path").and_then(JsonValue::as_str) == Some(path) {
                    if selected.replace(member).is_some() {
                        return Err("duplicate installed software role member".into());
                    }
                }
            }
            let selected = selected.ok_or("installed software role member absent")?;
            if string(selected, "sha256")? != string(receipt, "sha256")?
                || uint(selected, "size_bytes")? != uint(receipt, "size_bytes")?
            {
                return Err("installed software member differs from role proof".into());
            }
            Ok(())
        };
    check_member(PROGRAM, access, cancel)?;
    if let Some(commands) = commands {
        for name in COMMANDS {
            check_member(&command_member(name), field(commands, name)?, cancel)?;
        }
    }
    let lock = child(
        &mut held,
        software,
        "Cargo.lock",
        false,
        LOCK_BYTES as u64,
        deadline,
        cancel,
        budget,
    )?;
    let pin = child(
        &mut held,
        software,
        "rust-toolchain.toml",
        false,
        TOOLCHAIN_BYTES as u64,
        deadline,
        cancel,
        budget,
    )?;
    // Decide the entire known selection/recheck read plan before hashing any
    // lock/pin or opening the selected image. Manifest was the bounded input
    // needed to discover this plan; its consumed bytes stay on the same ledger.
    let metadata = held[lock]
        .identity
        .bytes
        .checked_add(held[pin].identity.bytes)
        .ok_or("installed metadata plan overflow")?;
    let io_required = bytes
        .checked_mul(2)
        .and_then(|v| metadata.checked_mul(2).and_then(|m| v.checked_add(m)))
        .and_then(|v| v.checked_add(held[manifest_index].identity.bytes))
        .and_then(|v| v.checked_add(64 + 7))
        .ok_or("installed software read plan overflow")?;
    budget.require_io(io_required)?;
    let metadata = usize::try_from(metadata).map_err(|_| "installed metadata plan overflow")?;
    let pin_state = usize::try_from(held[pin].identity.bytes)
        .map_err(|_| "installed pin state overflow")?
        .checked_mul(128)
        .and_then(|v| v.checked_add(16384))
        .ok_or("installed pin state overflow")?;
    let state = metadata
        .checked_add(pin_state)
        .and_then(|v| v.checked_add(BUFFER_BYTES))
        .ok_or("installed metadata state overflow")?;
    budget.require_metadata_state(metadata, state)?;
    let lock_raw = read_metadata(&mut held[lock], LOCK_BYTES, deadline, cancel, budget)?;
    if Digest256::of_bytes(&lock_raw) != expected_lock {
        return Err("installed software lock differs from proof".into());
    }
    drop(lock_raw);
    let pin_raw = read_metadata(&mut held[pin], TOOLCHAIN_BYTES, deadline, cancel, budget)?;
    budget.state(
        pin_raw
            .len()
            .checked_mul(128)
            .and_then(|v| v.checked_add(16384))
            .ok_or("installed Access toolchain state overflow")?,
    )?;
    check(deadline, cancel)?;
    if toolchain(&pin_raw)? != string(native, "toolchain")? {
        return Err("installed Access toolchain differs from proof".into());
    }
    drop(pin_raw);
    let mut parent = software;
    let parents: &[&str] = match role {
        SelectedRole::Access => &["access", "src", "tos_access"],
        SelectedRole::Command(_) => &["native", "bin"],
    };
    for &leaf in parents {
        parent = child(&mut held, parent, leaf, true, 0, deadline, cancel, budget)?;
    }
    let image_index = child(
        &mut held,
        parent,
        match role {
            SelectedRole::Access => "tos-access",
            SelectedRole::Command(name) => name,
        },
        false,
        bytes,
        deadline,
        cancel,
        budget,
    )?;
    if held[image_index].identity.bytes != bytes || held[image_index].identity.mode & 0o111 == 0 {
        return Err("installed Access executable size or mode differs".into());
    }
    let mut header = [0u8; 64];
    check(deadline, cancel)?;
    budget.io(64)?;
    held[image_index].file.read_exact(&mut header).checked()?;
    if &header[..7] != b"\x7fELF\x02\x01\x01" || header[18..20] != [0x3e, 0] {
        return Err("installed Access must be Linux x86_64 ELF64".into());
    }
    let actual = hash(&mut held[image_index].file, bytes, deadline, cancel, budget)?;
    if actual != expected_image {
        return Err("installed Access image differs from proof".into());
    }
    held[image_index].digest = Some(actual);
    let mut verified = VerifiedInstalledAccess {
        held,
        manifest,
        image_index,
        operation_id: budget.operation_id,
        deadline,
        role,
    };
    verified.verify_current(deadline, cancel, budget)?;
    Ok(verified)
}

/// Verify an explicit installed role and its association with this process.
/// Source/semantic admission and authority to execute remain caller-owned.
pub fn verify_running_role(
    prefix: &Path,
    role: SelectedRole,
    deadline: Instant,
    cancel: &mut dyn FnMut() -> Result<()>,
    budget: &mut InstalledAccessBudget,
) -> Result<VerifiedInstalledAccess> {
    let mut selected = verify_installed_role(prefix, role, deadline, cancel, budget)?;
    selected.verify_running_image(deadline, cancel, budget)?;
    Ok(selected)
}
