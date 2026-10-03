//! Bounded exact-revision point reads from the private native V2 store.
//! These observations confer no admission, rights or currentness after the
//! selected immutable session. Strict legacy V1 readers remain unchanged.
use super::source_admission::{active, invalid};
use super::source_admission_segment_v2::{SourceRevisionRootsV2, SourceRootSetV2};
use super::source_admission_store::AdmissionStore;
use std::{
    fs::File,
    io::{self, Read},
    os::unix::fs::MetadataExt,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};
use tos_foundation::{Digest256, Digest256Hasher, RelativePath, SourceRevision};
use tos_segment_store::{
    AuthenticatedTreeDescriptorV2, AuthenticatedTreeIoLedgerV1, AuthenticatedTreeLimitsV1,
    SegmentLimits, SegmentStore,
};
use tos_source_store::{
    CorpusCurrentSelection, CorpusPointerFormat, PinnedSqliteIoBudget, ReadLimits,
};

const DOMAIN: &[u8] = b"tos-native-admission-source-v2";
const ROOT_BYTES: usize = 65_536;
const BLOCK_BYTES: usize = 65_536;

#[derive(Clone, Copy)]
pub struct V2PointReadLimits {
    pub pointer: ReadLimits,
    pub segment: SegmentLimits,
    pub tree: AuthenticatedTreeLimitsV1,
    pub max_object_bytes: usize,
    /// Includes caller-retained data, session roots, decode/transient frame
    /// overlap and one returned object. No independently reset state grant.
    pub max_state_bytes: usize,
    pub caller_retained_state_bytes: usize,
}

impl V2PointReadLimits {
    fn validate(self) -> io::Result<Self> {
        self.pointer.validate().map_err(invalid)?;
        self.segment.validate().map_err(invalid)?;
        if self.max_object_bytes == 0
            || self.max_object_bytes == usize::MAX
            || self.max_state_bytes == 0
            || self.max_state_bytes == usize::MAX
            || self.tree.max_nodes == 0
            || self.tree.max_nodes == u64::MAX
            || self.tree.max_total_bytes == 0
            || self.tree.max_total_bytes == u64::MAX
        {
            return Err(invalid("V2 point reader finite profile differs"));
        }
        let required = self
            .caller_retained_state_bytes
            .checked_add(
                self.pointer
                    .max_manifest_bytes
                    .checked_mul(64)
                    .ok_or_else(|| invalid("V2 point state overflow"))?,
            )
            .and_then(|n| n.checked_add(self.tree.max_node_bytes.checked_mul(64)?))
            .and_then(|n| n.checked_add(self.max_object_bytes.checked_mul(2)?))
            .and_then(|n| n.checked_add(ROOT_BYTES * 128 + 8 * 1024 * 1024 + BLOCK_BYTES))
            .ok_or_else(|| invalid("V2 point state overflow"))?;
        if required > self.max_state_bytes {
            return Err(invalid("V2 point simultaneous state allowance exceeded"));
        }
        Ok(self)
    }
}

struct TreeIo(PinnedSqliteIoBudget);
impl AuthenticatedTreeIoLedgerV1 for TreeIo {
    fn charge_read(&self, n: u64) -> bool {
        self.0.charge_read(n).is_ok()
    }
    fn record_read_returned(&self, n: u64) -> bool {
        self.0.record_read_returned(n).is_ok()
    }
    fn charge_write(&self, n: u64) -> bool {
        self.0.charge_write(n).is_ok()
    }
    fn record_write_returned(&self, n: u64) -> bool {
        self.0.record_write_returned(n).is_ok()
    }
}

pub struct V2MemberObservation {
    pub revision: SourceRevision,
    pub path: RelativePath,
    pub sha256: Digest256,
    pub size_bytes: u64,
    pub source_mode: u32,
    pub bytes: Vec<u8>,
}

/// One pinned immutable selected cut. Caller-owned IO/deadline/cancellation
/// are shared by all its lookups and actual object reads.
pub struct V2ReadSession {
    store: AdmissionStore,
    segment: SegmentStore,
    roots: SourceRootSetV2,
    selection: CorpusCurrentSelection,
    limits: V2PointReadLimits,
    io: PinnedSqliteIoBudget,
    tree_io: Arc<dyn AuthenticatedTreeIoLedgerV1>,
    deadline: Instant,
    cancel: Arc<AtomicBool>,
    read_nodes: u64,
    tree_read_bytes: u64,
    failed: bool,
    observation: Option<V2MemberObservation>,
}

impl V2ReadSession {
    pub fn open(
        path: &Path,
        limits: V2PointReadLimits,
        original_io: PinnedSqliteIoBudget,
        deadline: Instant,
        cancel: Arc<AtomicBool>,
    ) -> io::Result<Self> {
        let limits = limits.validate()?;
        active(deadline, &cancel)?;
        let store = AdmissionStore::open_existing(path, deadline, &cancel)?;
        let selection = store
            .current_selection(limits.pointer, deadline, &cancel, Some(&original_io))?
            .ok_or_else(|| invalid("V2 point selected revision absent"))?;
        if selection.format != CorpusPointerFormat::V2 {
            return Err(invalid("V2 point reader requires explicit V2 selection"));
        }
        let digest = selection
            .rootset_sha256
            .ok_or_else(|| invalid("V2 point rootset digest absent"))?;
        let raw = store.read_v2_rootset(
            selection.revision.0,
            digest,
            ROOT_BYTES,
            deadline,
            &cancel,
            &original_io,
        )?;
        let roots = SourceRootSetV2::decode(&raw)?;
        if roots.current.revision != selection.revision
            || roots.current.base_revision != selection.previous
        {
            return Err(invalid("V2 point selector and roots differ"));
        }
        let (root, _, _) = store.backup_namespaces()?;
        let _existing =
            tos_fd_open::open_directory_at(root, Path::new("segments-v2")).map_err(invalid)?;
        let tree_io: Arc<dyn AuthenticatedTreeIoLedgerV1> = Arc::new(TreeIo(original_io.clone()));
        let segment = store.segment_store_v2_with_io(
            DOMAIN,
            limits.segment,
            tree_io.clone(),
            deadline,
            &cancel,
        )?;
        if segment.custody_domain() != DOMAIN {
            return Err(invalid("V2 point physical domain differs"));
        }
        roots.validate_store_binding(segment.store_id(), segment.domain_digest())?;
        let mut session = Self {
            store,
            segment,
            roots,
            selection,
            limits,
            io: original_io,
            tree_io,
            deadline,
            cancel,
            read_nodes: 0,
            tree_read_bytes: 0,
            failed: false,
            observation: None,
        };
        let history = session.roots.history.clone();
        let key = *session.selection.revision.0.as_bytes();
        let row = session
            .lookup(&history, &key)?
            .ok_or_else(|| invalid("V2 point current history row absent"))?;
        session.roots.verify_current_history_row(&key, &row)?;
        session.store.verify_layout()?;
        active(deadline, &session.cancel)?;
        Ok(session)
    }

    pub fn selected_revision(&self) -> SourceRevision {
        self.selection.revision
    }

    /// Explicit mutable-pointer fence for a caller that requires a current
    /// view; exact-revision reads themselves use the retained immutable cut.
    pub fn verify_current_fence(&self) -> io::Result<()> {
        if self.store.current_selection(
            self.limits.pointer,
            self.deadline,
            &self.cancel,
            Some(&self.io),
        )? != Some(self.selection.clone())
        {
            return Err(invalid("V2 point current selection advanced"));
        }
        self.store.verify_layout()?;
        active(self.deadline, &self.cancel)
    }

    fn lookup(
        &mut self,
        root: &AuthenticatedTreeDescriptorV2,
        key: &[u8],
    ) -> io::Result<Option<Vec<u8>>> {
        if self.failed {
            return Err(invalid("V2 point session already refused"));
        }
        active(self.deadline, &self.cancel)?;
        let mut limits = self.limits.tree;
        limits.max_nodes = limits
            .max_nodes
            .checked_sub(self.read_nodes)
            .filter(|n| *n > 0)
            .ok_or_else(|| invalid("V2 point cumulative node allowance exceeded"))?;
        limits.max_total_bytes = limits
            .max_total_bytes
            .checked_sub(self.tree_read_bytes)
            .filter(|n| *n > 0)
            .ok_or_else(|| invalid("V2 point cumulative tree bytes exceeded"))?;
        let lookup = self
            .segment
            .lookup_authenticated_tree_v2_with_work_and_io(
                root,
                key,
                limits,
                Some(self.tree_io.clone()),
                self.deadline,
                &self.cancel,
            )
            .map_err(invalid);
        let (value, work) = match lookup {
            Ok(value) => value,
            Err(error) => {
                self.failed = true;
                return Err(error);
            }
        };
        self.read_nodes = self
            .read_nodes
            .checked_add(work.read_nodes)
            .filter(|n| *n <= self.limits.tree.max_nodes)
            .ok_or_else(|| invalid("V2 point cumulative node allowance exceeded"))?;
        self.tree_read_bytes = self
            .tree_read_bytes
            .checked_add(work.read_bytes)
            .filter(|n| *n <= self.limits.tree.max_total_bytes)
            .ok_or_else(|| invalid("V2 point cumulative tree bytes exceeded"))?;
        self.store.verify_layout()?;
        active(self.deadline, &self.cancel)?;
        Ok(value)
    }

    fn revision_roots(
        &mut self,
        revision: SourceRevision,
    ) -> io::Result<Option<SourceRevisionRootsV2>> {
        if revision == self.roots.current.revision {
            return Ok(Some(self.roots.current.clone()));
        }
        let history = self.roots.history.clone();
        let Some(raw) = self.lookup(&history, revision.0.as_bytes())? else {
            return Ok(None);
        };
        let roots = SourceRevisionRootsV2::decode(&raw)?;
        if roots.revision != revision {
            return Err(invalid("V2 point history revision differs"));
        }
        roots.validate_store_binding(self.segment.store_id(), self.segment.domain_digest())?;
        Ok(Some(roots))
    }

    pub fn read_member(
        &mut self,
        revision: SourceRevision,
        path: &RelativePath,
    ) -> io::Result<Option<&V2MemberObservation>> {
        // Keep one owned observation. A live borrowed result prevents another
        // mutable read; explicit caller copies belong to its retained state.
        self.observation = None;
        let Some(roots) = self.revision_roots(revision)? else {
            return Ok(None);
        };
        self.observation = self.read_from_roots(&roots, path)?;
        Ok(self.observation.as_ref())
    }

    pub fn read_identity(
        &mut self,
        revision: SourceRevision,
        id: &str,
    ) -> io::Result<Option<&V2MemberObservation>> {
        self.observation = None;
        if id.is_empty() || id.len() > self.limits.tree.max_key_bytes {
            return Err(invalid("V2 point identity key exceeds profile"));
        }
        let Some(roots) = self.revision_roots(revision)? else {
            return Ok(None);
        };
        let Some(path) = self.lookup(&roots.identities, id.as_bytes())? else {
            return Ok(None);
        };
        let path =
            RelativePath::parse(std::str::from_utf8(&path).map_err(invalid)?).map_err(invalid)?;
        self.observation = Some(
            self.read_from_roots(&roots, &path)?
                .ok_or_else(|| invalid("V2 point identity member absent"))?,
        );
        Ok(self.observation.as_ref())
    }

    fn read_from_roots(
        &mut self,
        roots: &SourceRevisionRootsV2,
        path: &RelativePath,
    ) -> io::Result<Option<V2MemberObservation>> {
        let Some(row) = self.lookup(&roots.members, path.as_str().as_bytes())? else {
            return Ok(None);
        };
        if row.len() != 44 {
            return Err(invalid("V2 point member tuple width differs"));
        }
        let digest = Digest256::from_bytes(row[..32].try_into().map_err(invalid)?);
        let size = u64::from_be_bytes(row[32..40].try_into().map_err(invalid)?);
        let mode = u32::from_le_bytes(row[40..44].try_into().map_err(invalid)?);
        if mode & !0o777 != 0 || size > self.limits.max_object_bytes as u64 {
            return Err(invalid("V2 point object or mode exceeds profile"));
        }
        let (_, objects, _) = self.store.backup_namespaces()?;
        let name = digest.to_hex();
        let mut input = tos_fd_open::open_regular_at(objects, Path::new(&name)).map_err(invalid)?;
        let before = stamp(&input)?;
        let metadata = input.metadata()?;
        if metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & 0o222 != 0
            || metadata.len() != size
        {
            return Err(invalid("V2 point object custody differs"));
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(usize::try_from(size).map_err(invalid)?)
            .map_err(invalid)?;
        let mut block = [0u8; BLOCK_BYTES];
        let mut hash = Digest256Hasher::new();
        let mut remaining = size;
        while remaining > 0 {
            active(self.deadline, &self.cancel)?;
            let wanted = usize::try_from(remaining.min(BLOCK_BYTES as u64)).map_err(invalid)?;
            self.io.charge_read(wanted as u64).map_err(invalid)?;
            let read = match input.read(&mut block[..wanted]) {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                value => value?,
            };
            self.io.record_read_returned(read as u64).map_err(invalid)?;
            if read == 0 {
                return Err(invalid("V2 point object early EOF"));
            }
            bytes.extend_from_slice(&block[..read]);
            hash.update(&block[..read]);
            remaining -= read as u64;
        }
        self.io.charge_read(1).map_err(invalid)?;
        let tail = input.read(&mut block[..1])?;
        self.io.record_read_returned(tail as u64).map_err(invalid)?;
        let named = tos_fd_open::open_regular_at(objects, Path::new(&name)).map_err(invalid)?;
        if tail != 0
            || hash.finalize() != digest
            || stamp(&input)? != before
            || stamp(&named)? != before
        {
            return Err(invalid("V2 point object digest or EOF differs"));
        }
        self.store.verify_layout()?;
        active(self.deadline, &self.cancel)?;
        Ok(Some(V2MemberObservation {
            revision: roots.revision,
            path: path.clone(),
            sha256: digest,
            size_bytes: size,
            source_mode: mode,
            bytes,
        }))
    }
}

fn stamp(file: &File) -> io::Result<(u64, u64, u64, u32, u32, i64, i64, i64, i64)> {
    let m = file.metadata()?;
    Ok((
        m.dev(),
        m.ino(),
        m.len(),
        m.mode(),
        m.uid(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    ))
}
