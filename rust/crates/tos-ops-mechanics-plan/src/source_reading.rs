//! Bounded source reading and output custody shared by native consumers.
use crate::executor::{self, Limits};
use serde_json::Value;
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicI32, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};
use tos_foundation::{Digest256, Digest256Hasher};

const MAX_FILE: usize = 8 * 1024 * 1024;
const MAX_INPUT: usize = 64 * 1024 * 1024;
const MAX_OUTPUT: usize = 16 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;
/// Absolute ceiling for callers that have priced a larger whole-source pass.
/// Ordinary route APIs still receive `MAX_OPERATIONS`.
pub const MAX_BUDGETED_ROUTE_OPERATIONS: usize = 1_000_000;
/// Upper finite ceiling available only to callers that budget a wider tree.
/// Maintained whole-repository card readers select this streamed ceiling;
/// generic source discovery keeps its smaller historical ceiling.
pub const MAX_SELECTED_PATH_DISCOVERY_ENTRIES: usize = 131_072;
const MAX_RELATIVE_PATH_BYTES: usize = 4096;
const MAX_RELATIVE_COMPONENTS: usize = 128;
const MAX_OPERATIONS: usize = 100_000;
pub type Issue = (String, String);
fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
pub fn sha256_bytes(bytes: &[u8]) -> String {
    Digest256::of_bytes(bytes).to_hex()
}
pub fn python_space(c: char) -> bool {
    matches!(c, '\u{9}'..='\u{d}' | '\u{1c}'..='\u{20}' | '\u{85}' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}
pub fn whitespace_tokens(text: &str) -> usize {
    text.split(python_space).filter(|s| !s.is_empty()).count()
}
pub fn splitlines(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if matches!(
            c,
            '\n' | '\r'
                | '\u{b}'
                | '\u{c}'
                | '\u{1c}'
                | '\u{1d}'
                | '\u{1e}'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        ) {
            out.push(&text[start..i]);
            start = i + c.len_utf8();
            if c == '\r' && chars.peek().is_some_and(|(_, c)| *c == '\n') {
                let (j, c) = chars.next().unwrap();
                start = j + c.len_utf8();
            }
        }
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}
fn validate_relative(value: &str) -> io::Result<()> {
    if value.is_empty()
        || value.len() > MAX_RELATIVE_PATH_BYTES
        || value.contains('\0')
        || value.contains('\n')
        || Path::new(value).components().count() > MAX_RELATIVE_COMPONENTS
        || Path::new(value)
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        return Err(invalid(
            "route path must be bounded and repository relative",
        ));
    }
    Ok(())
}
fn canonical_selected_relative(value: &str) -> io::Result<String> {
    validate_relative(value)?;
    let mut components = Vec::with_capacity(128);
    for part in value.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if components.len() == 128 {
            return Err(invalid("selected target component bound"));
        }
        components.push(part);
    }
    Ok(if components.is_empty() {
        ".".into()
    } else {
        components.join("/")
    })
}
struct SourceFile {
    raw: Vec<u8>,
    text: String,
    digest: String,
    raw_lines: usize,
    raw_tokens: usize,
    text_lines: usize,
    text_tokens: usize,
}
/// Caller-owned shared read admission. Each physical read is permitted before
/// IO, then its actual returned bytes are recorded once, including EOF probes.
/// Hooks may narrow a caller's original budget and never grant source custody.
pub trait RouteSourceReadHooks {
    fn before_read(&mut self, requested_bytes: u64) -> io::Result<()>;
    fn read_returned(&mut self, actual_bytes: u64) -> io::Result<()>;
}
/// One operation's cached raw/text inputs and cumulative discovery/read deadline.
/// Raw cache is <=64MiB; normalized UTF-8 text occupies at most another64MiB.
/// Consumers copying bytes/text or constructing output must account for those copies.
pub struct RouteSources {
    root_dir: File,
    root_custody: Arc<()>,
    root_path: PathBuf,
    deadline: Instant,
    operations: Arc<AtomicUsize>,
    operation_limit: usize,
    root_component_opens: Cell<usize>,
    entries: usize,
    bytes_read: usize,
    files: BTreeMap<String, Arc<SourceFile>>,
}
/// Opaque identity of one held root. Related roots share an operation allowance
/// but receive distinct custody identities; this token never grants reads.
pub struct RouteRootCustody(Arc<()>);
/// A bounded resolution under this held root, optionally narrowed to one
/// explicitly selected directory. Outside targets are never opened.
pub enum RouteResolvedTarget {
    Inside {
        relative_path: String,
        metadata: Option<fs::Metadata>,
        topology_stamp: Digest256,
    },
    OutsideSelectedRoot,
}
impl RouteSources {
    pub fn new(root: &Path) -> io::Result<Self> {
        Self::new_until(root, Instant::now() + Duration::from_secs(30))
    }
    /// Join a concrete caller's whole operation clock without resetting it at
    /// capture or at a later read. Existing route callers retain their 30s clock.
    pub fn new_until(root: &Path, deadline: Instant) -> io::Result<Self> {
        Self::with_operations(
            root,
            deadline,
            Arc::new(AtomicUsize::new(0)),
            MAX_OPERATIONS,
        )
    }
    /// Create a held source root with a caller-priced finite operation ceiling.
    /// This does not extend the clock, source-byte limits, or custody scope.
    pub fn new_until_with_operation_limit(
        root: &Path,
        deadline: Instant,
        operation_limit: usize,
    ) -> io::Result<Self> {
        if operation_limit == 0 || operation_limit > MAX_BUDGETED_ROUTE_OPERATIONS {
            return Err(invalid("route operation limit outside finite ceiling"));
        }
        Self::with_operations(
            root,
            deadline,
            Arc::new(AtomicUsize::new(0)),
            operation_limit,
        )
    }
    /// Authenticate another explicitly selected root under the same operation
    /// allowance. This shares only the lookup counter, never root custody,
    /// cached bytes, or source authority. A related clock may only narrow.
    pub fn new_until_related(root: &Path, deadline: Instant, primary: &Self) -> io::Result<Self> {
        primary.check()?;
        if deadline > primary.deadline {
            return Err(invalid("related route root extends operation deadline"));
        }
        Self::with_operations(
            root,
            deadline,
            Arc::clone(&primary.operations),
            primary.operation_limit,
        )
    }
    fn with_operations(
        root: &Path,
        deadline: Instant,
        operations: Arc<AtomicUsize>,
        operation_limit: usize,
    ) -> io::Result<Self> {
        if operation_limit == 0
            || operation_limit > MAX_BUDGETED_ROUTE_OPERATIONS
            || operations.load(Ordering::Relaxed) >= operation_limit
        {
            return Err(invalid("route lookup operation bound exceeded"));
        }
        if Instant::now() >= deadline
            || root.as_os_str().len() > 4096
            || root.components().count() > 128
            || !root.is_absolute()
            || fs::canonicalize(root)? != root
            || !root.is_dir()
        {
            return Err(invalid("repository root must be absolute without symlinks"));
        }
        let root_component_opens = Cell::new(0);
        let root_dir = open_root(
            root,
            deadline,
            &operations,
            &root_component_opens,
            operation_limit,
        )?;
        Ok(Self {
            root_dir,
            root_custody: Arc::new(()),
            root_path: root.to_owned(),
            deadline,
            operations,
            operation_limit,
            root_component_opens,
            entries: 0,
            bytes_read: 0,
            files: BTreeMap::new(),
        })
    }
    /// Actual absolute-anchor and root-component opens, including initial custody and every recheck.
    /// These are distinct from relative lookup operations and must be included
    /// in the caller's whole IO/CPU envelope.
    pub fn root_component_open_count(&self) -> usize {
        self.root_component_opens.get()
    }
    /// Shared total across this root and all explicitly related roots.
    pub fn operation_count(&self) -> usize {
        self.operations.load(Ordering::Relaxed)
    }
    pub fn operation_limit(&self) -> usize {
        self.operation_limit
    }
    pub fn deadline(&self) -> Instant {
        self.deadline
    }
    /// Remaining time belongs to this same held-root operation.
    pub fn remaining_time(&self) -> io::Result<Duration> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| invalid("route operation deadline exceeded"))
    }
    /// Exact normalized path selected when this held root was authenticated.
    pub fn selected_root_path(&self) -> &Path {
        &self.root_path
    }
    pub fn root_custody(&self) -> RouteRootCustody {
        RouteRootCustody(Arc::clone(&self.root_custody))
    }
    /// Refuse substitution of another held root before accessing any operand.
    pub fn verify_custody(&self, selected: &RouteRootCustody) -> io::Result<()> {
        self.check()?;
        if !Arc::ptr_eq(&self.root_custody, &selected.0) {
            return Err(invalid("route held root custody differs"));
        }
        self.verify_root()
    }
    pub fn check(&self) -> io::Result<()> {
        if Instant::now() >= self.deadline {
            Err(invalid("route operation deadline exceeded"))
        } else {
            Ok(())
        }
    }
    #[cfg(target_os = "linux")]
    pub fn verify_root(&self) -> io::Result<()> {
        use std::os::unix::fs::MetadataExt;
        self.check()?;
        let retained = self.root_dir.metadata()?;
        let current = open_root(
            &self.root_path,
            self.deadline,
            &self.operations,
            &self.root_component_opens,
            self.operation_limit,
        )?
        .metadata()?;
        if (retained.dev(), retained.ino()) != (current.dev(), current.ino()) {
            return Err(invalid("route source root replaced"));
        }
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    pub fn verify_root(&self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "route root custody requires Linux",
        ))
    }
    fn charge(&mut self, value: &str) -> io::Result<()> {
        self.check()?;
        validate_relative(value)?;
        // Admit every relative component before the first open, sharing the
        // same operation cap with authenticated root rewalks.
        let count = Path::new(value)
            .components()
            .filter(|c| matches!(c, Component::Normal(_)))
            .count()
            .max(1);
        charge_operations(&self.operations, count, self.operation_limit)?;
        Ok(())
    }
    #[cfg(target_os = "linux")]
    fn open_child_directory(&mut self, parent: &File, name: &str) -> io::Result<File> {
        use std::os::fd::{AsRawFd, FromRawFd};
        self.check()?;
        if name.is_empty() || name.contains('/') || name.contains('\0') {
            return Err(invalid("route child directory name is invalid"));
        }
        charge_operations(&self.operations, 1, self.operation_limit)?;
        let name = std::ffi::CString::new(name.as_bytes())
            .map_err(|_| invalid("route child directory name is invalid"))?;
        let fd = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY
                    | libc::O_DIRECTORY
                    | libc::O_NOFOLLOW
                    | libc::O_CLOEXEC
                    | libc::O_NONBLOCK,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let child = unsafe { File::from_raw_fd(fd) };
        self.check()?;
        Ok(child)
    }
    #[cfg(not(target_os = "linux"))]
    fn open_child_directory(&mut self, _parent: &File, _name: &str) -> io::Result<File> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "route discovery requires Linux descriptor custody",
        ))
    }
    // Each component is opened relative to a retained parent descriptor. The
    // final nofollow/nonblock open prevents FIFO waits and symlink races.
    #[cfg(target_os = "linux")]
    fn open(&mut self, value: &str) -> io::Result<Option<File>> {
        self.open_with_owner(value, None)
    }
    #[cfg(target_os = "linux")]
    fn open_with_owner(&mut self, value: &str, owner: Option<u32>) -> io::Result<Option<File>> {
        self.open_with_mode(value, owner, false)
    }
    #[cfg(target_os = "linux")]
    fn open_with_mode(
        &mut self,
        value: &str,
        owner: Option<u32>,
        metadata_only: bool,
    ) -> io::Result<Option<File>> {
        use std::os::fd::{AsRawFd, FromRawFd};
        self.charge(value)?;
        let parts: Vec<_> = Path::new(value)
            .components()
            .filter_map(|c| match c {
                Component::Normal(s) => Some(s),
                _ => None,
            })
            .collect();
        let mut dir = self.root_dir.try_clone()?;
        if let Some(uid) = owner {
            use std::os::unix::fs::MetadataExt;
            let meta = dir.metadata()?;
            if meta.uid() != uid || meta.mode() & 0o022 != 0 {
                return Err(invalid("protected route root ownership/write boundary"));
            }
        }
        for (index, part) in parts.iter().enumerate() {
            self.check()?;
            use std::os::unix::ffi::OsStrExt;
            let name = std::ffi::CString::new(part.as_bytes()).map_err(|_| invalid("NUL path"))?;
            let final_component = index + 1 == parts.len();
            let flags = (if metadata_only && final_component {
                libc::O_PATH
            } else {
                libc::O_RDONLY
            }) | libc::O_NOFOLLOW
                | libc::O_CLOEXEC
                | libc::O_NONBLOCK
                | if index + 1 < parts.len() {
                    libc::O_DIRECTORY
                } else {
                    0
                };
            let fd = unsafe { libc::openat(dir.as_raw_fd(), name.as_ptr(), flags) };
            if fd < 0 {
                let e = io::Error::last_os_error();
                if e.kind() == io::ErrorKind::NotFound {
                    return Ok(None);
                }
                return Err(e);
            }
            let next = unsafe { File::from_raw_fd(fd) };
            let meta = next.metadata()?;
            if !(metadata_only && final_component) && !meta.is_file() && !meta.is_dir() {
                return Err(invalid("route input is not a regular file or directory"));
            }
            if let Some(uid) = owner {
                use std::os::unix::fs::MetadataExt;
                if meta.uid() != uid || meta.mode() & 0o022 != 0 {
                    return Err(invalid("protected route ownership/write boundary"));
                }
            }
            dir = next;
        }
        Ok(Some(dir))
    }
    #[cfg(not(target_os = "linux"))]
    fn open(&mut self, _value: &str) -> io::Result<Option<File>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "route reader requires Linux descriptor nofollow custody",
        ))
    }
    pub fn exists(&mut self, value: &str) -> io::Result<bool> {
        Ok(self.open(value)?.is_some())
    }
    pub fn is_file(&mut self, value: &str) -> io::Result<bool> {
        match self.open(value)? {
            Some(file) => Ok(file.metadata()?.is_file()),
            None => Ok(false),
        }
    }
    pub fn is_dir(&mut self, value: &str) -> io::Result<bool> {
        match self.open(value)? {
            Some(file) => Ok(file.metadata()?.is_dir()),
            None => Ok(false),
        }
    }
    /// Metadata from the same nofollow held descriptor route, without reading
    /// private contents merely to classify a selected inventory entry.
    pub fn metadata(&mut self, value: &str) -> io::Result<Option<fs::Metadata>> {
        self.verify_root()?;
        #[cfg(target_os = "linux")]
        let observed = self
            .open_with_mode(value, None, true)?
            .map(|file| file.metadata())
            .transpose()?;
        #[cfg(not(target_os = "linux"))]
        let observed = self.open(value)?.map(|file| file.metadata()).transpose()?;
        self.verify_root()?;
        self.check()?;
        Ok(observed)
    }
    #[cfg(target_os = "linux")]
    fn link_target_observation(
        &mut self,
        value: &str,
    ) -> io::Result<Option<(String, fs::Metadata)>> {
        use std::os::fd::AsRawFd;
        use std::os::unix::fs::MetadataExt;
        self.verify_root()?;
        let Some(link) = self.open_with_mode(value, None, true)? else {
            return Ok(None);
        };
        let before = link.metadata()?;
        if !before.is_symlink() {
            return Err(invalid("selected link operand is not a symlink"));
        }
        let mut raw = [0u8; 4097];
        let empty = c"";
        let count = unsafe {
            libc::readlinkat(
                link.as_raw_fd(),
                empty.as_ptr(),
                raw.as_mut_ptr().cast(),
                raw.len(),
            )
        };
        if count < 0 {
            return Err(io::Error::last_os_error());
        }
        let count = count as usize;
        if count == 0 || count > 4096 {
            return Err(invalid("selected link target byte bound"));
        }
        let target = std::str::from_utf8(&raw[..count])
            .map_err(|_| invalid("selected link target UTF-8"))?;
        let stamp = |m: &fs::Metadata| {
            (
                m.dev(),
                m.ino(),
                m.mode(),
                m.len(),
                m.nlink(),
                m.mtime(),
                m.mtime_nsec(),
                m.ctime(),
                m.ctime_nsec(),
            )
        };
        let current = self
            .open_with_mode(value, None, true)?
            .ok_or_else(|| invalid("selected link disappeared"))?;
        if stamp(&before) != stamp(&link.metadata()?)
            || stamp(&before) != stamp(&current.metadata()?)
        {
            return Err(invalid("selected link changed during observation"));
        }
        self.verify_root()?;
        self.check()?;
        Ok(Some((target.to_owned(), before)))
    }
    #[cfg(target_os = "linux")]
    pub fn resolve_selected_target(
        &mut self,
        value: &str,
        selected_directory: Option<&str>,
    ) -> io::Result<RouteResolvedTarget> {
        use std::os::unix::fs::MetadataExt;
        fn observed(hash: &mut Digest256Hasher, metadata: &fs::Metadata) {
            hash.update(&metadata.dev().to_be_bytes());
            hash.update(&metadata.ino().to_be_bytes());
            hash.update(&metadata.mode().to_be_bytes());
            hash.update(&metadata.len().to_be_bytes());
            hash.update(&metadata.nlink().to_be_bytes());
            hash.update(&metadata.mtime().to_be_bytes());
            hash.update(&metadata.mtime_nsec().to_be_bytes());
            hash.update(&metadata.ctime().to_be_bytes());
            hash.update(&metadata.ctime_nsec().to_be_bytes());
        }
        fn text(hash: &mut Digest256Hasher, value: &str) {
            hash.update(&(value.len() as u64).to_be_bytes());
            hash.update(value.as_bytes());
        }
        let value = canonical_selected_relative(value)?;
        let selected_directory_owned = selected_directory
            .map(canonical_selected_relative)
            .transpose()?;
        let selected_directory = selected_directory_owned.as_deref();
        let mut topology = Digest256Hasher::new();
        topology.update(b"tos-foundation-held-symlink-topology-v1\0");
        let root_metadata = self.root_dir.metadata()?;
        topology.update(&root_metadata.dev().to_be_bytes());
        topology.update(&root_metadata.ino().to_be_bytes());
        topology.update(&root_metadata.mode().to_be_bytes());
        text(&mut topology, &value);
        text(&mut topology, selected_directory.unwrap_or("."));
        if let Some(root) = selected_directory {
            if root != "."
                && value != root
                && !value
                    .strip_prefix(root)
                    .is_some_and(|tail| tail.starts_with('/'))
            {
                return Ok(RouteResolvedTarget::OutsideSelectedRoot);
            }
            let Some(metadata) = self.metadata(root)? else {
                // The held parent root proves this selected lane absent. No
                // target is followed; later physical recheck fences appearance.
                return Ok(RouteResolvedTarget::Inside {
                    relative_path: value,
                    metadata: None,
                    topology_stamp: topology.finalize(),
                });
            };
            if !metadata.is_dir() || metadata.is_symlink() {
                return Err(invalid("selected target directory custody"));
            }
            observed(&mut topology, &metadata);
        }
        let inside = |path: &str| {
            selected_directory.is_none_or(|root| {
                root == "."
                    || path == root
                    || path
                        .strip_prefix(root)
                        .is_some_and(|tail| tail.starts_with('/'))
            })
        };
        let mut candidate = value;
        let mut links = 0usize;
        loop {
            self.verify_root()?;
            if !inside(&candidate) {
                return Ok(RouteResolvedTarget::OutsideSelectedRoot);
            }
            match self.metadata(&candidate) {
                Ok(metadata) if metadata.as_ref().is_none_or(|m| !m.is_symlink()) => {
                    return Ok(RouteResolvedTarget::Inside {
                        relative_path: candidate,
                        metadata,
                        topology_stamp: topology.finalize(),
                    });
                }
                Ok(_) => (),
                Err(error)
                    if matches!(
                        error.raw_os_error(),
                        Some(libc::ELOOP) | Some(libc::ENOTDIR)
                    ) =>
                {
                    ()
                }
                Err(error) => return Err(error),
            }
            // Total component text <=4096, component slots <=128. Owning
            // these strings releases candidate borrows before replacement.
            let parts: Vec<_> = candidate.split('/').map(str::to_owned).collect();
            let mut prefix = String::with_capacity(candidate.len());
            let mut replaced = false;
            for (index, component) in parts.iter().enumerate() {
                if !prefix.is_empty() {
                    prefix.push('/');
                }
                prefix.push_str(component);
                let metadata = self.metadata(&prefix)?;
                let Some(metadata) = metadata else {
                    return Ok(RouteResolvedTarget::Inside {
                        relative_path: candidate,
                        metadata: None,
                        topology_stamp: topology.finalize(),
                    });
                };
                if !metadata.is_symlink() {
                    if index + 1 < parts.len() && !metadata.is_dir() {
                        return Ok(RouteResolvedTarget::Inside {
                            relative_path: candidate,
                            metadata: None,
                            topology_stamp: topology.finalize(),
                        });
                    }
                    continue;
                }
                links += 1;
                if links > 40 {
                    return Err(invalid("selected target symlink bound"));
                }
                let (target, link_metadata) = self
                    .link_target_observation(&prefix)?
                    .ok_or_else(|| invalid("selected target link disappeared"))?;
                let mut classified = Digest256Hasher::new();
                observed(&mut classified, &metadata);
                let mut selected_link = Digest256Hasher::new();
                observed(&mut selected_link, &link_metadata);
                if classified.finalize() != selected_link.finalize() {
                    return Err(invalid("selected target link changed before observation"));
                }
                topology.update(b"link\0");
                text(&mut topology, &prefix);
                observed(&mut topology, &link_metadata);
                text(&mut topology, &target);
                let mut replacement = String::with_capacity(12_288);
                if Path::new(&target).is_absolute() {
                    let Ok(relative) = Path::new(&target).strip_prefix(&self.root_path) else {
                        return Ok(RouteResolvedTarget::OutsideSelectedRoot);
                    };
                    replacement.push_str(
                        relative
                            .to_str()
                            .ok_or_else(|| invalid("selected target relative UTF-8"))?,
                    );
                } else {
                    if let Some((parent, _)) = prefix.rsplit_once('/') {
                        replacement.push_str(parent);
                        replacement.push('/');
                    }
                    replacement.push_str(&target);
                }
                for tail in &parts[index + 1..] {
                    replacement.push('/');
                    replacement.push_str(tail);
                }
                let mut normalized = Vec::with_capacity(128);
                for component in replacement.split('/') {
                    match component {
                        "" | "." => (),
                        ".." => {
                            if normalized.pop().is_none() {
                                return Ok(RouteResolvedTarget::OutsideSelectedRoot);
                            }
                        }
                        component => {
                            if normalized.len() == 128 {
                                return Err(invalid("selected target component bound"));
                            }
                            normalized.push(component);
                        }
                    }
                }
                let length = normalized
                    .iter()
                    .try_fold(0usize, |n, part| n.checked_add(part.len() + 1))
                    .ok_or_else(|| invalid("selected target length overflow"))?;
                if length > 4097 {
                    return Err(invalid("selected target normalized bound"));
                }
                candidate = if normalized.is_empty() {
                    ".".into()
                } else {
                    normalized.join("/")
                };
                replaced = true;
                break;
            }
            if !replaced {
                return Err(invalid("selected target resolution changed"));
            }
        }
    }
    /// Maintained foundation Git posture, before any schema child or stage
    /// writer is started. This reuses the dedicated bounded process custody;
    /// no writable stage/output descriptor may be inherited by the child.
    /// It makes no claim of arbitrary same-UID ptrace protection for Git.
    pub fn foundation_git_path_facts(
        &mut self,
        value: &str,
        output_bytes: usize,
        cleanup_grace: Duration,
        cancelled: &AtomicI32,
    ) -> io::Result<(Option<bool>, Option<bool>)> {
        self.charge(value)?;
        self.verify_root()?;
        if output_bytes == 0 || output_bytes > 65_536 {
            return Err(invalid("foundation Git output bound"));
        }
        #[cfg(target_os = "linux")]
        {
            // A regular writable descriptor without CLOEXEC could expose a
            // candidate/output inode. Standard streams are replaced by the
            // existing executor's bounded capture pipes before exec.
            let mut count = 0usize;
            for entry in fs::read_dir("/proc/self/fd")? {
                count += 1;
                if count > 1024 {
                    return Err(invalid("foundation Git FD census bound"));
                }
                let entry = entry?;
                let Some(fd) = entry
                    .file_name()
                    .to_str()
                    .and_then(|s| s.parse::<i32>().ok())
                else {
                    return Err(invalid("foundation Git FD census shape"));
                };
                if fd <= 2 {
                    continue;
                }
                let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
                if flags < 0 {
                    continue;
                } // census directory may just have closed
                let access = unsafe { libc::fcntl(fd, libc::F_GETFL) };
                if access < 0 {
                    return Err(io::Error::last_os_error());
                }
                if flags & libc::FD_CLOEXEC == 0
                    && access & libc::O_ACCMODE != libc::O_RDONLY
                    && fs::metadata(entry.path())?.is_file()
                {
                    return Err(invalid("foundation Git inherited writable regular FD"));
                }
            }
            let mut status = Vec::new();
            File::open("/proc/self/status")?
                .take(65_537)
                .read_to_end(&mut status)?;
            if status.len() > 65_536 {
                return Err(invalid("foundation Git status bound"));
            }
            let status =
                std::str::from_utf8(&status).map_err(|_| invalid("foundation Git status shape"))?;
            for name in ["CapInh:", "CapPrm:", "CapEff:", "CapBnd:", "CapAmb:"] {
                let raw = status
                    .lines()
                    .find_map(|line| line.strip_prefix(name))
                    .ok_or_else(|| invalid("foundation Git capability field missing"))?
                    .trim();
                if u64::from_str_radix(raw, 16).ok() != Some(0) {
                    return Err(invalid("foundation Git inherited writer capabilities"));
                }
            }
        }
        if !self.exists(".git")? {
            return Ok((None, None));
        }
        let mut results = [None, None];
        for (index, command) in ["ls-files", "check-ignore"].into_iter().enumerate() {
            self.check()?;
            let remaining = self.deadline.saturating_duration_since(Instant::now());
            if remaining <= cleanup_grace {
                return Err(invalid("foundation Git deadline"));
            }
            let wall = remaining - cleanup_grace;
            // Use the same child-local trace suppression as the existing
            // semantic-registry Git gate; parent environment is unchanged.
            let mut argv = vec!["/usr/bin/env".into()];
            let mut trace_name_bytes = 0usize;
            for (key, _) in std::env::vars_os() {
                if key.as_encoded_bytes().starts_with(b"GIT_TRACE") {
                    trace_name_bytes = trace_name_bytes
                        .checked_add(key.as_encoded_bytes().len())
                        .filter(|n| *n <= 4096)
                        .ok_or_else(|| invalid("foundation Git trace-name bound"))?;
                    if argv.len() > 256 {
                        return Err(invalid("foundation Git trace-count bound"));
                    }
                    argv.push("-u".into());
                    argv.push(
                        key.into_string()
                            .map_err(|_| invalid("foundation Git trace-name shape"))?,
                    );
                }
            }
            argv.extend([
                "--".into(),
                "GIT_NO_LAZY_FETCH=1".into(),
                "GIT_OPTIONAL_LOCKS=0".into(),
                "/usr/bin/git".into(),
                "--no-pager".into(),
                "--no-optional-locks".into(),
                "--no-replace-objects".into(),
                "--literal-pathspecs".into(),
                "-c".into(),
                "core.fsmonitor=false".into(),
                "-c".into(),
                "core.untrackedCache=false".into(),
                "-c".into(),
                "core.hooksPath=/dev/null".into(),
                "-c".into(),
                "trace2.eventTarget=0".into(),
                "-c".into(),
                "trace2.perfTarget=0".into(),
                "-c".into(),
                "trace2.normalTarget=0".into(),
                command.into(),
                if index == 0 {
                    "--error-unmatch"
                } else {
                    "--quiet"
                }
                .into(),
                "--".into(),
                value.into(),
            ]);
            let (code, _, _) = executor::capture_ci_git(
                &self.root_path,
                argv,
                Limits {
                    command_wall: wall,
                    lane_wall: wall,
                    cleanup_grace,
                    output_bytes,
                },
                cancelled,
            )?;
            results[index] = match code {
                0 => Some(true),
                1 => Some(false),
                _ => None,
            };
            self.verify_root()?;
            self.check()?;
        }
        Ok((results[0], results[1]))
    }
    /// Uncached physical operand read. The callback receives only a fixed
    /// chunk; the caller owns hash algorithms, cancellation and aggregate cost.
    /// Absence, directories and symlinks are not regular payload observations.
    pub fn stream_regular(
        &mut self,
        value: &str,
        max_file_bytes: u64,
        read_bytes: &mut u64,
        max_total_bytes: u64,
        consume: impl FnMut(&[u8]) -> io::Result<()>,
    ) -> io::Result<Option<fs::Metadata>> {
        self.stream_regular_inner(
            value,
            max_file_bytes,
            read_bytes,
            max_total_bytes,
            None,
            consume,
        )
    }
    /// Same held uncached reader, joined to the caller's original shared IO ledger.
    pub fn stream_regular_with_hooks(
        &mut self,
        value: &str,
        max_file_bytes: u64,
        read_bytes: &mut u64,
        max_total_bytes: u64,
        hooks: &mut dyn RouteSourceReadHooks,
        consume: impl FnMut(&[u8]) -> io::Result<()>,
    ) -> io::Result<Option<fs::Metadata>> {
        self.stream_regular_inner(
            value,
            max_file_bytes,
            read_bytes,
            max_total_bytes,
            Some(hooks),
            consume,
        )
    }
    fn stream_regular_inner(
        &mut self,
        value: &str,
        max_file_bytes: u64,
        read_bytes: &mut u64,
        max_total_bytes: u64,
        mut hooks: Option<&mut dyn RouteSourceReadHooks>,
        mut consume: impl FnMut(&[u8]) -> io::Result<()>,
    ) -> io::Result<Option<fs::Metadata>> {
        self.verify_root()?;
        let mut file = match self.open(value) {
            Ok(Some(file)) => file,
            Ok(None) => return Ok(None),
            #[cfg(target_os = "linux")]
            Err(error) if error.raw_os_error() == Some(libc::ELOOP) => return Ok(None),
            Err(error) => return Err(error),
        };
        let before = file.metadata()?;
        if !before.is_file() {
            return Ok(None);
        }
        let remaining = max_total_bytes
            .checked_sub(*read_bytes)
            .ok_or_else(|| invalid("physical operand aggregate accounting exceeded"))?;
        if before.len() > max_file_bytes.min(remaining) {
            return Err(invalid("physical operand byte bound exceeded"));
        }
        let mut buffer = [0u8; 32 * 1024];
        let mut observed = 0u64;
        loop {
            self.check()?;
            let requested = (before.len() - observed).max(1).min(buffer.len() as u64) as usize;
            if let Some(hooks) = hooks.as_deref_mut() {
                hooks.before_read(requested as u64)?;
            }
            let count = match file.read(&mut buffer[..requested]) {
                Ok(count) => count,
                Err(error) => {
                    if let Some(hooks) = hooks.as_deref_mut() {
                        hooks.read_returned(0)?;
                    }
                    return Err(error);
                }
            };
            *read_bytes = read_bytes
                .checked_add(count as u64)
                .ok_or_else(|| invalid("physical operand accounting overflow"))?;
            if let Some(hooks) = hooks.as_deref_mut() {
                hooks.read_returned(count as u64)?;
            }
            self.check()?;
            if count == 0 {
                break;
            }
            observed = observed
                .checked_add(count as u64)
                .filter(|n| *n <= before.len())
                .ok_or_else(|| invalid("physical operand grew during read"))?;
            consume(&buffer[..count])?;
        }
        if observed != before.len() {
            return Err(invalid("physical operand changed during read"));
        }
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::MetadataExt;
            let stamp = |m: &fs::Metadata| {
                (
                    m.dev(),
                    m.ino(),
                    m.len(),
                    m.mode(),
                    m.mtime(),
                    m.mtime_nsec(),
                    m.ctime(),
                    m.ctime_nsec(),
                )
            };
            let current = self
                .open(value)?
                .ok_or_else(|| invalid("physical operand disappeared during read"))?;
            if stamp(&before) != stamp(&file.metadata()?)
                || stamp(&before) != stamp(&current.metadata()?)
            {
                return Err(invalid("physical operand replaced during read"));
            }
        }
        self.verify_root()?;
        self.check()?;
        Ok(Some(before))
    }
    fn source(&mut self, value: &str) -> io::Result<Option<Arc<SourceFile>>> {
        self.charge(value)?;
        if let Some(file) = self.files.get(value) {
            return Ok(Some(file.clone()));
        }
        let Some(file) = self.open(value)? else {
            return Ok(None);
        };
        let meta = file.metadata()?;
        if !meta.is_file() {
            return Ok(None);
        }
        if self.files.len() >= MAX_ENTRIES {
            return Err(invalid("route input file count exceeded"));
        }
        let cap = MAX_FILE.min(MAX_INPUT - self.bytes_read);
        if meta.len() > cap as u64 {
            return Err(invalid("route input byte bound exceeded"));
        }
        let mut raw = Vec::with_capacity(meta.len() as usize);
        file.take(cap as u64 + 1).read_to_end(&mut raw)?;
        if raw.len() > cap || raw.len() as u64 != meta.len() {
            return Err(invalid("route input changed or exceeded byte bound"));
        }
        self.bytes_read += raw.len();
        let text = std::str::from_utf8(&raw)
            .map_err(|_| invalid("route input is not UTF-8"))?
            .replace("\r\n", "\n")
            .replace('\r', "\n");
        if value.ends_with("AGENTS.md")
            && (line_count(&text) > 65_536
                || text.split(is_line_separator).any(|line| line.len() > 8192))
        {
            return Err(invalid("route card line byte bound exceeded"));
        }
        let raw_text = std::str::from_utf8(&raw).map_err(io::Error::other)?;
        let raw_lines = line_count(raw_text);
        let raw_tokens = whitespace_tokens(raw_text);
        let text_lines = line_count(&text);
        let text_tokens = whitespace_tokens(&text);
        let digest = sha256_bytes(&raw);
        let file = Arc::new(SourceFile {
            raw,
            text,
            digest,
            raw_lines,
            raw_tokens,
            text_lines,
            text_tokens,
        });
        self.files.insert(value.into(), file.clone());
        Ok(Some(file))
    }
    pub fn text(&mut self, value: &str) -> io::Result<Option<String>> {
        Ok(self.source(value)?.map(|v| v.text.clone()))
    }
    pub fn bytes(&mut self, value: &str) -> io::Result<Vec<u8>> {
        self.source(value)?
            .map(|v| v.raw.clone())
            .ok_or_else(|| invalid(format!("missing route file: {value}")))
    }
    /// Byte-only read for an explicit consumer-owned operand profile. This
    /// reuses descriptor custody without admitting bytes into the route text
    /// cache. The caller owns the local aggregate budget and operand fixity.
    pub fn bounded_bytes(
        &mut self,
        value: &str,
        max_file_bytes: usize,
        read_bytes: &mut usize,
        max_total_bytes: usize,
    ) -> io::Result<Vec<u8>> {
        self.bounded_metadata_bytes(value, max_file_bytes, read_bytes, max_total_bytes)
            .map(|(raw, _)| raw)
    }
    /// Exact file metadata belongs to the same uncached descriptor read as the
    /// returned bytes. Snapshot capture must not invent a portable source mode.
    pub fn bounded_metadata_bytes(
        &mut self,
        value: &str,
        max_file_bytes: usize,
        read_bytes: &mut usize,
        max_total_bytes: usize,
    ) -> io::Result<(Vec<u8>, fs::Metadata)> {
        self.bounded_metadata_bytes_inner(value, max_file_bytes, read_bytes, max_total_bytes, None)
            .map(|(raw, metadata, _held)| (raw, metadata))
    }
    /// Exact mode from the existing stable held-descriptor operand read.
    pub fn bounded_bytes_with_mode(
        &mut self,
        value: &str,
        max_file_bytes: usize,
        read_bytes: &mut usize,
        max_total_bytes: usize,
    ) -> io::Result<(Vec<u8>, u32)> {
        let (raw, metadata) =
            self.bounded_metadata_bytes(value, max_file_bytes, read_bytes, max_total_bytes)?;
        #[cfg(unix)]
        let mode = {
            use std::os::unix::fs::MetadataExt;
            metadata.mode()
        };
        #[cfg(not(unix))]
        let mode = 0;
        Ok((raw, mode))
    }
    /// Descriptor bytes and metadata with pre-IO shared permits and actual returns.
    pub fn bounded_metadata_bytes_with_hooks(
        &mut self,
        value: &str,
        max_file_bytes: usize,
        read_bytes: &mut usize,
        max_total_bytes: usize,
        hooks: &mut dyn RouteSourceReadHooks,
    ) -> io::Result<(Vec<u8>, fs::Metadata)> {
        self.bounded_metadata_bytes_inner(
            value,
            max_file_bytes,
            read_bytes,
            max_total_bytes,
            Some(hooks),
        )
        .map(|(raw, metadata, _held)| (raw, metadata))
    }
    /// Same bounded reader with the observed descriptor retained for downstream custody.
    pub(crate) fn bounded_held_bytes(
        &mut self,
        value: &str,
        max_file_bytes: usize,
        read_bytes: &mut usize,
        max_total_bytes: usize,
    ) -> io::Result<(Vec<u8>, fs::Metadata, File)> {
        self.bounded_metadata_bytes_inner(value, max_file_bytes, read_bytes, max_total_bytes, None)
    }
    fn bounded_metadata_bytes_inner(
        &mut self,
        value: &str,
        max_file_bytes: usize,
        read_bytes: &mut usize,
        max_total_bytes: usize,
        mut hooks: Option<&mut dyn RouteSourceReadHooks>,
    ) -> io::Result<(Vec<u8>, fs::Metadata, File)> {
        self.verify_root()?;
        let remaining = max_total_bytes
            .checked_sub(*read_bytes)
            .ok_or_else(|| invalid("operand aggregate byte accounting exceeded"))?;
        let cap = max_file_bytes.min(remaining);
        let file = self
            .open(value)?
            .ok_or_else(|| invalid(format!("missing operand file: {value}")))?;
        let meta = file.metadata()?;
        if !meta.is_file() {
            return Err(invalid("operand input is not a regular file"));
        }
        if meta.len() > cap as u64 {
            return Err(invalid("operand input byte bound exceeded"));
        }
        // Fixed metadata-sized storage and a one-byte EOF probe avoid a Vec
        // growth copy when a large operand exactly fills the admitted buffer.
        let mut raw = vec![0; meta.len() as usize];
        let mut file = file;
        let mut observed = 0;
        while observed < raw.len() {
            self.check()?;
            let requested = (raw.len() - observed).min(32 * 1024);
            if let Some(hooks) = hooks.as_deref_mut() {
                hooks.before_read(requested as u64)?;
            }
            let count = match file.read(&mut raw[observed..observed + requested]) {
                Ok(count) => count,
                Err(error) => {
                    if let Some(hooks) = hooks.as_deref_mut() {
                        hooks.read_returned(0)?;
                    }
                    return Err(error);
                }
            };
            *read_bytes = read_bytes
                .checked_add(count)
                .ok_or_else(|| invalid("operand aggregate byte accounting overflow"))?;
            if let Some(hooks) = hooks.as_deref_mut() {
                hooks.read_returned(count as u64)?;
            }
            self.check()?;
            if count == 0 {
                return Err(invalid("operand input shortened during read"));
            }
            observed += count;
        }
        self.check()?;
        if let Some(hooks) = hooks.as_deref_mut() {
            hooks.before_read(1)?;
        }
        let count = match file.read(&mut [0; 1]) {
            Ok(count) => count,
            Err(error) => {
                if let Some(hooks) = hooks.as_deref_mut() {
                    hooks.read_returned(0)?;
                }
                return Err(error);
            }
        };
        *read_bytes = read_bytes
            .checked_add(count)
            .ok_or_else(|| invalid("operand aggregate byte accounting overflow"))?;
        if let Some(hooks) = hooks.as_deref_mut() {
            hooks.read_returned(count as u64)?;
        }
        self.check()?;
        if count != 0 {
            return Err(invalid("operand input changed or exceeded byte bound"));
        }
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::MetadataExt;
            let stamp = |m: &fs::Metadata| {
                (
                    m.dev(),
                    m.ino(),
                    m.len(),
                    m.mode(),
                    m.mtime(),
                    m.mtime_nsec(),
                    m.ctime(),
                    m.ctime_nsec(),
                )
            };
            let current = self
                .open(value)?
                .ok_or_else(|| invalid("operand input disappeared during read"))?;
            if stamp(&meta) != stamp(&file.metadata()?)
                || stamp(&meta) != stamp(&current.metadata()?)
            {
                return Err(invalid("operand input changed or replaced during read"));
            }
        }
        self.verify_root()?;
        self.check()?;
        Ok((raw, meta, file))
    }
    /// Re-select a participating publication control on every call. This
    /// supplies physical custody only; the source-store epoch codec owns its
    /// shape/token and the caller owns before/after selection. No cache hit is
    /// permitted, including for a control first observed absent.
    #[cfg(target_os = "linux")]
    pub fn protected_control_bytes(&mut self, value: &str) -> io::Result<Option<Vec<u8>>> {
        use std::os::unix::fs::MetadataExt;
        fn stamp(m: &fs::Metadata) -> (u64, u64, u64, u32, i64, i64, i64, i64) {
            (
                m.dev(),
                m.ino(),
                m.len(),
                m.mode(),
                m.mtime(),
                m.mtime_nsec(),
                m.ctime(),
                m.ctime_nsec(),
            )
        }
        self.check()?;
        let root = open_root(
            &self.root_path,
            self.deadline,
            &self.operations,
            &self.root_component_opens,
            self.operation_limit,
        )?;
        let retained = self.root_dir.metadata()?;
        let current = root.metadata()?;
        if (retained.dev(), retained.ino()) != (current.dev(), current.ino()) {
            return Err(invalid("protected route root replaced"));
        }
        let uid = unsafe { libc::geteuid() };
        let Some(mut file) = self.open_with_owner(value, Some(uid))? else {
            self.verify_root()?;
            return Ok(None);
        };
        let before = file.metadata()?;
        if !before.is_file()
            || before.mode() & 0o7000 != 0
            || !matches!(before.mode() & 0o777, 0o600 | 0o644)
            || before.len() > 8192
        {
            return Err(invalid(
                "protected publication control type/mode/byte boundary",
            ));
        }
        let size = before.len() as usize;
        self.bytes_read = self
            .bytes_read
            .checked_add(size)
            .filter(|n| *n <= MAX_INPUT)
            .ok_or_else(|| invalid("protected control cumulative read bound"))?;
        let mut raw = vec![0; size];
        file.read_exact(&mut raw)?;
        if file.read(&mut [0; 1])? != 0 || stamp(&before) != stamp(&file.metadata()?) {
            return Err(invalid("protected publication control changed during read"));
        }
        let again = self
            .open_with_owner(value, Some(uid))?
            .ok_or_else(|| invalid("protected publication control disappeared"))?;
        if stamp(&before) != stamp(&again.metadata()?) {
            return Err(invalid(
                "protected publication control replaced during read",
            ));
        }
        let root = open_root(
            &self.root_path,
            self.deadline,
            &self.operations,
            &self.root_component_opens,
            self.operation_limit,
        )?
        .metadata()?;
        if (retained.dev(), retained.ino()) != (root.dev(), root.ino()) {
            return Err(invalid("protected route root replaced during read"));
        }
        self.check()?;
        Ok(Some(raw))
    }
    #[cfg(not(target_os = "linux"))]
    pub fn protected_control_bytes(&mut self, _: &str) -> io::Result<Option<Vec<u8>>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "protected control requires Linux descriptor custody",
        ))
    }
    #[allow(clippy::too_many_arguments)]
    fn walk(
        &mut self,
        rel: &str,
        paths: &mut BTreeSet<String>,
        all_descendants: bool,
        files_only: bool,
        max_selected_files: usize,
        selected: &dyn Fn(&str, bool) -> bool,
        include_links: bool,
        max_entries: usize,
        max_relative_path_bytes: usize,
    ) -> io::Result<()> {
        let Some(dir) = self.open(rel)? else {
            return Ok(());
        };
        if !dir.metadata()?.is_dir() {
            return Ok(());
        }
        let mut local_entries = 0usize;
        self.walk_held(
            &dir,
            rel,
            paths,
            &mut local_entries,
            all_descendants,
            files_only,
            max_selected_files,
            true,
            selected,
            include_links,
            max_entries,
            max_relative_path_bytes,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn walk_held(
        &mut self,
        dir: &File,
        rel: &str,
        paths: &mut BTreeSet<String>,
        local_entries: &mut usize,
        all_descendants: bool,
        files_only: bool,
        max_selected_files: usize,
        cumulative_entry_limit: bool,
        selected: &dyn Fn(&str, bool) -> bool,
        include_links: bool,
        max_entries: usize,
        max_relative_path_bytes: usize,
    ) -> io::Result<()> {
        // /proc/self/fd resolves our retained directory, not an authored pathname.
        #[cfg(target_os = "linux")]
        let path = {
            use std::os::fd::AsRawFd;
            PathBuf::from(format!("/proc/self/fd/{}", dir.as_raw_fd()))
        };
        #[cfg(not(target_os = "linux"))]
        let path: PathBuf = return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "route discovery requires Linux descriptor custody",
        ));
        let mut children = Vec::new();
        for entry in fs::read_dir(path)? {
            self.check()?;
            self.entries = self
                .entries
                .checked_add(1)
                .ok_or_else(|| invalid("route discovery entry count overflow"))?;
            *local_entries = (*local_entries)
                .checked_add(1)
                .ok_or_else(|| invalid("route discovery local entry count overflow"))?;
            if (cumulative_entry_limit && self.entries > max_entries)
                || (!cumulative_entry_limit && *local_entries > max_entries)
            {
                return Err(invalid("route discovery entry bound exceeded"));
            }
            let entry = entry?;
            let kind = entry.file_type()?;
            let component = entry
                .file_name()
                .into_string()
                .map_err(|_| invalid("non-UTF-8 route path"))?;
            let name = format!("{rel}/{component}");
            if name.len() > max_relative_path_bytes {
                return Err(invalid("route discovery path byte bound exceeded"));
            }
            validate_relative(&name)?;
            if !selected(&name, kind.is_dir()) {
                continue;
            }
            if kind.is_symlink() && !include_links {
                return Err(invalid("route discovery refuses symlinks"));
            }
            children.push((component, name, kind.is_dir(), kind.is_file()));
        }
        children.sort_by(|a, b| a.1.cmp(&b.1));
        for (component, name, directory, file) in children {
            if all_descendants && (!files_only || file) {
                if files_only && paths.len() >= max_selected_files {
                    return Err(invalid("selected route file count bound exceeded"));
                }
                paths.insert(name.clone());
            }
            if directory {
                let child = self.open_child_directory(dir, &component)?;
                self.walk_held(
                    &child,
                    &name,
                    paths,
                    local_entries,
                    all_descendants,
                    files_only,
                    max_selected_files,
                    cumulative_entry_limit,
                    selected,
                    include_links,
                    max_entries,
                    max_relative_path_bytes,
                )?;
            } else if files_only && !file {
                return Err(invalid("selected route entry is not a regular file"));
            } else if !all_descendants
                && file
                && Path::new(&name)
                    .file_name()
                    .is_some_and(|n| n == "AGENTS.md")
            {
                paths.insert(name);
            }
        }
        Ok(())
    }
    /// Sorted descendants of one held directory, including files and directories.
    /// Enumeration reads no payload bytes and uses the same custody and limits as
    /// route-card discovery; the supplied directory itself is excluded.
    pub fn paths(&mut self, relative: &str) -> io::Result<Vec<String>> {
        let mut paths = BTreeSet::new();
        self.walk(
            relative,
            &mut paths,
            true,
            false,
            usize::MAX,
            &|_, _| true,
            false,
            MAX_ENTRIES,
            MAX_RELATIVE_PATH_BYTES,
        )?;
        Ok(paths.into_iter().collect())
    }
    /// Conservative source-state reservation for selected_paths(), including
    /// changed-current names. Derived from the same traversal count/path/depth
    /// laws; performs no I/O/allocation and grants no quota or RSS guarantee.
    /// Linux walk charges each entry before allocating its name. It retains
    /// child vectors and cloned tree keys before consuming keys into a Vec.
    pub fn selected_paths_workspace_upper_bound_bytes() -> io::Result<usize> {
        Self::selected_paths_workspace_upper_bound_bytes_with_limits(
            MAX_ENTRIES,
            MAX_RELATIVE_PATH_BYTES,
        )
    }
    /// Conservative traversal workspace for an explicit finite discovery
    /// ceiling. Wider callers must retain this reservation in their own
    /// operation profile; it is not an estimate of observed allocation.
    pub fn selected_paths_workspace_upper_bound_bytes_with_limits(
        max_entries: usize,
        max_relative_path_bytes: usize,
    ) -> io::Result<usize> {
        if max_entries == 0
            || max_entries > MAX_SELECTED_PATH_DISCOVERY_ENTRIES
            || max_relative_path_bytes == 0
            || max_relative_path_bytes > MAX_RELATIVE_PATH_BYTES
        {
            return Err(invalid(
                "selected route traversal limit outside finite ceiling",
            ));
        }
        let word = std::mem::size_of::<usize>();
        let string = std::mem::size_of::<String>();
        let child = std::mem::size_of::<(String, String, bool, bool)>();
        // Retained names already passed the actual full relative path bound.
        // One format buffer plus one cloned tree key; Vec capacity reserves
        // twice live entries. Tree/iterator bookkeeping uses 64 words/name.
        let entry = max_relative_path_bytes
            .checked_mul(3)
            .and_then(|n| n.checked_add(child.checked_mul(2)?))
            .and_then(|n| n.checked_add(string.checked_mul(4)?))
            .and_then(|n| n.checked_add(word.checked_mul(64)?));
        let frames = max_relative_path_bytes
            .checked_mul(2)
            .and_then(|n| n.checked_add(word.checked_mul(64)?))
            .and_then(|n| n.checked_mul(MAX_RELATIVE_COMPONENTS));
        // A rejected overlong next filename is transient, never retained for
        // every entry. Linux getdents64 d_reclen is u16; d_name fits that record.
        // Reserve its name buffer and formatting alongside the existing prefix.
        let failed_entry = (u16::MAX as usize).checked_mul(2).and_then(|n| {
            let formatted = max_relative_path_bytes
                .checked_add(u16::MAX as usize)?
                .checked_add(1)?
                .checked_mul(2)?;
            n.checked_add(formatted)
        });
        entry
            .and_then(|n| n.checked_mul(max_entries))
            .and_then(|n| n.checked_add(frames?))
            .and_then(|n| n.checked_add(failed_entry?))
            .and_then(|n| n.checked_add(8192))
            .ok_or_else(|| invalid("selected route traversal workspace overflow"))
    }
    /// Caller-owned eligibility prunes directories before traversal and files
    /// before opening. It conveys no source membership or read authority.
    pub fn selected_paths(
        &mut self,
        relative: &str,
        selected: &dyn Fn(&str, bool) -> bool,
    ) -> io::Result<Vec<String>> {
        self.selected_paths_with_limits(relative, selected, MAX_ENTRIES, MAX_RELATIVE_PATH_BYTES)
    }
    /// Caller-owned wider discovery for a separately budgeted whole-source
    /// operation. This does not change the ordinary selected-path ceiling.
    pub fn selected_paths_with_limits(
        &mut self,
        relative: &str,
        selected: &dyn Fn(&str, bool) -> bool,
        max_entries: usize,
        max_relative_path_bytes: usize,
    ) -> io::Result<Vec<String>> {
        if max_entries == 0
            || max_entries > MAX_SELECTED_PATH_DISCOVERY_ENTRIES
            || max_relative_path_bytes == 0
            || max_relative_path_bytes > MAX_RELATIVE_PATH_BYTES
        {
            return Err(invalid(
                "selected route traversal limit outside finite ceiling",
            ));
        }
        if !selected(relative, true) {
            return Err(invalid("selected discovery root is outside caller scope"));
        }
        let mut paths = BTreeSet::new();
        self.walk(
            relative,
            &mut paths,
            true,
            false,
            usize::MAX,
            selected,
            false,
            max_entries,
            max_relative_path_bytes,
        )?;
        Ok(paths.into_iter().collect())
    }
    /// Sorted selected regular files under a held root. The bounded walk
    /// retains only file paths, opens child directories relative to held parent
    /// descriptors, and refuses selected special files and symlinks.
    pub fn selected_files_with_limits(
        &mut self,
        relative: &str,
        selected: &dyn Fn(&str, bool) -> bool,
        max_entries: usize,
        max_relative_path_bytes: usize,
        max_selected_files: usize,
    ) -> io::Result<Vec<String>> {
        if max_entries == 0
            || max_entries > MAX_SELECTED_PATH_DISCOVERY_ENTRIES
            || max_relative_path_bytes == 0
            || max_relative_path_bytes > MAX_RELATIVE_PATH_BYTES
            || max_selected_files == 0
        {
            return Err(invalid(
                "selected file discovery limit outside finite ceiling",
            ));
        }
        if !selected(relative, true) {
            return Err(invalid("selected discovery root is outside caller scope"));
        }
        let mut paths = BTreeSet::new();
        let Some(dir) = self.open(relative)? else {
            return Ok(Vec::new());
        };
        if dir.metadata()?.is_dir() {
            let mut local_entries = 0usize;
            self.walk_held(
                &dir,
                relative,
                &mut paths,
                &mut local_entries,
                true,
                true,
                max_selected_files,
                false,
                selected,
                false,
                max_entries,
                max_relative_path_bytes,
            )?;
        }
        Ok(paths.into_iter().collect())
    }
    /// Explicit physical inventory. Symlink names are included without
    /// following their targets or recursing into symlink directories; callers
    /// classify each selected inode through `metadata` and optionally resolve
    /// it under their separately selected directory. Existing authored/card
    /// discovery continues to reject symlinks.
    pub fn selected_physical_paths(
        &mut self,
        relative: &str,
        selected: &dyn Fn(&str, bool) -> bool,
    ) -> io::Result<Vec<String>> {
        if !selected(relative, true) {
            return Err(invalid("physical discovery root is outside caller scope"));
        }
        self.verify_root()?;
        let mut paths = BTreeSet::new();
        self.walk(
            relative,
            &mut paths,
            true,
            false,
            usize::MAX,
            selected,
            true,
            MAX_ENTRIES,
            MAX_RELATIVE_PATH_BYTES,
        )?;
        self.verify_root()?;
        Ok(paths.into_iter().collect())
    }
}
pub fn write_output(root: &Path, path: &Path, rendered: &str) -> io::Result<()> {
    if rendered.len() > MAX_OUTPUT {
        return Err(invalid("route output byte bound exceeded"));
    }
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        root.join(path)
    };
    if path.as_os_str().len() > 4096
        || path
            .components()
            .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
    {
        return Err(invalid("invalid route output path"));
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::{AsRawFd, FromRawFd};
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::MetadataExt;
        let parts: Vec<_> = path
            .components()
            .filter_map(|c| {
                if let Component::Normal(s) = c {
                    Some(s)
                } else {
                    None
                }
            })
            .collect();
        if parts.is_empty() || parts.len() > 128 {
            return Err(invalid("invalid route output depth"));
        }
        let mut dir = File::open("/")?;
        for part in &parts[..parts.len() - 1] {
            let name =
                std::ffi::CString::new(part.as_bytes()).map_err(|_| invalid("NUL output path"))?;
            let mut fd = unsafe {
                libc::openat(
                    dir.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 && io::Error::last_os_error().kind() == io::ErrorKind::NotFound {
                if unsafe { libc::mkdirat(dir.as_raw_fd(), name.as_ptr(), 0o755) } < 0
                    && io::Error::last_os_error().kind() != io::ErrorKind::AlreadyExists
                {
                    return Err(io::Error::last_os_error());
                }
                fd = unsafe {
                    libc::openat(
                        dir.as_raw_fd(),
                        name.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )
                };
            }
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            dir = unsafe { File::from_raw_fd(fd) };
        }
        let name = std::ffi::CString::new(parts.last().unwrap().as_bytes())
            .map_err(|_| invalid("NUL output path"))?;
        let fd = unsafe {
            libc::openat(
                dir.as_raw_fd(),
                name.as_ptr(),
                libc::O_WRONLY
                    | libc::O_CREAT
                    | libc::O_NOFOLLOW
                    | libc::O_NONBLOCK
                    | libc::O_CLOEXEC,
                0o644,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut file = unsafe { File::from_raw_fd(fd) };
        let meta = file.metadata()?;
        if !meta.is_file() || meta.len() > MAX_OUTPUT as u64 || meta.nlink() > 1 {
            return Err(invalid(
                "route output must be a bounded unshared regular file",
            ));
        }
        file.set_len(0)?;
        file.write_all(rendered.as_bytes())?;
        file.flush()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "route output requires Linux descriptor custody",
        ))
    }
}
/// Read an explicit external/relative output without following any component.
pub fn read_output(path: &Path) -> io::Result<Option<String>> {
    if !path.is_absolute() {
        return Err(invalid("output read requires absolute path"));
    }
    let relative = path
        .strip_prefix("/")
        .map_err(io::Error::other)?
        .to_str()
        .ok_or_else(|| invalid("non-UTF-8 output path"))?;
    let mut sources = RouteSources::new(Path::new("/"))?;
    let Some(file) = sources.open(relative)? else {
        return Ok(None);
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Ok(None);
    }
    if metadata.len() > MAX_OUTPUT as u64 {
        return Err(invalid("route output read byte bound exceeded"));
    }
    let mut raw = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_OUTPUT as u64 + 1).read_to_end(&mut raw)?;
    if raw.len() > MAX_OUTPUT || raw.len() as u64 != metadata.len() {
        return Err(invalid("route output changed or exceeded byte bound"));
    }
    let text = String::from_utf8(raw).map_err(io::Error::other)?;
    Ok(Some(text.replace("\r\n", "\n").replace('\r', "\n")))
}
fn charge_operations(
    operations: &AtomicUsize,
    count: usize,
    operation_limit: usize,
) -> io::Result<()> {
    operations
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
            used.checked_add(count)
                .filter(|total| *total <= operation_limit)
        })
        .map(|_| ())
        .map_err(|_| invalid("route lookup operation bound exceeded"))
}
#[cfg(target_os = "linux")]
fn open_root(
    root: &Path,
    deadline: Instant,
    operations: &AtomicUsize,
    component_opens: &Cell<usize>,
    operation_limit: usize,
) -> io::Result<File> {
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    if Instant::now() >= deadline {
        return Err(invalid("route operation deadline exceeded"));
    }
    // The absolute anchor is an actual open too: exhaust/deadline refusal
    // precedes it rather than allowing an uncharged extra descriptor.
    charge_operations(operations, 1, operation_limit)?;
    component_opens.set(
        component_opens
            .get()
            .checked_add(1)
            .filter(|n| *n <= operation_limit)
            .ok_or_else(|| invalid("route root component operation bound exceeded"))?,
    );
    let mut dir = File::open("/")?;
    for part in root.components() {
        if let Component::Normal(part) = part {
            if Instant::now() >= deadline {
                return Err(invalid("route operation deadline exceeded"));
            }
            charge_operations(operations, 1, operation_limit)?;
            let count = component_opens
                .get()
                .checked_add(1)
                .filter(|n| *n <= operation_limit)
                .ok_or_else(|| invalid("route root component operation bound exceeded"))?;
            component_opens.set(count);
            let name =
                std::ffi::CString::new(part.as_bytes()).map_err(|_| invalid("NUL root path"))?;
            let fd = unsafe {
                libc::openat(
                    dir.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            dir = unsafe { File::from_raw_fd(fd) };
        }
    }
    Ok(dir)
}
#[cfg(not(target_os = "linux"))]
fn open_root(
    _root: &Path,
    _deadline: Instant,
    _operations: &AtomicUsize,
    _component_opens: &Cell<usize>,
    _operation_limit: usize,
) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "route source requires Linux descriptor custody",
    ))
}
/// Conservative pre-copy accounting for pretty JSON fields and containers.
/// The byte bound covers constructed logical output, not allocator/RSS overhead.
pub struct OutputBudget {
    used: usize,
}
impl Default for OutputBudget {
    fn default() -> Self {
        Self::new()
    }
}
impl OutputBudget {
    pub fn new() -> Self {
        Self { used: 0 }
    }
    pub fn reserve(&mut self, bytes: usize) -> io::Result<()> {
        self.used = self
            .used
            .checked_add(bytes)
            .filter(|n| *n <= MAX_OUTPUT)
            .ok_or_else(|| invalid("route pre-copy output budget exceeded"))?;
        Ok(())
    }
    pub fn text(&mut self, text: &str) -> io::Result<()> {
        self.reserve(
            text.len()
                .checked_mul(6)
                .and_then(|n| n.checked_add(258))
                .ok_or_else(|| invalid("route text size overflow"))?,
        )
    }
    pub fn value(&mut self, value: &Value) -> io::Result<()> {
        fn walk(b: &mut OutputBudget, v: &Value, depth: usize) -> io::Result<()> {
            if depth > 128 {
                return Err(invalid("route pre-copy output depth exceeded"));
            }
            b.reserve(258)?;
            match v {
                Value::String(s) => b.text(s)?,
                Value::Object(m) => {
                    for (k, v) in m {
                        b.text(k)?;
                        walk(b, v, depth + 1)?;
                    }
                }
                Value::Array(a) => {
                    for v in a {
                        walk(b, v, depth + 1)?;
                    }
                }
                Value::Number(n) => b.reserve(n.as_str().len().saturating_mul(6))?,
                _ => b.reserve(8)?,
            }
            Ok(())
        }
        walk(self, value, 0)
    }
}
fn is_line_separator(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r'
            | '\u{b}'
            | '\u{c}'
            | '\u{1c}'
            | '\u{1d}'
            | '\u{1e}'
            | '\u{85}'
            | '\u{2028}'
            | '\u{2029}'
    )
}
fn line_count(text: &str) -> usize {
    let mut count = 0;
    let mut chars = text.chars().peekable();
    let mut trailing = false;
    while let Some(c) = chars.next() {
        trailing = is_line_separator(c);
        if trailing {
            count += 1;
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
        }
    }
    count + usize::from(!text.is_empty() && !trailing)
}
#[cfg(all(test, target_os = "linux"))]
mod foundation_custody_cases {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn related_roots_exhaust_one_allowance_before_another_anchor_open() {
        let deadline = Instant::now() + Duration::from_secs(1);
        let primary = RouteSources::new_until(Path::new("/"), deadline).expect("primary root");
        let related = RouteSources::new_until_related(Path::new("/"), deadline, &primary)
            .expect("separately held related root");
        assert!(Arc::ptr_eq(&primary.operations, &related.operations));
        assert_eq!(primary.operation_count(), 2);
        assert_eq!(related.operation_count(), 2);
        let custody = primary.root_custody();
        let related_opens = related.root_component_open_count();
        let operations_before = primary.operation_count();
        assert!(related.verify_custody(&custody).is_err());
        assert_eq!(related.root_component_open_count(), related_opens);
        assert_eq!(primary.operation_count(), operations_before);
        primary
            .verify_custody(&custody)
            .expect("original held root");
        primary
            .operations
            .store(MAX_OPERATIONS - 1, Ordering::Relaxed);
        primary.verify_root().expect("last admitted anchor");
        let before = related.root_component_open_count();
        assert!(related.verify_root().is_err());
        assert_eq!(related.root_component_open_count(), before);
        assert_eq!(primary.operation_count(), MAX_OPERATIONS);
        assert_eq!(related.operation_count(), MAX_OPERATIONS);
        assert!(RouteSources::new_until_related(Path::new("/"), deadline, &primary).is_err());
        assert_eq!(primary.operation_count(), MAX_OPERATIONS);
    }

    #[test]
    fn related_root_cannot_extend_clock_or_receive_a_reset_budget() {
        let deadline = Instant::now() + Duration::from_secs(1);
        let primary = RouteSources::new_until(Path::new("/"), deadline).expect("primary root");
        let before = primary.operation_count();
        assert!(
            RouteSources::new_until_related(
                Path::new("/"),
                deadline + Duration::from_secs(1),
                &primary,
            )
            .is_err()
        );
        assert_eq!(primary.operation_count(), before);
    }

    #[test]
    fn budgeted_root_limit_is_finite_and_related_roots_inherit_it() {
        let deadline = Instant::now() + Duration::from_secs(1);
        let primary =
            RouteSources::new_until_with_operation_limit(Path::new("/"), deadline, 200_000)
                .expect("explicit bounded route operation profile");
        let related = RouteSources::new_until_related(Path::new("/"), deadline, &primary)
            .expect("related root shares the selected finite profile");
        assert_eq!(primary.operation_limit(), 200_000);
        assert_eq!(related.operation_limit(), 200_000);
        assert!(
            RouteSources::new_until_with_operation_limit(
                Path::new("/"),
                deadline,
                MAX_BUDGETED_ROUTE_OPERATIONS + 1,
            )
            .is_err()
        );
    }

    #[test]
    fn selected_file_walk_keeps_only_regular_files_and_applies_both_limits() {
        struct Root(PathBuf);
        impl Drop for Root {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = Root(
            std::env::temp_dir().join(format!("tos-selected-files-{}-{nonce}", std::process::id())),
        );
        fs::create_dir(&root.0).expect("exclusive test root");
        fs::create_dir(root.0.join("ToS")).expect("source root");
        fs::create_dir(root.0.join("ToS/nested")).expect("nested source root");
        fs::write(root.0.join("ToS/a.md"), b"a").expect("first source");
        fs::write(root.0.join("ToS/nested/b.md"), b"b").expect("second source");
        let absolute = fs::canonicalize(&root.0).expect("absolute test root");
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut sources = RouteSources::new_until_with_operation_limit(&absolute, deadline, 64)
            .expect("held test root");
        let selected = |path: &str, _: bool| path == "ToS" || path.starts_with("ToS/");
        let paths = sources
            .selected_files_with_limits("ToS", &selected, 8, 64, 2)
            .expect("bounded selected file walk");
        assert_eq!(paths, ["ToS/a.md", "ToS/nested/b.md"]);
        assert!(
            sources
                .selected_files_with_limits("ToS", &selected, 8, 64, 1)
                .is_err()
        );
        assert!(
            sources
                .selected_files_with_limits("ToS", &selected, 8, 8, 2)
                .is_err()
        );
    }

    #[test]
    fn exhausted_root_budget_refuses_before_anchor_or_component_open() {
        let operations = AtomicUsize::new(MAX_OPERATIONS);
        let opens = Cell::new(0);
        let error = open_root(
            Path::new("/tos-route-custody-absent-probe"),
            Instant::now() + Duration::from_secs(1),
            &operations,
            &opens,
            MAX_OPERATIONS,
        )
        .err()
        .expect("exhausted root walk must refuse");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(operations.load(Ordering::Relaxed), MAX_OPERATIONS);
        assert_eq!(opens.get(), 0);
        let operations = AtomicUsize::new(MAX_OPERATIONS - 1);
        let opens = Cell::new(0);
        let error = open_root(
            Path::new("/tos-route-custody-absent-probe"),
            Instant::now() + Duration::from_secs(1),
            &operations,
            &opens,
            MAX_OPERATIONS,
        )
        .err()
        .expect("component exhaustion must precede absent target open");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(operations.load(Ordering::Relaxed), MAX_OPERATIONS);
        assert_eq!(opens.get(), 1); // only the admitted absolute anchor
    }

    #[test]
    fn expired_root_deadline_refuses_before_anchor_open() {
        let operations = AtomicUsize::new(0);
        let opens = Cell::new(0);
        let error = open_root(
            Path::new("/tos-route-custody-absent-probe"),
            Instant::now(),
            &operations,
            &opens,
            MAX_OPERATIONS,
        )
        .err()
        .expect("expired root walk must refuse");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(operations.load(Ordering::Relaxed), 0);
        assert_eq!(opens.get(), 0);
    }

    #[test]
    fn replacement_symlink_changes_topology_even_with_same_endpoint() {
        struct Root(PathBuf);
        impl Drop for Root {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = Root(
            std::env::temp_dir().join(format!("tos-route-custody-{}-{nonce}", std::process::id())),
        );
        fs::create_dir(&root.0).expect("exclusive test root");
        fs::write(root.0.join("target"), b"bounded fixture").expect("target");
        symlink("target", root.0.join("selected")).expect("selected link");
        let absolute = fs::canonicalize(&root.0).expect("absolute test root");
        let mut sources = RouteSources::new(&absolute).expect("held test root");
        let before = match sources
            .resolve_selected_target("selected", None)
            .expect("resolve")
        {
            RouteResolvedTarget::Inside {
                relative_path,
                topology_stamp,
                metadata: Some(m),
            } => {
                assert_eq!(relative_path, "target");
                assert!(m.is_file());
                topology_stamp
            }
            _ => panic!("selected test link must resolve inside"),
        };
        fs::remove_file(root.0.join("selected")).expect("replace selected link");
        symlink("./target", root.0.join("selected")).expect("replacement link");
        match sources
            .resolve_selected_target("selected", None)
            .expect("resolve replacement")
        {
            RouteResolvedTarget::Inside {
                relative_path,
                topology_stamp,
                metadata: Some(m),
            } => {
                assert_eq!(relative_path, "target");
                assert!(m.is_file());
                assert_ne!(before, topology_stamp);
            }
            _ => panic!("replacement test link must resolve inside"),
        }
    }
}
