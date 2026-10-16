//! Native owner-local Profile and Claim commands over a bounded PG source
//! epoch and the isolated private-package broker. Semantic decisions remain
//! in the maintained Profile and Claim owners.

use super::pg_claims as pg;
use crate::durable_private_packages::{
    PrivateOwnerPackageArchiveV1, PrivateOwnerPackageCapabilityV1, PrivateOwnerPackageFamilyV1,
    PrivateOwnerPackageReplayV1, PrivateOwnerPackageSnapshotV1, PrivateOwnerPackageWriteV1,
    PrivateOwnerScopeV1, PrivateOwnerSourceBindingV1, capability_seal,
    private_owner_dependency_digest, private_owner_readset_digest,
};
use crate::source_cohort::revision::SourceRevisionReadSession;
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_revisions::{PgRecordRevisionIdentity, ReadonlyRecordFiles, RecordRead};
use crate::source_text_owner::{OwnerTextContext, read_absolute};
use crate::source_text_private_store::{PrivateArchiveReader, PrivateIdentityInputs};
use crate::{DurableError, DurablePgCoordinator};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath, SourceRevision};
use tos_segment_store::SegmentStore;
use tos_source_store::{SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::SchemaResource;
use tos_validation::executor::{ExactWorkerIdentity, ExecutorBudget, VerifiedWorkerImageHandle};
use tos_validation::item_rules::ItemRefusal;
use tos_validation::record_biblio_cut::SourceCutPrefixCoverage;
use tos_validation::source_cut::{
    CandidateCutWorkerSchemaExecutor, CutSchemaExecutor, CutWorkerLimits,
};

const INVOCATION_SCHEMA: &str = "tos_local_pg_private_owner_invocation_v1";
const MAX_SCHEMA_FILES: usize = 128;
const MAX_SCHEMA_BYTES: u64 = 67_108_864;
const MAX_SCHEMA_FILE_BYTES: u64 = 4_194_304;
const MAX_CONTEXT_MEMBERS: u64 = 4_096;
const MAX_CONTEXT_BYTES: u64 = 67_108_864;
const MAX_CONTEXT_MEMBER_BYTES: u64 = 33_554_432;
const MAX_IDENTITY_FILES: usize = 2_048;
const MAX_IDENTITY_ENTRIES: usize = 32_768;
const MAX_IDENTITY_BYTES: u64 = 67_108_864;
const MAX_IDENTITY_MEMBER_BYTES: u64 = 33_554_432;
const ENTITY_REGISTRY: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const RELATION_REGISTRY: &str = "ToS/doctrine/semantic-interchange/relation-types.v1.json";
const CONTEXT_SCHEMA: &str = "ToS/contracts/owner-local-source-context.schema.json";

#[derive(Clone, Debug, Eq, PartialEq)]
struct SelectedEpoch {
    domain: String,
    job_id: String,
    job_fence: u64,
    contract: Digest256,
    head_seq: u64,
    epoch: u64,
    source_generation: u64,
    source_revision: Digest256,
    source_audit_generation: u64,
    rights_version: u64,
    rule_version: u64,
}

fn epoch(session: &SourceRevisionReadSession<'_>) -> SelectedEpoch {
    let selected = session.selection();
    SelectedEpoch {
        domain: selected.cohort.domain().to_owned(),
        job_id: selected.job_id.clone(),
        job_fence: selected.job_fence,
        contract: selected.contract,
        head_seq: selected.head_seq,
        epoch: selected.epoch,
        source_generation: selected.source_generation,
        source_revision: selected.cohort.initial_revision().0,
        source_audit_generation: selected.audit_generation,
        rights_version: selected.rights_version,
        rule_version: selected.rule_version,
    }
}

struct PgPrivateSelectedSource<'a, 'b> {
    session: Rc<RefCell<&'a mut SourceRevisionReadSession<'b>>>,
    identity: PgRecordRevisionIdentity,
    public_identity_paths: Option<Vec<String>>,
}

struct PgPrivateSessionRecordRead<'a, 'b> {
    command: cmd::CommandContext,
    session: Rc<RefCell<&'a mut SourceRevisionReadSession<'b>>>,
    identity: PgRecordRevisionIdentity,
}

impl RecordRead for PgPrivateSessionRecordRead<'_, '_> {
    type Identity = PgRecordRevisionIdentity;

    fn files(&self) -> &[cmd::SourceFile] {
        &self.command.files
    }

    fn input_identity(&self) -> Self::Identity {
        self.identity
    }

    fn effective_uid(&self) -> u64 {
        self.command.effective_uid
    }

    fn schema_source_path(&self) -> SourceCommandResult<String> {
        self.command.schema_source_path()
    }

    fn writer(&self) -> Option<&cmd::CommandContext> {
        Some(&self.command)
    }

    fn check(&self) -> SourceCommandResult<()> {
        self.command.check()?;
        if self.session.borrow().selection().identity() != self.identity {
            return Err(SourceCommandError::Conflict(
                "private owner RecordRead no longer matches its held PG session",
            ));
        }
        Ok(())
    }
}

impl crate::source_native_cli::private_owner::profile::PrivateSelectedSourceInput
    for PgPrivateSelectedSource<'_, '_>
{
    fn selected_public_identity_paths(
        &self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<Vec<String>>, ItemRefusal> {
        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(ItemRefusal::Source("private source read cancelled".into()));
        }
        if Instant::now() >= deadline {
            return Err(ItemRefusal::Deadline);
        }
        if self.session.borrow().selection().identity() != self.identity {
            return Err(ItemRefusal::Source(
                "private identity selection changed".into(),
            ));
        }
        Ok(self.public_identity_paths.clone())
    }

    fn private_selection_identity(
        &self,
    ) -> crate::source_native_cli::private_owner::profile::PrivateSelectedSourceIdentity {
        crate::source_native_cli::private_owner::profile::PrivateSelectedSourceIdentity::PgRecord(
            self.identity,
        )
    }

    fn annotation_identity_owners(
        &self,
        tokens: &BTreeSet<String>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<
        Option<
            BTreeMap<
                String,
                BTreeSet<crate::source_annotation_identity_index::AnnotationEntityOwner>,
            >,
        >,
        ItemRefusal,
    > {
        self.session
            .borrow_mut()
            .lookup_annotation_identity_owners(tokens, deadline, cancelled)
            .map(Some)
            .map_err(|_| {
                ItemRefusal::Source("private Claim annotation identity lookup failed".into())
            })
    }

    fn for_each_current_member_path_under(
        &self,
        directory: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
        visit: &mut dyn FnMut(&str) -> Result<(), ItemRefusal>,
    ) -> Result<SourceCutPrefixCoverage, ItemRefusal> {
        let parsed = RelativePath::parse(directory)
            .map_err(|_| ItemRefusal::Source("private source prefix path".into()))?;
        if parsed.as_str() != directory {
            return Err(ItemRefusal::Source(
                "private source prefix path is not canonical".into(),
            ));
        }
        let mut pending = vec![directory.to_owned()];
        let mut count = 0u64;
        while let Some(parent) = pending.pop() {
            if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(ItemRefusal::Source("private source read cancelled".into()));
            }
            if Instant::now() >= deadline {
                return Err(ItemRefusal::Deadline);
            }
            let parent_path = RelativePath::parse(&parent)
                .map_err(|_| ItemRefusal::Source("private source prefix path".into()))?;
            let children = self
                .session
                .borrow_mut()
                .list_immediate_children(&parent_path, deadline, cancelled)
                .map_err(|_| ItemRefusal::Source("private source prefix read failed".into()))?;
            for (name, is_directory) in children {
                let child = format!("{parent}/{name}");
                RelativePath::parse(&child)
                    .map_err(|_| ItemRefusal::Source("private source child path".into()))?;
                if is_directory {
                    pending.push(child);
                } else {
                    count = count.checked_add(1).ok_or(ItemRefusal::Budget)?;
                    visit(&child)?;
                }
            }
        }
        SourceCutPrefixCoverage::after_verified_prefix_eof(directory, count)
    }

    fn with_current_member(
        &self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
        visit: &mut dyn FnMut(&str, &[u8]) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(ItemRefusal::Source("private source read cancelled".into()));
        }
        if Instant::now() >= deadline {
            return Err(ItemRefusal::Deadline);
        }
        let relative = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Source("private source member path".into()))?;
        if relative.as_str() != path {
            return Err(ItemRefusal::Source(
                "private source member path is not canonical".into(),
            ));
        }
        let mut session = self.session.borrow_mut();
        let exists =
            ReadonlyRecordFiles::has_file(&mut **session, relative.as_str(), deadline, cancelled)
                .map_err(|_| {
                ItemRefusal::Source("private selected source presence read failed".into())
            })?;
        if !exists {
            return Ok(());
        }
        let member = session
            .read_current_member(&relative, max_bytes as u64, deadline, cancelled)
            .map_err(|_| ItemRefusal::Source("private selected source read failed".into()))?;
        visit(path, &member.raw)
    }
}

fn selected_asserted_candidate_schema_from_context(
    invocation: &Value,
    files: &[cmd::SourceFile],
    identity: PgRecordRevisionIdentity,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CandidateCutWorkerSchemaExecutor<PgRecordRevisionIdentity>> {
    let worker = &invocation["schema_worker"];
    super::exact(worker, &["absolute_path", "sha256"])?;
    let budgets = &invocation["budgets"];
    let mut worker_budget = ExecutorBudget::laboratory();
    worker_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    worker_budget.cpu_seconds = super::capped(budgets, "worker_cpu_seconds", 3)?;
    worker_budget.address_space_bytes =
        super::capped(budgets, "worker_address_space_bytes", 1_073_741_824)?;
    let worker_identity = ExactWorkerIdentity {
        absolute_path: super::absolute(super::text(worker, "absolute_path")?)?,
        sha256: super::digest(super::text(worker, "sha256")?)?,
    };
    let worker_image =
        VerifiedWorkerImageHandle::prepare(worker_identity, worker_budget, deadline, cancelled)
            .map_err(|_| SourceCommandError::Denied("native schema worker image"))?;
    let limits = CutWorkerLimits {
        max_receipts: usize::try_from(super::capped(budgets, "max_schema_receipts", 128)?)
            .map_err(|_| SourceCommandError::Invalid("schema receipt budget"))?,
        max_receipt_bytes: usize::try_from(super::capped(
            budgets,
            "max_schema_receipt_bytes",
            262_144,
        )?)
        .map_err(|_| SourceCommandError::Invalid("schema receipt budget"))?,
    };
    let mut resources = Vec::new();
    for file in files.iter().filter(|file| {
        file.path.as_str().starts_with("ToS/contracts/")
            && file.path.as_str().ends_with(".schema.json")
    }) {
        if resources.len() >= MAX_SCHEMA_FILES || file.raw.len() as u64 > MAX_SCHEMA_FILE_BYTES {
            return Err(SourceCommandError::Invalid(
                "private owner selected schema budget",
            ));
        }
        let parsed = cmd::parse(&file.raw)?;
        resources.push((
            file.path.as_str().to_owned(),
            SchemaResource {
                uri: cmd::text(&parsed, "$id")?.to_owned(),
                raw: file.raw.clone(),
            },
        ));
    }
    CandidateCutWorkerSchemaExecutor::from_asserted_source_candidate_resources(
        identity,
        resources,
        &worker_image,
        worker_budget,
        limits,
        deadline,
        cancelled,
    )
    .map_err(|_| SourceCommandError::Denied("native asserted-source candidate schema worker"))
}

struct PgPrivateArchiveReader<'a> {
    coordinator: &'a mut DurablePgCoordinator,
    scope: PrivateOwnerScopeV1,
    target_ref: String,
    family: PrivateOwnerPackageFamilyV1,
    generation: u64,
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
    observations: BTreeMap<String, BTreeMap<String, Vec<u8>>>,
}

impl PrivateArchiveReader for PgPrivateArchiveReader<'_> {
    fn read_archive(
        &mut self,
        reference: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
        active(deadline, cancelled)?;
        let snapshot = self
            .coordinator
            .read_private_owner_archive(&self.scope, &self.target_ref, reference)
            .map_err(pg::source_error)?
            .ok_or(SourceCommandError::Conflict(
                "PostgreSQL private owner archive absent",
            ))?;
        if snapshot.scope != self.scope
            || snapshot.target_ref != self.target_ref
            || snapshot.reference != reference
            || snapshot.family != self.family
            || snapshot.private_audit_generation != self.generation
        {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL private owner archive binding changed",
            ));
        }
        if snapshot.members.len() > self.max_members
            || snapshot
                .members
                .values()
                .any(|raw| raw.len() as u64 > self.max_member_bytes)
        {
            return Err(SourceCommandError::Unsupported(
                "PostgreSQL private owner archive member budget",
            ));
        }
        let is_new = !self.observations.contains_key(reference);
        let existing_members = self
            .observations
            .values()
            .try_fold(0usize, |sum, members| sum.checked_add(members.len()))
            .ok_or(SourceCommandError::Unsupported(
                "PostgreSQL private owner archive member budget",
            ))?;
        let existing_bytes = self
            .observations
            .values()
            .flat_map(BTreeMap::values)
            .try_fold(0u64, |sum, raw| sum.checked_add(raw.len() as u64))
            .ok_or(SourceCommandError::Unsupported(
                "PostgreSQL private owner archive byte budget",
            ))?;
        let archive_bytes = snapshot
            .members
            .values()
            .try_fold(0u64, |sum, raw| sum.checked_add(raw.len() as u64))
            .ok_or(SourceCommandError::Unsupported(
                "PostgreSQL private owner archive byte budget",
            ))?;
        if is_new
            && (self.observations.len() >= 128
                || existing_members
                    .checked_add(snapshot.members.len())
                    .is_none_or(|count| count > self.max_members)
                || existing_bytes
                    .checked_add(archive_bytes)
                    .is_none_or(|bytes| bytes > self.max_total_bytes))
        {
            return Err(SourceCommandError::Unsupported(
                "PostgreSQL private owner archive inventory budget",
            ));
        }
        active(deadline, cancelled)?;
        if self
            .observations
            .get(reference)
            .is_some_and(|members| members != &snapshot.members)
        {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL private owner archive changed during preparation",
            ));
        }
        self.observations
            .entry(reference.to_owned())
            .or_insert_with(|| snapshot.members.clone());
        Ok(snapshot.members)
    }
}

fn active(deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
    if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL private owner operation cancelled",
        ));
    }
    if Instant::now() >= deadline {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL private owner operation deadline",
        ));
    }
    Ok(())
}

fn add_bytes(
    files: &mut BTreeMap<String, Vec<u8>>,
    path: String,
    raw: Vec<u8>,
) -> SourceCommandResult<()> {
    if let Some(previous) = files.get(&path) {
        if previous != &raw {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL private owner selected dependency differs",
            ));
        }
    } else {
        files.insert(path, raw);
    }
    Ok(())
}

fn source_files(files: &BTreeMap<String, Vec<u8>>) -> SourceCommandResult<Vec<cmd::SourceFile>> {
    files
        .iter()
        .map(|(path, raw)| {
            Ok(cmd::SourceFile {
                path: RelativePath::parse(path)
                    .map_err(|_| SourceCommandError::Invalid("private owner selected path"))?,
                raw: raw.clone(),
            })
        })
        .collect()
}

fn read_member(
    session: &mut SourceRevisionReadSession<'_>,
    path: &str,
    max_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let path = RelativePath::parse(path)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL private owner source path"))?;
    session
        .read_current_member(&path, max_bytes as u64, deadline, cancelled)
        .map(|member| member.raw)
        .map_err(pg::source_error)
}

/// Read only the schema closure declared by the selected owner family and its
/// exact current registries. No directory-wide contracts scan is performed.
fn selected_contracts(
    session: &mut SourceRevisionReadSession<'_>,
    family: PrivateOwnerPackageFamilyV1,
    grant: &JsonValue,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let entities_raw = read_member(session, ENTITY_REGISTRY, 8_388_608, deadline, cancelled)?;
    let entities = cmd::parse(&entities_raw)?;
    let paths = match family {
        PrivateOwnerPackageFamilyV1::Profile => super::private_owner::profile::pg_schema_contracts(
            &entities,
            cmd::text(grant, "profile_type_id")?,
        )?,
        PrivateOwnerPackageFamilyV1::Claim => {
            let relations_raw =
                read_member(session, RELATION_REGISTRY, 8_388_608, deadline, cancelled)?;
            let relations = cmd::parse(&relations_raw)?;
            super::private_owner::claim::pg_schema_contracts(&entities, &relations, grant)?
        }
        _ => {
            return Err(SourceCommandError::Denied(
                "PostgreSQL private owner schema family",
            ));
        }
    };
    if paths.is_empty() || paths.len() > MAX_SCHEMA_FILES {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL private owner schema count budget",
        ));
    }
    let mut files = BTreeMap::new();
    let mut total = 0u64;
    for path in paths {
        if !path.starts_with("ToS/contracts/")
            || !path.ends_with(".schema.json")
            || path["ToS/contracts/".len()..].contains('/')
        {
            return Err(SourceCommandError::Denied(
                "private owner schema reference leaves contracts namespace",
            ));
        }
        RelativePath::parse(&path)
            .map_err(|_| SourceCommandError::Invalid("private owner schema path"))?;
        let raw = read_member(
            session,
            &path,
            MAX_SCHEMA_FILE_BYTES as usize,
            deadline,
            cancelled,
        )?;
        total = total
            .checked_add(raw.len() as u64)
            .filter(|bytes| *bytes <= MAX_SCHEMA_BYTES)
            .ok_or(SourceCommandError::Unsupported(
                "PostgreSQL private owner schema byte budget",
            ))?;
        files.insert(path, raw);
    }
    if !files.contains_key(CONTEXT_SCHEMA)
        || !files.contains_key("ToS/contracts/semantic-entity-type-registry.schema.json")
    {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL private owner required schema absent",
        ));
    }
    Ok(files)
}

fn read_authored(
    session: &mut SourceRevisionReadSession<'_>,
    path: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    read_member(
        session,
        path,
        MAX_CONTEXT_MEMBER_BYTES as usize,
        deadline,
        cancelled,
    )
}

fn identity_candidate(name: &str, basenames: &BTreeSet<String>, include_provenance: bool) -> bool {
    basenames.contains(name)
        || name.ends_with(".human-forms.json")
        || name.starts_with("semantic-annotation") && name.ends_with(".json")
        || include_provenance && name.contains("provenance") && name.ends_with(".jsonl")
}

fn public_identity_files(
    session: &mut SourceRevisionReadSession<'_>,
    basenames: &BTreeSet<String>,
    include_provenance: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(BTreeMap<String, Vec<u8>>, usize)> {
    let mut pending = vec!["ToS/source-witnesses".to_owned()];
    let mut files = BTreeMap::new();
    let mut entries = 0usize;
    let mut total = 0u64;
    while let Some(parent) = pending.pop() {
        active(deadline, cancelled)?;
        let parent_path = RelativePath::parse(&parent)
            .map_err(|_| SourceCommandError::Invalid("private owner identity directory"))?;
        let children = session
            .list_immediate_children(&parent_path, deadline, cancelled)
            .map_err(pg::source_error)?;
        for (name, directory) in children {
            entries += 1;
            if entries > MAX_IDENTITY_ENTRIES {
                return Err(SourceCommandError::Unsupported(
                    "PostgreSQL private owner identity entry budget",
                ));
            }
            if name.starts_with('.')
                || matches!(name.as_str(), "payload" | "local-content" | "catalog")
            {
                continue;
            }
            let path = format!("{parent}/{name}");
            if path == "ToS/source-witnesses/owner-local"
                || path.starts_with("ToS/source-witnesses/owner-local/")
            {
                return Err(SourceCommandError::Denied(
                    "reserved owner-local namespace in selected public identity prefix",
                ));
            }
            if directory {
                pending.push(path);
            } else if identity_candidate(&name, basenames, include_provenance) {
                if files.len() >= MAX_IDENTITY_FILES {
                    return Err(SourceCommandError::Unsupported(
                        "PostgreSQL private owner identity file budget",
                    ));
                }
                let max = usize::try_from(MAX_IDENTITY_MEMBER_BYTES)
                    .unwrap_or(usize::MAX)
                    .min(
                        usize::try_from(MAX_IDENTITY_BYTES.saturating_sub(total))
                            .unwrap_or(usize::MAX),
                    );
                let raw = read_member(session, &path, max, deadline, cancelled)?;
                total = total
                    .checked_add(raw.len() as u64)
                    .filter(|bytes| *bytes <= MAX_IDENTITY_BYTES)
                    .ok_or(SourceCommandError::Unsupported(
                        "PostgreSQL private owner identity byte budget",
                    ))?;
                files.insert(path, raw);
            }
        }
    }
    Ok((files, entries))
}

fn profile_family(value: &str) -> SourceCommandResult<PrivateOwnerPackageFamilyV1> {
    match value {
        crate::source_native_cli::private_owner::profile::CONFIG => {
            Ok(PrivateOwnerPackageFamilyV1::Profile)
        }
        crate::source_native_cli::private_owner::claim::CONFIG_V1
        | crate::source_native_cli::private_owner::claim::CONFIG_V2 => {
            Ok(PrivateOwnerPackageFamilyV1::Claim)
        }
        _ => Err(SourceCommandError::Denied(
            "PostgreSQL private owner exact family",
        )),
    }
}

/// Select exact endpoint and native-binding packages from the protected grant.
/// The identity census intentionally omits non-identity packet/payload members;
/// it cannot substitute for this source dependency closure.
fn private_dependency_targets(
    owner: &OwnerTextContext,
    grant: &JsonValue,
    family: PrivateOwnerPackageFamilyV1,
) -> SourceCommandResult<BTreeSet<String>> {
    use crate::source_private_assessment_pg_sources::{add_binding_targets, add_private_target};
    let mut targets = BTreeSet::new();
    match family {
        PrivateOwnerPackageFamilyV1::Profile => {
            let binding = cmd::field(grant, "source_binding")?;
            if !binding.is_null() {
                add_binding_targets(owner, binding, &mut targets)?;
            }
        }
        PrivateOwnerPackageFamilyV1::Claim => {
            for selection in cmd::array(grant, "claim_selections")? {
                for endpoint in cmd::array(selection, "source_records")? {
                    add_private_target(owner, cmd::text(endpoint, "path")?, &mut targets)?;
                    let binding = cmd::field(endpoint, "source_binding")?;
                    if !binding.is_null() {
                        add_binding_targets(owner, binding, &mut targets)?;
                    }
                }
                for native in cmd::array(selection, "native_bindings")? {
                    add_binding_targets(owner, cmd::field(native, "binding")?, &mut targets)?;
                }
            }
        }
        _ => {
            return Err(SourceCommandError::Denied(
                "private owner dependency family",
            ));
        }
    }
    if targets.len() as u64 > MAX_CONTEXT_MEMBERS {
        return Err(SourceCommandError::Unsupported(
            "private owner dependency target budget",
        ));
    }
    Ok(targets)
}

fn source_binding(
    readset: &crate::source_cohort::revision::SourceRevisionReadSet,
    dependencies: &BTreeMap<String, Vec<u8>>,
    private_generation: u64,
) -> DurableErrorResult<PrivateOwnerSourceBindingV1> {
    let selected = readset.selection();
    let reads = dependencies
        .iter()
        .map(|(path, bytes)| (path.clone(), bytes.clone()))
        .collect::<Vec<_>>();
    Ok(PrivateOwnerSourceBindingV1 {
        head_seq: selected.head_seq,
        source_epoch: selected.epoch,
        source_generation: selected.source_generation,
        source_revision: selected.cohort.initial_revision().0,
        source_audit_generation: selected.audit_generation,
        rights_version: selected.rights_version,
        rule_version: selected.rule_version,
        contract_digest: selected.contract,
        job_id: selected.job_id.clone(),
        job_fence: selected.job_fence,
        readset_digest: private_owner_readset_digest(readset)?,
        dependency_digest: private_owner_dependency_digest(&reads)?,
        private_owner_audit_generation: private_generation,
    })
}

type DurableErrorResult<T> = Result<T, DurableError>;

struct PreparedOwnerPlan {
    before: Option<BTreeMap<String, Vec<u8>>>,
    after: Option<BTreeMap<String, Vec<u8>>>,
    response: JsonValue,
    archive: Option<(String, BTreeMap<String, Vec<u8>>)>,
    reads: Vec<cmd::SourceFile>,
    replayed: bool,
}

struct PrivateOwnerCapability<'a> {
    write: PrivateOwnerPackageWriteV1,
    expected_after: BTreeMap<String, Vec<u8>>,
    invocation_path: &'a Path,
    invocation_raw: &'a [u8],
    grant_path: &'a Path,
    grant_raw: &'a [u8],
    owner: &'a OwnerTextContext,
    context: &'a cmd::CommandContext,
    context_binding: Digest256,
    software: &'a SoftwareCaptureReader,
    components: &'a SoftwareComponentSelectionV1,
    uid: u32,
    runtime: &'a crate::source_serialization::ExecutableObservation,
    executable_digest: Digest256,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}

impl PrivateOwnerCapability<'_> {
    fn revalidate_inputs(&self) -> DurableErrorResult<()> {
        let invocation = read_absolute(
            self.invocation_path,
            self.uid,
            true,
            self.invocation_raw.len(),
            self.deadline,
            self.cancelled,
        )
        .map_err(DurableError::Source)?;
        let grant = read_absolute(
            self.grant_path,
            self.uid,
            true,
            1_048_576,
            self.deadline,
            self.cancelled,
        )
        .map_err(DurableError::Source)?;
        if invocation != self.invocation_raw || grant != self.grant_raw {
            return Err(DurableError::Conflict(
                "private owner invocation or grant changed during commit",
            ));
        }
        if Digest256::of_bytes(&grant) != self.write.owner_configuration_digest {
            return Err(DurableError::Conflict(
                "private owner configuration digest changed",
            ));
        }
        let grant_value = cmd::parse(&grant).map_err(DurableError::Source)?;
        cmd::validate_expiry(
            cmd::text(&grant_value, "expires_at").map_err(DurableError::Source)?,
            &crate::source_serialization::instant().map_err(DurableError::Source)?,
        )
        .map_err(DurableError::Source)?;
        if self
            .owner
            .private_owner_scope_binding(self.deadline, self.cancelled)
            .map_err(DurableError::Source)?
            != self.context_binding
        {
            return Err(DurableError::Conflict(
                "private owner context changed during commit",
            ));
        }
        self.owner
            .snapshot(self.deadline, self.cancelled)
            .map_err(DurableError::Source)?;
        if self
            .runtime
            .current_digest(self.deadline, self.cancelled)
            .map_err(DurableError::Source)?
            != self.executable_digest
        {
            return Err(DurableError::Conflict(
                "private owner executable changed during commit",
            ));
        }
        self.context
            .check_selected_software_inputs(
                self.software,
                self.components,
                self.deadline,
                self.cancelled,
            )
            .map_err(DurableError::Source)
    }
}

impl capability_seal::Sealed for PrivateOwnerCapability<'_> {}

impl PrivateOwnerPackageCapabilityV1 for PrivateOwnerCapability<'_> {
    fn write(&self) -> &PrivateOwnerPackageWriteV1 {
        &self.write
    }

    fn revalidate(
        &mut self,
        current: Option<&PrivateOwnerPackageSnapshotV1>,
        replay: Option<&PrivateOwnerPackageReplayV1>,
    ) -> DurableErrorResult<()> {
        if replay.is_none() && current != self.write.expected_current.as_ref() {
            return Err(DurableError::Conflict(
                "private owner current package changed",
            ));
        }
        if let Some(current) = current {
            if current.scope != self.write.scope
                || current.target_ref != self.write.target_ref
                || current.family != self.write.family
            {
                return Err(DurableError::Conflict(
                    "private owner current package identity changed",
                ));
            }
        }
        if let Some(replay) = replay {
            let digest = crate::durable_private_packages::package_digest(&self.expected_after)?;
            if replay.package_digest != digest
                || current.map(|snapshot| snapshot.package_digest) != Some(digest)
                || replay.response_bytes != self.write.response_bytes
            {
                return Err(DurableError::Conflict(
                    "private owner replay receipt differs from prepared package",
                ));
            }
        }
        self.revalidate_inputs()
    }
}

fn insert_dependency(
    dependencies: &mut BTreeMap<String, Vec<u8>>,
    path: String,
    raw: Vec<u8>,
) -> SourceCommandResult<()> {
    add_bytes(dependencies, path, raw)
}

fn check_dependency_budget(
    dependencies: &BTreeMap<String, Vec<u8>>,
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
) -> SourceCommandResult<()> {
    if dependencies.len() > max_members
        || dependencies
            .values()
            .any(|raw| raw.len() as u64 > max_member_bytes)
        || dependencies
            .values()
            .try_fold(0u64, |sum, raw| sum.checked_add(raw.len() as u64))
            .is_none_or(|sum| sum > max_total_bytes)
    {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL private owner dependency inventory budget",
        ));
    }
    Ok(())
}

fn private_generation(
    coordinator: &mut DurablePgCoordinator,
    scope: &PrivateOwnerScopeV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<u64> {
    active(deadline, cancelled)?;
    let lookup = coordinator
        .read_private_text_identity_matches(
            scope,
            &BTreeSet::new(),
            None,
            &BTreeMap::new(),
            deadline,
            cancelled,
        )
        .map_err(pg::source_error)?;
    active(deadline, cancelled)?;
    if !lookup.matches.is_empty() {
        return Err(SourceCommandError::Conflict(
            "empty private identity lookup returned matches",
        ));
    }
    Ok(lookup.private_audit_generation)
}

fn same_private_current(
    coordinator: &mut DurablePgCoordinator,
    scope: &PrivateOwnerScopeV1,
    target_ref: &str,
    expected: &Option<PrivateOwnerPackageSnapshotV1>,
    generation: u64,
) -> SourceCommandResult<()> {
    let current = coordinator
        .read_private_owner_current(scope, target_ref)
        .map_err(pg::source_error)?;
    if current != *expected
        || current
            .as_ref()
            .is_some_and(|snapshot| snapshot.private_audit_generation != generation)
    {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL private owner current package changed",
        ));
    }
    Ok(())
}

pub(crate) fn run(
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    input: impl Read,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    let parsed = cmd::parse(invocation_raw)?;
    cmd::exact_keys(
        &parsed,
        &[
            "schema_version",
            "owner_context",
            "owner_config",
            "native_executable",
            "native_executable_sha256",
            "source_selection",
            "software_capture",
            "software_restored_root",
            "software_selection",
            "software_components",
            "schema_worker",
            "budgets",
        ],
    )?;
    if cmd::text(&parsed, "schema_version")? != INVOCATION_SCHEMA {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL private owner invocation profile",
        ));
    }
    let selection = pg::selected_source(invocation)?;
    let (max_members, max_total_bytes, max_member_bytes) = owner_budgets(invocation)?;
    if max_members == 0 || max_total_bytes == 0 || max_member_bytes == 0 {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL private owner empty budget",
        ));
    }
    let _ =
        crate::source_text_owner::normalized_absolute(cmd::text(&parsed, "native_executable")?)?;
    let runtime = crate::source_serialization::observe_executable(deadline, cancelled)?;
    let executable_digest = super::digest(cmd::text(&parsed, "native_executable_sha256")?)?;
    if runtime.current_digest(deadline, cancelled)? != executable_digest {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL private owner executable identity",
        ));
    }
    let (software, components) = pg::software_selection(invocation, deadline, cancelled)?;
    check_software_budget(
        &software,
        &components,
        max_members,
        max_total_bytes,
        max_member_bytes,
        deadline,
        cancelled,
    )?;
    let request_raw = pg::read_request(input)?;
    let request = cmd::parse(&request_raw)?;
    let context_path = super::absolute(cmd::text(&parsed, "owner_context")?)?;
    let grant_path = super::absolute(cmd::text(&parsed, "owner_config")?)?;
    let grant_raw = read_absolute(&grant_path, uid, true, 1_048_576, deadline, cancelled)?;
    let grant_value = cmd::parse(&grant_raw)?;
    let grant_schema = cmd::text(&grant_value, "schema_version")?;
    let family = profile_family(grant_schema)?;
    let family_schema = match family {
        PrivateOwnerPackageFamilyV1::Profile => "profile",
        PrivateOwnerPackageFamilyV1::Claim => "claim",
        _ => return Err(SourceCommandError::Denied("private owner family")),
    };
    let operation = cmd::text(&request, "operation")?.to_owned();
    let store = SegmentStore::open_existing(&selection.store_root, selection.store_limits)
        .map_err(|_| SourceCommandError::Conflict("selected private owner source store"))?;
    let mut db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected private owner PostgreSQL"))?,
    );
    let mut archive_db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected private owner PostgreSQL"))?,
    );

    let (
        (
            first_epoch,
            schema_members,
            owner,
            context_config,
            mut context,
            mut worker,
            basenames,
        ),
        _,
    ) = db
        .with_source_revision_session(
            &store,
            &selection.domain,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                let selected_epoch = epoch(session);
                if selected_epoch.contract != selection.contract {
                    return Err(DurableError::Conflict(
                        "selected private owner source contract differs",
                    ));
                }
                let schema_members =
                    selected_contracts(session, family, &grant_value, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                let context_schema =
                    schema_members
                        .get(CONTEXT_SCHEMA)
                        .ok_or(DurableError::Conflict(
                            "selected private owner context schema absent",
                        ))?;
                let mut authored = BTreeMap::new();
                for path in [ENTITY_REGISTRY, RELATION_REGISTRY] {
                    insert_dependency(
                        &mut authored,
                        path.to_owned(),
                        read_authored(session, path, deadline, cancelled)
                            .map_err(DurableError::Source)?,
                    )
                    .map_err(DurableError::Source)?;
                }
                for (path, raw) in &schema_members {
                    insert_dependency(&mut authored, path.clone(), raw.clone())
                        .map_err(DurableError::Source)?;
                }
                let schema_files = source_files(&schema_members).map_err(DurableError::Source)?;
                let identity = session.selection().identity();
                let mut worker = selected_asserted_candidate_schema_from_context(
                    invocation,
                    &schema_files,
                    identity,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                let (owner, context_config) = OwnerTextContext::select_pg(
                    &context_path,
                    context_schema,
                    &mut worker,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                owner
                    .install_pg_selected_members(authored.clone())
                    .map_err(DurableError::Source)?;
                let mut files = authored;
                for member in components.members() {
                    let path = member.path.as_str();
                    let raw = software
                        .read_selected_component(
                            &components,
                            &member.path,
                            max_member_bytes,
                            deadline,
                            cancelled,
                        )
                        .map_err(|_| {
                            DurableError::Conflict("selected private owner software changed")
                        })?;
                    if path.starts_with("ToS/") {
                        if !super::private_owner::RULE_SCHEMA_COMPONENTS.contains(&path) {
                            return Err(DurableError::Refused(
                                "private owner rule software schema scope",
                            ));
                        }
                    } else {
                        insert_dependency(&mut files, path.to_owned(), raw)
                            .map_err(DurableError::Source)?;
                    }
                }
                let command = cmd::CommandContext {
                    base_revision: SourceRevision(selected_epoch.source_revision),
                    configuration_raw: grant_raw.clone(),
                    request_raw: request_raw.clone(),
                    recorded_at: crate::source_serialization::instant()
                        .map_err(DurableError::Source)?,
                    effective_uid: u64::from(uid),
                    files: source_files(&files).map_err(DurableError::Source)?,
                };
                command.check().map_err(DurableError::Source)?;
                let session = Rc::new(RefCell::new(session));
                let selected = PgPrivateSessionRecordRead {
                    command: command.clone(),
                    session: session.clone(),
                    identity,
                };
                let cut = PgPrivateSelectedSource {
                    session,
                    identity,
                    public_identity_paths: None,
                };
                let basenames = match family {
                    PrivateOwnerPackageFamilyV1::Profile => {
                        super::private_owner::profile::identity_basenames_pg(
                            &command,
                            &cut,
                            &selected,
                            &mut worker,
                            deadline,
                            cancelled,
                        )
                    }
                    PrivateOwnerPackageFamilyV1::Claim => {
                        super::private_owner::claim::claim_identity_basenames_pg(
                            &command,
                            &cut,
                            &selected,
                            &mut worker,
                            deadline,
                            cancelled,
                        )
                    }
                    _ => unreachable!(),
                }
                .map_err(DurableError::Source)?;
                Ok((
                    selected_epoch,
                    schema_members,
                    owner,
                    context_config,
                    command,
                    worker,
                    basenames,
                ))
            },
        )
        .map_err(pg::source_error)?;

    if context.base_revision != SourceRevision(first_epoch.source_revision) {
        return Err(SourceCommandError::Conflict(
            "private owner command source revision changed",
        ));
    }
    let context_binding = owner.private_owner_scope_binding(deadline, cancelled)?;
    let scope = PrivateOwnerScopeV1::for_context(&selection.domain, context_binding)
        .map_err(pg::source_error)?;
    if cmd::text(&grant_value, "source_context_ref")?
        != context_path
            .to_str()
            .ok_or(SourceCommandError::Invalid("private owner context path"))?
    {
        return Err(SourceCommandError::Denied(
            "private owner grant context differs",
        ));
    }
    if cmd::integer(&grant_value, "uid")? != u64::from(uid) {
        return Err(SourceCommandError::Denied(
            "private owner grant account differs",
        ));
    }
    cmd::validate_expiry(
        cmd::text(&grant_value, "expires_at")?,
        &crate::source_serialization::instant()?,
    )?;
    let source_path = cmd::text(&grant_value, "source_path")?;
    let target_ref = owner.private_package_target_ref(source_path)?;
    let target_current = db
        .read_private_owner_current(&scope, &target_ref)
        .map_err(pg::source_error)?;
    if target_current
        .as_ref()
        .is_some_and(|snapshot| snapshot.family != family)
    {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL private owner target family differs",
        ));
    }
    let creating = matches!(
        operation.as_str(),
        "source.create" | "claims.create" | "prepare-create"
    );
    let identity_snapshot = db
        .read_private_owner_identity_inputs(&scope, Some(&target_ref), &basenames, creating)
        .map_err(pg::source_error)?;
    let private_generation = identity_snapshot.private_audit_generation;
    let mut identity_inputs = PrivateIdentityInputs {
        files: identity_snapshot.files,
        visited_entries: identity_snapshot.visited_entries,
    };
    if target_current
        .as_ref()
        .is_some_and(|snapshot| snapshot.private_audit_generation != private_generation)
    {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL private owner package changed during identity selection",
        ));
    }
    let mut selected_private = identity_inputs.files.clone();
    for dependency_target in private_dependency_targets(&owner, &grant_value, family)? {
        active(deadline, cancelled)?;
        let dependency = db
            .read_private_owner_current(&scope, &dependency_target)
            .map_err(pg::source_error)?
            .ok_or(SourceCommandError::Conflict(
                "private owner selected dependency is absent",
            ))?;
        if dependency.private_audit_generation != private_generation {
            return Err(SourceCommandError::Conflict(
                "private owner dependency generation changed",
            ));
        }
        for (name, bytes) in dependency.members {
            add_bytes(
                &mut selected_private,
                format!("{dependency_target}/{name}"),
                bytes,
            )?;
        }
        if selected_private.len() > max_members
            || selected_private
                .values()
                .any(|raw| raw.len() as u64 > max_member_bytes)
            || selected_private
                .values()
                .try_fold(0u64, |sum, raw| sum.checked_add(raw.len() as u64))
                .is_none_or(|sum| sum > max_total_bytes)
        {
            return Err(SourceCommandError::Unsupported(
                "private owner selected dependency budget",
            ));
        }
    }
    if let Some(current) = &target_current {
        for (name, bytes) in &current.members {
            add_bytes(
                &mut selected_private,
                format!("{}/{name}", current.target_ref),
                bytes.clone(),
            )?;
        }
    }
    owner.extend_pg_selected_members(selected_private)?;

    let ((prepared, dependency_members, archive_observations), readset) = db
        .with_source_revision_session(
            &store,
            &selection.domain,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                if epoch(session) != first_epoch {
                    return Err(DurableError::Conflict(
                        "PostgreSQL private owner source epoch changed",
                    ));
                }
                let selected_schemas =
                    selected_contracts(session, family, &grant_value, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                if selected_schemas != schema_members {
                    return Err(DurableError::Conflict(
                        "PostgreSQL private owner contracts changed",
                    ));
                }
                let (public_identity, public_visited) =
                    public_identity_files(session, &basenames, creating, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                if public_visited > MAX_IDENTITY_ENTRIES {
                    return Err(DurableError::Refused(
                        "private owner public identity entry budget",
                    ));
                }
                let mut files = BTreeMap::new();
                for file in &context.files {
                    let path = file.path.as_str();
                    let raw = if path.starts_with("ToS/") {
                        let current = read_member(
                            session,
                            path,
                            MAX_CONTEXT_MEMBER_BYTES as usize,
                            deadline,
                            cancelled,
                        )
                        .map_err(DurableError::Source)?;
                        if current != file.raw {
                            return Err(DurableError::Conflict(
                                "PostgreSQL private owner selected source inputs changed",
                            ));
                        }
                        current
                    } else {
                        file.raw.clone()
                    };
                    add_bytes(&mut files, path.to_owned(), raw).map_err(DurableError::Source)?;
                }
                for (path, raw) in &public_identity {
                    add_bytes(&mut files, path.clone(), raw.clone())
                        .map_err(DurableError::Source)?;
                }
                if files.len() > max_members
                    || files
                        .values()
                        .any(|raw| raw.len() as u64 > max_member_bytes)
                    || files
                        .values()
                        .try_fold(0u64, |sum, raw| sum.checked_add(raw.len() as u64))
                        .is_none_or(|sum| sum > max_total_bytes)
                {
                    return Err(DurableError::Refused("private owner selected input budget"));
                }
                owner
                    .extend_pg_selected_members(public_identity.clone())
                    .map_err(DurableError::Source)?;
                context.files = source_files(&files).map_err(DurableError::Source)?;
                let identity = session.selection().identity();
                let session = Rc::new(RefCell::new(session));
                let selected = PgPrivateSessionRecordRead {
                    command: context.clone(),
                    session: session.clone(),
                    identity,
                };
                let cut = PgPrivateSelectedSource {
                    session,
                    identity,
                    public_identity_paths: Some(public_identity.keys().cloned().collect()),
                };
                let mut semantic_worker =
                    super::private_owner::profile::CandidatePrivateSchemaExecutor::new(
                        &context,
                        &cut,
                        &selected,
                        &mut worker,
                    )
                    .map_err(DurableError::Source)?;
                let mut plan = PreparedOwnerPlan {
                    before: None,
                    after: None,
                    response: JsonValue::Null,
                    archive: None,
                    reads: Vec::new(),
                    replayed: false,
                };
                let mut archive_reader = PgPrivateArchiveReader {
                    coordinator: &mut archive_db,
                    scope: scope.clone(),
                    target_ref: target_ref.clone(),
                    family,
                    generation: private_generation,
                    max_members,
                    max_total_bytes,
                    max_member_bytes,
                    observations: BTreeMap::new(),
                };
                match family {
                    PrivateOwnerPackageFamilyV1::Profile => {
                        let value = super::private_owner::profile::prepare(
                            &context,
                            &cut,
                            &software,
                            &components,
                            &owner,
                            &context_config,
                            target_current.as_ref().map(|current| &current.members),
                            &identity_inputs,
                            &mut archive_reader,
                            &mut semantic_worker,
                            deadline,
                            cancelled,
                        )
                        .map_err(DurableError::Source)?;
                        plan.before = value.before;
                        plan.after = value.after;
                        plan.response = value.response;
                        plan.archive = value.archive;
                        plan.reads = value.reads;
                        plan.replayed = value.replayed;
                    }
                    PrivateOwnerPackageFamilyV1::Claim => {
                        // The maintained Claim identity census charges all
                        // selected public and private directory entries.
                        identity_inputs.visited_entries = identity_inputs
                            .visited_entries
                            .checked_add(public_visited)
                            .ok_or(DurableError::Refused(
                                "private Claim identity entry budget overflow",
                            ))?;
                        let value = super::private_owner::claim::prepare(
                            &context,
                            &cut,
                            &software,
                            &components,
                            &owner,
                            &context_config,
                            target_current.as_ref().map(|current| &current.members),
                            &identity_inputs,
                            &mut archive_reader,
                            &mut semantic_worker,
                            deadline,
                            cancelled,
                        )
                        .map_err(DurableError::Source)?;
                        plan.before = value.before;
                        plan.after = value.files;
                        plan.response = value.response;
                        plan.archive = value.archive;
                        plan.reads = value.reads;
                        plan.replayed =
                            plan.response.object_get("replayed") == Some(&JsonValue::Bool(true));
                    }
                    _ => unreachable!(),
                }
                let expected_before = target_current.as_ref().map(|current| &current.members);
                if plan.before.as_ref() != expected_before {
                    return Err(DurableError::Conflict(
                        "private owner semantic predecessor differs from PostgreSQL current",
                    ));
                }
                for (path, raw) in &public_identity {
                    add_bytes(&mut files, path.clone(), raw.clone())
                        .map_err(DurableError::Source)?;
                }
                for read in &plan.reads {
                    add_bytes(&mut files, read.path.as_str().to_owned(), read.raw.clone())
                        .map_err(DurableError::Source)?;
                }
                for (path, raw) in &identity_inputs.files {
                    add_bytes(&mut files, path.clone(), raw.clone())
                        .map_err(DurableError::Source)?;
                }
                if let Some(current) = &target_current {
                    for (name, bytes) in &current.members {
                        add_bytes(
                            &mut files,
                            format!("{}/{name}", current.target_ref),
                            bytes.clone(),
                        )
                        .map_err(DurableError::Source)?;
                    }
                }
                for (reference, members) in &archive_reader.observations {
                    for (name, bytes) in members {
                        add_bytes(&mut files, format!("{reference}/{name}"), bytes.clone())
                            .map_err(DurableError::Source)?;
                    }
                }
                // `context.files` is the bounded selected owner input set;
                // retain semantic reads separately in the durable dependency
                // root so private package bytes never enter public source.
                Ok((plan, files, archive_reader.observations))
            },
        )
        .map_err(pg::source_error)?;

    let mut dependencies = dependency_members;
    for file in &context.files {
        insert_dependency(
            &mut dependencies,
            file.path.as_str().to_owned(),
            file.raw.clone(),
        )?;
    }
    for (path, raw) in &identity_inputs.files {
        insert_dependency(&mut dependencies, path.clone(), raw.clone())?;
    }
    if let Some(current) = &target_current {
        for (name, raw) in &current.members {
            insert_dependency(
                &mut dependencies,
                format!("{}/{name}", current.target_ref),
                raw.clone(),
            )?;
        }
    }
    for (reference, members) in archive_observations {
        for (name, raw) in members {
            insert_dependency(&mut dependencies, format!("{reference}/{name}"), raw)?;
        }
    }
    check_dependency_budget(
        &dependencies,
        max_members,
        max_total_bytes,
        max_member_bytes,
    )?;
    let binding =
        source_binding(&readset, &dependencies, private_generation).map_err(pg::source_error)?;
    if binding.source_revision != first_epoch.source_revision
        || binding.source_audit_generation != first_epoch.source_audit_generation
    {
        return Err(SourceCommandError::Conflict(
            "private owner source binding differs from selected epoch",
        ));
    }

    worker
        .finish(deadline, cancelled)
        .map_err(|_| SourceCommandError::Denied("PostgreSQL private owner worker FINAL"))?;
    if let Some(after) = prepared.after.clone() {
        if prepared.replayed {
            return Err(SourceCommandError::Conflict(
                "private owner replay unexpectedly prepared a new package",
            ));
        }
        let request_digest = Digest256::of_bytes(&cmd::canonical(&request)?);
        let archive = prepared
            .archive
            .as_ref()
            .map(|(reference, members)| PrivateOwnerPackageArchiveV1 {
                reference: reference.clone(),
                source_revision: binding.source_revision,
                members: members.clone(),
            })
            .into_iter()
            .collect::<Vec<_>>();
        let response_bytes = cmd::canonical(&prepared.response)?;
        let write = PrivateOwnerPackageWriteV1 {
            scope: scope.clone(),
            family,
            target_ref: target_ref.clone(),
            expected_current: target_current.clone(),
            members: after.clone(),
            archives: archive,
            command_id: cmd::text(&request, "command_id")?.to_owned(),
            request_digest,
            raw_request_digest: Digest256::of_bytes(&request_raw),
            owner_configuration_digest: Digest256::of_bytes(&grant_raw),
            source_binding: binding,
            response_bytes,
        };
        let mut capability = PrivateOwnerCapability {
            write,
            expected_after: after,
            invocation_path,
            invocation_raw,
            grant_path: &grant_path,
            grant_raw: &grant_raw,
            owner: &owner,
            context: &context,
            context_binding,
            software: &software,
            components: &components,
            uid,
            runtime: &runtime,
            executable_digest,
            deadline,
            cancelled,
        };
        let replay = db
            .commit_private_owner_package(&mut capability)
            .map_err(pg::source_error)?;
        let result: Value = serde_json::from_slice(&cmd::canonical(&prepared.response)?)
            .map_err(|_| SourceCommandError::Invalid("private owner result JSON"))?;
        return Ok(json!({
            "schema_version":"tos_native_pg_private_owner_result_v1",
            "authentication":"local-unix-account",
            "family":family_schema,
            "command_id":cmd::text(&request,"command_id")?,
            "commit_seq":replay.commit_seq,
            "package_sha256":replay.package_digest.to_prefixed(),
            "receipt_sha256":Digest256::of_bytes(&replay.response_bytes).to_prefixed(),
            "replayed":replay.replayed,
            "grants_admission":false,
            "content_disclosure":"owner-local",
            "result":result
        }));
    }
    let retained_replay = if prepared.replayed {
        Some(
            db.read_private_owner_command_replay(
                &scope,
                &target_ref,
                family,
                cmd::text(&request, "command_id")?,
                Digest256::of_bytes(&cmd::canonical(&request)?),
                Digest256::of_bytes(&request_raw),
                Digest256::of_bytes(&grant_raw),
                private_generation,
                deadline,
                cancelled,
            )
            .map_err(pg::source_error)?,
        )
    } else {
        None
    };
    verify_readonly(
        &mut db,
        &store,
        &selection,
        &scope,
        &target_ref,
        &target_current,
        private_generation,
        &first_epoch,
        family,
        &grant_value,
        &schema_members,
        invocation_path,
        invocation_raw,
        &grant_path,
        &grant_raw,
        &owner,
        context_binding,
        &context,
        &software,
        &components,
        &runtime,
        executable_digest,
        uid,
        deadline,
        cancelled,
    )?;
    let response_bytes = match &retained_replay {
        Some(replay) => replay.response_bytes.clone(),
        None => cmd::canonical(&prepared.response)?,
    };
    let result: Value = serde_json::from_slice(&response_bytes)
        .map_err(|_| SourceCommandError::Invalid("private owner result JSON"))?;
    Ok(json!({
        "schema_version":"tos_native_pg_private_owner_result_v1",
        "authentication":"local-unix-account",
        "family":family_schema,
        "command_id":request.object_get("command_id").and_then(JsonValue::as_str),
        "commit_seq":retained_replay.as_ref().map(|replay| replay.commit_seq),
        "package_sha256":retained_replay.as_ref().map(|replay| replay.package_digest.to_prefixed())
            .or_else(|| target_current.as_ref().map(|current| current.package_digest.to_prefixed())),
        "receipt_sha256":Digest256::of_bytes(&response_bytes).to_prefixed(),
        "replayed":prepared.replayed,
        "grants_admission":false,
        "content_disclosure":"owner-local",
        "result":result
    }))
}

fn verify_readonly(
    db: &mut DurablePgCoordinator,
    store: &SegmentStore,
    selection: &pg::SourceSelection,
    scope: &PrivateOwnerScopeV1,
    target_ref: &str,
    expected_current: &Option<PrivateOwnerPackageSnapshotV1>,
    expected_private_generation: u64,
    first_epoch: &SelectedEpoch,
    family: PrivateOwnerPackageFamilyV1,
    grant: &JsonValue,
    expected_schemas: &BTreeMap<String, Vec<u8>>,
    invocation_path: &Path,
    invocation_raw: &[u8],
    grant_path: &Path,
    grant_raw: &[u8],
    owner: &OwnerTextContext,
    context_binding: Digest256,
    context: &cmd::CommandContext,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    runtime: &crate::source_serialization::ExecutableObservation,
    executable_digest: Digest256,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    if read_absolute(
        invocation_path,
        uid,
        true,
        invocation_raw.len(),
        deadline,
        cancelled,
    )? != invocation_raw
        || read_absolute(grant_path, uid, true, 1_048_576, deadline, cancelled)? != grant_raw
        || owner.private_owner_scope_binding(deadline, cancelled)? != context_binding
    {
        return Err(SourceCommandError::Conflict(
            "private owner invocation, grant or context changed",
        ));
    }
    owner.snapshot(deadline, cancelled)?;
    if runtime.current_digest(deadline, cancelled)? != executable_digest {
        return Err(SourceCommandError::Conflict(
            "private owner executable changed",
        ));
    }
    context.check_selected_software_inputs(software, components, deadline, cancelled)?;
    same_private_current(
        db,
        scope,
        target_ref,
        expected_current,
        expected_private_generation,
    )?;
    if private_generation(db, scope, deadline, cancelled)? != expected_private_generation {
        return Err(SourceCommandError::Conflict(
            "private owner identity generation changed",
        ));
    }
    let _ = db
        .with_source_revision_session(
            store,
            &selection.domain,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                if epoch(session) != *first_epoch {
                    return Err(DurableError::Conflict(
                        "private owner source epoch changed before disclosure",
                    ));
                }
                let schemas = selected_contracts(session, family, grant, deadline, cancelled)
                    .map_err(DurableError::Source)?;
                if &schemas != expected_schemas {
                    return Err(DurableError::Conflict(
                        "private owner contracts changed before disclosure",
                    ));
                }
                Ok(())
            },
        )
        .map_err(pg::source_error)?;
    Ok(())
}

fn owner_budgets(invocation: &Value) -> SourceCommandResult<(usize, u64, u64)> {
    let budgets = &invocation["budgets"];
    super::exact(
        budgets,
        &[
            "max_members",
            "max_total_bytes",
            "max_member_bytes",
            "max_schema_receipts",
            "max_schema_receipt_bytes",
            "worker_cpu_seconds",
            "worker_address_space_bytes",
        ],
    )?;
    let max_members = usize::try_from(super::capped(budgets, "max_members", MAX_CONTEXT_MEMBERS)?)
        .map_err(|_| SourceCommandError::Invalid("private owner member budget"))?;
    let max_total_bytes = super::capped(budgets, "max_total_bytes", MAX_CONTEXT_BYTES)?;
    let max_member_bytes = super::capped(budgets, "max_member_bytes", MAX_CONTEXT_MEMBER_BYTES)?;
    let _ = super::capped(budgets, "max_schema_receipts", 128)?;
    let _ = super::capped(budgets, "max_schema_receipt_bytes", 262_144)?;
    let _ = super::capped(budgets, "worker_cpu_seconds", 3)?;
    let _ = super::capped(budgets, "worker_address_space_bytes", 1_073_741_824)?;
    Ok((max_members, max_total_bytes, max_member_bytes))
}

fn check_software_budget(
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let members = components.members().collect::<Vec<_>>();
    if members.len() > max_members
        || members
            .iter()
            .any(|member| member.size_bytes > max_member_bytes)
        || members
            .iter()
            .try_fold(0u64, |sum, member| sum.checked_add(member.size_bytes))
            .is_none_or(|sum| sum > max_total_bytes)
    {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL private owner software budget",
        ));
    }
    for member in members {
        active(deadline, cancelled)?;
        software
            .read_selected_component(
                components,
                &member.path,
                max_member_bytes,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("selected private owner software changed"))?;
    }
    Ok(())
}
