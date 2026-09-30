//! Descriptor-bound maintained creation publication. This coordinates byte
//! mechanics in an independently selected owner filesystem, not source admission.
//! The corpus lock name and rename-no-replace protocol interoperate with Python.

#[path = "source_forms_publication.rs"]
pub(crate) mod forms_publication;
#[path = "source_revision_publication.rs"]
pub(crate) mod revision_publication;
pub(crate) use revision_publication::CommittedRecordObservation;

#[path = "source_creation_cli_selection.rs"]
mod cli_selection;

use crate::source_claims::SerializedClaimCreation;
use crate::source_command::{self as cmd, SourceChange, SourceCommandError, SourceCommandResult};
use crate::source_creation::{CreationPackage, SerializedCreation};
use rustix::fs::{AtFlags, FlockOperation, Mode, OFlags, RenameFlags};
use rustix::io::Errno;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, Metadata, Permissions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

#[path = "source_claim_publication_owner.rs"]
mod claim_publication_owner;
pub(crate) use claim_publication_owner::CommittedClaimObservation;

const CORPUS_LOCK: &str = ".historical-create.writer.lock";
const CLAIM_CAPTURE_HOME: &str = ".claim-retained";
const CLAIM_CAPTURE_INDEX: &str = "capture-index.json";
const CLAIM_CAPTURE_INDEX_BYTES: usize = 524_288;
#[path = "source_catalog_selection.rs"]
mod catalog_selection;
#[path = "source_object_link.rs"]
mod object_link;
pub(crate) use object_link::current_result_fields as object_link_result_fields;
pub use object_link::{
    ObjectLinkPreparation, ObjectLinkPublication, ObjectLinkRecoveryDecision,
    execute_isolated_object_link_from_captures, prepare_isolated_object_link_from_proposal,
    recover_isolated_object_link_from_captures, replay_isolated_object_link_from_captures,
};
#[path = "source_work_expression.rs"]
mod work_expression;
pub use work_expression::{
    WorkExpressionPreparation, WorkExpressionPublication, WorkRecoveryDecision,
    execute_isolated_work_expression_from_captures, prepare_isolated_work_expression_from_proposal,
    recover_isolated_work_expression_from_captures, replay_isolated_work_expression_from_captures,
};
#[path = "source_item_adoption.rs"]
mod item_adoption;
pub(crate) use item_adoption::current_result_fields as item_result_fields;
pub use item_adoption::{
    ItemAdoptionPreparation, ItemAdoptionPublication, execute_isolated_item_adoption_from_captures,
    prepare_isolated_item_adoption_from_proposal, recover_isolated_item_adoption_from_captures,
    replay_isolated_item_adoption_from_captures,
};
#[path = "source_collection_membership.rs"]
mod collection_membership;
pub(crate) use collection_membership::current_result_fields as collection_result_fields;
pub use collection_membership::{
    CollectionMembershipPreparation, CollectionMembershipPublication, CollectionRecoveryDecision,
    execute_isolated_collection_membership_from_captures,
    prepare_isolated_collection_membership_from_proposal,
    recover_isolated_collection_membership_from_captures,
    replay_isolated_collection_membership_from_captures,
};
#[path = "source_work_transaction.rs"]
pub(crate) mod work_transaction;
pub(crate) const MAX_FILES: usize = 4096;
pub(crate) const MAX_BYTES: usize = 33_554_432;

fn charge_claim_read(
    budget: &Rc<RefCell<crate::source_claims::ClaimCallBudget>>,
    bytes: usize,
    scratch: usize,
) -> SourceCommandResult<()> {
    let mut budget = budget.borrow_mut();
    budget.check_live(scratch)?;
    budget.read(bytes as u64)
}

fn charge_claim_reselection(
    budget: &Rc<RefCell<crate::source_claims::ClaimCallBudget>>,
    context: &cmd::CommandContext,
    changes: Option<&[SourceChange]>,
) -> SourceCommandResult<()> {
    let mut total = context
        .files
        .iter()
        .try_fold(0usize, |sum, file| {
            if file.path.as_str().starts_with("ToS/") {
                sum.checked_add(file.raw.len())
            } else {
                Some(sum)
            }
        })
        .ok_or(SourceCommandError::Unsupported(
            "Claim selected source size overflow",
        ))?;
    if let Some(changes) = changes {
        for change in changes {
            if let Some(before) = context.files.iter().find(|file| file.path == change.path) {
                total =
                    total
                        .checked_sub(before.raw.len())
                        .ok_or(SourceCommandError::Unsupported(
                            "Claim selected source size underflow",
                        ))?;
            }
            total = total
                .checked_add(change.after.as_ref().map_or(0, Vec::len))
                .ok_or(SourceCommandError::Unsupported(
                    "Claim changed source size overflow",
                ))?;
        }
    }
    charge_claim_read(budget, total, total)
}

pub(crate) fn active(deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
        Err(SourceCommandError::Denied(
            "creation filesystem cancelled or expired",
        ))
    } else {
        Ok(())
    }
}
fn stamp(m: &Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
// A v1 cut carries the portable declared mode, not the current inode's
// permissions. The maintained Claim revision and selected Work transaction
// writers use private 0600 temp files for metadata whose portable cut mode is
// 0644. This opt-in applies only to those owner readers; special bits never match.
fn member_mode_matches(actual: u32, declared: u32, private_metadata_read: bool) -> bool {
    actual & 0o7000 == 0
        && (actual & 0o777 == declared
            || private_metadata_read && actual & 0o777 == 0o600 && declared == 0o644)
}

/// The owner-local native command uses the existing protected authored walker
/// to bind an immutable source cut to its currently named public checkout.
/// Portable/private mode exceptions belong to their specific writer routes;
/// this public Text selection requires the exact declared mode.
pub(crate) fn verify_owner_text_current_cut(
    root: &File,
    uid: u32,
    cut: &CorpusCutReader,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let mut observed = BTreeMap::new();
    let mut total = 0usize;
    let mut directories = 0usize;
    let tos = walk(root, "ToS", uid)?;
    scan(
        &tos,
        "ToS",
        uid,
        None,
        None,
        None,
        &mut observed,
        &mut total,
        &mut directories,
        deadline,
        cancelled,
    )?;
    let selected = cut
        .current()
        .members()
        .map(|member| {
            (
                member.path.as_str(),
                (member.sha256, member.size_bytes, member.mode),
            )
        })
        .collect::<BTreeMap<_, _>>();
    if observed.len() != selected.len()
        || observed.iter().any(|(path, actual)| {
            selected
                .get(path.as_str())
                .is_none_or(|expected| actual != expected)
        })
    {
        return Err(SourceCommandError::Conflict(
            "owner-local selected cut differs from current authored checkout",
        ));
    }
    Ok(())
}
pub(crate) fn inode(m: &Metadata) -> (u64, u64) {
    (m.dev(), m.ino())
}
pub(crate) fn owned(file: &File, uid: u32, directory: bool) -> SourceCommandResult<Metadata> {
    let m = file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("creation fd metadata"))?;
    if m.uid() != uid
        || m.mode() & 0o022 != 0
        || m.is_dir() != directory
        || (!directory && !m.is_file())
    {
        return Err(SourceCommandError::Denied(
            "creation path ownership/type/write boundary",
        ));
    }
    Ok(m)
}
fn child(parent: &File, leaf: &str) -> SourceCommandResult<File> {
    tos_fd_open::open_directory_at(parent, Path::new(leaf))
        .map_err(|_| SourceCommandError::Denied("creation directory absent or unsafe"))
}
pub(crate) fn protected_configuration_parents(path: &Path, uid: u32) -> SourceCommandResult<()> {
    use std::path::Component;
    let mut components = path.components();
    if components.next() != Some(Component::RootDir) {
        return Err(SourceCommandError::Denied(
            "configuration path is not absolute",
        ));
    }
    let parts = components.collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|p| !matches!(p, Component::Normal(_))) {
        return Err(SourceCommandError::Denied(
            "configuration path is not normalized",
        ));
    }
    let mut parent = tos_fd_open::open_absolute_directory(Path::new("/"))
        .map_err(|_| SourceCommandError::Denied("configuration root descriptor"))?;
    for part in std::iter::once(None).chain(parts[..parts.len() - 1].iter().map(Some)) {
        if let Some(Component::Normal(name)) = part {
            parent = tos_fd_open::open_directory_at(&parent, Path::new(name))
                .map_err(|_| SourceCommandError::Denied("configuration ancestor unsafe"))?;
        }
        let metadata = parent
            .metadata()
            .map_err(|_| SourceCommandError::Denied("configuration ancestor metadata"))?;
        let root_sticky = metadata.uid() == 0 && metadata.mode() & 0o1000 != 0;
        if !metadata.is_dir()
            || ![0, uid].contains(&metadata.uid())
            || metadata.mode() & 0o022 != 0 && !root_sticky
        {
            return Err(SourceCommandError::Denied(
                "configuration ancestor ownership/write boundary",
            ));
        }
    }
    Ok(())
}
fn walk(root: &File, path: &str, uid: u32) -> SourceCommandResult<File> {
    let relative = RelativePath::parse(path)
        .map_err(|_| SourceCommandError::Invalid("creation directory relative path"))?;
    let mut fd = tos_fd_open::reopen_directory(root)
        .map_err(|_| SourceCommandError::Denied("creation root descriptor"))?;
    for part in relative.as_str().split('/') {
        fd = child(&fd, part)?;
        owned(&fd, uid, true)?;
    }
    Ok(fd)
}
pub(crate) fn raw(
    file: &mut File,
    cap: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let before = file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("creation read metadata"))?;
    if !before.is_file() || before.len() > cap as u64 {
        return Err(SourceCommandError::Invalid(
            "creation read byte/type budget",
        ));
    }
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 65536];
    loop {
        active(deadline, cancelled)?;
        let n = file
            .read(&mut buffer)
            .map_err(|_| SourceCommandError::Invalid("creation descriptor read"))?;
        if n == 0 {
            break;
        }
        if bytes.len().checked_add(n).is_none_or(|size| size > cap) {
            return Err(SourceCommandError::Invalid(
                "creation read exceeds byte budget",
            ));
        }
        bytes.extend_from_slice(&buffer[..n]);
    }
    let after = file
        .metadata()
        .map_err(|_| SourceCommandError::Invalid("creation readback metadata"))?;
    if bytes.len() as u64 != before.len() || stamp(&before) != stamp(&after) {
        return Err(SourceCommandError::Conflict(
            "creation input changed during read",
        ));
    }
    Ok(bytes)
}

/// Normal Unix ownership and exact protected configuration are selected here.
/// Construction creates no directory, writes no record and issues no admission.
pub struct CreationFilesystem {
    root_path: PathBuf,
    root: File,
    root_identity: (u64, u64),
    configuration_path: PathBuf,
    configuration_raw: Vec<u8>,
    uid: u32,
}

/// Exact generated bytes selected separately from an authored source cut.
/// This is Claim-local evidence, never a source member or a software rule.
pub(crate) struct ClaimCatalogCapture {
    catalog: Option<File>,
    catalog_identity: Option<(u64, u64)>,
    snapshot: Option<work_transaction::PublicationSnapshot>,
    control: Option<Arc<[u8]>>,
    manifest: Option<Arc<[u8]>>,
    routes: BTreeMap<String, Arc<[u8]>>,
    uid: u32,
}

impl ClaimCatalogCapture {
    fn canonical_index(
        &self,
        receipt_sha256: Digest256,
        source_revision: tos_foundation::SourceRevision,
        whole_call: Option<&Rc<RefCell<crate::source_claims::ClaimCallBudget>>>,
        overlapping_state: usize,
    ) -> SourceCommandResult<Vec<u8>> {
        if self.routes.len() > 32 {
            return Err(SourceCommandError::Unsupported(
                "Claim retained catalog route count",
            ));
        }
        // The fixed outer fields and two byte bindings fit in 2048 bytes.
        // A route contributes its two JSON-escaped strings (at most six bytes
        // per input byte) and one fixed digest/size/leaf binding. Charge both
        // the reconstructed value and its canonical buffer before either is
        // allocated. The exact wire is still produced only by cmd::canonical.
        let state_bound = self.routes.keys().try_fold(2048usize, |sum, path| {
            let leaf = path
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid("Claim catalog captured path"))?
                .1;
            path.len()
                .checked_add(leaf.len())
                .and_then(|n| n.checked_mul(6))
                .and_then(|n| n.checked_add(256))
                .and_then(|n| sum.checked_add(n))
                .ok_or(SourceCommandError::Unsupported(
                    "Claim retained catalog index state overflow",
                ))
        })?;
        let peak = state_bound
            .checked_mul(2)
            .and_then(|n| n.checked_add(overlapping_state))
            .ok_or(SourceCommandError::Unsupported(
                "Claim retained catalog index state overflow",
            ))?;
        if peak > MAX_BYTES {
            return Err(SourceCommandError::Unsupported(
                "Claim retained catalog index state budget",
            ));
        }
        if let Some(budget) = whole_call {
            budget.borrow().check_live(peak)?;
        }
        let index = cmd::canonical(&self.index(receipt_sha256, source_revision)?)?;
        if index.len() > CLAIM_CAPTURE_INDEX_BYTES {
            return Err(SourceCommandError::Unsupported(
                "Claim retained catalog index byte budget",
            ));
        }
        Ok(index)
    }

    fn index(
        &self,
        receipt_sha256: Digest256,
        source_revision: tos_foundation::SourceRevision,
    ) -> SourceCommandResult<JsonValue> {
        let manifest = self.manifest.as_ref().ok_or(SourceCommandError::Conflict(
            "Claim original catalog manifest was not captured",
        ))?;
        if self.routes.is_empty() {
            return Err(SourceCommandError::Conflict(
                "Claim original catalog has no selected route",
            ));
        }
        let binding = |raw: &[u8]| {
            cmd::object(vec![
                (
                    "sha256",
                    cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
                ),
                ("size_bytes", cmd::number(raw.len() as u64)),
            ])
        };
        let mut routes = cmd::object(vec![]);
        for (path, raw) in &self.routes {
            let leaf = path
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid("Claim catalog captured path"))?
                .1;
            let mut row = binding(raw);
            cmd::set(&mut row, "leaf", cmd::string(leaf))?;
            cmd::set(&mut routes, path, row)?;
        }
        Ok(cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_claim_generated_catalog_capture_v1"),
            ),
            (
                "source_revision",
                cmd::string(&source_revision.0.to_prefixed()),
            ),
            ("receipt_sha256", cmd::string(&receipt_sha256.to_prefixed())),
            (
                "publication_control",
                self.control
                    .as_ref()
                    .map(|raw| binding(raw))
                    .unwrap_or(JsonValue::Null),
            ),
            ("catalog_manifest", binding(manifest)),
            ("catalog_files", routes),
        ]))
    }
    pub(crate) fn route(
        &mut self,
        path: &str,
        max_route_bytes: usize,
        max_new_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<(Arc<[u8]>, usize)> {
        const PREFIX: &str = "ToS/source-witnesses/catalog/";
        let name = path.strip_prefix(PREFIX).ok_or(SourceCommandError::Denied(
            "Claim generated catalog route namespace",
        ))?;
        if name.is_empty() || name.contains('/') || name == "catalog.manifest.json" {
            return Err(SourceCommandError::Denied("Claim generated catalog route"));
        }
        if let Some(raw) = self.routes.get(path) {
            return Ok((Arc::clone(raw), 0));
        }
        let catalog = self.catalog.as_ref().ok_or(SourceCommandError::Conflict(
            "retained Claim catalog route was not captured",
        ))?;
        let mut newly_read = 0usize;
        if self.manifest.is_none() {
            let raw = work_transaction::read_at(
                catalog,
                "catalog.manifest.json",
                self.uid,
                2_097_152.min(max_new_bytes),
                deadline,
                cancelled,
            )?
            .ok_or(SourceCommandError::Conflict(
                "Claim generated catalog manifest absent",
            ))?;
            newly_read = raw.len();
            self.manifest = Some(Arc::from(raw));
        }
        let remaining =
            max_new_bytes
                .checked_sub(newly_read)
                .ok_or(SourceCommandError::Unsupported(
                    "Claim generated catalog read budget",
                ))?;
        let raw = work_transaction::read_at(
            catalog,
            name,
            self.uid,
            remaining.min(max_route_bytes),
            deadline,
            cancelled,
        )?
        .ok_or(SourceCommandError::Conflict(
            "Claim generated catalog route absent",
        ))?;
        newly_read = newly_read
            .checked_add(raw.len())
            .ok_or(SourceCommandError::Unsupported(
                "Claim generated catalog read overflow",
            ))?;
        let raw: Arc<[u8]> = Arc::from(raw);
        self.routes.insert(path.to_owned(), Arc::clone(&raw));
        Ok((raw, newly_read))
    }

    pub(crate) fn manifest(&self) -> Option<&[u8]> {
        self.manifest.as_deref()
    }

    pub(crate) fn control(&self) -> Option<&[u8]> {
        self.control.as_deref()
    }

    pub(crate) fn selected_bytes(&self) -> SourceCommandResult<usize> {
        let base = self
            .control
            .as_ref()
            .map_or(0, |raw| raw.len())
            .checked_add(self.manifest.as_ref().map_or(0, |raw| raw.len()))
            .ok_or(SourceCommandError::Unsupported(
                "Claim selected catalog byte overflow",
            ))?;
        self.routes.values().try_fold(base, |sum, raw| {
            sum.checked_add(raw.len())
                .ok_or(SourceCommandError::Unsupported(
                    "Claim selected catalog byte overflow",
                ))
        })
    }

    pub(crate) fn bind_cut(
        &self,
        cut: &CorpusCutReader,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        let control_path =
            RelativePath::parse("ToS/source-witnesses/.metadata-publication.json")
                .map_err(|_| SourceCommandError::Invalid("Claim publication control path"))?;
        match (self.control.as_deref(), cut.current().member(&control_path)) {
            (None, None) => Ok(()),
            (Some(raw), Some(member)) => {
                if member.size_bytes != raw.len() as u64
                    || member.sha256 != Digest256::of_bytes(raw)
                    || cut
                        .read_member(
                            cut.current().revision(),
                            &control_path,
                            8192,
                            deadline,
                            cancelled,
                        )
                        .map_err(|_| SourceCommandError::Conflict("Claim publication cut read"))?
                        .raw
                        != raw
                {
                    return Err(SourceCommandError::Conflict(
                        "Claim publication control differs from selected cut",
                    ));
                }
                Ok(())
            }
            _ => Err(SourceCommandError::Conflict(
                "Claim publication control membership differs",
            )),
        }
    }

    pub(crate) fn verify_current(
        &self,
        fs: &CreationFilesystem,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        let snapshot = self
            .snapshot
            .as_ref()
            .ok_or(SourceCommandError::Unsupported(
                "retained Claim catalog is not a live current snapshot",
            ))?;
        snapshot.verify_current(fs, deadline, cancelled)?;
        let current_catalog = walk(&fs.root, "ToS/source-witnesses/catalog", self.uid)?;
        if Some(inode(&owned(&current_catalog, self.uid, true)?)) != self.catalog_identity {
            return Err(SourceCommandError::Conflict(
                "Claim selected catalog directory replaced",
            ));
        }
        let witness = walk(&fs.root, "ToS/source-witnesses", self.uid)?;
        let control = work_transaction::read_at(
            &witness,
            ".metadata-publication.json",
            self.uid,
            8192,
            deadline,
            cancelled,
        )?;
        if control.as_deref() != self.control.as_deref() {
            return Err(SourceCommandError::Conflict(
                "Claim publication control bytes changed",
            ));
        }
        if let Some(prior) = &self.manifest {
            let now = work_transaction::read_at(
                &current_catalog,
                "catalog.manifest.json",
                self.uid,
                prior.len(),
                deadline,
                cancelled,
            )?
            .ok_or(SourceCommandError::Conflict(
                "Claim catalog manifest disappeared",
            ))?;
            if now.as_slice() != prior.as_ref() {
                return Err(SourceCommandError::Conflict(
                    "Claim catalog manifest changed",
                ));
            }
        }
        for (path, prior) in &self.routes {
            let name = path
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid("Claim generated catalog route"))?
                .1;
            let now = work_transaction::read_at(
                &current_catalog,
                name,
                self.uid,
                prior.len(),
                deadline,
                cancelled,
            )?
            .ok_or(SourceCommandError::Conflict(
                "Claim generated catalog route disappeared",
            ))?;
            if now.as_slice() != prior.as_ref() {
                return Err(SourceCommandError::Conflict(
                    "Claim generated catalog changed",
                ));
            }
        }
        Ok(())
    }
}

/// The existing owner corpus mutex, held across a managed durable creation.
/// This observes current protected configuration; it grants no source admission
/// and says nothing about the managed cohort's index completeness.
pub(crate) struct CreationOwnerFence<'a> {
    filesystem: &'a CreationFilesystem,
    package: CreationPackage<'a>,
    witness: File,
    lock: File,
}
impl CreationOwnerFence<'_> {
    pub(crate) fn verify_current(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.filesystem
            .current_package(self.package, deadline, cancelled)?;
        let current = tos_fd_open::open_regular_at(&self.witness, Path::new(CORPUS_LOCK))
            .map_err(|_| SourceCommandError::Conflict("creation corpus lock path changed"))?;
        if inode(&owned(&self.lock, self.filesystem.uid, false)?)
            != inode(&owned(&current, self.filesystem.uid, false)?)
        {
            return Err(SourceCommandError::Conflict(
                "creation locked inode detached",
            ));
        }
        Ok(())
    }
}

/// Bounded execution authority is an actually newly created private directory,
/// not a caller-supplied Boolean or an arbitrary existing canonical root. Its
/// opaque identity is retained through fixture seeding and publication.
pub struct IsolatedCreationRoot {
    path: PathBuf,
    directory: File,
    identity: (u64, u64),
}
impl IsolatedCreationRoot {
    pub fn create(
        parent: &Path,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        let uid = rustix::process::geteuid().as_raw();
        let parent_fd = tos_fd_open::open_absolute_directory(parent)
            .map_err(|_| SourceCommandError::Denied("isolated creation parent unsafe"))?;
        owned(&parent_fd, uid, true)?;
        for _ in 0..32 {
            active(deadline, cancelled)?;
            let mut entropy = [0u8; 24];
            File::open("/dev/urandom")
                .and_then(|mut f| f.read_exact(&mut entropy))
                .map_err(|_| SourceCommandError::Invalid("isolated root entropy"))?;
            let name = format!(
                "tos-isolated-create-{}",
                Digest256::of_bytes(&entropy).to_hex()
            );
            match rustix::fs::mkdirat(&parent_fd, name.as_str(), Mode::from_raw_mode(0o700)) {
                Ok(()) => {
                    let directory = child(&parent_fd, &name)?;
                    let identity = inode(&owned(&directory, uid, true)?);
                    parent_fd
                        .sync_all()
                        .map_err(|_| SourceCommandError::Invalid("isolated root parent fsync"))?;
                    return Ok(Self {
                        path: parent.join(name),
                        directory,
                        identity,
                    });
                }
                Err(Errno::EXIST) => continue,
                Err(_) => return Err(SourceCommandError::Invalid("isolated root mkdir")),
            }
        }
        Err(SourceCommandError::Conflict(
            "isolated root name collision budget",
        ))
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub(crate) fn verify_current(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<File> {
        active(deadline, cancelled)?;
        let uid = rustix::process::geteuid().as_raw();
        if rustix::process::getuid().as_raw() != uid {
            return Err(SourceCommandError::Denied("isolated root account changed"));
        }
        let current = tos_fd_open::open_absolute_directory(&self.path)
            .map_err(|_| SourceCommandError::Conflict("isolated root path replaced"))?;
        if inode(&owned(&current, uid, true)?) != self.identity
            || inode(&owned(&self.directory, uid, true)?) != self.identity
        {
            return Err(SourceCommandError::Conflict(
                "isolated root identity changed",
            ));
        }
        Ok(current)
    }
}
impl CreationFilesystem {
    /// Separate-process Item access reuses the existing independently protected
    /// typed grant. This is crate-private and cannot construct other owners.
    pub(crate) fn select_item_owner(
        configuration_path: &Path,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        active(deadline, cancelled)?;
        let uid = rustix::process::geteuid().as_raw();
        if rustix::process::getuid().as_raw() != uid {
            return Err(SourceCommandError::Denied("Item setuid owner selection"));
        }
        protected_configuration_parents(configuration_path, uid)?;
        let mut file = tos_fd_open::open_absolute_regular(configuration_path, 1_048_576)
            .map_err(|_| SourceCommandError::Denied("Item protected owner selection"))?;
        if owned(&file, uid, false)?.mode() & 0o7777 != 0o600 {
            return Err(SourceCommandError::Denied("Item owner must be mode0600"));
        }
        let configuration_raw = raw(&mut file, 1_048_576, deadline, cancelled)?;
        let config = cmd::parse(&configuration_raw)?;
        if cmd::text(&config, "schema_version")? != "tos_local_item_adoption_owner_v1"
            || cmd::integer(&config, "uid")? != u64::from(uid)
        {
            return Err(SourceCommandError::Denied("Item typed owner account"));
        }
        let root_path =
            crate::source_text_owner::normalized_absolute(cmd::text(&config, "source_root")?)?;
        if configuration_path.starts_with(root_path.join("ToS")) {
            return Err(SourceCommandError::Denied(
                "Item authority cannot be authored content",
            ));
        }
        protected_configuration_parents(&root_path.join("root-pin"), uid)?;
        let root = tos_fd_open::open_absolute_directory(&root_path)
            .map_err(|_| SourceCommandError::Denied("Item exact source root"))?;
        let root_identity = inode(&owned(&root, uid, true)?);
        Ok(Self {
            root_path,
            root,
            root_identity,
            configuration_path: configuration_path.to_path_buf(),
            configuration_raw,
            uid,
        })
    }
    /// Select only the explicitly protected Collection membership owner.
    pub(crate) fn select_collection_owner(
        configuration_path: &Path,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<(Self, Vec<u8>)> {
        active(deadline, cancelled)?;
        let uid = rustix::process::geteuid().as_raw();
        if rustix::process::getuid().as_raw() != uid {
            return Err(SourceCommandError::Denied(
                "Collection setuid owner selection",
            ));
        }
        protected_configuration_parents(configuration_path, uid)?;
        let mut file = tos_fd_open::open_absolute_regular(configuration_path, 1_048_576)
            .map_err(|_| SourceCommandError::Denied("Collection protected owner selection"))?;
        if owned(&file, uid, false)?.mode() & 0o7777 != 0o600 {
            return Err(SourceCommandError::Denied(
                "Collection owner must be mode0600",
            ));
        }
        let configuration_raw = raw(&mut file, 1_048_576, deadline, cancelled)?;
        let config = cmd::parse(&configuration_raw)?;
        if cmd::text(&config, "schema_version")? != "tos_local_collection_membership_owner_v1"
            || cmd::integer(&config, "uid")? != u64::from(uid)
        {
            return Err(SourceCommandError::Denied("Collection typed owner account"));
        }
        let root_path =
            crate::source_text_owner::normalized_absolute(cmd::text(&config, "source_root")?)?;
        if configuration_path.starts_with(root_path.join("ToS")) {
            return Err(SourceCommandError::Denied(
                "Collection authority cannot be authored content",
            ));
        }
        protected_configuration_parents(&root_path.join("root-pin"), uid)?;
        let root = tos_fd_open::open_absolute_directory(&root_path)
            .map_err(|_| SourceCommandError::Denied("Collection exact source root"))?;
        let root_identity = inode(&owned(&root, uid, true)?);
        let selected_raw = configuration_raw.clone();
        Ok((
            Self {
                root_path,
                root,
                root_identity,
                configuration_path: configuration_path.to_path_buf(),
                configuration_raw,
                uid,
            },
            selected_raw,
        ))
    }
    fn claim_capture_home(&self, create: bool) -> SourceCommandResult<File> {
        let catalog = walk(&self.root, "ToS/source-witnesses/catalog", self.uid)?;
        if create {
            match rustix::fs::mkdirat(&catalog, CLAIM_CAPTURE_HOME, Mode::from_raw_mode(0o700)) {
                Ok(()) | Err(Errno::EXIST) => (),
                Err(_) => {
                    return Err(SourceCommandError::Denied(
                        "Claim retained catalog home unsafe",
                    ));
                }
            }
        }
        let home = child(&catalog, CLAIM_CAPTURE_HOME)?;
        let metadata = owned(&home, self.uid, true)?;
        if metadata.mode() & 0o7777 != 0o700 {
            return Err(SourceCommandError::Denied(
                "Claim retained catalog home mode",
            ));
        }
        Ok(home)
    }

    fn verify_claim_capture_files(
        &self,
        home: &File,
        name: &str,
        expected: &[(&str, &[u8])],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        let directory = child(home, name)?;
        owned(&directory, self.uid, true)?;
        if expected.len() > 40 {
            return Err(SourceCommandError::Unsupported(
                "Claim retained catalog file count",
            ));
        }
        let mut seen = vec![false; expected.len()];
        active(deadline, cancelled)?;
        let entries = std::fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
            .map_err(|_| SourceCommandError::Denied("Claim retained catalog enumeration"))?;
        for entry in entries {
            active(deadline, cancelled)?;
            let name = entry
                .map_err(|_| SourceCommandError::Denied("Claim retained catalog entry"))?
                .file_name()
                .into_string()
                .map_err(|_| SourceCommandError::Denied("Claim retained catalog name"))?;
            let Some(index) = expected.iter().position(|(leaf, _)| *leaf == name.as_str()) else {
                return Err(SourceCommandError::Conflict(
                    "Claim retained catalog file set differs",
                ));
            };
            if std::mem::replace(&mut seen[index], true) {
                return Err(SourceCommandError::Conflict(
                    "Claim retained catalog file set differs",
                ));
            }
        }
        active(deadline, cancelled)?;
        if seen.iter().any(|seen| !seen) {
            return Err(SourceCommandError::Conflict(
                "Claim retained catalog file set differs",
            ));
        }
        for (leaf, bytes) in expected {
            let raw = work_transaction::read_at(
                &directory,
                leaf,
                self.uid,
                bytes.len(),
                deadline,
                cancelled,
            )?
            .ok_or(SourceCommandError::Conflict(
                "Claim retained catalog file missing",
            ))?;
            if raw.as_slice() != *bytes {
                return Err(SourceCommandError::Conflict(
                    "Claim retained catalog bytes changed",
                ));
            }
        }
        Ok(())
    }

    fn install_claim_capture(
        &self,
        capture: &ClaimCatalogCapture,
        receipt: &[u8],
        cut: &CorpusCutReader,
        whole_call: Option<&Rc<RefCell<crate::source_claims::ClaimCallBudget>>>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        capture.bind_cut(cut, deadline, cancelled)?;
        if let Some(budget) = whole_call {
            let bytes = capture.selected_bytes()?;
            charge_claim_read(budget, bytes, bytes)?;
        }
        capture.verify_current(self, deadline, cancelled)?;
        let receipt_digest = Digest256::of_bytes(receipt);
        // With a whole-call budget the selected capture was retained by the
        // Claim reader. Without one, include its still-live bytes locally.
        let local_capture = if whole_call.is_some() {
            0
        } else {
            capture.selected_bytes()?
        };
        let index = capture.canonical_index(
            receipt_digest,
            cut.current().revision(),
            whole_call,
            local_capture,
        )?;
        let name = receipt_digest.to_hex();
        let home = self.claim_capture_home(true)?;
        let manifest = capture
            .manifest
            .as_ref()
            .ok_or(SourceCommandError::Conflict(
                "Claim catalog manifest capture missing",
            ))?;
        let mut files: Vec<(&str, &[u8])> = vec![
            (CLAIM_CAPTURE_INDEX, &index),
            ("catalog.manifest.json", manifest),
        ];
        if let Some(control) = &capture.control {
            files.push(("publication-control.json", control));
        }
        for (path, raw) in &capture.routes {
            let leaf = path
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid("Claim captured route"))?
                .1;
            files.push((leaf, raw));
        }
        let capture_bytes = files.iter().try_fold(0usize, |sum, (_, raw)| {
            sum.checked_add(raw.len()).filter(|size| *size <= MAX_BYTES)
        });
        if files.len() > 40 || capture_bytes.is_none() {
            return Err(SourceCommandError::Unsupported(
                "Claim retained catalog capture budget",
            ));
        }
        let wire_bytes = capture_bytes.ok_or(SourceCommandError::Unsupported(
            "Claim retained catalog capture budget",
        ))?;
        if rustix::fs::statat(&home, name.as_str(), AtFlags::SYMLINK_NOFOLLOW).is_ok() {
            if let Some(budget) = whole_call {
                charge_claim_read(budget, wire_bytes, wire_bytes)?;
            }
            return self.verify_claim_capture_files(&home, &name, &files, deadline, cancelled);
        }
        let mut stage = PendingCreation::create(&home, self.uid, deadline, cancelled)?;
        let prepared = (|| {
            for (leaf, bytes) in &files {
                if let Some(budget) = whole_call {
                    charge_claim_read(budget, bytes.len(), bytes.len())?;
                }
                stage.write_claim_private(leaf, bytes, 40, MAX_BYTES, deadline, cancelled)?;
            }
            stage
                .directory
                .sync_all()
                .map_err(|_| SourceCommandError::Invalid("Claim retained catalog stage fsync"))?;
            if let Some(budget) = whole_call {
                let bytes = capture.selected_bytes()?;
                charge_claim_read(budget, bytes, bytes)?;
            }
            capture.verify_current(self, deadline, cancelled)?;
            rustix::fs::renameat_with(
                &home,
                stage.name.as_str(),
                &home,
                name.as_str(),
                RenameFlags::NOREPLACE,
            )
            .map_err(|error| {
                if error == Errno::EXIST {
                    SourceCommandError::Conflict("Claim retained catalog capture occupied")
                } else {
                    SourceCommandError::Invalid("Claim retained catalog atomic install")
                }
            })?;
            stage.published = true;
            home.sync_all().map_err(|_| {
                SourceCommandError::Invalid("Claim retained catalog directory fsync")
            })?;
            Ok(())
        })();
        if let Err(error) = prepared {
            stage.rollback()?;
            return Err(error);
        }
        if let Some(budget) = whole_call {
            charge_claim_read(budget, wire_bytes, wire_bytes)?;
        }
        self.verify_claim_capture_files(&home, &name, &files, deadline, cancelled)
    }

    pub(crate) fn read_claim_catalog_capture(
        &self,
        receipt: &[u8],
        original_cut: &CorpusCutReader,
        whole_call: Option<&Rc<RefCell<crate::source_claims::ClaimCallBudget>>>,
        retain_result: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<ClaimCatalogCapture> {
        let receipt_sha256 = Digest256::of_bytes(receipt);
        let home = self.claim_capture_home(false)?;
        let directory = child(&home, &receipt_sha256.to_hex())?;
        owned(&directory, self.uid, true)?;
        let index_limit = if let Some(budget) = whole_call {
            let budget = budget.borrow();
            CLAIM_CAPTURE_INDEX_BYTES
                .min(budget.remaining_read()? as usize)
                .min(budget.remaining_live()? / 3)
        } else {
            CLAIM_CAPTURE_INDEX_BYTES
        };
        let index_raw = work_transaction::read_at(
            &directory,
            CLAIM_CAPTURE_INDEX,
            self.uid,
            index_limit,
            deadline,
            cancelled,
        )?
        .ok_or(SourceCommandError::Conflict(
            "Claim retained catalog index absent",
        ))?;
        if let Some(budget) = whole_call {
            let mut budget = budget.borrow_mut();
            budget.read(index_raw.len() as u64)?;
            budget.check_live(index_raw.len().checked_mul(3).ok_or(
                SourceCommandError::Unsupported("Claim retained catalog index state overflow"),
            )?)?;
        }
        let index = cmd::parse(&index_raw)?;
        if index_raw != cmd::canonical(&index)? {
            return Err(SourceCommandError::Conflict(
                "Claim retained catalog index bytes differ",
            ));
        }
        cmd::exact_keys(
            &index,
            &[
                "schema_version",
                "source_revision",
                "receipt_sha256",
                "publication_control",
                "catalog_manifest",
                "catalog_files",
            ],
        )?;
        if cmd::text(&index, "schema_version")? != "tos_claim_generated_catalog_capture_v1"
            || cmd::text(&index, "source_revision")?
                != original_cut.current().revision().0.to_prefixed()
            || cmd::text(&index, "receipt_sha256")? != receipt_sha256.to_prefixed()
        {
            return Err(SourceCommandError::Conflict(
                "Claim retained catalog source/receipt binding",
            ));
        }
        // The raw index and its decoded representation remain live while the
        // selected catalog members are read. `total` tracks raw index plus
        // already selected member bytes; neither is in the whole-call retained
        // state yet. Two more index lengths cover its decoded representation
        // and the selected route/closure names derived from it.
        let mut total = index_raw.len();
        let mut required = BTreeSet::from([
            CLAIM_CAPTURE_INDEX.to_owned(),
            "catalog.manifest.json".to_owned(),
        ]);
        let read_bound = |name: &str,
                          binding: &JsonValue,
                          total: &mut usize|
         -> SourceCommandResult<Arc<[u8]>> {
            cmd::exact_keys(binding, &["sha256", "size_bytes"])?;
            let declared = cmd::integer(binding, "size_bytes")?;
            if declared > (MAX_BYTES - *total) as u64 {
                return Err(SourceCommandError::Unsupported(
                    "Claim retained catalog byte budget",
                ));
            }
            let member_peak = index_raw
                .len()
                .checked_mul(2)
                .and_then(|index_state| total.checked_add(index_state))
                .and_then(|n| {
                    (declared as usize)
                        .checked_mul(2)
                        .and_then(|new| n.checked_add(new))
                })
                .ok_or(SourceCommandError::Unsupported(
                    "Claim retained catalog state overflow",
                ))?;
            if member_peak > MAX_BYTES {
                return Err(SourceCommandError::Unsupported(
                    "Claim retained catalog state budget",
                ));
            }
            if let Some(budget) = whole_call {
                let mut budget = budget.borrow_mut();
                budget.check_live(member_peak)?;
                budget.read(declared)?;
            }
            let raw = work_transaction::read_at(
                &directory,
                name,
                self.uid,
                declared as usize,
                deadline,
                cancelled,
            )?
            .ok_or(SourceCommandError::Conflict(
                "Claim retained catalog member absent",
            ))?;
            if raw.len() as u64 != declared
                || Digest256::of_bytes(&raw).to_prefixed() != cmd::text(binding, "sha256")?
            {
                return Err(SourceCommandError::Conflict(
                    "Claim retained catalog member fixity",
                ));
            }
            *total += raw.len();
            Ok(Arc::from(raw))
        };
        let manifest = read_bound(
            "catalog.manifest.json",
            cmd::field(&index, "catalog_manifest")?,
            &mut total,
        )?;
        let control = if cmd::field(&index, "publication_control")? == &JsonValue::Null {
            None
        } else {
            required.insert("publication-control.json".to_owned());
            Some(read_bound(
                "publication-control.json",
                cmd::field(&index, "publication_control")?,
                &mut total,
            )?)
        };
        let mut routes = BTreeMap::new();
        let entries = cmd::field(&index, "catalog_files")?
            .as_object()
            .ok_or(SourceCommandError::Invalid("Claim retained catalog routes"))?;
        if entries.is_empty() || entries.len() > 32 {
            return Err(SourceCommandError::Unsupported(
                "Claim retained catalog route count",
            ));
        }
        for (path, binding) in entries {
            let path = path
                .as_str()
                .ok_or(SourceCommandError::Invalid("Claim retained catalog path"))?;
            let leaf = path.strip_prefix("ToS/source-witnesses/catalog/").ok_or(
                SourceCommandError::Denied("Claim retained catalog namespace"),
            )?;
            if leaf.is_empty() || leaf.contains('/') || leaf == "catalog.manifest.json" {
                return Err(SourceCommandError::Denied(
                    "Claim retained catalog direct route",
                ));
            }
            cmd::exact_keys(binding, &["leaf", "sha256", "size_bytes"])?;
            if cmd::text(binding, "leaf")? != leaf || !required.insert(leaf.to_owned()) {
                return Err(SourceCommandError::Conflict(
                    "Claim retained catalog leaf collision",
                ));
            }
            let mut bytes_binding = binding.clone();
            if let JsonValue::Object(fields) = &mut bytes_binding {
                fields.retain(|(key, _)| key.as_str() != Some("leaf"));
            }
            routes.insert(
                path.to_owned(),
                read_bound(leaf, &bytes_binding, &mut total)?,
            );
        }
        active(deadline, cancelled)?;
        let entries = std::fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
            .map_err(|_| SourceCommandError::Denied("Claim retained catalog directory listing"))?;
        for entry in entries {
            active(deadline, cancelled)?;
            let name = entry
                .map_err(|_| SourceCommandError::Denied("Claim retained catalog directory entry"))?
                .file_name()
                .into_string()
                .map_err(|_| SourceCommandError::Denied("Claim retained catalog directory name"))?;
            if !required.remove(&name) {
                return Err(SourceCommandError::Conflict(
                    "Claim retained catalog file closure",
                ));
            }
        }
        active(deadline, cancelled)?;
        if !required.is_empty() {
            return Err(SourceCommandError::Conflict(
                "Claim retained catalog file closure",
            ));
        }
        let capture = ClaimCatalogCapture {
            catalog: None,
            catalog_identity: None,
            snapshot: None,
            control,
            manifest: Some(manifest),
            routes,
            uid: self.uid,
        };
        capture.bind_cut(original_cut, deadline, cancelled)?;
        if capture.canonical_index(
            receipt_sha256,
            original_cut.current().revision(),
            whole_call,
            index_raw
                .len()
                .checked_mul(2)
                .and_then(|index_state| total.checked_add(index_state))
                .ok_or(SourceCommandError::Unsupported(
                    "Claim retained catalog index state overflow",
                ))?,
        )? != index_raw
        {
            return Err(SourceCommandError::Conflict(
                "Claim retained catalog index differs",
            ));
        }
        drop(index);
        drop(index_raw);
        // Only the caller that keeps this capture beyond the read retains its
        // selected bytes. Validation-only readers discard the local copy; they
        // paid its full overlap above without permanently debiting live state.
        if retain_result {
            if let Some(budget) = whole_call {
                budget.borrow_mut().retain(capture.selected_bytes()?)?;
            }
        }
        Ok(capture)
    }

    fn verify_claim_original_capture(
        &self,
        package: &SerializedClaimCreation,
        original_cut: &CorpusCutReader,
        whole_call: Option<&Rc<RefCell<crate::source_claims::ClaimCallBudget>>>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        let Some(selected) = package.catalog_capture() else {
            return Ok(());
        };
        let capture = selected.borrow();
        capture.bind_cut(original_cut, deadline, cancelled)?;
        let receipt = &package.files()["source-create-receipt.json"];
        let digest = Digest256::of_bytes(receipt);
        let local_capture = if whole_call.is_some() {
            0
        } else {
            capture.selected_bytes()?
        };
        let index = capture.canonical_index(
            digest,
            original_cut.current().revision(),
            whole_call,
            local_capture,
        )?;
        let manifest = capture
            .manifest
            .as_ref()
            .ok_or(SourceCommandError::Conflict(
                "Claim retained catalog manifest absent",
            ))?;
        let mut files: Vec<(&str, &[u8])> = vec![
            (CLAIM_CAPTURE_INDEX, &index),
            ("catalog.manifest.json", manifest),
        ];
        if let Some(control) = &capture.control {
            files.push(("publication-control.json", control));
        }
        for (path, raw) in &capture.routes {
            files.push((
                path.rsplit_once('/')
                    .ok_or(SourceCommandError::Invalid("Claim retained catalog path"))?
                    .1,
                raw,
            ));
        }
        let home = self.claim_capture_home(false)?;
        if let Some(budget) = whole_call {
            let bytes = files
                .iter()
                .try_fold(0usize, |sum, (_, raw)| sum.checked_add(raw.len()))
                .ok_or(SourceCommandError::Unsupported(
                    "Claim retained capture read overflow",
                ))?;
            charge_claim_read(budget, bytes, bytes)?;
        }
        self.verify_claim_capture_files(&home, &digest.to_hex(), &files, deadline, cancelled)
    }
    pub(crate) fn select_claim_catalog(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<ClaimCatalogCapture> {
        let snapshot = work_transaction::PublicationSnapshot::select(self, deadline, cancelled)?;
        let witness = walk(&self.root, "ToS/source-witnesses", self.uid)?;
        let control = work_transaction::read_at(
            &witness,
            ".metadata-publication.json",
            self.uid,
            8192,
            deadline,
            cancelled,
        )?;
        snapshot.verify_current(self, deadline, cancelled)?;
        let catalog = walk(&self.root, "ToS/source-witnesses/catalog", self.uid)?;
        let catalog_identity = inode(&owned(&catalog, self.uid, true)?);
        Ok(ClaimCatalogCapture {
            catalog: Some(catalog),
            catalog_identity: Some(catalog_identity),
            snapshot: Some(snapshot),
            control: control.map(Arc::from),
            manifest: None,
            routes: BTreeMap::new(),
            uid: self.uid,
        })
    }
    /// Cross-process Claim access keeps the existing protected typed grant.
    /// The selected bytes grant neither canonical admission nor another owner.
    pub(crate) fn select_claim_owner(
        configuration_path: &Path,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<(Self, Vec<u8>)> {
        let selected =
            Self::select_protected_native_owner(configuration_path, deadline, cancelled)?;
        let config = cmd::parse(&selected.1)?;
        crate::source_claims::family(cmd::text(&config, "schema_version")?)?;
        Ok(selected)
    }

    /// Protected transport only: the fixed typed caller must validate its
    /// exact family and scope before this filesystem can authorize an operation.
    pub(crate) fn select_protected_native_owner(
        configuration_path: &Path,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<(Self, Vec<u8>)> {
        active(deadline, cancelled)?;
        let uid = rustix::process::geteuid().as_raw();
        if rustix::process::getuid().as_raw() != uid {
            return Err(SourceCommandError::Denied("native owner setuid selection"));
        }
        protected_configuration_parents(configuration_path, uid)?;
        let mut file = tos_fd_open::open_absolute_regular(configuration_path, 1_048_576)
            .map_err(|_| SourceCommandError::Denied("native protected owner selection"))?;
        if owned(&file, uid, false)?.mode() & 0o7777 != 0o600 {
            return Err(SourceCommandError::Denied("native owner must be mode0600"));
        }
        let configuration_raw = raw(&mut file, 1_048_576, deadline, cancelled)?;
        let config = cmd::parse(&configuration_raw)?;
        cmd::validate_expiry(
            cmd::text(&config, "expires_at")?,
            &crate::source_serialization::instant()?,
        )?;
        if cmd::integer(&config, "uid")? != u64::from(uid) {
            return Err(SourceCommandError::Denied("native typed owner account"));
        }
        let root_path =
            crate::source_text_owner::normalized_absolute(cmd::text(&config, "source_root")?)?;
        if configuration_path.starts_with(root_path.join("ToS")) {
            return Err(SourceCommandError::Denied(
                "native owner authority cannot be authored content",
            ));
        }
        protected_configuration_parents(&root_path.join("root-pin"), uid)?;
        let root = tos_fd_open::open_absolute_directory(&root_path)
            .map_err(|_| SourceCommandError::Denied("native owner exact source root"))?;
        let root_metadata = owned(&root, uid, true)?;
        if root_metadata.mode() & 0o7777 != 0o700 {
            return Err(SourceCommandError::Denied(
                "native owner CLI requires a private source root",
            ));
        }
        let root_identity = inode(&root_metadata);
        let selected = Self {
            root_path,
            root,
            root_identity,
            configuration_path: configuration_path.to_path_buf(),
            configuration_raw: configuration_raw.clone(),
            uid,
        };
        Ok((selected, configuration_raw))
    }
    pub fn select_isolated(
        isolated: &IsolatedCreationRoot,
        configuration_path: &Path,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        active(deadline, cancelled)?;
        let root = isolated.path();
        let uid = rustix::process::geteuid().as_raw();
        if rustix::process::getuid().as_raw() != uid {
            return Err(SourceCommandError::Denied(
                "creation refuses changed real/effective account",
            ));
        }
        let fd = tos_fd_open::open_absolute_directory(root)
            .map_err(|_| SourceCommandError::Denied("creation selected absolute root"))?;
        let m = owned(&fd, uid, true)?;
        if inode(&m) != isolated.identity
            || inode(&owned(&isolated.directory, uid, true)?) != isolated.identity
        {
            return Err(SourceCommandError::Conflict(
                "isolated creation root replaced",
            ));
        }
        protected_configuration_parents(configuration_path, uid)?;
        let mut config = tos_fd_open::open_absolute_regular(configuration_path, 1_048_576)
            .map_err(|_| SourceCommandError::Denied("creation protected configuration path"))?;
        let config_m = owned(&config, uid, false)?;
        if config_m.mode() & 0o077 != 0 || configuration_path.starts_with(root.join("ToS")) {
            return Err(SourceCommandError::Denied(
                "creation configuration is not privately protected",
            ));
        }
        let configuration_raw = raw(&mut config, 1_048_576, deadline, cancelled)?;
        let value = cmd::parse(&configuration_raw)?;
        if Path::new(cmd::text(&value, "source_root")?) != root
            || cmd::integer(&value, "uid")? != u64::from(uid)
        {
            return Err(SourceCommandError::Denied(
                "creation configuration selects another root/account",
            ));
        }
        Ok(Self {
            root_path: root.to_path_buf(),
            root: fd,
            root_identity: inode(&m),
            configuration_path: configuration_path.to_path_buf(),
            configuration_raw,
            uid,
        })
    }
    pub(crate) fn current(
        &self,
        package: &SerializedCreation,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.current_package(CreationPackage::V1(package), deadline, cancelled)
    }
    fn current_package(
        &self,
        package: CreationPackage<'_>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.current_context(package.prepared().context(), deadline, cancelled)
    }
    pub(crate) fn current_configuration_bytes(
        &self,
        context: &cmd::CommandContext,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        active(deadline, cancelled)?;
        if rustix::process::geteuid().as_raw() != self.uid
            || rustix::process::getuid().as_raw() != self.uid
        {
            return Err(SourceCommandError::Denied("creation account changed"));
        }
        let root = tos_fd_open::open_absolute_directory(&self.root_path)
            .map_err(|_| SourceCommandError::Conflict("creation root replaced or unsafe"))?;
        if inode(&owned(&root, self.uid, true)?) != self.root_identity {
            return Err(SourceCommandError::Conflict(
                "creation selected root identity changed",
            ));
        }
        protected_configuration_parents(&self.configuration_path, self.uid)?;
        let mut fd = tos_fd_open::open_absolute_regular(&self.configuration_path, 1_048_576)
            .map_err(|_| {
                SourceCommandError::Denied("creation current configuration unavailable")
            })?;
        if owned(&fd, self.uid, false)?.mode() & 0o077 != 0 {
            return Err(SourceCommandError::Denied(
                "creation current configuration protection changed",
            ));
        }
        let bytes = raw(&mut fd, 1_048_576, deadline, cancelled)?;
        if bytes != self.configuration_raw || bytes != context.configuration_raw {
            return Err(SourceCommandError::Conflict(
                "creation delegation changed before publication",
            ));
        }
        if context.effective_uid != u64::from(self.uid) {
            return Err(SourceCommandError::Denied(
                "creation prepared account differs",
            ));
        }
        Ok(())
    }
    pub(crate) fn current_context(
        &self,
        context: &cmd::CommandContext,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.current_configuration_bytes(context, deadline, cancelled)?;
        let config = cmd::parse(&context.configuration_raw)?;
        cmd::validate_expiry(
            cmd::text(&config, "expires_at")?,
            &crate::source_serialization::instant()?,
        )?;
        Ok(())
    }

    /// Acquire before STO/PG commit locks and retain until the actual outcome.
    /// The consumer rechecks this fence at its final atomic write edge.
    pub(crate) fn hold_creation_owner<'a>(
        &'a self,
        package: CreationPackage<'a>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<CreationOwnerFence<'a>> {
        self.current_package(package, deadline, cancelled)?;
        let witness = walk(&self.root, "ToS/source-witnesses", self.uid)?;
        let lock = self.lock(&witness, deadline, cancelled)?;
        let fence = CreationOwnerFence {
            filesystem: self,
            package,
            witness,
            lock,
        };
        fence.verify_current(deadline, cancelled)?;
        Ok(fence)
    }

    /// Real maintained corpus mutex; separate open descriptions also conflict
    /// inside one process. No lock acquisition blocks beyond cancellation/time.
    fn lock(
        &self,
        witness: &File,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<File> {
        let fd: File = rustix::fs::openat(
            witness,
            CORPUS_LOCK,
            OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map(File::from)
        .map_err(|_| SourceCommandError::Denied("creation corpus lock open"))?;
        owned(&fd, self.uid, false)?;
        loop {
            active(deadline, cancelled)?;
            match rustix::fs::flock(&fd, FlockOperation::NonBlockingLockExclusive) {
                Ok(()) => break,
                Err(Errno::AGAIN) => std::thread::sleep(Duration::from_millis(5)),
                Err(_) => {
                    return Err(SourceCommandError::Denied(
                        "creation corpus lock unsupported",
                    ));
                }
            }
        }
        owned(&fd, self.uid, false)?;
        // Verify pathname still identifies the locked inode, not a replacement.
        let current = tos_fd_open::open_regular_at(witness, Path::new(CORPUS_LOCK))
            .map_err(|_| SourceCommandError::Conflict("creation corpus lock path changed"))?;
        if inode(
            &fd.metadata()
                .map_err(|_| SourceCommandError::Invalid("creation lock identity"))?,
        ) != inode(
            &current
                .metadata()
                .map_err(|_| SourceCommandError::Invalid("creation current lock identity"))?,
        ) {
            return Err(SourceCommandError::Conflict(
                "creation locked inode detached",
            ));
        }
        Ok(fd)
    }

    /// The Claim owner uses the same held corpus lock, secure directory
    /// staging and NOREPLACE publication as the source creation families.
    /// The package is privately built from a complete selected Claim cut.
    pub(crate) fn publish_claim_isolated(
        &self,
        package: &SerializedClaimCreation,
        cut: &CorpusCutReader,
        whole_call: Option<&Rc<RefCell<crate::source_claims::ClaimCallBudget>>>,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<CreationPublication> {
        if components != package.components() || software.selection() != components.capture() {
            return Err(SourceCommandError::Conflict(
                "Claim selected producer differs",
            ));
        }
        self.current_context(package.context(), deadline, cancelled)?;
        package
            .context()
            .check_from_selected_captures(cut, software, components, deadline, cancelled)?;
        let tos = walk(&self.root, "ToS", self.uid)?;
        let witness = walk(&self.root, "ToS/source-witnesses", self.uid)?;
        let _lock = self.lock(&witness, deadline, cancelled)?;
        self.current_context(package.context(), deadline, cancelled)?;
        if let Some(budget) = whole_call {
            charge_claim_reselection(budget, package.context(), None)?;
        }
        self.reselect_context(
            package.context(),
            package.home(),
            package.files(),
            Some(package.operational_sidecars()),
            cut,
            None,
            false,
            deadline,
            cancelled,
        )?;
        self.reselect_components(software, components, deadline, cancelled)?;
        if let Some(capture) = package.catalog_capture() {
            self.install_claim_capture(
                &capture.borrow(),
                &package.files()["source-create-receipt.json"],
                cut,
                whole_call,
                deadline,
                cancelled,
            )?;
        }
        let (parent_path, target_name) = package
            .home()
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Claim target parent"))?;
        let parent = walk(&self.root, parent_path, self.uid)?;
        let parent_identity = inode(&owned(&parent, self.uid, true)?);
        let mut stage = PendingCreation::create(&tos, self.uid, deadline, cancelled)?;
        let preparation = (|| {
            for (name, bytes) in package.files() {
                if let Some(budget) = whole_call {
                    charge_claim_read(budget, bytes.len(), bytes.len())?;
                }
                stage.write(name, bytes, deadline, cancelled)?;
            }
            stage
                .directory
                .sync_all()
                .map_err(|_| SourceCommandError::Invalid("Claim staging directory fsync"))?;
            self.current_context(package.context(), deadline, cancelled)?;
            if let Some(budget) = whole_call {
                charge_claim_reselection(budget, package.context(), None)?;
            }
            self.reselect_context(
                package.context(),
                package.home(),
                package.files(),
                Some(package.operational_sidecars()),
                cut,
                Some(&stage.name),
                false,
                deadline,
                cancelled,
            )?;
            self.reselect_components(software, components, deadline, cancelled)?;
            if let Some(capture) = package.catalog_capture() {
                if let Some(budget) = whole_call {
                    let bytes = capture.borrow().selected_bytes()?;
                    charge_claim_read(budget, bytes, bytes)?;
                }
                capture.borrow().verify_current(self, deadline, cancelled)?;
            }
            let current_parent = walk(&self.root, parent_path, self.uid)?;
            if inode(&owned(&current_parent, self.uid, true)?) != parent_identity {
                return Err(SourceCommandError::Conflict("Claim target parent changed"));
            }
            rustix::fs::renameat_with(
                &tos,
                stage.name.as_str(),
                &parent,
                target_name,
                RenameFlags::NOREPLACE,
            )
            .map_err(|error| {
                if error == Errno::EXIST {
                    SourceCommandError::Conflict("Claim creation target occupied")
                } else {
                    SourceCommandError::Invalid("Claim creation atomic publication")
                }
            })?;
            stage.published = true;
            Ok(())
        })();
        if let Err(error) = preparation {
            stage.rollback()?;
            return Err(error);
        }
        let durable = parent.sync_all().is_ok() && tos.sync_all().is_ok();
        Ok(CreationPublication {
            home: package.home().clone(),
            receipt_sha256: Digest256::of_bytes(&package.files()["source-create-receipt.json"]),
            durability: if durable {
                CreationDurability::DirectoriesSynced
            } else {
                CreationDurability::PublishedSyncIncomplete
            },
            replayed: false,
        })
    }

    /// Cold exact replay: the caller must first reconstruct this package from
    /// retained bytes and the original selected cut. The current filesystem is
    /// independently reselected under the same corpus lock before success.
    pub(crate) fn replay_claim_isolated(
        &self,
        package: &SerializedClaimCreation,
        original_cut: &CorpusCutReader,
        current_context: &cmd::CommandContext,
        current_cut: &CorpusCutReader,
        current_catalog: Option<&ClaimCatalogCapture>,
        whole_call: Option<&Rc<RefCell<crate::source_claims::ClaimCallBudget>>>,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<CreationPublication> {
        if !package.command().replayed
            || components != package.components()
            || software.selection() != components.capture()
        {
            return Err(SourceCommandError::Conflict(
                "Claim retained replay basis differs",
            ));
        }
        self.current_context(package.context(), deadline, cancelled)?;
        if current_context.base_revision != current_cut.current().revision() {
            return Err(SourceCommandError::Conflict("Claim current cut differs"));
        }
        if let Some(catalog) = current_catalog {
            if let Some(budget) = whole_call {
                charge_claim_read(budget, catalog.selected_bytes()?, catalog.selected_bytes()?)?;
            }
            catalog.bind_cut(current_cut, deadline, cancelled)?;
            catalog.verify_current(self, deadline, cancelled)?;
        }
        package.context().check_from_selected_captures(
            original_cut,
            software,
            components,
            deadline,
            cancelled,
        )?;
        let witness = walk(&self.root, "ToS/source-witnesses", self.uid)?;
        let _lock = self.lock(&witness, deadline, cancelled)?;
        self.current_context(package.context(), deadline, cancelled)?;
        self.verify_claim_original_capture(package, original_cut, whole_call, deadline, cancelled)?;
        if let Some(budget) = whole_call {
            charge_claim_reselection(budget, current_context, None)?;
        }
        self.reselect_context(
            current_context,
            package.home(),
            &BTreeMap::new(),
            Some(package.operational_sidecars()),
            current_cut,
            None,
            false,
            deadline,
            cancelled,
        )?;
        self.reselect_components(software, components, deadline, cancelled)?;
        if let Some(catalog) = current_catalog {
            if let Some(budget) = whole_call {
                charge_claim_read(budget, catalog.selected_bytes()?, catalog.selected_bytes()?)?;
            }
            catalog.verify_current(self, deadline, cancelled)?;
        }
        let directory = walk(&self.root, package.home().as_str(), self.uid)?;
        let parent_path = package
            .home()
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Claim replay parent"))?
            .0;
        let parent = walk(&self.root, parent_path, self.uid)?;
        let tos = walk(&self.root, "ToS", self.uid)?;
        let durable =
            directory.sync_all().is_ok() && parent.sync_all().is_ok() && tos.sync_all().is_ok();
        Ok(CreationPublication {
            home: package.home().clone(),
            receipt_sha256: Digest256::of_bytes(&package.files()["source-create-receipt.json"]),
            durability: if durable {
                CreationDurability::DirectoriesSynced
            } else {
                CreationDurability::PublishedSyncIncomplete
            },
            replayed: true,
        })
    }

    /// Apply only an internally reconstructed Claim correction under the
    /// historical-create lock. The proposal is checked against the exact
    /// protected predecessor; generic PreparedCommand has no writer route.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn publish_claim_revision_isolated(
        &self,
        context: &cmd::CommandContext,
        home: &RelativePath,
        before: &BTreeMap<String, Vec<u8>>,
        operational_sidecars: &BTreeSet<String>,
        plan: &cmd::PreparedCommand,
        original_cut: &CorpusCutReader,
        current_cut: &CorpusCutReader,
        current_catalog: Option<&ClaimCatalogCapture>,
        whole_call: &Rc<RefCell<crate::source_claims::ClaimCallBudget>>,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<CreationPublication> {
        if plan.operation != "claim.revise"
            || plan.base_revision != current_cut.current().revision()
            || context.base_revision != plan.base_revision
        {
            return Err(SourceCommandError::Conflict(
                "Claim revision selected operation/cut",
            ));
        }
        context.check_from_selected_captures(
            current_cut,
            software,
            components,
            deadline,
            cancelled,
        )?;
        self.current_context(context, deadline, cancelled)?;
        let request = cmd::parse(&context.request_raw)?;
        if cmd::text(&request, "operation")? != "claim.revise"
            || plan.request_canonical_sha256 != cmd::record_digest(&request)?
            || plan.configuration_raw_sha256 != Digest256::of_bytes(&context.configuration_raw)
        {
            return Err(SourceCommandError::Conflict(
                "Claim revision prepared request differs",
            ));
        }
        let receipt = cmd::field(&plan.response, "receipt")?;
        let receipt_digest = cmd::record_digest(receipt)?;
        if plan.replayed {
            if !plan.changes.is_empty() {
                return Err(SourceCommandError::Conflict(
                    "Claim replay proposal changes bytes",
                ));
            }
            let witness = walk(&self.root, "ToS/source-witnesses", self.uid)?;
            let _lock = self.lock(&witness, deadline, cancelled)?;
            self.current_context(context, deadline, cancelled)?;
            charge_claim_reselection(whole_call, context, None)?;
            self.reselect_context(
                context,
                home,
                &BTreeMap::new(),
                Some(operational_sidecars),
                current_cut,
                None,
                false,
                deadline,
                cancelled,
            )?;
            self.reselect_components(software, components, deadline, cancelled)?;
            if let Some(catalog) = current_catalog {
                charge_claim_read(
                    whole_call,
                    catalog.selected_bytes()?,
                    catalog.selected_bytes()?,
                )?;
                catalog.bind_cut(current_cut, deadline, cancelled)?;
                catalog.verify_current(self, deadline, cancelled)?;
                self.read_claim_catalog_capture(
                    before.get("source-create-receipt.json").ok_or(
                        SourceCommandError::Conflict("Claim original creation receipt absent"),
                    )?,
                    original_cut,
                    Some(whole_call),
                    false,
                    deadline,
                    cancelled,
                )?;
            }
            let target = walk(&self.root, home.as_str(), self.uid)?;
            if !claim_flat_matches(
                &target, self.uid, before, whole_call, 8_388_608, deadline, cancelled,
            )? {
                return Err(SourceCommandError::Conflict(
                    "Claim replay current package differs",
                ));
            }
            target
                .sync_all()
                .map_err(|_| SourceCommandError::Invalid("Claim replay directory fsync"))?;
            return Ok(CreationPublication {
                home: home.clone(),
                receipt_sha256: receipt_digest,
                durability: CreationDurability::DirectoriesSynced,
                replayed: true,
            });
        }
        let previous_revision = cmd::field(receipt, "previous_revision")?;
        if previous_revision != &crate::source_claims::revision(before)? {
            return Err(SourceCommandError::Conflict(
                "Claim revision predecessor package digest",
            ));
        }
        let archive_path = cmd::text(receipt, "archive_path")?;
        let claim_id = cmd::text(cmd::field(receipt, "previous_source")?, "id")?;
        let expected_archive = format!(
            "ToS/source-witnesses/.record-revisions/{}-{}",
            Digest256::of_bytes(claim_id.as_bytes()).to_hex(),
            cmd::text(receipt, "previous_revision")?
                .strip_prefix("sha256:")
                .ok_or(SourceCommandError::Invalid(
                    "Claim predecessor revision encoding"
                ))?
        );
        if archive_path != expected_archive || before.len() > 64 {
            return Err(SourceCommandError::Conflict(
                "Claim revision archive locator/scope",
            ));
        }
        let source_path =
            cmd::text(&cmd::parse(&context.configuration_raw)?, "source_path")?.to_owned();
        if source_path != format!("{}/source-claims.jsonl", home.as_str()) {
            return Err(SourceCommandError::Conflict(
                "Claim revision owner home differs",
            ));
        }
        let manifest = cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_source_package_archive_v1"),
            ),
            ("source_path", cmd::string(&source_path)),
            ("source", cmd::field(receipt, "previous_source")?.clone()),
            ("revision", previous_revision.clone()),
            ("files", crate::source_claims::archive_refs(before)),
        ]);
        let manifest_raw = cmd::published(&manifest)?;
        let mut archive: BTreeMap<String, &[u8]> = BTreeMap::new();
        for bytes in before.values() {
            archive.insert(
                format!("{}.blob", Digest256::of_bytes(bytes).to_hex()),
                bytes,
            );
        }
        archive.insert("manifest.json".into(), &manifest_raw);
        let archive_total = archive
            .values()
            .try_fold(0usize, |sum, bytes| {
                sum.checked_add(bytes.len())
                    .filter(|value| *value <= 10_485_760)
            })
            .ok_or(SourceCommandError::Invalid(
                "Claim predecessor archive byte budget",
            ))?;
        if archive.len() > 65 || archive_total == 0 {
            return Err(SourceCommandError::Invalid(
                "Claim predecessor archive scope",
            ));
        }
        let mut after: BTreeMap<String, &[u8]> = before
            .iter()
            .map(|(name, bytes)| (name.clone(), bytes.as_slice()))
            .collect();
        let mut archive_seen = BTreeSet::new();
        let mut current_seen = BTreeSet::new();
        let archive_prefix = format!("{archive_path}/");
        let home_prefix = format!("{}/", home.as_str());
        let mut archive_changes = Vec::new();
        for change in &plan.changes {
            let path = change.path.as_str();
            let bytes = change.after.as_ref().ok_or(SourceCommandError::Invalid(
                "Claim revision removes a member",
            ))?;
            if let Some(name) = path.strip_prefix(&archive_prefix) {
                if name.contains('/')
                    || change.before.is_some()
                    || archive.get(name).copied() != Some(bytes.as_slice())
                    || !archive_seen.insert(name.to_owned())
                {
                    return Err(SourceCommandError::Conflict(
                        "Claim archive proposal differs",
                    ));
                }
                archive_changes.push(change.clone());
            } else if let Some(name) = path.strip_prefix(&home_prefix) {
                if name.contains('/')
                    || !["source-claims.jsonl", "claim-revision-history.json"].contains(&name)
                        && !name.ends_with(".human-forms.json")
                    || !current_seen.insert(name.to_owned())
                    || change.before != before.get(name).map(|raw| Digest256::of_bytes(raw))
                {
                    return Err(SourceCommandError::Conflict(
                        "Claim current proposal differs",
                    ));
                }
                after.insert(name.to_owned(), bytes);
            } else {
                return Err(SourceCommandError::Conflict(
                    "Claim revision changes another path",
                ));
            }
        }
        let mut expected_archive_additions = BTreeSet::new();
        for (name, bytes) in &archive {
            let path = RelativePath::parse(&format!("{archive_path}/{name}"))
                .map_err(|_| SourceCommandError::Invalid("Claim archive member path"))?;
            if let Some(member) = current_cut.current().member(&path) {
                if member.sha256 != Digest256::of_bytes(bytes)
                    || member.size_bytes != bytes.len() as u64
                    || context.file(&path)? != Some(*bytes)
                {
                    return Err(SourceCommandError::Conflict(
                        "Claim selected archive differs",
                    ));
                }
            } else {
                expected_archive_additions.insert(name.clone());
            }
        }
        if archive_seen != expected_archive_additions
            || current_seen.len() != 3
            || !current_seen.contains("source-claims.jsonl")
            || !current_seen.contains("claim-revision-history.json")
        {
            return Err(SourceCommandError::Conflict(
                "Claim revision plan incomplete",
            ));
        }
        let after_total = after
            .values()
            .try_fold(0usize, |sum, bytes| {
                sum.checked_add(bytes.len())
                    .filter(|value| *value <= 8_388_608)
            })
            .ok_or(SourceCommandError::Invalid(
                "Claim revised package byte budget",
            ))?;
        if after.len() > 64 || after_total == 0 || after.values().any(|raw| raw.len() > 8_388_608) {
            return Err(SourceCommandError::Invalid(
                "Claim revised package capacity",
            ));
        }
        let witness = walk(&self.root, "ToS/source-witnesses", self.uid)?;
        let _lock = self.lock(&witness, deadline, cancelled)?;
        self.current_context(context, deadline, cancelled)?;
        if let Some(catalog) = current_catalog {
            charge_claim_read(
                whole_call,
                catalog.selected_bytes()?,
                catalog.selected_bytes()?,
            )?;
            catalog.bind_cut(current_cut, deadline, cancelled)?;
            catalog.verify_current(self, deadline, cancelled)?;
            self.read_claim_catalog_capture(
                before
                    .get("source-create-receipt.json")
                    .ok_or(SourceCommandError::Conflict(
                        "Claim original creation receipt absent",
                    ))?,
                original_cut,
                Some(whole_call),
                false,
                deadline,
                cancelled,
            )?;
        }
        let archive_leaf = archive_path
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Claim archive parent"))?
            .1;
        let archived_before = if let Ok(parent) = child(&witness, ".record-revisions") {
            match rustix::fs::statat(&parent, archive_leaf, AtFlags::SYMLINK_NOFOLLOW) {
                Ok(_) => {
                    let directory = child(&parent, archive_leaf)?;
                    if !claim_flat_matches(
                        &directory, self.uid, &archive, whole_call, 10_485_760, deadline, cancelled,
                    )? {
                        return Err(SourceCommandError::Conflict(
                            "Claim existing archive differs",
                        ));
                    }
                    true
                }
                Err(Errno::NOENT) => false,
                Err(_) => return Err(SourceCommandError::Denied("Claim archive path unsafe")),
            }
        } else {
            false
        };
        charge_claim_reselection(
            whole_call,
            context,
            archived_before.then_some(archive_changes.as_slice()),
        )?;
        self.reselect_context_with_claim_changes(
            context,
            home,
            &BTreeMap::new(),
            Some(operational_sidecars),
            current_cut,
            None,
            false,
            archived_before.then_some(archive_changes.as_slice()),
            deadline,
            cancelled,
        )?;
        self.reselect_components(software, components, deadline, cancelled)?;
        let target = walk(&self.root, home.as_str(), self.uid)?;
        if !claim_flat_matches(
            &target, self.uid, before, whole_call, 8_388_608, deadline, cancelled,
        )? {
            return Err(SourceCommandError::Conflict(
                "Claim live predecessor changed",
            ));
        }
        let tos = walk(&self.root, "ToS", self.uid)?;
        let archive_parent = match child(&witness, ".record-revisions") {
            Ok(directory) => directory,
            Err(_) => {
                rustix::fs::mkdirat(&witness, ".record-revisions", Mode::from_raw_mode(0o700))
                    .map_err(|_| SourceCommandError::Denied("Claim archive parent creation"))?;
                witness
                    .sync_all()
                    .map_err(|_| SourceCommandError::Invalid("Claim archive parent fsync"))?;
                child(&witness, ".record-revisions")?
            }
        };
        owned(&archive_parent, self.uid, true)?;
        let archived =
            rustix::fs::statat(&archive_parent, archive_leaf, AtFlags::SYMLINK_NOFOLLOW).is_ok();
        if archived {
            let directory = child(&archive_parent, archive_leaf)?;
            if !claim_flat_matches(
                &directory, self.uid, &archive, whole_call, 10_485_760, deadline, cancelled,
            )? {
                return Err(SourceCommandError::Conflict(
                    "Claim existing archive differs",
                ));
            }
        } else {
            let mut archive_stage = PendingCreation::create(&tos, self.uid, deadline, cancelled)?;
            for (name, bytes) in &archive {
                charge_claim_read(whole_call, bytes.len(), bytes.len())?;
                archive_stage
                    .write_claim_private(name, bytes, 65, 8_388_608, deadline, cancelled)?;
            }
            archive_stage
                .directory
                .sync_all()
                .map_err(|_| SourceCommandError::Invalid("Claim archive stage fsync"))?;
            rustix::fs::renameat_with(
                &tos,
                archive_stage.name.as_str(),
                &archive_parent,
                archive_leaf,
                RenameFlags::NOREPLACE,
            )
            .map_err(|_| SourceCommandError::Conflict("Claim archive no-replace publication"))?;
            archive_stage.published = true;
            archive_parent
                .sync_all()
                .map_err(|_| SourceCommandError::Invalid("Claim archive parent fsync"))?;
            witness
                .sync_all()
                .map_err(|_| SourceCommandError::Invalid("Claim archive witness fsync"))?;
            let directory = child(&archive_parent, archive_leaf)?;
            if !claim_flat_matches(
                &directory, self.uid, &archive, whole_call, 10_485_760, deadline, cancelled,
            )? {
                return Err(SourceCommandError::Conflict("Claim stored archive differs"));
            }
        }
        let mut stage = PendingCreation::create(&tos, self.uid, deadline, cancelled)?;
        for (name, bytes) in &after {
            charge_claim_read(whole_call, bytes.len(), bytes.len())?;
            stage.write_claim_private(name, bytes, 64, 8_388_608, deadline, cancelled)?;
        }
        stage
            .directory
            .sync_all()
            .map_err(|_| SourceCommandError::Invalid("Claim revision stage fsync"))?;
        // The archive is now durably installed. Its exact added members are
        // part of the physical authored tree even though they were absent
        // from the selected predecessor cut.
        charge_claim_reselection(whole_call, context, Some(&archive_changes))?;
        self.reselect_context_with_claim_changes(
            context,
            home,
            &BTreeMap::new(),
            Some(operational_sidecars),
            current_cut,
            Some(&stage.name),
            false,
            Some(&archive_changes),
            deadline,
            cancelled,
        )?;
        self.reselect_components(software, components, deadline, cancelled)?;
        if let Some(catalog) = current_catalog {
            charge_claim_read(
                whole_call,
                catalog.selected_bytes()?,
                catalog.selected_bytes()?,
            )?;
            catalog.verify_current(self, deadline, cancelled)?;
        }
        let current_target = walk(&self.root, home.as_str(), self.uid)?;
        if inode(&owned(&current_target, self.uid, true)?)
            != inode(&owned(&target, self.uid, true)?)
            || !claim_flat_matches(
                &current_target,
                self.uid,
                before,
                whole_call,
                8_388_608,
                deadline,
                cancelled,
            )?
        {
            return Err(SourceCommandError::Conflict(
                "Claim revision target changed before exchange",
            ));
        }
        let parent_path = home
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Claim revision target parent"))?
            .0;
        let parent = walk(&self.root, parent_path, self.uid)?;
        let target_leaf = home.as_str().rsplit_once('/').unwrap().1;
        // After EXCHANGE, failure to read either side must never be caused by
        // an avoidable exhausted invocation allowance. Both possible layouts
        // and the committed archive/source/current checks are known now.
        let predecessor_bytes = before
            .values()
            .try_fold(0usize, |sum, raw| sum.checked_add(raw.len()))
            .ok_or(SourceCommandError::Unsupported(
                "Claim predecessor byte overflow",
            ))?;
        let selected_source_bytes = context
            .files
            .iter()
            .filter(|file| file.path.as_str().starts_with("ToS/"))
            .try_fold(0usize, |sum, file| sum.checked_add(file.raw.len()))
            .and_then(|sum| {
                plan.changes.iter().try_fold(sum, |sum, change| {
                    sum.checked_add(change.after.as_ref().map_or(0, Vec::len))
                })
            })
            .ok_or(SourceCommandError::Unsupported(
                "Claim successor source byte overflow",
            ))?;
        let catalog_bytes = current_catalog
            .map(|catalog| catalog.selected_bytes())
            .transpose()?
            .unwrap_or(0);
        let post_exchange_read = predecessor_bytes
            .checked_add(after_total)
            .and_then(|sum| sum.checked_mul(3))
            .and_then(|sum| sum.checked_add(archive_total))
            .and_then(|sum| sum.checked_add(selected_source_bytes))
            .and_then(|sum| sum.checked_add(catalog_bytes))
            .ok_or(SourceCommandError::Unsupported(
                "Claim post-exchange read overflow",
            ))?;
        if whole_call.borrow().remaining_read()? < post_exchange_read as u64 {
            return Err(SourceCommandError::Unsupported(
                "Claim post-exchange read budget",
            ));
        }
        whole_call.borrow().check_live(
            selected_source_bytes
                .max(predecessor_bytes)
                .max(after_total)
                .max(archive_total),
        )?;
        let exchange = rustix::fs::renameat_with(
            &tos,
            stage.name.as_str(),
            &parent,
            target_leaf,
            RenameFlags::EXCHANGE,
        );
        // From this point a returned error does not prove which package the
        // stage contains. Drop must never erase an unclassified predecessor.
        stage.published = true;
        // EXCHANGE may have taken effect even if its return or following fsync
        // failed. Inspect both exact sides before reporting or cleaning up.
        let now_target = child(&parent, target_leaf)?;
        let now_stage = child(&tos, &stage.name)?;
        let committed = claim_flat_matches(
            &now_target,
            self.uid,
            &after,
            whole_call,
            8_388_608,
            deadline,
            cancelled,
        )? && claim_flat_matches(
            &now_stage, self.uid, before, whole_call, 8_388_608, deadline, cancelled,
        )?;
        let unchanged = claim_flat_matches(
            &now_target,
            self.uid,
            before,
            whole_call,
            8_388_608,
            deadline,
            cancelled,
        )? && claim_flat_matches(
            &now_stage, self.uid, &after, whole_call, 8_388_608, deadline, cancelled,
        )?;
        if committed {
            let durable = parent.sync_all().is_ok() && tos.sync_all().is_ok();
            let archived_dir = child(&archive_parent, archive_leaf)?;
            if !claim_flat_matches(
                &archived_dir,
                self.uid,
                &archive,
                whole_call,
                10_485_760,
                deadline,
                cancelled,
            )? {
                return Err(SourceCommandError::Conflict(
                    "Claim predecessor archive changed after exchange",
                ));
            }
            self.current_context(context, deadline, cancelled)?;
            charge_claim_reselection(whole_call, context, Some(&plan.changes))?;
            self.reselect_context_with_claim_changes(
                context,
                home,
                &BTreeMap::new(),
                Some(operational_sidecars),
                current_cut,
                Some(&stage.name),
                false,
                Some(&plan.changes),
                deadline,
                cancelled,
            )?;
            self.reselect_components(software, components, deadline, cancelled)?;
            if let Some(catalog) = current_catalog {
                charge_claim_read(
                    whole_call,
                    catalog.selected_bytes()?,
                    catalog.selected_bytes()?,
                )?;
                catalog.verify_current(self, deadline, cancelled)?;
            }
            discard_claim_stage_exact(
                &tos,
                &stage.name,
                &now_stage,
                self.uid,
                before,
                whole_call,
                8_388_608,
                deadline,
                cancelled,
            )?;
            return Ok(CreationPublication {
                home: home.clone(),
                receipt_sha256: receipt_digest,
                durability: if durable {
                    CreationDurability::DirectoriesSynced
                } else {
                    CreationDurability::PublishedSyncIncomplete
                },
                replayed: false,
            });
        }
        if unchanged {
            discard_claim_stage_exact(
                &tos,
                &stage.name,
                &now_stage,
                self.uid,
                &after,
                whole_call,
                8_388_608,
                deadline,
                cancelled,
            )?;
            return Err(if exchange.is_err() {
                SourceCommandError::Unsupported("Claim atomic exchange unavailable")
            } else {
                SourceCommandError::Conflict("Claim exchange returned without new package")
            });
        }
        Err(SourceCommandError::Conflict(
            "Claim exchange left an unclassified package state",
        ))
    }

    /// A cold bounded read of the current Claim package. The Claim owner
    /// validates original/revised closure before it becomes a replay.
    pub(crate) fn read_claim_retained(
        &self,
        context: &cmd::CommandContext,
        home: &RelativePath,
        allowed_names: &BTreeSet<String>,
        whole_call: Option<&Rc<RefCell<crate::source_claims::ClaimCallBudget>>>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Option<BTreeMap<String, Vec<u8>>>> {
        self.current_context(context, deadline, cancelled)?;
        let (parent_path, name) = home
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Claim retained parent"))?;
        let parent = walk(&self.root, parent_path, self.uid)?;
        match rustix::fs::statat(&parent, name, AtFlags::SYMLINK_NOFOLLOW) {
            Err(Errno::NOENT) => return Ok(None),
            Err(_) => return Err(SourceCommandError::Denied("Claim retained target unsafe")),
            Ok(_) => {}
        }
        let directory = walk(&self.root, home.as_str(), self.uid)?;
        let entries = std::fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
            .map_err(|_| SourceCommandError::Invalid("Claim retained directory listing"))?;
        let mut names = BTreeSet::new();
        for entry in entries {
            active(deadline, cancelled)?;
            let name = entry
                .map_err(|_| SourceCommandError::Invalid("Claim retained entry"))?
                .file_name()
                .into_string()
                .map_err(|_| SourceCommandError::Invalid("Claim retained name"))?;
            if !allowed_names.contains(&name)
                || !names.insert(name)
                || names.len() > allowed_names.len()
            {
                return Err(SourceCommandError::Invalid("Claim retained entry budget"));
            }
        }
        let required = BTreeSet::from([
            "source-claims.jsonl".to_owned(),
            "source-create-request.json".to_owned(),
            "source-create-environment.json".to_owned(),
            "source-create-provenance.jsonl".to_owned(),
            "source-create-receipt.json".to_owned(),
        ]);
        if !required.is_subset(&names) {
            return Err(SourceCommandError::Conflict(
                "Claim retained package incomplete",
            ));
        }
        // Maintained unrevised creation reads only its named adjacent members
        // with a per-file limit. Once HISTORY exists, its revision owner uses
        // the stricter flat-package 64-file/8 MiB aggregate contract.
        let revised = names.contains("claim-revision-history.json");
        if revised && names.len() > 64 {
            return Err(SourceCommandError::Invalid(
                "Claim revised package file budget",
            ));
        }
        let aggregate_limit = if revised {
            8_388_608u64
        } else {
            MAX_BYTES as u64
        };
        let mut inspected = BTreeMap::new();
        let mut declared_total = 0u64;
        for name in &names {
            active(deadline, cancelled)?;
            let file = tos_fd_open::open_regular_at(&directory, Path::new(name))
                .map_err(|_| SourceCommandError::Denied("Claim retained member unsafe"))?;
            let metadata = owned(&file, self.uid, false)?;
            let lock = name.ends_with(".writer.lock");
            let mode = metadata.mode() & 0o7777;
            if (lock && (mode != 0o600 || metadata.len() != 0))
                || (!lock && !member_mode_matches(mode, 0o644, true))
            {
                return Err(SourceCommandError::Conflict(
                    "Claim retained member mode or operational lock contents differ",
                ));
            }
            if metadata.len() > 8_388_608 {
                return Err(SourceCommandError::Invalid(
                    "Claim retained per-file byte budget",
                ));
            }
            declared_total = declared_total
                .checked_add(metadata.len())
                .ok_or(SourceCommandError::Invalid("Claim retained byte overflow"))?;
            if declared_total > aggregate_limit {
                return Err(SourceCommandError::Invalid(if revised {
                    "Claim revised package byte budget"
                } else {
                    "Claim native complete input byte budget"
                }));
            }
            inspected.insert(name.clone(), stamp(&metadata));
        }
        if let Some(whole_call) = whole_call {
            let mut budget = whole_call.borrow_mut();
            budget.check_live(declared_total as usize)?;
            budget.read(declared_total)?;
            budget.retain(declared_total as usize)?;
        }
        let mut files = BTreeMap::new();
        let mut total = 0u64;
        for name in names {
            let mut file = tos_fd_open::open_regular_at(&directory, Path::new(&name))
                .map_err(|_| SourceCommandError::Denied("Claim retained member unsafe"))?;
            let lock = name.ends_with(".writer.lock");
            let metadata = owned(&file, self.uid, false)?;
            let mode = metadata.mode() & 0o7777;
            let accepted_mode = if lock {
                mode == 0o600
            } else {
                member_mode_matches(mode, 0o644, true)
            };
            if !accepted_mode || inspected.get(&name) != Some(&stamp(&metadata)) {
                return Err(SourceCommandError::Conflict(
                    "Claim retained member changed after metadata preflight",
                ));
            }
            let bytes = raw(
                &mut file,
                if lock { 1 } else { 8_388_608 },
                deadline,
                cancelled,
            )?;
            if lock && !bytes.is_empty() {
                return Err(SourceCommandError::Conflict(
                    "Claim operational lock contains data",
                ));
            }
            total = total
                .checked_add(bytes.len() as u64)
                .ok_or(SourceCommandError::Invalid("Claim retained byte overflow"))?;
            if total > aggregate_limit {
                return Err(SourceCommandError::Invalid(if revised {
                    "Claim revised package byte budget"
                } else {
                    "Claim native complete input byte budget"
                }));
            }
            files.insert(name, bytes);
        }
        Ok(Some(files))
    }

    /// Publish only a privately constructed, genuinely serialized handler
    /// package. Canonical source admission is deliberately a separate gate.
    /// This reusable filesystem mechanism is executed only with independently
    /// selected isolated-owner authority until that gate exists.
    pub fn publish_isolated(
        &self,
        package: &SerializedCreation,
        cut: &CorpusCutReader,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<CreationPublication> {
        self.publish(
            package, cut, software, components, deadline, cancelled, None,
        )
    }

    pub fn publish_sign_isolated(
        &self,
        package: &SerializedCreation,
        cut: &CorpusCutReader,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        local_worker: &mut tos_validation::source_cut::CutWorkerSchemaExecutor,
        assessment_worker: &mut tos_validation::source_cut::CutWorkerSchemaExecutor,
        limits: tos_validation::assessment::AssessmentLimits,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<CreationPublication> {
        crate::source_sign::require_assessment_profile(assessment_worker)?;
        self.current(package, limits.deadline, cancelled)?;
        let mut read = crate::source_sign::SignPromotionRead::select(
            &self.configuration_path,
            package.prepared.context(),
            cut,
            limits.deadline,
            cancelled,
        )?;
        read.prepare_sources(package.prepared.context(), local_worker, limits, cancelled)?;
        crate::source_sign::finish_worker(local_worker, limits.deadline, cancelled)?;
        self.publish(
            package,
            cut,
            software,
            components,
            limits.deadline,
            cancelled,
            Some((&mut read, local_worker, assessment_worker, limits)),
        )
    }

    fn publish(
        &self,
        package: &SerializedCreation,
        cut: &CorpusCutReader,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        deadline: Instant,
        cancelled: &AtomicBool,
        sign: Option<(
            &mut crate::source_sign::SignPromotionRead<'_>,
            &mut tos_validation::source_cut::CutWorkerSchemaExecutor,
            &mut tos_validation::source_cut::CutWorkerSchemaExecutor,
            tos_validation::assessment::AssessmentLimits,
        )>,
    ) -> SourceCommandResult<CreationPublication> {
        if components != package.prepared.components() {
            return Err(SourceCommandError::Conflict(
                "creation sealed software subset differs",
            ));
        }
        if (package.prepared.family() == crate::source_creation::CreationFamily::Sign)
            != sign.is_some()
        {
            return Err(SourceCommandError::Unsupported(
                "Sign publication requires held current assessment journal fences",
            ));
        }
        self.current(package, deadline, cancelled)?;
        package
            .prepared
            .context()
            .check_from_selected_captures(cut, software, components, deadline, cancelled)?;
        let tos = walk(&self.root, "ToS", self.uid)?;
        let witness = walk(&self.root, "ToS/source-witnesses", self.uid)?;
        let _lock = self.lock(&witness, deadline, cancelled)?;
        self.current(package, deadline, cancelled)?;
        self.reselect(package, cut, None, false, deadline, cancelled)?;
        self.reselect_components(software, components, deadline, cancelled)?;
        let (parent_path, target_name) = package
            .prepared
            .home()
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("creation target parent"))?;
        let parent = walk(&self.root, parent_path, self.uid)?;
        let parent_identity = inode(&owned(&parent, self.uid, true)?);
        let mut stage = PendingCreation::create(&tos, self.uid, deadline, cancelled)?;
        let preparation = (|| {
            for (name, bytes) in package.prepared.files() {
                stage.write(name, bytes, deadline, cancelled)?;
            }
            stage
                .directory
                .sync_all()
                .map_err(|_| SourceCommandError::Invalid("creation staging directory fsync"))?;
            let mut publish = |guard: Option<&mut dyn FnMut() -> SourceCommandResult<()>>| {
                self.current(package, deadline, cancelled)?;
                self.reselect(package, cut, Some(&stage.name), false, deadline, cancelled)?;
                self.reselect_components(software, components, deadline, cancelled)?;
                if let Some(guard) = guard {
                    guard()?;
                }
                let current_parent = walk(&self.root, parent_path, self.uid)?;
                if inode(&owned(&current_parent, self.uid, true)?) != parent_identity {
                    return Err(SourceCommandError::Conflict(
                        "creation target parent changed",
                    ));
                }
                // NOREPLACE treats files, directories and symlinks as occupied.
                rustix::fs::renameat_with(
                    &tos,
                    stage.name.as_str(),
                    &parent,
                    target_name,
                    RenameFlags::NOREPLACE,
                )
                .map_err(|error| {
                    if error == Errno::EXIST {
                        SourceCommandError::Conflict("creation target is already occupied")
                    } else {
                        SourceCommandError::Invalid("creation atomic no-replace publication")
                    }
                })?;
                stage.published = true;
                Ok(())
            };
            if let Some((read, local, assessment, limits)) = sign {
                read.with_current_basis(
                    package.prepared.context(),
                    local,
                    assessment,
                    limits,
                    cancelled,
                    |basis, guard| {
                        let request = cmd::parse(&package.prepared.context().request_raw)?;
                        if !cmd::same(
                            cmd::field(cmd::field(&request, "record")?, "promotion_basis")?,
                            basis,
                        )? {
                            return Err(SourceCommandError::Conflict(
                                "Sign promotion basis changed before publication",
                            ));
                        }
                        publish(Some(guard))
                    },
                )
            } else {
                publish(None)
            }
        })();
        if let Err(error) = preparation {
            stage.rollback()?;
            return Err(error);
        }
        // After rename, a failed directory sync is an observed publication with
        // uncertain crash durability. Never misreport it as rolled back.
        let parent_synced = parent.sync_all().is_ok();
        let staging_parent_synced = tos.sync_all().is_ok();
        let durable = parent_synced && staging_parent_synced;
        Ok(CreationPublication {
            home: package.prepared.home().clone(),
            receipt_sha256: Digest256::of_bytes(
                package
                    .prepared
                    .files()
                    .get("source-create-receipt.json")
                    .ok_or(SourceCommandError::Invalid("creation receipt absent"))?,
            ),
            durability: if durable {
                CreationDurability::DirectoriesSynced
            } else {
                CreationDurability::PublishedSyncIncomplete
            },
            replayed: false,
        })
    }

    /// Exact repeat of the original package. A later source/Claim successor
    /// needs the separate maintained history reconstruction route; it is never
    /// treated as the original package by a current-version fallback here.
    pub fn replay_isolated(
        &self,
        package: &SerializedCreation,
        original_base: &CorpusCutReader,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<CreationPublication> {
        self.replay(
            package,
            original_base,
            software,
            components,
            deadline,
            cancelled,
            None,
        )
    }

    pub fn replay_sign_isolated(
        &self,
        package: &SerializedCreation,
        original_base: &CorpusCutReader,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        local_worker: &mut tos_validation::source_cut::CutWorkerSchemaExecutor,
        assessment_worker: &mut tos_validation::source_cut::CutWorkerSchemaExecutor,
        limits: tos_validation::assessment::AssessmentLimits,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<CreationPublication> {
        crate::source_sign::require_assessment_profile(assessment_worker)?;
        self.current(package, limits.deadline, cancelled)?;
        let mut read = crate::source_sign::SignPromotionRead::select(
            &self.configuration_path,
            package.prepared.context(),
            original_base,
            limits.deadline,
            cancelled,
        )?;
        read.prepare_sources(package.prepared.context(), local_worker, limits, cancelled)?;
        crate::source_sign::finish_worker(local_worker, limits.deadline, cancelled)?;
        self.replay(
            package,
            original_base,
            software,
            components,
            limits.deadline,
            cancelled,
            Some((&mut read, local_worker, assessment_worker, limits)),
        )
    }

    fn replay(
        &self,
        package: &SerializedCreation,
        original_base: &CorpusCutReader,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        deadline: Instant,
        cancelled: &AtomicBool,
        sign: Option<(
            &mut crate::source_sign::SignPromotionRead<'_>,
            &mut tos_validation::source_cut::CutWorkerSchemaExecutor,
            &mut tos_validation::source_cut::CutWorkerSchemaExecutor,
            tos_validation::assessment::AssessmentLimits,
        )>,
    ) -> SourceCommandResult<CreationPublication> {
        if components != package.prepared.components() {
            return Err(SourceCommandError::Conflict(
                "creation sealed software subset differs",
            ));
        }
        if (package.prepared.family() == crate::source_creation::CreationFamily::Sign)
            != sign.is_some()
        {
            return Err(SourceCommandError::Unsupported(
                "Sign replay requires current assessment journal fences",
            ));
        }
        if components != package.prepared.components() {
            return Err(SourceCommandError::Conflict(
                "creation sealed software subset differs",
            ));
        }
        self.current(package, deadline, cancelled)?;
        package.prepared.context().check_from_selected_captures(
            original_base,
            software,
            components,
            deadline,
            cancelled,
        )?;
        let witness = walk(&self.root, "ToS/source-witnesses", self.uid)?;
        let _lock = self.lock(&witness, deadline, cancelled)?;
        self.current(package, deadline, cancelled)?;
        let replay = |guard: Option<&mut dyn FnMut() -> SourceCommandResult<()>>| {
            self.reselect(package, original_base, None, true, deadline, cancelled)?;
            self.reselect_components(software, components, deadline, cancelled)?;
            if let Some(guard) = guard {
                guard()?;
            }
            let directory = walk(&self.root, package.prepared.home().as_str(), self.uid)?;
            let parent_path = package
                .prepared
                .home()
                .as_str()
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid("creation replay parent"))?
                .0;
            let parent = walk(&self.root, parent_path, self.uid)?;
            // Repeating fsync can resolve a prior post-rename durability ambiguity.
            let tos = walk(&self.root, "ToS", self.uid)?;
            let directory_synced = directory.sync_all().is_ok();
            let parent_synced = parent.sync_all().is_ok();
            let staging_parent_synced = tos.sync_all().is_ok();
            let durable = directory_synced && parent_synced && staging_parent_synced;
            Ok(CreationPublication {
                home: package.prepared.home().clone(),
                receipt_sha256: Digest256::of_bytes(
                    package
                        .prepared
                        .files()
                        .get("source-create-receipt.json")
                        .ok_or(SourceCommandError::Invalid(
                            "creation replay receipt absent",
                        ))?,
                ),
                durability: if durable {
                    CreationDurability::DirectoriesSynced
                } else {
                    CreationDurability::PublishedSyncIncomplete
                },
                replayed: true,
            })
        };
        if let Some((read, local, assessment, limits)) = sign {
            read.with_current_basis(
                package.prepared.context(),
                local,
                assessment,
                limits,
                cancelled,
                |basis, guard| {
                    let request = cmd::parse(&package.prepared.context().request_raw)?;
                    if !cmd::same(
                        cmd::field(cmd::field(&request, "record")?, "promotion_basis")?,
                        basis,
                    )? {
                        return Err(SourceCommandError::Conflict(
                            "Sign current promotion basis differs on replay",
                        ));
                    }
                    replay(Some(guard))
                },
            )
        } else {
            replay(None)
        }
    }

    fn reselect_components(
        &self,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        if components.capture() != software.selection() {
            return Err(SourceCommandError::Conflict(
                "creation current component capture changed",
            ));
        }
        let mut remaining = 16_777_216u64;
        for member in components.members() {
            active(deadline, cancelled)?;
            if member.path.as_str().starts_with("ToS/") || member.size_bytes > remaining {
                return Err(SourceCommandError::Invalid(
                    "creation selected software namespace/aggregate budget",
                ));
            }
            remaining -= member.size_bytes;
            let captured = software
                .read_selected_component(components, &member.path, 8_388_608, deadline, cancelled)
                .map_err(|_| {
                    SourceCommandError::Conflict("creation selected software custody changed")
                })?;
            let (parent_path, leaf) = member
                .path
                .as_str()
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid("creation component parent"))?;
            let parent = walk(&self.root, parent_path, self.uid)?;
            let mut file =
                tos_fd_open::open_regular_at(&parent, Path::new(leaf)).map_err(|_| {
                    SourceCommandError::Conflict("creation current component unavailable")
                })?;
            owned(&file, self.uid, false)?;
            if raw(&mut file, 8_388_608, deadline, cancelled)? != captured {
                return Err(SourceCommandError::Conflict(
                    "creation current producer component differs from selected capture",
                ));
            }
        }
        Ok(())
    }

    fn reselect(
        &self,
        package: &SerializedCreation,
        cut: &CorpusCutReader,
        staging: Option<&str>,
        published: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.reselect_context(
            package.prepared.context(),
            package.prepared.home(),
            package.prepared.files(),
            None,
            cut,
            staging,
            published,
            deadline,
            cancelled,
        )
    }
    fn reselect_context(
        &self,
        context: &cmd::CommandContext,
        home: &RelativePath,
        package_files: &BTreeMap<String, Vec<u8>>,
        operational_sidecars: Option<&BTreeSet<String>>,
        cut: &CorpusCutReader,
        staging: Option<&str>,
        published: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.reselect_context_with_claim_changes(
            context,
            home,
            package_files,
            operational_sidecars,
            cut,
            staging,
            published,
            None,
            deadline,
            cancelled,
        )
    }
    fn reselect_context_with_claim_changes(
        &self,
        context: &cmd::CommandContext,
        home: &RelativePath,
        package_files: &BTreeMap<String, Vec<u8>>,
        operational_sidecars: Option<&BTreeSet<String>>,
        cut: &CorpusCutReader,
        staging: Option<&str>,
        published: bool,
        claim_changes: Option<&[SourceChange]>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        if cut.current().revision() != context.base_revision {
            return Err(SourceCommandError::Conflict("creation base cut changed"));
        }
        let mut observed = BTreeMap::new();
        let mut total = 0usize;
        let mut directories = 0usize;
        let tos = walk(&self.root, "ToS", self.uid)?;
        scan(
            &tos,
            "ToS",
            self.uid,
            staging,
            operational_sidecars,
            None,
            &mut observed,
            &mut total,
            &mut directories,
            deadline,
            cancelled,
        )?;
        let mut selected: BTreeMap<_, _> = cut
            .current()
            .members()
            .map(|m| (m.path.as_str().to_owned(), (m.sha256, m.size_bytes, m.mode)))
            .collect();
        if published {
            for (name, bytes) in package_files {
                let path = format!("{}/{name}", home.as_str());
                if selected
                    .insert(
                        path,
                        (Digest256::of_bytes(bytes), bytes.len() as u64, 0o644),
                    )
                    .is_some()
                {
                    return Err(SourceCommandError::Conflict(
                        "creation replay original base already contains target",
                    ));
                }
            }
        }
        if let Some(changes) = claim_changes {
            for change in changes {
                let path = change.path.as_str();
                let current = selected.get(path).copied();
                if current.map(|(sha, _, _)| sha) != change.before {
                    return Err(SourceCommandError::Conflict(
                        "Claim revision selected predecessor differs",
                    ));
                }
                let after = change.after.as_ref().ok_or(SourceCommandError::Invalid(
                    "Claim revision cannot remove selected member",
                ))?;
                selected.insert(
                    path.to_owned(),
                    (Digest256::of_bytes(after), after.len() as u64, 0o644),
                );
            }
        }
        if observed.len() != selected.len()
            || observed.iter().any(|(path, (sha, size, mode))| {
                selected
                    .get(path)
                    .is_none_or(|(expected_sha, expected_size, declared_mode)| {
                        sha != expected_sha
                            || size != expected_size
                            || !member_mode_matches(
                                *mode,
                                *declared_mode,
                                operational_sidecars.is_some(),
                            )
                    })
            })
        {
            return Err(SourceCommandError::Conflict(
                "creation current authored membership/bytes/modes differ from selected base",
            ));
        }
        // Software inputs are selected separately and also reselected from the
        // actual owner tree. A restored capture alone cannot substitute them.
        for input in &context.files {
            active(deadline, cancelled)?;
            if input.path.as_str().starts_with("ToS/") {
                continue;
            }
            let (parent_path, name) = input
                .path
                .as_str()
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid("creation software parent"))?;
            let parent = walk(&self.root, parent_path, self.uid)?;
            let mut fd = tos_fd_open::open_regular_at(&parent, Path::new(name)).map_err(|_| {
                SourceCommandError::Conflict("creation current software input unavailable")
            })?;
            owned(&fd, self.uid, false)?;
            if raw(&mut fd, 8_388_608, deadline, cancelled)? != input.raw {
                return Err(SourceCommandError::Conflict(
                    "creation current software input changed",
                ));
            }
        }
        Ok(())
    }
}

fn scan(
    directory: &File,
    prefix: &str,
    uid: u32,
    staging: Option<&str>,
    operational_sidecars: Option<&BTreeSet<String>>,
    work_auxiliary: Option<&BTreeSet<String>>,
    files: &mut BTreeMap<String, (Digest256, u64, u32)>,
    total: &mut usize,
    directories: &mut usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    active(deadline, cancelled)?;
    *directories = directories
        .checked_add(1)
        .ok_or(SourceCommandError::Invalid(
            "creation directory count overflow",
        ))?;
    if *directories > 8192 {
        return Err(SourceCommandError::Invalid(
            "creation directory traversal budget",
        ));
    }
    let before = owned(directory, uid, true)?;
    // This kernel-owned path addresses the pinned FD, never a request path.
    let entries = std::fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
        .map_err(|_| SourceCommandError::Invalid("creation current directory enumeration"))?;
    let mut names = BTreeSet::new();
    // The Work-only exact auxiliary names are checked and charged by their
    // owning journal/archive reader after this traversal. They must not use
    // up the ordinary authored-entry allowance in a shared parent directory.
    let entry_limit = MAX_FILES
        .checked_add(work_auxiliary.map_or(0, BTreeSet::len))
        .ok_or(SourceCommandError::Invalid(
            "creation directory entry budget overflow",
        ))?;
    for entry in entries {
        active(deadline, cancelled)?;
        let name = entry
            .map_err(|_| SourceCommandError::Invalid("creation directory entry"))?
            .file_name()
            .into_string()
            .map_err(|_| SourceCommandError::Invalid("creation non-UTF8 source path"))?;
        if names.len() >= entry_limit || !names.insert(name) {
            return Err(SourceCommandError::Invalid(
                "creation directory entry budget",
            ));
        }
    }
    for name in names {
        let path = format!("{prefix}/{name}");
        if path == format!("ToS/source-witnesses/{CORPUS_LOCK}")
            || (prefix == "ToS" && staging == Some(name.as_str()))
        {
            continue;
        }
        if path == format!("ToS/source-witnesses/catalog/{CLAIM_CAPTURE_HOME}") {
            let retained = child(directory, &name)?;
            if owned(&retained, uid, true)?.mode() & 0o7777 != 0o700 {
                return Err(SourceCommandError::Denied(
                    "Claim retained catalog home mode",
                ));
            }
            // This exact generated companion has its own protected capture
            // reader. It is neither an authored member nor an arbitrary dotfile.
            continue;
        }
        if path.ends_with(".writer.lock") && operational_sidecars.is_some() {
            if !operational_sidecars.is_some_and(|sidecars| sidecars.contains(&path)) {
                return Err(SourceCommandError::Conflict(
                    "Claim current operational lock path is not delegated",
                ));
            }
            let mut sidecar = tos_fd_open::open_regular_at(directory, Path::new(&name))
                .map_err(|_| SourceCommandError::Denied("Claim operational lock unsafe"))?;
            let before = owned(&sidecar, uid, false)?;
            if before.mode() & 0o777 != 0o600
                || !raw(&mut sidecar, 1, deadline, cancelled)?.is_empty()
            {
                return Err(SourceCommandError::Conflict(
                    "Claim operational lock mode or contents changed",
                ));
            }
            let after = tos_fd_open::open_regular_at(directory, Path::new(&name))
                .map_err(|_| SourceCommandError::Conflict("Claim operational lock detached"))?;
            if stamp(&before)
                != stamp(
                    &after
                        .metadata()
                        .map_err(|_| SourceCommandError::Invalid("Claim lock metadata"))?,
                )
            {
                return Err(SourceCommandError::Conflict(
                    "Claim operational lock replaced",
                ));
            }
            continue;
        }
        // Only the Work owner may supply exact archive/journal/control names,
        // after separately verifying their protected bytes and publication
        // state. They remain eligible authored paths for every other cut.
        if work_auxiliary.is_some_and(|members| members.contains(&path)) {
            continue;
        }
        // Directory traversal and file membership are different: weak output
        // parents can contain authored Markdown. Use the existing cut owner's
        // shared component exclusions without reading its excluded payloads.
        if !tos_source_store::has_authored_source_descendants_v1(&path) {
            continue;
        }
        // Secure open rejects symlinks, devices/FIFOs and replaced components.
        if let Ok(child) = tos_fd_open::open_directory_at(directory, Path::new(&name)) {
            if path.split('/').count() > 64 {
                return Err(SourceCommandError::Invalid("creation source depth budget"));
            }
            scan(
                &child,
                &path,
                uid,
                staging,
                operational_sidecars,
                work_auxiliary,
                files,
                total,
                directories,
                deadline,
                cancelled,
            )?;
            let now =
                tos_fd_open::open_directory_at(directory, Path::new(&name)).map_err(|_| {
                    SourceCommandError::Conflict("creation traversed directory detached")
                })?;
            if inode(
                &now.metadata()
                    .map_err(|_| SourceCommandError::Invalid("creation child metadata"))?,
            ) != inode(
                &child
                    .metadata()
                    .map_err(|_| SourceCommandError::Invalid("creation child identity"))?,
            ) {
                return Err(SourceCommandError::Conflict(
                    "creation traversed directory replaced",
                ));
            }
        } else {
            if !tos_source_store::is_authored_source_path_v1(&path) {
                continue;
            }
            let mut file = tos_fd_open::open_regular_at(directory, Path::new(&name))
                .map_err(|_| SourceCommandError::Denied("creation current member unsafe"))?;
            let metadata = owned(&file, uid, false)?;
            let bytes = raw(&mut file, 8_388_608, deadline, cancelled)?;
            *total = total
                .checked_add(bytes.len())
                .ok_or(SourceCommandError::Invalid("creation total byte overflow"))?;
            if *total > MAX_BYTES || files.len() >= MAX_FILES {
                return Err(SourceCommandError::Invalid(
                    "creation current source budget",
                ));
            }
            let current = tos_fd_open::open_regular_at(directory, Path::new(&name))
                .map_err(|_| SourceCommandError::Conflict("creation current member detached"))?;
            if stamp(
                &current
                    .metadata()
                    .map_err(|_| SourceCommandError::Invalid("creation member current metadata"))?,
            ) != stamp(&metadata)
            {
                return Err(SourceCommandError::Conflict(
                    "creation current member replaced",
                ));
            }
            files.insert(
                path,
                (
                    Digest256::of_bytes(&bytes),
                    bytes.len() as u64,
                    metadata.mode() & 0o7777,
                ),
            );
        }
    }
    let after = owned(directory, uid, true)?;
    if stamp(&before) != stamp(&after) {
        return Err(SourceCommandError::Conflict(
            "creation source directory changed during enumeration",
        ));
    }
    Ok(())
}

fn claim_flat_matches<B: AsRef<[u8]>>(
    directory: &File,
    uid: u32,
    expected: &BTreeMap<String, B>,
    whole_call: &Rc<RefCell<crate::source_claims::ClaimCallBudget>>,
    limit: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<bool> {
    let before = owned(directory, uid, true)?;
    let entries = std::fs::read_dir(format!("/proc/self/fd/{}", directory.as_raw_fd()))
        .map_err(|_| SourceCommandError::Denied("Claim flat directory listing"))?;
    let mut names = BTreeSet::new();
    for entry in entries {
        active(deadline, cancelled)?;
        let name = entry
            .map_err(|_| SourceCommandError::Denied("Claim flat entry"))?
            .file_name()
            .into_string()
            .map_err(|_| SourceCommandError::Denied("Claim flat non-UTF8 name"))?;
        if names.len() >= 65 || !names.insert(name) {
            return Err(SourceCommandError::Invalid("Claim flat file count"));
        }
    }
    if names != expected.keys().cloned().collect() {
        return Ok(false);
    }
    let mut total = 0usize;
    for (name, bytes) in expected {
        let bytes = bytes.as_ref();
        let mut file = tos_fd_open::open_regular_at(directory, Path::new(name))
            .map_err(|_| SourceCommandError::Denied("Claim flat member unsafe"))?;
        let metadata = owned(&file, uid, false)?;
        let mode = metadata.mode() & 0o7777;
        if (name.ends_with(".writer.lock") && mode != 0o600)
            || (!name.ends_with(".writer.lock") && !member_mode_matches(mode, 0o644, true))
            || metadata.len() != bytes.len() as u64
        {
            return Ok(false);
        }
        total = total
            .checked_add(bytes.len())
            .filter(|size| *size <= limit)
            .ok_or(SourceCommandError::Invalid("Claim flat byte budget"))?;
        charge_claim_read(whole_call, bytes.len(), bytes.len())?;
        if raw(&mut file, bytes.len(), deadline, cancelled)?.as_slice() != bytes {
            return Ok(false);
        }
    }
    let after = owned(directory, uid, true)?;
    if stamp(&before) != stamp(&after) {
        return Err(SourceCommandError::Conflict("Claim flat directory changed"));
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn discard_claim_stage_exact<B: AsRef<[u8]>>(
    parent: &File,
    name: &str,
    directory: &File,
    uid: u32,
    expected: &BTreeMap<String, B>,
    whole_call: &Rc<RefCell<crate::source_claims::ClaimCallBudget>>,
    limit: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    if !claim_flat_matches(
        directory, uid, expected, whole_call, limit, deadline, cancelled,
    )? || inode(&owned(&child(parent, name)?, uid, true)?)
        != inode(&owned(directory, uid, true)?)
    {
        return Err(SourceCommandError::Conflict(
            "Claim owned staging changed before cleanup",
        ));
    }
    for leaf in expected.keys() {
        rustix::fs::unlinkat(directory, leaf.as_str(), AtFlags::empty())
            .map_err(|_| SourceCommandError::Invalid("Claim owned staging member cleanup"))?;
    }
    rustix::fs::unlinkat(parent, name, AtFlags::REMOVEDIR)
        .map_err(|_| SourceCommandError::Invalid("Claim owned staging directory cleanup"))?;
    parent
        .sync_all()
        .map_err(|_| SourceCommandError::Invalid("Claim staging cleanup fsync"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreationDurability {
    DirectoriesSynced,
    PublishedSyncIncomplete,
}
pub struct CreationPublication {
    pub home: RelativePath,
    pub receipt_sha256: Digest256,
    pub durability: CreationDurability,
    pub replayed: bool,
}

/// A complete executable initial-package path in an actually isolated owner
/// corpus. PreparedCommand remains an unauthorised canonical-write proposal.
pub fn execute_isolated_creation_from_captures(
    filesystem: &CreationFilesystem,
    context: &cmd::CommandContext,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(
    SerializedCreation,
    CreationPublication,
    tos_foundation::JsonValue,
)> {
    let prepared = crate::source_creation::prepare_source_creation_from_captures(
        context, cut, software, components, worker, deadline, cancelled,
    )?;
    let serialized = prepared.serialize(software, components, worker, deadline, cancelled)?;
    finish_creation_worker(worker, deadline, cancelled)?;
    active(deadline, cancelled)?;
    let publication =
        filesystem.publish_isolated(&serialized, cut, software, components, deadline, cancelled)?;
    let response = serialized.published_result(publication.replayed)?;
    Ok((serialized, publication, response))
}

pub(crate) fn finish_creation_worker(
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    worker
        .finish(deadline, cancelled)
        .map_err(|error| match error {
            tos_validation::item_rules::ItemRefusal::Deadline => {
                SourceCommandError::Denied("creation schema operation deadline")
            }
            tos_validation::item_rules::ItemRefusal::Budget
            | tos_validation::item_rules::ItemRefusal::BudgetCheck { .. } => {
                SourceCommandError::Invalid("creation schema operation budget")
            }
            tos_validation::item_rules::ItemRefusal::Source(_)
                if cancelled.load(Ordering::Relaxed) =>
            {
                SourceCommandError::Denied("creation schema operation cancelled")
            }
            _ => SourceCommandError::Unsupported("creation schema operation incomplete"),
        })
}

pub(crate) struct PendingCreation<'a> {
    parent: &'a File,
    pub(crate) directory: File,
    identity: (u64, u64),
    pub(crate) name: String,
    names: BTreeSet<String>,
    pub(crate) published: bool,
}
impl<'a> PendingCreation<'a> {
    pub(crate) fn create(
        parent: &'a File,
        uid: u32,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        for _ in 0..32 {
            active(deadline, cancelled)?;
            let mut entropy = [0u8; 24];
            File::open("/dev/urandom")
                .and_then(|mut f| f.read_exact(&mut entropy))
                .map_err(|_| SourceCommandError::Invalid("creation staging entropy unavailable"))?;
            let name = format!(
                ".source-create-{}.pending",
                Digest256::of_bytes(&entropy).to_hex()
            );
            match rustix::fs::mkdirat(parent, name.as_str(), Mode::from_raw_mode(0o700)) {
                Ok(()) => {
                    let directory = child(parent, &name)?;
                    let identity = inode(&owned(&directory, uid, true)?);
                    return Ok(Self {
                        parent,
                        directory,
                        identity,
                        name,
                        names: BTreeSet::new(),
                        published: false,
                    });
                }
                Err(Errno::EXIST) => continue,
                Err(_) => return Err(SourceCommandError::Invalid("creation staging mkdir")),
            }
        }
        Err(SourceCommandError::Conflict(
            "creation staging name collisions",
        ))
    }
    pub(crate) fn write(
        &mut self,
        name: &str,
        bytes: &[u8],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        let path = RelativePath::parse(name)
            .map_err(|_| SourceCommandError::Invalid("creation package file name"))?;
        if path.as_str().contains('/') || self.names.len() >= 40 || bytes.len() > 8_388_608 {
            return Err(SourceCommandError::Invalid(
                "creation package leaf/count/byte budget",
            ));
        }
        active(deadline, cancelled)?;
        let mut file: File = rustix::fs::openat(
            &self.directory,
            name,
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map(File::from)
        .map_err(|_| SourceCommandError::Conflict("creation staging file occupied or unsafe"))?;
        self.names.insert(name.to_owned());
        for block in bytes.chunks(65536) {
            active(deadline, cancelled)?;
            file.write_all(block)
                .map_err(|_| SourceCommandError::Invalid("creation staging write"))?;
        }
        file.set_permissions(Permissions::from_mode(0o644))
            .map_err(|_| SourceCommandError::Invalid("creation package file permissions"))?;
        file.sync_all()
            .map_err(|_| SourceCommandError::Invalid("creation package file fsync"))?;
        let mut check = tos_fd_open::open_regular_at(&self.directory, Path::new(name))
            .map_err(|_| SourceCommandError::Conflict("creation staging readback path"))?;
        if raw(&mut check, 8_388_608, deadline, cancelled)? != bytes {
            return Err(SourceCommandError::Conflict(
                "creation staging readback differs",
            ));
        }
        Ok(())
    }
    fn write_claim_private(
        &mut self,
        name: &str,
        bytes: &[u8],
        max_names: usize,
        max_file_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        let path = RelativePath::parse(name)
            .map_err(|_| SourceCommandError::Invalid("Claim staging basename"))?;
        if path.as_str().contains('/')
            || self.names.len() >= max_names
            || bytes.len() > max_file_bytes
        {
            return Err(SourceCommandError::Invalid("Claim staging file budget"));
        }
        active(deadline, cancelled)?;
        let mut file: File = rustix::fs::openat(
            &self.directory,
            name,
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map(File::from)
        .map_err(|_| SourceCommandError::Conflict("Claim staging member occupied"))?;
        self.names.insert(name.to_owned());
        for block in bytes.chunks(65536) {
            active(deadline, cancelled)?;
            file.write_all(block)
                .map_err(|_| SourceCommandError::Invalid("Claim staging write"))?;
        }
        file.sync_all()
            .map_err(|_| SourceCommandError::Invalid("Claim staging member fsync"))?;
        let mut check = tos_fd_open::open_regular_at(&self.directory, Path::new(name))
            .map_err(|_| SourceCommandError::Conflict("Claim staging member detached"))?;
        if owned(&check, rustix::process::geteuid().as_raw(), false)?.mode() & 0o7777 != 0o600
            || raw(&mut check, max_file_bytes, deadline, cancelled)? != bytes
        {
            return Err(SourceCommandError::Conflict(
                "Claim staging readback differs",
            ));
        }
        Ok(())
    }
    pub(crate) fn rollback(&mut self) -> SourceCommandResult<()> {
        if self.published {
            return Ok(());
        }
        let current = child(self.parent, &self.name)?;
        if inode(
            &current
                .metadata()
                .map_err(|_| SourceCommandError::Invalid("creation rollback metadata"))?,
        ) != self.identity
        {
            return Err(SourceCommandError::Conflict(
                "creation rollback directory replaced",
            ));
        }
        for name in &self.names {
            rustix::fs::unlinkat(&self.directory, name.as_str(), AtFlags::empty())
                .map_err(|_| SourceCommandError::Invalid("creation owned pending file cleanup"))?;
        }
        rustix::fs::unlinkat(self.parent, self.name.as_str(), AtFlags::REMOVEDIR).map_err(
            |_| SourceCommandError::Conflict("creation pending directory not empty or replaced"),
        )?;
        self.published = true;
        self.parent
            .sync_all()
            .map_err(|_| SourceCommandError::Invalid("creation pending cleanup fsync"))?;
        Ok(())
    }
}
impl Drop for PendingCreation<'_> {
    fn drop(&mut self) {
        if !self.published {
            let _ = self.rollback();
        }
    }
}

#[path = "source_expression_responsibility.rs"]
mod expression_responsibility;
pub(crate) use expression_responsibility::current_result_fields as responsibility_result_fields;
pub use expression_responsibility::{
    ExpressionResponsibilityPreparation, ExpressionResponsibilityPublication,
    ExpressionResponsibilityRecoveryDecision, check_expression_responsibility_configuration,
    execute_isolated_expression_responsibility_from_captures,
    prepare_isolated_expression_responsibility_from_proposal,
    recover_isolated_expression_responsibility_from_captures,
    replay_isolated_expression_responsibility_from_captures,
};

#[path = "source_expression_edition.rs"]
mod expression_edition;
pub(crate) use expression_edition::current_result_fields as edition_result_fields;
pub use expression_edition::{
    ExpressionEditionPreparation, ExpressionEditionPublication, ExpressionEditionRecoveryDecision,
    check_expression_edition_configuration, execute_isolated_expression_edition_from_captures,
    prepare_isolated_expression_edition_from_proposal,
    recover_isolated_expression_edition_from_captures,
    replay_isolated_expression_edition_from_captures,
};

pub use work_expression::resume_isolated_work_expression_from_captures;
pub(crate) use work_expression::{
    published_work_materializations, retained_work_request, work_expression_materializations,
    work_expression_owner_result,
};
