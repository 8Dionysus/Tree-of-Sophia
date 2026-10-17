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
use std::collections::{BTreeMap, BTreeSet};
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

const CURRENT_SOURCE_EXPORT_LIMIT_REASON: &str = "current source exceeds configured export limits";
const RETAINED_SOURCE_HISTORY_LIMIT_REASON: &str =
    "retained source history exceeds bounded carrier";

#[path = "source_foundation_artifact_replay.rs"]
pub(crate) mod foundation_artifact_replay;
#[path = "source_foundation_bootstrap.rs"]
pub(crate) mod foundation_bootstrap;
#[path = "source_foundation_bootstrap_config.rs"]
pub(crate) mod foundation_bootstrap_config;
#[path = "source_foundation_candidate_catalog.rs"]
pub(crate) mod foundation_candidate_catalog;
#[path = "source_foundation_capture.rs"]
pub(crate) mod foundation_capture;
#[path = "source_foundation_catalog.rs"]
pub(crate) mod foundation_catalog;
#[path = "source_foundation_cli.rs"]
pub(crate) mod foundation_cli;
#[path = "source_foundation_command.rs"]
pub mod foundation_command;
#[path = "source_foundation_entry.rs"]
pub(crate) mod foundation_entry;
#[path = "source_foundation_execution_limits.rs"]
pub(crate) mod foundation_execution_limits;
#[path = "source_foundation_lab_resolution.rs"]
pub(crate) mod foundation_lab_resolution;
#[path = "source_foundation_orchestrator.rs"]
pub(crate) mod foundation_orchestrator;
#[path = "source_foundation_output.rs"]
pub(crate) mod foundation_output;
#[path = "source_foundation_payload.rs"]
pub(crate) mod foundation_payload;
#[path = "source_foundation_physical.rs"]
pub(crate) mod foundation_physical;
#[path = "source_foundation_reader.rs"]
pub(crate) mod foundation_reader;
#[path = "source_foundation_rule_diagnostics.rs"]
pub(crate) mod foundation_rule_diagnostics;
#[path = "source_foundation_run.rs"]
pub(crate) mod foundation_run;
#[path = "source_foundation_selection.rs"]
pub(crate) mod foundation_selection;
#[path = "source_catalog_cli.rs"]
pub mod source_catalog_cli;

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
    pub(crate) fn membership(&self) -> SourceMembershipV1 {
        self.membership
    }
    pub(crate) fn into_generation(self) -> ManagedCurrentSourceGeneration {
        self.generation
    }
}

/// Metadata-only carrier for an explicit export. Ordinary command selection
/// keeps its own smaller admission contract and never uses this full visitor.
pub(crate) struct ManagedCurrentSourceExport {
    pub generation: ManagedCurrentSourceGeneration,
    pub read_limits: ReadLimits,
    pub cut: CorpusCutReader,
    pub membership: SourceMembershipV1,
    pub source_bytes: u64,
}

// Finite source-export parser/history ceilings, independent of command batches
// and the separately bounded portable model. The selected owner must also
// reserve the actual host export bytes/inodes before this route is called.
pub(crate) const INITIAL_EXPORT_MANIFEST_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const INITIAL_EXPORT_MANIFEST_ENTRIES: usize = 262_144;
pub(crate) const INITIAL_EXPORT_HISTORY_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub(crate) const INITIAL_EXPORT_JSON_VISITS: usize = 3_000_000;

/// Deliver one selected generation without rebuilding its source assessment
/// or retaining corpus bodies in a complete map. PG/rights/custody fences and
/// each authenticated current placement are checked by the export visitor.
pub(crate) fn export_current_source_generation_addressed(
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    generation: ManagedCurrentSourceGeneration,
    original: &CorpusCutReader,
    isolated: &IsolatedCreationRoot,
    read_limits: ReadLimits,
    current_limits: CutReadLimits,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<ManagedCurrentSourceExport> {
    active(deadline, cancel)?;
    if generation.cohort().initial_revision() != original.current().revision()
        || generation.cohort().initial_membership()
            != original
                .stream(original.current().revision())
                .map_err(|_| Error::Invalid("initial export original membership"))?
                .expectation()
        || read_limits.max_manifest_bytes > INITIAL_EXPORT_MANIFEST_BYTES
        || read_limits.max_manifest_entries > INITIAL_EXPORT_MANIFEST_ENTRIES
        || current_limits.max_total_bytes > INITIAL_EXPORT_HISTORY_BYTES
        || current_limits.max_member_bytes > read_limits.max_selected_object_bytes
    {
        return Err(Error::Conflict("selected export source/profile differs"));
    }
    let uid = rustix::process::geteuid().as_raw();
    let root = isolated.verify_current(deadline, cancel)?;
    let export = directory(&root, ".managed-source-cut", uid)?;
    let objects = directory(&export, "objects", uid)?;
    let revisions = directory(&export, "revisions", uid)?;
    let max_member_bytes = usize::try_from(current_limits.max_member_bytes)
        .map_err(|_| Error::Invalid("initial export member bound"))?;
    copy_cut_with_member_limit(
        original,
        &objects,
        &revisions,
        uid,
        read_limits,
        max_member_bytes,
        deadline,
        cancel,
    )?;
    let source = coordinator
        .visit_selected_source_export(
            store,
            &generation,
            current_limits,
            (read_limits.max_manifest_bytes as u64)
                .checked_mul(8)
                .ok_or(Error::Invalid("initial export metadata bound"))?,
            read_limits.max_manifest_entries,
            deadline,
            cancel,
            |member, raw| {
                object_with_limit(
                    &objects,
                    member.sha256,
                    raw,
                    uid,
                    max_member_bytes,
                    deadline,
                    cancel,
                )
                .map_err(DurableError::Source)
            },
        )
        .map_err(durable)?;
    let (history_read_limits, history_cut_limits) = current_and_history_cut_limits_bounded(
        original,
        &source.metadata,
        &source.indexes,
        &source.dependencies,
        read_limits,
        current_limits,
        INITIAL_EXPORT_MANIFEST_ENTRIES,
        INITIAL_EXPORT_HISTORY_BYTES,
    )?;
    let mut identities = BTreeMap::<String, RelativePath>::new();
    for (_, id, path) in &source.indexes {
        let path = RelativePath::parse(path)
            .map_err(|_| Error::Invalid("initial export identity path"))?;
        if identities
            .insert(id.clone(), path.clone())
            .is_some_and(|prior| prior != path)
        {
            return Err(Error::Conflict(
                "current source stable identity has multiple paths",
            ));
        }
    }
    for snapshot in original.revisions() {
        for (id, path) in snapshot.indexed_identities() {
            if identities.get(id).is_some_and(|current| current != path) {
                return Err(Error::Conflict(
                    "current identity changes retained owning path",
                ));
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
        source
            .dependencies
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
        Some(original.current().revision()),
        original.current().validator_sha256(),
        members(source.metadata.values().cloned()),
        ids,
        deps,
        retirements(original.current().retirements()),
        history_read_limits,
    )?;
    install_manifest_with_limit(
        &revisions,
        encoded.0,
        &encoded.1,
        uid,
        history_read_limits.max_manifest_bytes,
        deadline,
        cancel,
    )?;
    verify_export_chain(
        isolated, &export, &objects, &revisions, uid, deadline, cancel,
    )?;
    let reader = CorpusReader::open_existing(
        &isolated.path().join(".managed-source-cut"),
        history_read_limits,
    )
    .map_err(|error| Error::Invalid(error.detail))?;
    let cut = reader
        .open_source_cut(encoded.0, history_cut_limits, deadline, cancel)
        .map_err(|error| Error::Invalid(error.detail))?;
    let mut stream = cut
        .stream(encoded.0)
        .map_err(|_| Error::Invalid("initial export source stream"))?;
    while stream
        .next_member(deadline, cancel)
        .map_err(|error| Error::Invalid(error.detail))?
        .is_some()
    {}
    if stream.coverage() != Some(source.membership) {
        return Err(Error::Conflict(
            "initial export membership differs from selected generation",
        ));
    }
    drop(stream);
    verify_export_chain(
        isolated, &export, &objects, &revisions, uid, deadline, cancel,
    )?;
    Ok(ManagedCurrentSourceExport {
        generation,
        read_limits: history_read_limits,
        cut,
        membership: source.membership,
        source_bytes: source.source_bytes,
    })
}

/// Explicit cold selection of existing durable roots without a v1 export.
/// This strict legacy route refuses any active addressed protocol marker so it
/// cannot select or rebuild V1 beneath V2 state. The present owner still
/// performs a complete source/index/custody audit;
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
/// A retained V2 descriptor may be reselected after the restored database's
/// own source, rights, index, overlay, and custody checks. A transitional V1
/// descriptor is accepted only after its complete OID-bound transcript and
/// physical membership are recomputed against the restored database; the
/// returned selection is newly published with the target database identity.
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
    let reopened = coordinator
        .cold_reopen_source_cohort_addressed_reselect(
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
        )
        .map_err(durable)?;
    if reopened.selected.cut().through_commit_seq() != reopened.cohort.generation() {
        return Err(Error::Conflict(
            "managed generation and source cohort differ",
        ));
    }
    let cold = ManagedCurrentSourceGeneration {
        store_id: store.store_id(),
        cohort: reopened.cohort,
        selected: SelectedSourceGeneration::Cold(reopened.selected),
    };
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
        DurableError::Refused(reason) => Error::Denied(reason),
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
    object_inner(objects, digest, bytes, uid, 8_388_608, deadline, cancel)
}

fn object_with_limit(
    objects: &File,
    digest: Digest256,
    bytes: &[u8],
    uid: u32,
    max_file_bytes: usize,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<()> {
    active(deadline, cancel)?;
    if max_file_bytes == 0 || max_file_bytes == usize::MAX || bytes.len() > max_file_bytes {
        return Err(Error::Unsupported(
            "foundation candidate staged file byte limit",
        ));
    }
    object_inner(
        objects,
        digest,
        bytes,
        uid,
        max_file_bytes,
        deadline,
        cancel,
    )
}

fn object_inner(
    objects: &File,
    digest: Digest256,
    bytes: &[u8],
    uid: u32,
    max_file_bytes: usize,
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
    stage.write_with_limit(&name, bytes, max_file_bytes, deadline, cancel)?;
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
    install_manifest_inner(revisions, revision, bytes, uid, 8_388_608, deadline, cancel)
}

fn install_manifest_with_limit(
    revisions: &File,
    revision: SourceRevision,
    bytes: &[u8],
    uid: u32,
    max_file_bytes: usize,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<()> {
    active(deadline, cancel)?;
    if max_file_bytes == 0 || max_file_bytes == usize::MAX || bytes.len() > max_file_bytes {
        return Err(Error::Unsupported(
            "foundation candidate staged file byte limit",
        ));
    }
    install_manifest_inner(
        revisions,
        revision,
        bytes,
        uid,
        max_file_bytes,
        deadline,
        cancel,
    )
}

fn install_manifest_inner(
    revisions: &File,
    revision: SourceRevision,
    bytes: &[u8],
    uid: u32,
    max_file_bytes: usize,
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
    stage.write_with_limit("snapshot.json", bytes, max_file_bytes, deadline, cancel)?;
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
    copy_cut_with_member_limit(
        cut, objects, revisions, uid, limits, 8_388_608, deadline, cancel,
    )
}
fn copy_cut_with_member_limit(
    cut: &CorpusCutReader,
    objects: &File,
    revisions: &File,
    uid: u32,
    limits: ReadLimits,
    max_member_bytes: usize,
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
                install_manifest_with_limit(
                    revisions,
                    encoded.0,
                    &encoded.1,
                    uid,
                    limits.max_manifest_bytes,
                    deadline,
                    cancel,
                )?;
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
            object_with_limit(
                objects,
                Digest256::of_bytes(&member.raw),
                &member.raw,
                uid,
                max_member_bytes,
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
            object_with_limit(
                objects,
                retired.metadata.sha256,
                &retired.raw,
                uid,
                max_member_bytes,
                deadline,
                cancel,
            )?;
            object_with_limit(
                objects,
                retired.metadata.event_sha256,
                &retired.event_raw,
                uid,
                max_member_bytes,
                deadline,
                cancel,
            )?;
        }
        install_manifest_with_limit(
            revisions,
            encoded.0,
            &encoded.1,
            uid,
            limits.max_manifest_bytes,
            deadline,
            cancel,
        )?;
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
    verify_named_export_chain(
        isolated,
        ".managed-source-cut",
        export,
        objects,
        revisions,
        uid,
        deadline,
        cancel,
    )
}

fn verify_named_export_chain(
    isolated: &IsolatedCreationRoot,
    name: &str,
    export: &File,
    objects: &File,
    revisions: &File,
    uid: u32,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<()> {
    let current_root = isolated.verify_current(deadline, cancel)?;
    let current_export = tos_fd_open::open_directory_at(&current_root, Path::new(name))
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

fn snapshot_manifest_entry_count(snapshot: &Snapshot) -> Result<usize> {
    let dependency_entries = snapshot
        .members()
        .try_fold(0usize, |total, member| {
            match snapshot.indexed_dependencies(&member.path) {
                Some(refs) => total.checked_add(1)?.checked_add(refs.len()),
                None => Some(total),
            }
        })
        .ok_or(Error::Invalid(RETAINED_SOURCE_HISTORY_LIMIT_REASON))?;
    snapshot
        .member_count()
        .checked_add(snapshot.identity_count())
        .and_then(|count| count.checked_add(dependency_entries))
        .and_then(|count| count.checked_add(snapshot.retirement_count()))
        .ok_or(Error::Invalid(RETAINED_SOURCE_HISTORY_LIMIT_REASON))
}

fn current_and_history_cut_limits(
    parent: &CorpusCutReader,
    current: &BTreeMap<String, MemberMetadata>,
    indexes: &[(String, String, String)],
    dependency_claims: &BTreeMap<String, Option<Vec<RelativePath>>>,
    current_read_limits: ReadLimits,
    current_cut_limits: CutReadLimits,
) -> Result<(ReadLimits, CutReadLimits)> {
    current_and_history_cut_limits_bounded(
        parent,
        current,
        indexes,
        dependency_claims,
        current_read_limits,
        current_cut_limits,
        MAX_FILES,
        MAX_BYTES as u64,
    )
}
fn current_and_history_cut_limits_bounded(
    parent: &CorpusCutReader,
    current: &BTreeMap<String, MemberMetadata>,
    indexes: &[(String, String, String)],
    dependency_claims: &BTreeMap<String, Option<Vec<RelativePath>>>,
    current_read_limits: ReadLimits,
    current_cut_limits: CutReadLimits,
    max_entries: usize,
    max_bytes: u64,
) -> Result<(ReadLimits, CutReadLimits)> {
    if current_cut_limits.max_revisions == 0
        || current_cut_limits.max_revisions == usize::MAX
        || current_cut_limits.max_members == 0
        || current_cut_limits.max_members == u64::MAX
        || current_cut_limits.max_total_bytes == 0
        || current_cut_limits.max_total_bytes == u64::MAX
        || current_cut_limits.max_member_bytes == 0
        || current_cut_limits.max_member_bytes == u64::MAX
    {
        return Err(Error::Invalid(CURRENT_SOURCE_EXPORT_LIMIT_REASON));
    }
    let current_count = u64::try_from(current.len())
        .map_err(|_| Error::Invalid(CURRENT_SOURCE_EXPORT_LIMIT_REASON))?;
    let current_bytes = current
        .values()
        .try_fold(0u64, |total, member| total.checked_add(member.size_bytes));
    let current_bytes = current_bytes.ok_or(Error::Invalid(CURRENT_SOURCE_EXPORT_LIMIT_REASON))?;
    let current_max_member = current
        .values()
        .map(|member| member.size_bytes)
        .max()
        .unwrap_or(0);
    if current_count > current_cut_limits.max_members
        || current_bytes > current_cut_limits.max_total_bytes
        || current_max_member > current_cut_limits.max_member_bytes
        || current_max_member > current_read_limits.max_selected_object_bytes
    {
        return Err(Error::Invalid(CURRENT_SOURCE_EXPORT_LIMIT_REASON));
    }

    // The source-owner member and byte caps above stay authoritative. Reader
    // entry caps count files, IDs, dependency edges and retirements too, so
    // derive that parser budget from the exact current and retained metadata.
    let current_identity_count = indexes
        .iter()
        .filter(|(kind, _, _)| kind != "path")
        .map(|(_, id, _)| id)
        .collect::<BTreeSet<_>>()
        .len();
    let current_dependency_entries = dependency_claims
        .values()
        .try_fold(0usize, |total, claims| match claims {
            Some(refs) => total.checked_add(1)?.checked_add(refs.len()),
            None => Some(total),
        })
        .ok_or(Error::Invalid(CURRENT_SOURCE_EXPORT_LIMIT_REASON))?;
    let current_manifest_entries = current
        .len()
        .checked_add(current_identity_count)
        .and_then(|count| count.checked_add(current_dependency_entries))
        .and_then(|count| count.checked_add(parent.current().retirement_count()))
        .ok_or(Error::Invalid(CURRENT_SOURCE_EXPORT_LIMIT_REASON))?;
    if current_manifest_entries > max_entries {
        return Err(Error::Invalid(CURRENT_SOURCE_EXPORT_LIMIT_REASON));
    }

    let parent_cut_limits = parent.limits();
    // Preserve the parent cut's admitted ceiling for retired raw objects whose
    // byte length is intentionally absent from retirement metadata.
    let mut history_max_member = parent_cut_limits.max_member_bytes.max(current_max_member);
    let mut history_manifest_entries = current_manifest_entries;
    for snapshot in parent.revisions() {
        history_manifest_entries =
            history_manifest_entries.max(snapshot_manifest_entry_count(snapshot)?);
        for member in snapshot.members() {
            history_max_member = history_max_member.max(member.size_bytes);
        }
        for retirement in snapshot.retirements() {
            history_max_member = history_max_member.max(retirement.event_size_bytes);
        }
    }
    for retirement in parent.current().retirements() {
        history_max_member = history_max_member.max(retirement.event_size_bytes);
    }
    if history_max_member == 0 || history_max_member > max_bytes {
        return Err(Error::Invalid(RETAINED_SOURCE_HISTORY_LIMIT_REASON));
    }

    let history_revisions = parent
        .revisions()
        .count()
        .checked_add(1)
        .ok_or(Error::Invalid(RETAINED_SOURCE_HISTORY_LIMIT_REASON))?;
    let mut history_members = current_count;
    let mut history_bytes = current_bytes;
    for snapshot in parent.revisions() {
        let snapshot_members = u64::try_from(snapshot.member_count())
            .map_err(|_| Error::Invalid(RETAINED_SOURCE_HISTORY_LIMIT_REASON))?;
        history_members = history_members
            .checked_add(snapshot_members)
            .ok_or(Error::Invalid(RETAINED_SOURCE_HISTORY_LIMIT_REASON))?;
        for member in snapshot.members() {
            history_bytes = history_bytes
                .checked_add(member.size_bytes)
                .ok_or(Error::Invalid(RETAINED_SOURCE_HISTORY_LIMIT_REASON))?;
        }
        for retirement in snapshot.retirements() {
            history_members = history_members
                .checked_add(2)
                .ok_or(Error::Invalid(RETAINED_SOURCE_HISTORY_LIMIT_REASON))?;
            history_bytes = history_bytes
                .checked_add(history_max_member)
                .and_then(|total| total.checked_add(retirement.event_size_bytes))
                .ok_or(Error::Invalid(RETAINED_SOURCE_HISTORY_LIMIT_REASON))?;
        }
    }
    // The new successor carries the prior current revision's retirement ledger.
    for retirement in parent.current().retirements() {
        history_members = history_members
            .checked_add(2)
            .ok_or(Error::Invalid(RETAINED_SOURCE_HISTORY_LIMIT_REASON))?;
        history_bytes = history_bytes
            .checked_add(history_max_member)
            .and_then(|total| total.checked_add(retirement.event_size_bytes))
            .ok_or(Error::Invalid(RETAINED_SOURCE_HISTORY_LIMIT_REASON))?;
    }
    if history_members == 0
        || history_members > max_entries as u64
        || history_bytes == 0
        || history_bytes > max_bytes
        || history_manifest_entries == 0
        || history_manifest_entries > max_entries
        || history_revisions == 0
        || history_revisions > current_cut_limits.max_revisions
    {
        return Err(Error::Invalid(RETAINED_SOURCE_HISTORY_LIMIT_REASON));
    }

    let history_read_limits = ReadLimits {
        max_manifest_bytes: current_read_limits.max_manifest_bytes,
        max_manifest_entries: current_read_limits
            .max_manifest_entries
            .min(max_entries)
            .max(history_manifest_entries),
        max_selected_object_bytes: current_read_limits
            .max_selected_object_bytes
            .max(history_max_member),
        json: current_read_limits.json,
    };
    if history_read_limits.max_selected_object_bytes > max_bytes {
        return Err(Error::Invalid(RETAINED_SOURCE_HISTORY_LIMIT_REASON));
    }
    let history_cut_limits = CutReadLimits {
        max_revisions: history_revisions,
        max_members: history_members,
        max_total_bytes: history_bytes,
        max_member_bytes: history_max_member,
    };
    Ok((history_read_limits, history_cut_limits))
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
    select_current_source_cut_inner(
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
        isolated,
        parent,
        read_limits,
        cut_limits,
        None,
        None,
        deadline,
        cancel,
    )
}

/// Explicit complete compatibility export from an addressed source generation.
/// Re-selects the retained addressed descriptor through the full current source,
/// index and custody audit, then exports the exact current and historical bytes.
/// This bounded cold/export operation is not the ordinary mutation or query path.
/// The legacy export above continues to refuse an active addressed protocol.
/// The caller must explicitly install the addressed audit profile in the
/// selected private database before opting into this operation.
pub fn select_current_source_cut_addressed(
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
    tree_limits: tos_segment_store::AuthenticatedTreeLimitsV1,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<ManagedCurrentSourceCut> {
    select_current_source_cut_inner(
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
        isolated,
        parent,
        read_limits,
        cut_limits,
        Some(tree_limits),
        None,
        deadline,
        cancel,
    )
}

fn select_current_source_cut_inner(
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
    addressed_tree_limits: Option<tos_segment_store::AuthenticatedTreeLimitsV1>,
    current_export_limits: Option<(ReadLimits, CutReadLimits)>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<ManagedCurrentSourceCut> {
    active(deadline, cancel)?;
    read_limits
        .validate()
        .map_err(|_| Error::Invalid("current cut read profile"))?;
    let reopened = if addressed_tree_limits.is_some() {
        coordinator.cold_reopen_source_cohort_addressed_reselect(
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
        )
    } else {
        match streamed {
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
    let (history_read_limits, history_cut_limits) = match current_export_limits {
        Some((current_read_limits, current_cut_limits)) => current_and_history_cut_limits(
            parent_cut,
            &reopened.metadata,
            &reopened.indexes,
            &reopened.dependency_claims,
            current_read_limits,
            current_cut_limits,
        )?,
        None => (read_limits, cut_limits),
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
        history_read_limits,
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
    let reader = CorpusReader::open_existing(
        &isolated.path().join(".managed-source-cut"),
        history_read_limits,
    )
    .map_err(|error| {
        if current_export_limits.is_some() {
            Error::Invalid(error.detail)
        } else {
            Error::Invalid("current source exact store open")
        }
    })?;
    let cut = reader
        .open_source_cut(encoded.0, history_cut_limits, deadline, cancel)
        .map_err(|error| {
            if current_export_limits.is_some() {
                Error::Invalid(error.detail)
            } else {
                Error::Invalid("current source successor/history carrier")
            }
        })?;
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
    let export = ManagedCurrentSourceCut {
        generation: ManagedCurrentSourceGeneration {
            store_id: store.store_id(),
            cohort: reopened.cohort,
            selected: SelectedSourceGeneration::Cold(reopened.selected),
        },
        cut,
        membership: reopened.current_membership,
    };
    match addressed_tree_limits {
        Some(tree_limits) => migrate_current_source_cut_addressed(
            coordinator,
            store,
            export,
            tree_limits,
            deadline,
            cancel,
        ),
        None => Ok(export),
    }
}
