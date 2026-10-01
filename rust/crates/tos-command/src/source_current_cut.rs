//! Explicit immutable current-cut selection for the controlled creation owner.
//! This is not canonical source admission or automatic export on each commit.

use crate::durable_adapter::source_cohort::ManagedSourceCohort;
use crate::durable_adapter::{DurableError, DurablePgCoordinator, SelectedSourceGeneration};
use crate::source_command::{
    self as cmd, CommandContext, SourceCommandError as Error, SourceCommandResult as Result,
};
use crate::source_creation_store::{
    IsolatedCreationRoot, MAX_BYTES, MAX_FILES, PendingCreation, active, inode, owned, raw,
};
use rustix::fs::{AtFlags, Mode, RenameFlags};
use rustix::io::Errno;
use std::collections::BTreeMap;
use std::fs::{File, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{
    CanonicalProfile, Digest256, JsonValue, RelativePath, SourceRevision, canonical_bytes_v1,
};
use tos_segment_store::SegmentStore;
use tos_source_store::{
    CorpusCutReader, CorpusReader, CutReadLimits, MemberMetadata, ReadLimits, RetirementMetadata,
    Snapshot, SoftwareCaptureReader, SoftwareComponentSelectionV1, SourceMembershipV1,
};
use tos_validation::source_cut::CutWorkerSchemaExecutor;

/// A verified controlled source generation, never a v1 SourceRevision.
/// Installed current/history roots bind custody membership; the managed
/// cohort separately binds the actual Agent identity/home predicate law.
/// Undemonstrated owner rules still require their explicit complete audit.
pub struct ManagedCurrentSourceGeneration {
    store_id: [u8; 16],
    cohort: ManagedSourceCohort,
    selected: SelectedSourceGeneration,
}
impl ManagedCurrentSourceGeneration {
    /// Authenticated descriptor count; no complete metadata materialization.
    pub fn member_count(&self) -> u64 {
        self.selected.current_count()
    }
    pub fn member(
        &self,
        coordinator: &mut DurablePgCoordinator,
        store: &SegmentStore,
        path: &RelativePath,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<Option<MemberMetadata>> {
        coordinator
            .read_generation_source_metadata(store, self, path, deadline, cancel)
            .map_err(durable)
    }
    pub fn digest(&self) -> Digest256 {
        self.selected.digest()
    }
    pub fn commit_seq(&self) -> u64 {
        self.selected.through_seq()
    }
    pub fn agent_definition_digest(&self) -> Digest256 {
        self.cohort.definition_digest()
    }
    pub fn epoch(&self) -> u64 {
        self.cohort.epoch()
    }
    pub(crate) fn audit_generation(&self) -> u64 {
        self.selected.audit_generation()
    }
    pub fn cohort(&self) -> &ManagedSourceCohort {
        &self.cohort
    }
    pub(crate) fn selected(&self) -> &SelectedSourceGeneration {
        &self.selected
    }
    pub(crate) fn from_verified_successor(
        store: &SegmentStore,
        cohort: ManagedSourceCohort,
        selected: crate::durable_adapter::VerifiedWarmGeneration,
    ) -> Self {
        Self {
            store_id: store.store_id(),
            cohort,
            selected: SelectedSourceGeneration::Warm(selected),
        }
    }

    pub(crate) fn from_verified_addressed(
        store: &SegmentStore,
        cohort: ManagedSourceCohort,
        selected: crate::durable_adapter::source_cohort::VerifiedAddressedGeneration,
    ) -> Self {
        Self {
            store_id: store.store_id(),
            cohort,
            selected: SelectedSourceGeneration::Addressed(selected),
        }
    }

    /// Addressed member batch under one generation/rights/custody fence.
    /// Operational creation member and selected-source byte limits apply;
    /// these bounds do not define corpus membership or completeness.
    pub fn read_current_members(
        &self,
        coordinator: &mut DurablePgCoordinator,
        store: &SegmentStore,
        paths: &[RelativePath],
        max_member_bytes: u64,
        max_total_bytes: u64,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<Vec<crate::durable_adapter::source_cohort::ManagedCurrentMember>> {
        active(deadline, cancel)?;
        if store.store_id() != self.store_id {
            return Err(Error::Conflict("managed generation byte store differs"));
        }
        coordinator
            .read_generation_source_members(
                store,
                self,
                paths,
                max_member_bytes,
                max_total_bytes,
                deadline,
                cancel,
            )
            .map_err(durable)
    }

    /// Exact retained revision under this selection's current rights and
    /// custody fences. A historical digest alone never grants disclosure.
    pub fn read_historical_member(
        &self,
        coordinator: &mut DurablePgCoordinator,
        store: &SegmentStore,
        path: &RelativePath,
        revision: u64,
        max_bytes: u64,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<crate::durable_adapter::source_cohort::ManagedCurrentMember> {
        active(deadline, cancel)?;
        if store.store_id() != self.store_id {
            return Err(Error::Conflict("managed generation byte store differs"));
        }
        coordinator
            .read_generation_historical_source_member(
                store, self, path, revision, max_bytes, deadline, cancel,
            )
            .map_err(durable)
    }

    pub fn read_current_member(
        &self,
        coordinator: &mut DurablePgCoordinator,
        store: &SegmentStore,
        path: &RelativePath,
        max_bytes: u64,
        deadline: Instant,
        cancel: &AtomicBool,
    ) -> Result<crate::durable_adapter::source_cohort::ManagedCurrentMember> {
        active(deadline, cancel)?;
        if store.store_id() != self.store_id {
            return Err(Error::Conflict("managed generation byte store differs"));
        }
        coordinator
            .read_generation_source_member(store, self, path, max_bytes, deadline, cancel)
            .map_err(durable)
    }
}

/// V1 compatibility export of a separately selected durable generation.
/// Its manifest retains genuine v1 parent/history and complete fixity law.
pub struct ManagedCurrentSourceCut {
    generation: ManagedCurrentSourceGeneration,
    cut: CorpusCutReader,
    membership: SourceMembershipV1,
    files: BTreeMap<String, Vec<u8>>,
}
impl ManagedCurrentSourceCut {
    pub fn generation(&self) -> &ManagedCurrentSourceGeneration {
        &self.generation
    }
    pub fn cut(&self) -> &CorpusCutReader {
        &self.cut
    }
    pub(crate) fn cohort(&self) -> &ManagedSourceCohort {
        self.generation.cohort()
    }
    pub(crate) fn selected_digest(&self) -> Digest256 {
        self.generation.digest()
    }
    pub(crate) fn membership(&self) -> SourceMembershipV1 {
        self.membership
    }
    pub(crate) fn files(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.files
    }
    pub(crate) fn into_generation(self) -> ManagedCurrentSourceGeneration {
        self.generation
    }
}

/// Explicit cold selection of existing durable roots without a v1 export.
/// The present owner still performs a complete source/index/custody audit;
/// this removes the mandatory filesystem export, not that audit's O(N) cost.
/// An explicit private workspace/profile selects the existing streamed custody
/// route for all cold audits. Current bodies/metadata still obey the finite
/// creation contract; this option grants no larger complete source selection.
pub fn select_current_source_generation(
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    domain: &str,
    original: &CorpusCutReader,
    initial_revision: SourceRevision,
    initial_membership: SourceMembershipV1,
    bootstrap_context: &CommandContext,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    streamed: Option<(
        &crate::durable_adapter::PrivateGenerationWorkspace,
        crate::durable_adapter::StreamedGenerationProfile,
    )>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<ManagedCurrentSourceGeneration> {
    let reopened = match streamed {
        Some((workspace, profile)) => coordinator.cold_reopen_source_cohort_streamed(
            store,
            domain,
            original,
            initial_revision,
            initial_membership,
            bootstrap_context,
            software,
            components,
            worker,
            workspace,
            profile,
            deadline,
            cancel,
        ),
        None => coordinator.cold_reopen_source_cohort(
            store,
            domain,
            original,
            initial_revision,
            initial_membership,
            bootstrap_context,
            software,
            components,
            worker,
            deadline,
            cancel,
        ),
    }
    .map_err(durable)?;
    if reopened.selected.cut().through_commit_seq() != reopened.cohort.generation() {
        return Err(Error::Conflict(
            "managed generation and source cohort differ",
        ));
    }
    Ok(ManagedCurrentSourceGeneration {
        store_id: store.store_id(),
        cohort: reopened.cohort,
        selected: SelectedSourceGeneration::Cold(reopened.selected),
    })
}

/// Explicit versioned source-only opt-in. Cold verification still uses the
/// maintained source/index assessment, then installs independent authenticated
/// state and membership roots. This is the same operation on a cold restore;
/// prior V1/V2 descriptor objects and exact historical references are retained.
/// The caller installs the audit protocol explicitly in its isolated private DB
/// before this function; no installed-release writer is switched by selection.
pub fn select_current_source_generation_addressed(
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    domain: &str,
    original: &CorpusCutReader,
    initial_revision: SourceRevision,
    initial_membership: SourceMembershipV1,
    bootstrap_context: &CommandContext,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    streamed: Option<(
        &crate::durable_adapter::PrivateGenerationWorkspace,
        crate::durable_adapter::StreamedGenerationProfile,
    )>,
    tree_limits: tos_segment_store::AuthenticatedTreeLimitsV1,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<ManagedCurrentSourceGeneration> {
    let cold = select_current_source_generation(
        coordinator,
        store,
        domain,
        original,
        initial_revision,
        initial_membership,
        bootstrap_context,
        software,
        components,
        worker,
        streamed,
        deadline,
        cancel,
    )?;
    coordinator
        .migrate_current_source_generation_addressed(store, &cold, tree_limits, deadline, cancel)
        .map_err(durable)
}

/// Migrate the freshly verified complete export's selection while retaining
/// its genuine source bytes and export membership for the first V2 producer.
/// The audit protocol must already be installed explicitly by the caller.
pub fn migrate_current_source_cut_addressed(
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    mut export: ManagedCurrentSourceCut,
    tree_limits: tos_segment_store::AuthenticatedTreeLimitsV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<ManagedCurrentSourceCut> {
    export.generation = coordinator
        .migrate_current_source_generation_addressed(
            store,
            &export.generation,
            tree_limits,
            deadline,
            cancelled,
        )
        .map_err(durable)?;
    Ok(export)
}

pub(crate) fn durable(error: DurableError) -> Error {
    match error {
        DurableError::Source(error) => error,
        DurableError::Conflict(_) => Error::Conflict("managed current selection changed"),
        DurableError::Refused(_) => Error::Denied("managed current selection refused"),
        // These are source-owned static labels. Never expose database messages,
        // SQL or the optional storage io::Error (which may carry private paths).
        DurableError::Storage(error) => Error::Invalid(error.detail),
        DurableError::Invalid(reason) | DurableError::Corrupt(reason) => Error::Invalid(reason),
        DurableError::Indeterminate(_) => Error::Invalid("managed current selection indeterminate"),
        DurableError::Database(_) => {
            Error::Invalid("managed current selection database operation failed")
        }
    }
}
fn directory(parent: &File, name: &str, uid: u32) -> Result<File> {
    match rustix::fs::mkdirat(parent, name, Mode::from_raw_mode(0o700)) {
        Ok(()) | Err(Errno::EXIST) => (),
        Err(_) => return Err(Error::Invalid("current cut directory creation")),
    }
    let fd = tos_fd_open::open_directory_at(parent, Path::new(name))
        .map_err(|_| Error::Denied("current cut directory unsafe"))?;
    owned(&fd, uid, true)?;
    parent
        .sync_all()
        .map_err(|_| Error::Invalid("current cut directory fsync"))?;
    Ok(fd)
}
fn existing_object(
    objects: &File,
    digest: Digest256,
    bytes: &[u8],
    uid: u32,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<bool> {
    let mut file = match tos_fd_open::open_regular_at(objects, Path::new(&digest.to_hex())) {
        Ok(file) => file,
        Err(error)
            if error
                .source
                .as_ref()
                .is_some_and(|s| s.kind() == std::io::ErrorKind::NotFound) =>
        {
            return Ok(false);
        }
        Err(_) => return Err(Error::Conflict("current cut object unsafe")),
    };
    let metadata = owned(&file, uid, false)?;
    if metadata.permissions().mode() & 0o777 != 0o444 {
        return Err(Error::Conflict("current cut object is not immutable"));
    }
    if raw(&mut file, bytes.len(), deadline, cancel)? != bytes {
        return Err(Error::Conflict("current cut immutable object differs"));
    }
    Ok(true)
}
fn object(
    objects: &File,
    digest: Digest256,
    bytes: &[u8],
    uid: u32,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<()> {
    active(deadline, cancel)?;
    if Digest256::of_bytes(bytes) != digest {
        return Err(Error::Invalid("current cut object SHA differs"));
    }
    if existing_object(objects, digest, bytes, uid, deadline, cancel)? {
        return Ok(());
    }
    let mut stage = PendingCreation::create(objects, uid, deadline, cancel)?;
    let name = digest.to_hex();
    stage.write(&name, bytes, deadline, cancel)?;
    let file = tos_fd_open::open_regular_at(&stage.directory, Path::new(&name))
        .map_err(|_| Error::Conflict("current cut staged object unavailable"))?;
    file.set_permissions(Permissions::from_mode(0o444))
        .map_err(|_| Error::Invalid("current cut object mode"))?;
    file.sync_all()
        .map_err(|_| Error::Invalid("current cut object fsync"))?;
    match rustix::fs::linkat(
        &stage.directory,
        name.as_str(),
        objects,
        name.as_str(),
        AtFlags::empty(),
    ) {
        Ok(()) => (),
        Err(Errno::EXIST) => {
            if !existing_object(objects, digest, bytes, uid, deadline, cancel)? {
                return Err(Error::Conflict(
                    "current cut object disappeared during install",
                ));
            }
        }
        Err(_) => return Err(Error::Invalid("current cut immutable object install")),
    }
    stage.rollback()?;
    objects
        .sync_all()
        .map_err(|_| Error::Invalid("current cut objects fsync"))?;
    Ok(())
}
fn members(values: impl Iterator<Item = MemberMetadata>) -> JsonValue {
    JsonValue::Array(
        values
            .map(|m| {
                cmd::object(vec![
                    ("path", cmd::string(m.path.as_str())),
                    ("sha256", cmd::string(&m.sha256.to_hex())),
                    ("size_bytes", cmd::number(m.size_bytes)),
                    ("mode", cmd::number(u64::from(m.mode))),
                ])
            })
            .collect(),
    )
}
fn retirements(values: &[RetirementMetadata]) -> JsonValue {
    JsonValue::Array(
        values
            .iter()
            .map(|m| {
                cmd::object(vec![
                    ("path", cmd::string(m.path.as_str())),
                    ("sha256", cmd::string(&m.sha256.to_hex())),
                    ("event_ref", cmd::string(m.event_ref.as_str())),
                    ("event_sha256", cmd::string(&m.event_sha256.to_hex())),
                    ("event_size_bytes", cmd::number(m.event_size_bytes)),
                ])
            })
            .collect(),
    )
}
fn manifest(
    base: Option<SourceRevision>,
    validator: Digest256,
    files: JsonValue,
    identities: JsonValue,
    dependencies: JsonValue,
    retired: JsonValue,
    limits: ReadLimits,
) -> Result<(SourceRevision, Vec<u8>)> {
    let mut body = cmd::object(vec![
        ("schema_version", cmd::string("tos_corpus_snapshot_v1")),
        (
            "base_revision",
            base.map_or(JsonValue::Null, |r| cmd::string(&r.0.to_hex())),
        ),
        ("validator_sha256", cmd::string(&validator.to_hex())),
        ("files", files),
        ("identities", identities),
        ("dependencies", dependencies),
        ("retirements", retired),
    ]);
    let encoded = canonical_bytes_v1(&body, CanonicalProfile::CorpusSnapshotV1, limits.json)
        .map_err(|_| Error::Invalid("current cut manifest encoding budget"))?;
    let revision = SourceRevision(Digest256::of_bytes(&encoded));
    cmd::set(&mut body, "revision", cmd::string(&revision.0.to_hex()))?;
    let encoded = canonical_bytes_v1(&body, CanonicalProfile::CorpusSnapshotV1, limits.json)
        .map_err(|_| Error::Invalid("current cut manifest encoding"))?;
    if encoded.len() > limits.max_manifest_bytes {
        return Err(Error::Invalid("current cut manifest byte budget"));
    }
    Ok((revision, encoded))
}
fn snapshot_manifest(snapshot: &Snapshot, limits: ReadLimits) -> Result<(SourceRevision, Vec<u8>)> {
    let ids = snapshot
        .indexed_identities()
        .map(|(id, path)| (id, cmd::string(path.as_str())))
        .collect();
    let dependencies = snapshot
        .members()
        .filter_map(|m| {
            snapshot.indexed_dependencies(&m.path).map(|refs| {
                (
                    m.path.as_str(),
                    JsonValue::Array(refs.iter().map(|p| cmd::string(p.as_str())).collect()),
                )
            })
        })
        .collect();
    let encoded = manifest(
        snapshot.base_revision(),
        snapshot.validator_sha256(),
        members(snapshot.members().cloned()),
        cmd::object(ids),
        cmd::object(dependencies),
        retirements(snapshot.retirements()),
        limits,
    )?;
    if encoded.0 != snapshot.revision() {
        return Err(Error::Conflict(
            "retained original manifest reconstruction differs",
        ));
    }
    Ok(encoded)
}
fn install_manifest(
    revisions: &File,
    revision: SourceRevision,
    bytes: &[u8],
    uid: u32,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<()> {
    let name = revision.0.to_hex();
    let existing = match tos_fd_open::open_directory_at(revisions, Path::new(&name)) {
        Ok(existing) => Some(existing),
        Err(error)
            if error
                .source
                .as_ref()
                .is_some_and(|s| s.kind() == std::io::ErrorKind::NotFound) =>
        {
            None
        }
        Err(_) => return Err(Error::Conflict("current cut revision directory unsafe")),
    };
    if let Some(existing) = existing {
        owned(&existing, uid, true)?;
        let mut file = tos_fd_open::open_regular_at(&existing, Path::new("snapshot.json"))
            .map_err(|_| Error::Conflict("current cut existing manifest unsafe"))?;
        let metadata = owned(&file, uid, false)?;
        if metadata.permissions().mode() & 0o777 != 0o444 {
            return Err(Error::Conflict("current cut manifest is not immutable"));
        }
        if raw(&mut file, bytes.len(), deadline, cancel)? != bytes {
            return Err(Error::Conflict("current cut existing manifest differs"));
        }
        return Ok(());
    }
    let mut stage = PendingCreation::create(revisions, uid, deadline, cancel)?;
    stage.write("snapshot.json", bytes, deadline, cancel)?;
    let file = tos_fd_open::open_regular_at(&stage.directory, Path::new("snapshot.json"))
        .map_err(|_| Error::Conflict("current cut manifest readback unavailable"))?;
    file.set_permissions(Permissions::from_mode(0o444))
        .map_err(|_| Error::Invalid("current cut manifest mode"))?;
    file.sync_all()
        .map_err(|_| Error::Invalid("current cut manifest fsync"))?;
    stage
        .directory
        .sync_all()
        .map_err(|_| Error::Invalid("current cut snapshot fsync"))?;
    rustix::fs::renameat_with(
        revisions,
        stage.name.as_str(),
        revisions,
        name.as_str(),
        RenameFlags::NOREPLACE,
    )
    .map_err(|_| Error::Conflict("current cut revision install raced or failed"))?;
    stage.published = true;
    revisions
        .sync_all()
        .map_err(|_| Error::Invalid("current cut revisions fsync"))?;
    Ok(())
}
fn copy_cut(
    cut: &CorpusCutReader,
    objects: &File,
    revisions: &File,
    uid: u32,
    limits: ReadLimits,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<()> {
    for snapshot in cut.revisions() {
        active(deadline, cancel)?;
        let encoded = snapshot_manifest(snapshot, limits)?;
        // An already installed exact revision is immutable; validation of its
        // addressed object reads remains with CorpusReader, not a mtime cache.
        match tos_fd_open::open_directory_at(revisions, Path::new(&encoded.0.0.to_hex())) {
            Ok(existing) => {
                owned(&existing, uid, true)?;
                install_manifest(revisions, encoded.0, &encoded.1, uid, deadline, cancel)?;
                continue;
            }
            Err(error)
                if error
                    .source
                    .as_ref()
                    .is_some_and(|s| s.kind() == std::io::ErrorKind::NotFound) =>
            {
                ()
            }
            Err(_) => return Err(Error::Conflict("retained cut revision directory unsafe")),
        }
        let mut stream = cut
            .stream(snapshot.revision())
            .map_err(|_| Error::Invalid("retained source stream"))?;
        while let Some(member) = stream
            .next_member(deadline, cancel)
            .map_err(|_| Error::Invalid("retained source object/EOF"))?
        {
            object(
                objects,
                Digest256::of_bytes(&member.raw),
                &member.raw,
                uid,
                deadline,
                cancel,
            )?;
        }
        if stream.coverage() != Some(stream.expectation()) {
            return Err(Error::Invalid("retained source EOF coverage"));
        }
        for index in 0..snapshot.retirement_count() {
            let retired = cut
                .read_retirement(
                    snapshot.revision(),
                    index,
                    limits.max_selected_object_bytes,
                    deadline,
                    cancel,
                )
                .map_err(|_| Error::Invalid("retained retirement object"))?;
            object(
                objects,
                retired.metadata.sha256,
                &retired.raw,
                uid,
                deadline,
                cancel,
            )?;
            object(
                objects,
                retired.metadata.event_sha256,
                &retired.event_raw,
                uid,
                deadline,
                cancel,
            )?;
        }
        install_manifest(revisions, encoded.0, &encoded.1, uid, deadline, cancel)?;
    }
    Ok(())
}

fn verify_export_chain(
    isolated: &IsolatedCreationRoot,
    export: &File,
    objects: &File,
    revisions: &File,
    uid: u32,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<()> {
    let current_root = isolated.verify_current(deadline, cancel)?;
    let current_export =
        tos_fd_open::open_directory_at(&current_root, Path::new(".managed-source-cut"))
            .map_err(|_| Error::Conflict("current source export replaced"))?;
    let current_objects = tos_fd_open::open_directory_at(&current_export, Path::new("objects"))
        .map_err(|_| Error::Conflict("current source objects replaced"))?;
    let current_revisions = tos_fd_open::open_directory_at(&current_export, Path::new("revisions"))
        .map_err(|_| Error::Conflict("current source revisions replaced"))?;
    for (selected, current) in [
        (export, &current_export),
        (objects, &current_objects),
        (revisions, &current_revisions),
    ] {
        if inode(&owned(selected, uid, true)?) != inode(&owned(current, uid, true)?) {
            return Err(Error::Conflict(
                "current source export descriptor identity changed",
            ));
        }
    }
    Ok(())
}

/// Explicit selection after a commit or cold restore. It never runs beneath
/// the short commit fence. V1 needs O(N) metadata/hash and byte verification,
/// plus opening the retained base chain; only absent SHA objects are written.
/// This bounded compatibility bridge does not close affected scalability.
pub fn select_current_source_cut(
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    domain: &str,
    original: &CorpusCutReader,
    initial_revision: SourceRevision,
    initial_membership: SourceMembershipV1,
    bootstrap_context: &CommandContext,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    streamed: Option<(
        &crate::durable_adapter::PrivateGenerationWorkspace,
        crate::durable_adapter::StreamedGenerationProfile,
    )>,
    isolated: &IsolatedCreationRoot,
    parent: Option<&ManagedCurrentSourceCut>,
    read_limits: ReadLimits,
    cut_limits: CutReadLimits,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<ManagedCurrentSourceCut> {
    active(deadline, cancel)?;
    read_limits
        .validate()
        .map_err(|_| Error::Invalid("current cut read profile"))?;
    let reopened = match streamed {
        Some((workspace, profile)) => coordinator.cold_reopen_source_cohort_streamed(
            store,
            domain,
            original,
            initial_revision,
            initial_membership,
            bootstrap_context,
            software,
            components,
            worker,
            workspace,
            profile,
            deadline,
            cancel,
        ),
        None => coordinator.cold_reopen_source_cohort(
            store,
            domain,
            original,
            initial_revision,
            initial_membership,
            bootstrap_context,
            software,
            components,
            worker,
            deadline,
            cancel,
        ),
    }
    .map_err(durable)?;
    let total = reopened
        .files
        .values()
        .try_fold(0usize, |n, bytes| n.checked_add(bytes.len()));
    if reopened.files.len() > MAX_FILES
        || total.is_none_or(|n| n > MAX_BYTES)
        || reopened.metadata.len() != reopened.files.len()
        || reopened.dependency_claims.len() != reopened.files.len()
    {
        return Err(Error::Invalid(
            "current source existing bounded complete inventory",
        ));
    }
    for (path, bytes) in &reopened.files {
        let metadata = reopened
            .metadata
            .get(path)
            .ok_or(Error::Invalid("current source metadata missing"))?;
        if metadata.path.as_str() != path
            || metadata.sha256 != Digest256::of_bytes(bytes)
            || metadata.size_bytes != bytes.len() as u64
            || metadata.mode > 0o7777
        {
            return Err(Error::Conflict("current source metadata or fixity differs"));
        }
    }
    let parent_cut = if let Some(parent) = parent {
        if parent.generation.store_id != store.store_id()
            || parent.cohort().domain() != domain
            || parent.cohort().initial_revision() != initial_revision
            || parent.cohort().initial_membership() != initial_membership
            || parent.cohort().generation() > reopened.cohort.generation()
        {
            return Err(Error::Conflict("current source retained parent differs"));
        }
        &parent.cut
    } else {
        original
    };
    let uid = rustix::process::geteuid().as_raw();
    let root = isolated.verify_current(deadline, cancel)?;
    let export = directory(&root, ".managed-source-cut", uid)?;
    let objects = directory(&export, "objects", uid)?;
    let revisions = directory(&export, "revisions", uid)?;
    copy_cut(
        parent_cut,
        &objects,
        &revisions,
        uid,
        read_limits,
        deadline,
        cancel,
    )?;
    for (path, bytes) in &reopened.files {
        object(
            &objects,
            reopened.metadata[path].sha256,
            bytes,
            uid,
            deadline,
            cancel,
        )?;
    }
    let mut identities = BTreeMap::<String, RelativePath>::new();
    for (kind, id, path) in &reopened.indexes {
        if kind == "path" {
            continue;
        }
        let path =
            RelativePath::parse(path).map_err(|_| Error::Invalid("current source indexed path"))?;
        if !reopened.metadata.contains_key(path.as_str()) {
            return Err(Error::Invalid("current source index outside membership"));
        }
        if identities
            .insert(id.clone(), path.clone())
            .is_some_and(|prior| prior != path)
        {
            return Err(Error::Conflict(
                "current source stable identity has multiple paths",
            ));
        }
    }
    // Retained IDs remain reserved, as in the existing corpus store writer.
    for snapshot in parent_cut.revisions() {
        for (id, path) in snapshot.indexed_identities() {
            if let Some(current) = identities.get(id) {
                if current != path {
                    return Err(Error::Conflict(
                        "current identity changes retained owning path",
                    ));
                }
            }
        }
    }
    let ids = cmd::object(
        identities
            .iter()
            .map(|(id, path)| (id.as_str(), cmd::string(path.as_str())))
            .collect(),
    );
    let deps = cmd::object(
        reopened
            .dependency_claims
            .iter()
            .filter_map(|(path, claims)| {
                claims.as_ref().map(|refs| {
                    (
                        path.as_str(),
                        JsonValue::Array(refs.iter().map(|r| cmd::string(r.as_str())).collect()),
                    )
                })
            })
            .collect(),
    );
    let encoded = manifest(
        Some(parent_cut.current().revision()),
        original.current().validator_sha256(),
        members(reopened.metadata.values().cloned()),
        ids,
        deps,
        retirements(parent_cut.current().retirements()),
        read_limits,
    )?;
    install_manifest(&revisions, encoded.0, &encoded.1, uid, deadline, cancel)?;
    active(deadline, cancel)?;
    verify_export_chain(
        isolated, &export, &objects, &revisions, uid, deadline, cancel,
    )?;
    let reader =
        CorpusReader::open_existing(&isolated.path().join(".managed-source-cut"), read_limits)
            .map_err(|_| Error::Invalid("current source exact store open"))?;
    let cut = reader
        .open_source_cut(encoded.0, cut_limits, deadline, cancel)
        .map_err(|_| Error::Invalid("current source successor/history carrier"))?;
    let mut stream = cut
        .stream(encoded.0)
        .map_err(|_| Error::Invalid("current source stream"))?;
    while stream
        .next_member(deadline, cancel)
        .map_err(|_| Error::Invalid("current source byte/EOF verification"))?
        .is_some()
    {}
    if stream.coverage() != Some(reopened.current_membership) {
        return Err(Error::Conflict(
            "current source membership differs from managed generation",
        ));
    }
    drop(stream);
    verify_export_chain(
        isolated, &export, &objects, &revisions, uid, deadline, cancel,
    )?;
    if reopened.selected.cut().through_commit_seq() != reopened.cohort.generation() {
        return Err(Error::Conflict(
            "managed generation and source cohort differ",
        ));
    }
    Ok(ManagedCurrentSourceCut {
        generation: ManagedCurrentSourceGeneration {
            store_id: store.store_id(),
            cohort: reopened.cohort,
            selected: SelectedSourceGeneration::Cold(reopened.selected),
        },
        cut,
        membership: reopened.current_membership,
        files: reopened.files,
    })
}
