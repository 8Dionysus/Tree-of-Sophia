//! Public project Text create over a bounded PG source epoch. The public grant
//! still decides authority; this adapter only selects its exact ToS and
//! software-capture evidence and commits the owner's unchanged twelve-file
//! package through the shared source revision writer.

use super::{absolute, exact, text};
use crate::source_cohort::revision::{
    PgRecordRevisionIdentity, SourceRevisionReadSession, SourceRevisionReadSet,
};
use crate::source_cohort::{IndexRows, PreparedPgSourceRevision, SourceRevisionWrite};
use crate::source_command::{
    self as cmd, CommandContext, PreparedCommand, SourceChange, SourceCommandError,
    SourceCommandResult,
};
use crate::source_creation_store::{CreationFilesystem, finish_creation_worker};
use crate::source_native_cli::pg_claims as pg;
use crate::source_public_text_entry as public_text;
use crate::source_public_text_owner::{PublicNativeTextSelection, public_reference};
use crate::source_revisions::ReadonlyRecordFiles;
use crate::source_sign_native::{
    NativeReadKind, NativeReadScope, SignNativeRead, resolve_bindings,
    resolve_public_text_authority,
};
use crate::source_text_identity;
use crate::{DurableError, DurablePgCoordinator, PredicateKind, PredicateRead};
use postgres::Transaction;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath, SourceRevision};
use tos_segment_store::SegmentStore;
use tos_source_store::{SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::source_cut::CutWorkerSchemaExecutor;

const MAX_SCHEMA_FILES: usize = 128;
const MAX_SCHEMA_BYTES: usize = 16_777_216;
const MAX_SCHEMA_FILE_BYTES: u64 = 4_194_304;
const MAX_PACKAGE_BYTES: usize = 2 * 1024 * 1024;
const FAMILY: &str = crate::source_public_text_entry::PG_FAMILY_TAG;

// These are the schemas actually consumed by the public construction route.
// The three grant schemas form the selection closure; the additional seven
// are read only for preparation/recovery, where public authority, source
// endpoints, native bindings and the generated provenance event are checked.
const PUBLIC_TEXT_CONSTRUCTION_SCHEMAS: [&str; 10] = [
    "ToS/contracts/public-native-text-create-owner.schema.json",
    "ToS/contracts/public-native-text-authority.schema.json",
    "ToS/contracts/source-text-unit-packet-v1.schema.json",
    "ToS/contracts/provenance-event-v2.schema.json",
    "ToS/contracts/corpus-record.schema.json",
    "ToS/contracts/source-item-manifest.schema.json",
    "ToS/contracts/rights-record.schema.json",
    "ToS/contracts/native-text-unit-binding.schema.json",
    "ToS/contracts/source-text-layer.schema.json",
    "ToS/contracts/source-anchor-v2.schema.json",
];

fn value(value: &JsonValue) -> SourceCommandResult<Value> {
    serde_json::from_slice(&cmd::canonical(value)?)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL public Text response JSON"))
}

fn prepare_id(domain: &str, command_id: &str) -> Vec<u8> {
    let mut seed = b"tos-pg-public-native-text-v1\0".to_vec();
    seed.extend_from_slice(&(domain.len() as u64).to_be_bytes());
    seed.extend_from_slice(domain.as_bytes());
    seed.extend_from_slice(&(command_id.len() as u64).to_be_bytes());
    seed.extend_from_slice(command_id.as_bytes());
    Digest256::of_bytes(&seed).as_bytes().to_vec()
}

fn schemas(
    session: &mut SourceRevisionReadSession<'_>,
    include_construction: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    let mut total = 0usize;
    let selected = if include_construction {
        &PUBLIC_TEXT_CONSTRUCTION_SCHEMAS[..]
    } else {
        &crate::source_public_text_owner::SCHEMAS[..]
    };
    if selected.len() > MAX_SCHEMA_FILES {
        return Err(SourceCommandError::Unsupported(
            "public Text schema file budget",
        ));
    }
    for reference in selected {
        let path = RelativePath::parse(reference)
            .map_err(|_| SourceCommandError::Invalid("public Text schema path"))?;
        let raw = session
            .read_current_member(&path, MAX_SCHEMA_FILE_BYTES, deadline, cancelled)
            .map_err(pg::source_error)?
            .raw;
        total = total
            .checked_add(raw.len())
            .filter(|size| *size <= MAX_SCHEMA_BYTES)
            .ok_or(SourceCommandError::Unsupported(
                "public Text schema byte budget",
            ))?;
        files.insert((*reference).to_owned(), raw);
    }
    Ok(files)
}

fn checked_point_observations(
    requested: &BTreeSet<String>,
    observations: &BTreeMap<String, source_text_identity::PgSourceTextIdentityObservation>,
    expected_owners: &BTreeMap<String, BTreeSet<source_text_identity::TextIdentityOwner>>,
    ancestry: &BTreeMap<String, Digest256>,
) -> SourceCommandResult<(
    BTreeMap<String, BTreeSet<source_text_identity::TextIdentityOwner>>,
    BTreeMap<String, u64>,
)> {
    if observations.len() != requested.len() || observations.keys().ne(requested.iter()) {
        return Err(SourceCommandError::Conflict(
            "public Text identity point observation set changed",
        ));
    }
    let mut owners_by_token = BTreeMap::new();
    let mut generations_by_token = BTreeMap::new();
    for token in requested {
        let observed = observations.get(token).ok_or(SourceCommandError::Conflict(
            "public Text identity point observation absent",
        ))?;
        if observed
            .owners
            .iter()
            .any(|owner| ancestry.get(&owner.path) != Some(&owner.member_digest))
        {
            return Err(SourceCommandError::Conflict(
                "public Text delegated identity already owned",
            ));
        }
        if let Some(expected) = expected_owners.get(token) {
            if expected != &observed.owners {
                return Err(SourceCommandError::Conflict(
                    "public Text retained identity index differs from package bytes",
                ));
            }
        }
        owners_by_token.insert(token.clone(), observed.owners.clone());
        generations_by_token.insert(token.clone(), observed.generation);
    }
    Ok((owners_by_token, generations_by_token))
}

fn identity_snapshot(
    session: &mut SourceRevisionReadSession<'_>,
    delegated: &[String],
    retained_home: Option<&str>,
    retained_files: Option<&BTreeMap<String, Vec<u8>>>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(Digest256, BTreeMap<String, u64>)> {
    let requested = delegated.iter().cloned().collect::<BTreeSet<_>>();
    if requested.len() != delegated.len() || requested.iter().any(|token| token.is_empty()) {
        return Err(SourceCommandError::Invalid(
            "public Text delegated identity set",
        ));
    }

    let mut ancestry = BTreeMap::new();
    let mut expected_owners =
        BTreeMap::<String, BTreeSet<source_text_identity::TextIdentityOwner>>::new();
    if let (Some(home), Some(files)) = (retained_home, retained_files) {
        let mut package_ids = BTreeSet::new();
        for (name, raw) in files {
            if !source_text_identity::is_candidate(name) {
                continue;
            }
            let path = format!("{home}/{name}");
            let digest = Digest256::of_bytes(raw);
            ancestry.insert(path.clone(), digest);
            for token in source_text_identity::text_identity_ids_for_member(name, raw)? {
                package_ids.insert(token.clone());
                expected_owners.entry(token).or_default().insert(
                    source_text_identity::TextIdentityOwner {
                        path: path.clone(),
                        member_digest: digest,
                    },
                );
            }
        }
        if package_ids != requested {
            return Err(SourceCommandError::Conflict(
                "public Text retained package identity set",
            ));
        }
    } else if retained_home.is_some() || retained_files.is_some() {
        return Err(SourceCommandError::Invalid(
            "public Text retained identity evidence pair",
        ));
    }

    let observations = session
        .lookup_text_identity_owners(&requested, deadline, cancelled)
        .map_err(pg::source_error)?;
    let (owners_by_token, generations_by_token) =
        checked_point_observations(&requested, &observations, &expected_owners, &ancestry)?;
    let selection = session.selection();
    let digest = source_text_identity::point_identity_snapshot_digest(
        &selection,
        &requested,
        &owners_by_token,
        &generations_by_token,
        &ancestry,
    )?;
    Ok((digest, generations_by_token))
}

fn merge_files(
    target: &mut BTreeMap<String, Vec<u8>>,
    source: impl IntoIterator<Item = (String, Vec<u8>)>,
) -> SourceCommandResult<()> {
    for (path, raw) in source {
        if let Some(previous) = target.get(&path) {
            if previous != &raw {
                return Err(SourceCommandError::Conflict(
                    "public Text selected source bytes disagree",
                ));
            }
        } else {
            target.insert(path, raw);
        }
    }
    Ok(())
}

struct PublicReader<'s, 'db> {
    session: &'s mut SourceRevisionReadSession<'db>,
    software: &'s SoftwareCaptureReader,
    docs: BTreeMap<RelativePath, (SoftwareComponentSelectionV1, Vec<u8>)>,
    files: BTreeMap<String, Vec<u8>>,
    overlay: Option<(&'s str, &'s BTreeMap<String, Vec<u8>>)>,
}

impl PublicReader<'_, '_> {
    fn verify_docs(&self, deadline: Instant, cancelled: &AtomicBool) -> SourceCommandResult<()> {
        for (path, (selection, expected)) in &self.docs {
            let raw = self
                .software
                .read_selected_component(
                    selection,
                    path,
                    expected.len() as u64,
                    deadline,
                    cancelled,
                )
                .map_err(|_| {
                    SourceCommandError::Conflict("public Text selected docs input changed")
                })?;
            if &raw != expected {
                return Err(SourceCommandError::Conflict(
                    "public Text selected docs input changed",
                ));
            }
        }
        Ok(())
    }
}

impl SignNativeRead for PublicReader<'_, '_> {
    fn read(
        &mut self,
        reference: &str,
        _kind: NativeReadKind,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>> {
        if let Some((home, files)) = self.overlay {
            if let Some(name) = reference
                .strip_prefix(home)
                .and_then(|tail| tail.strip_prefix('/'))
            {
                if let Some(raw) = files.get(name) {
                    if raw.len() > max_bytes {
                        return Err(SourceCommandError::Unsupported(
                            "public Text proposed read cap",
                        ));
                    }
                    return Ok(raw.clone());
                }
            }
        }
        let path = public_reference(reference, false)?;
        if reference.starts_with("ToS/") {
            let raw = self
                .session
                .read_current_member(&path, max_bytes as u64, deadline, cancelled)
                .map_err(pg::source_error)?
                .raw;
            self.files.insert(reference.to_owned(), raw.clone());
            return Ok(raw);
        }
        if !reference.starts_with("docs/") {
            return Err(SourceCommandError::Denied(
                "public Text PG read outside selected ToS or docs namespaces",
            ));
        }
        if let Some((_, raw)) = self.docs.get(&path) {
            if raw.len() > max_bytes {
                return Err(SourceCommandError::Unsupported(
                    "public Text selected docs member budget",
                ));
            }
            return Ok(raw.clone());
        }
        let selection = self
            .software
            .select_components(std::slice::from_ref(&path))
            .map_err(|_| SourceCommandError::Conflict("public Text selected docs membership"))?;
        let raw = self
            .software
            .read_selected_component(&selection, &path, max_bytes as u64, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("public Text selected docs member"))?;
        self.files.insert(reference.to_owned(), raw.clone());
        self.docs.insert(path, (selection, raw.clone()));
        Ok(raw)
    }

    fn verify_current(
        &mut self,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self.verify_docs(deadline, cancelled)
    }

    fn owner_local(&self, _reference: &str) -> SourceCommandResult<bool> {
        Ok(false)
    }
}

struct SemanticPackage {
    selection: PublicNativeTextSelection,
    assembly: public_text::PublicTextAssembly,
    input_files: BTreeMap<String, Vec<u8>>,
    identity_generations: BTreeMap<String, u64>,
    dependencies: String,
    software_rows: Vec<Value>,
    software_files: Vec<cmd::SourceFile>,
    runtime_digest: Digest256,
    home: String,
    docs: BTreeMap<RelativePath, (SoftwareComponentSelectionV1, Vec<u8>)>,
    schemas: BTreeMap<String, Vec<u8>>,
    worker: CutWorkerSchemaExecutor,
}

pub(crate) struct PreparedPgPublicText {
    context: CommandContext,
    proposal: PreparedCommand,
    identity: PgRecordRevisionIdentity,
    selection: PublicNativeTextSelection,
    home: String,
    package_files: BTreeMap<String, Vec<u8>>,
    source_paths: BTreeSet<String>,
    identity_generations: BTreeMap<String, u64>,
    docs: BTreeMap<RelativePath, (SoftwareComponentSelectionV1, Vec<u8>)>,
    implementation: Vec<Value>,
    runtime: Digest256,
}

impl PreparedPgPublicText {
    fn new(
        context: CommandContext,
        proposal: PreparedCommand,
        identity: PgRecordRevisionIdentity,
        selection: PublicNativeTextSelection,
        home: String,
        package: BTreeMap<String, Vec<u8>>,
        source_paths: BTreeSet<String>,
        identity_generations: BTreeMap<String, u64>,
        docs: BTreeMap<RelativePath, (SoftwareComponentSelectionV1, Vec<u8>)>,
        implementation: Vec<Value>,
        runtime: Digest256,
    ) -> SourceCommandResult<Self> {
        let paths = package
            .keys()
            .map(|name| format!("{home}/{name}"))
            .collect::<BTreeSet<_>>();
        let changes = proposal
            .changes
            .iter()
            .map(|change| change.path.as_str().to_owned())
            .collect::<BTreeSet<_>>();
        let exact_new = package.len() == public_text::FILES.len()
            && public_text::FILES
                .iter()
                .all(|name| package.contains_key(*name))
            && paths == changes
            && proposal.changes.len() == package.len()
            && proposal.changes.iter().all(|change| {
                change.before.is_none()
                    && change.after.as_ref()
                        == change
                            .path
                            .as_str()
                            .rsplit_once('/')
                            .and_then(|(_, name)| package.get(name))
            });
        let exact_replay = proposal.replayed && proposal.changes.is_empty();
        if proposal.operation != "native-text.create" || (!exact_new && !exact_replay) {
            return Err(SourceCommandError::Conflict(
                "public Text PG package is not the exact twelve-file closure",
            ));
        }
        Ok(Self {
            context,
            proposal,
            identity,
            selection,
            home,
            package_files: package,
            source_paths,
            identity_generations,
            docs,
            implementation,
            runtime,
        })
    }

    pub(crate) fn proposal(&self) -> &PreparedCommand {
        &self.proposal
    }

    fn verify_docs(
        &self,
        software: &SoftwareCaptureReader,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        for (path, (selection, expected)) in &self.docs {
            let raw = software
                .read_selected_component(
                    selection,
                    path,
                    expected.len() as u64,
                    deadline,
                    cancelled,
                )
                .map_err(|_| {
                    SourceCommandError::Conflict("public Text selected docs input changed")
                })?;
            if &raw != expected {
                return Err(SourceCommandError::Conflict(
                    "public Text selected docs input changed",
                ));
            }
        }
        Ok(())
    }

    fn verify_current_owner(
        &self,
        invocation_path: &Path,
        invocation_raw: &[u8],
        uid: u32,
        owner_path: &Path,
        owner_raw: &[u8],
        store_root: &Path,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        runtime: &crate::source_serialization::ExecutableObservation,
        executable_digest: Digest256,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        if super::read_absolute(
            invocation_path,
            uid,
            true,
            invocation_raw.len(),
            deadline,
            cancelled,
        )? != invocation_raw
        {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL public Text invocation changed",
            ));
        }
        let (_owner, current_raw) = CreationFilesystem::select_protected_native_owner_at_root(
            owner_path, store_root, deadline, cancelled,
        )?;
        if current_raw != owner_raw {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL public Text owner grant changed",
            ));
        }
        self.selection.verify_current_from_pg(deadline, cancelled)?;
        self.verify_docs(software, deadline, cancelled)?;
        if crate::source_serialization::selected_components(
            software, components, deadline, cancelled,
        )? != self.implementation
            || runtime.current_digest(deadline, cancelled)? != executable_digest
            || self.runtime != executable_digest
        {
            return Err(SourceCommandError::Conflict(
                "PostgreSQL public Text implementation changed",
            ));
        }
        Ok(())
    }
}

impl PreparedPgSourceRevision for PreparedPgPublicText {
    fn context(&self) -> &CommandContext {
        &self.context
    }

    fn proposal(&self) -> &PreparedCommand {
        &self.proposal
    }

    fn identity(&self) -> PgRecordRevisionIdentity {
        self.identity
    }

    fn family_tag(&self) -> &str {
        FAMILY
    }

    fn legacy_family_encoding(&self) -> bool {
        false
    }

    fn validate_readset(&self, readset: &SourceRevisionReadSet) -> Result<(), DurableError> {
        if self.identity != PgRecordRevisionIdentity::from_selection(&readset.selection)
            || self.context.base_revision != readset.selection.cohort.initial_revision()
            || self.proposal.base_revision != readset.selection.cohort.initial_revision()
            || self.proposal.operation != "native-text.create"
        {
            return Err(DurableError::Conflict(
                "public Text PG source epoch or operation changed",
            ));
        }
        let mut observed_identity_generations = BTreeMap::new();
        for read in &readset.reads.reads {
            let PredicateRead::Generation {
                predicate,
                observed_generation,
            } = read
            else {
                continue;
            };
            if predicate.kind == PredicateKind::Unique
                && predicate.owner == crate::source_cohort::OWNER
                && predicate.scope == "text-identity"
                && observed_identity_generations
                    .insert(predicate.token.clone(), *observed_generation)
                    .is_some()
            {
                return Err(DurableError::Conflict(
                    "public Text PG identity predicate repeated",
                ));
            }
        }
        if observed_identity_generations != self.identity_generations {
            return Err(DurableError::Conflict(
                "public Text PG identity predicates differ from the prepared selection",
            ));
        }
        for path in &self.source_paths {
            let expected = self
                .context
                .files
                .iter()
                .find(|file| file.path.as_str() == path)
                .map(|file| &file.raw)
                .ok_or(DurableError::Conflict(
                    "public Text PG source evidence absent from context",
                ))?;
            let Some(observation) = readset.observations.get(path) else {
                return Err(DurableError::Conflict(
                    "public Text PG input is absent from the fenced readset",
                ));
            };
            if &observation.member.raw != expected {
                return Err(DurableError::Conflict(
                    "public Text PG input differs from the fenced readset",
                ));
            }
        }
        Ok(())
    }

    fn work_revision_evidence(
        &self,
        _writes: &[SourceRevisionWrite],
    ) -> Result<Option<crate::source_pg_work_revision::CommitEvidence>, DurableError> {
        Ok(None)
    }

    fn check_indexes(
        &self,
        tx: &mut Transaction<'_>,
        domain: &str,
        writes: &[SourceRevisionWrite],
    ) -> Result<IndexRows, DurableError> {
        crate::source_cohort::check_revision_path_indexes_for_family(tx, domain, writes, FAMILY)
    }

    fn inventory_owner_contributions(
        &self,
        writes: &[SourceRevisionWrite],
    ) -> Result<BTreeMap<String, JsonValue>, DurableError> {
        if writes.is_empty() {
            return Ok(BTreeMap::new());
        }
        let expected_paths = self
            .package_files
            .keys()
            .map(|name| format!("{}/{name}", self.home))
            .collect::<BTreeSet<_>>();
        let write_paths = writes
            .iter()
            .map(|write| write.subject.clone())
            .collect::<BTreeSet<_>>();
        if expected_paths != write_paths || writes.len() != self.package_files.len() {
            return Err(DurableError::Conflict(
                "public Text owner projection does not cover exact package writes",
            ));
        }
        let mut contributions = BTreeMap::new();
        for write in writes {
            let name = write
                .subject
                .strip_prefix(&format!("{}/", self.home))
                .ok_or(DurableError::Conflict(
                    "public Text owner projection path leaves package",
                ))?;
            if self.package_files.get(name) != Some(&write.exact_bytes) {
                return Err(DurableError::Conflict(
                    "public Text owner projection bytes differ from package",
                ));
            }
            let contribution = public_text::public_text_inventory_projection(
                &self.home,
                &write.subject,
                &self.package_files,
            )
            .map_err(DurableError::Source)?;
            contributions.insert(write.subject.clone(), contribution);
        }
        Ok(contributions)
    }
}

fn target_files(
    session: &mut SourceRevisionReadSession<'_>,
    home: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<BTreeMap<String, Vec<u8>>>> {
    let home_path = RelativePath::parse(home)
        .map_err(|_| SourceCommandError::Invalid("public Text package home"))?;
    let children = session
        .list_immediate_children(&home_path, deadline, cancelled)
        .map_err(pg::source_error)?;
    if children.is_empty() {
        return Ok(None);
    }
    let expected_names = public_text::FILES
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<BTreeSet<_>>();
    let observed_names = children
        .iter()
        .map(|(name, _)| name.clone())
        .collect::<BTreeSet<_>>();
    if children.len() != public_text::FILES.len()
        || observed_names.len() != children.len()
        || children
            .iter()
            .any(|(name, is_dir)| *is_dir || !expected_names.contains(name))
        || observed_names != expected_names
    {
        return Err(SourceCommandError::Conflict(
            "public Text package home has a different member set",
        ));
    }
    let mut present = Vec::new();
    for name in public_text::FILES {
        let path = format!("{home}/{name}");
        present.push(
            session
                .has_file(&path, deadline, cancelled)
                .map_err(|_| SourceCommandError::Conflict("public Text package target check"))?,
        );
    }
    if present.iter().all(|exists| !exists) {
        return Ok(None);
    }
    if present.iter().any(|exists| !exists) {
        return Err(SourceCommandError::Conflict(
            "public Text partial package target",
        ));
    }
    let mut files = BTreeMap::new();
    let mut total = 0usize;
    for name in public_text::FILES {
        let path = format!("{home}/{name}");
        let path = RelativePath::parse(&path)
            .map_err(|_| SourceCommandError::Invalid("public Text package member"))?;
        let raw = session
            .read_current_member(&path, MAX_PACKAGE_BYTES as u64, deadline, cancelled)
            .map_err(pg::source_error)?
            .raw;
        total = total
            .checked_add(raw.len())
            .filter(|size| *size <= MAX_PACKAGE_BYTES)
            .ok_or(SourceCommandError::Unsupported(
                "public Text package byte budget",
            ))?;
        files.insert(name.to_owned(), raw);
    }
    Ok(Some(files))
}

fn semantic_package(
    invocation: &Value,
    session: &mut SourceRevisionReadSession<'_>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    runtime: &crate::source_serialization::ExecutableObservation,
    excluded_home: Option<&str>,
    retained_files: Option<BTreeMap<String, Vec<u8>>>,
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<SemanticPackage> {
    let schemas = schemas(session, true, deadline, cancelled)?;
    let source_revision = session.selection().cohort.initial_revision();
    let schema_files = schemas
        .iter()
        .map(|(path, raw)| {
            Ok(cmd::SourceFile {
                path: RelativePath::parse(path)
                    .map_err(|_| SourceCommandError::Invalid("public Text schema path"))?,
                raw: raw.clone(),
            })
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let (software_files, software_count, software_bytes) =
        pg::software_files(software, components, max_member_bytes, deadline, cancelled)?;
    if software_count > max_members || software_bytes > max_total_bytes {
        return Err(SourceCommandError::Invalid(
            "public Text implementation budget",
        ));
    }
    let mut worker = super::revisions::selected_schema_from_context(
        invocation,
        &schema_files,
        source_revision,
        deadline,
        cancelled,
    )?;
    let selection = PublicNativeTextSelection::select_from_pg(
        &absolute(text(invocation, "owner_config")?)?,
        &schemas,
        &mut worker,
        deadline,
        cancelled,
    )?;
    let source_path = cmd::text(&selection.config, "source_path")?;
    let home = public_text::public_text_package_home(source_path)?;
    let ids = public_text::identities(&selection.config)?;
    let (current_identity_snapshot, identity_generations) = identity_snapshot(
        session,
        &ids,
        excluded_home,
        retained_files.as_ref(),
        deadline,
        cancelled,
    )?;
    let identity_inventory = if let Some(files) = &retained_files {
        let captured = cmd::parse(&files["source-create-inputs.json"])?;
        Digest256::from_prefixed(cmd::text(&captured, "identity_inventory")?)
            .map_err(|_| SourceCommandError::Invalid("public Text retained inventory"))?
    } else {
        current_identity_snapshot
    };
    let mut reader = PublicReader {
        session,
        software,
        docs: BTreeMap::new(),
        files: BTreeMap::new(),
        overlay: None,
    };
    let authority = resolve_public_text_authority(
        &mut reader,
        &mut worker,
        &selection.config,
        deadline,
        cancelled,
    )?;
    let source = cmd::field(&selection.config, "source")?;
    let source_ref = cmd::text(source, "ref")?;
    let source_limit = usize::try_from(cmd::integer(
        cmd::field(&selection.config, "limits")?,
        "max_source_bytes",
    )?)
    .map_err(|_| SourceCommandError::Invalid("public Text source budget"))?;
    let original = reader.read(
        source_ref,
        NativeReadKind::Content,
        source_limit,
        deadline,
        cancelled,
    )?;
    if Digest256::of_bytes(&original).to_hex() != cmd::text(source, "sha256")?
        || original.len() as u64 != cmd::integer(source, "byte_size")?
    {
        return Err(SourceCommandError::Conflict(
            "public Text original byte identity",
        ));
    }
    let assembly = public_text::assemble(
        &selection.config,
        &authority.authority,
        &selection.public_plan()?,
        &original,
        deadline,
        cancelled,
    )?;
    let rows = cmd::array(&assembly.bindings, "bindings")?;
    reader.overlay = Some((home.as_str(), &assembly.files));
    let closure = resolve_bindings(
        &mut reader,
        &mut worker,
        rows,
        NativeReadScope::PublicContent,
        deadline,
        cancelled,
    )?;
    if closure.summaries.iter().any(|summary| {
        cmd::field(summary, "public_content_available").ok() != Some(&JsonValue::Bool(true))
    }) {
        return Err(SourceCommandError::Denied(
            "public Text genuine public closure",
        ));
    }
    reader.verify_docs(deadline, cancelled)?;
    let input_files = std::mem::take(&mut reader.files);
    let docs = std::mem::take(&mut reader.docs);
    drop(reader);
    let software_rows = crate::source_serialization::selected_components(
        software, components, deadline, cancelled,
    )?;
    let runtime_digest = runtime.current_digest(deadline, cancelled)?;
    let input_digests = input_files
        .iter()
        .map(|(reference, raw)| (reference.clone(), json!(Digest256::of_bytes(raw).to_hex())))
        .collect::<serde_json::Map<_, _>>();
    let docs_members = docs
        .iter()
        .map(|(path, (selection, raw))| {
            let member = selection.member(path).ok_or(SourceCommandError::Conflict(
                "public Text docs selection membership changed",
            ))?;
            Ok(json!({
                "path":path.as_str(),
                "sha256":Digest256::of_bytes(raw).to_prefixed(),
                "size_bytes":member.size_bytes
            }))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let docs_capture = &components.capture();
    let captured = cmd::parse(
        &serde_json::to_vec(&json!({
            "schema_version":"tos_public_native_construction_inputs_v1",
            "inputs":input_digests,
            "identity_inventory":identity_inventory.to_prefixed(),
            "runtime":runtime_digest.to_prefixed(),
            "implementation":software_rows.clone(),
            "docs_capture":{
                "source_git_commit":docs_capture.source_git_commit,
                "source_git_tree":docs_capture.source_git_tree,
                "capture_manifest_sha256":docs_capture.capture_manifest_sha256.to_prefixed(),
                "components":docs_members
            },
            "source_files_verified":true,
            "original_payload_opened":false,
            "source_authorship_authenticated":false,
            "assessment_performed":false
        }))
        .map_err(|_| SourceCommandError::Invalid("public Text dependency snapshot"))?,
    )?;
    let dependencies = Digest256::of_bytes(&cmd::canonical(&captured)?).to_prefixed();
    let captured_raw = crate::source_text_layer_entry::line(&captured)?;
    let mut assembly = assembly;
    assembly
        .files
        .insert("source-create-inputs.json".into(), captured_raw);
    let _ = max_members;
    let _ = max_total_bytes;
    let _ = max_member_bytes;
    Ok(SemanticPackage {
        selection,
        assembly,
        input_files,
        identity_generations,
        dependencies,
        software_rows,
        software_files,
        runtime_digest,
        home,
        docs,
        schemas,
        worker,
    })
}

fn context(
    source_revision: SourceRevision,
    owner_raw: &[u8],
    request_raw: &[u8],
    uid: u32,
    semantic: &SemanticPackage,
    retained_files: Option<&BTreeMap<String, Vec<u8>>>,
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(CommandContext, BTreeSet<String>)> {
    let mut selected = BTreeMap::<String, Vec<u8>>::new();
    let mut source_paths = BTreeSet::new();
    merge_files(&mut selected, semantic.schemas.clone())?;
    merge_files(&mut selected, semantic.input_files.clone())?;
    source_paths.extend(semantic.schemas.keys().cloned());
    source_paths.extend(
        semantic
            .input_files
            .keys()
            .filter(|path| path.starts_with("ToS/"))
            .cloned(),
    );
    merge_files(
        &mut selected,
        semantic
            .software_files
            .iter()
            .map(|file| (file.path.as_str().to_owned(), file.raw.clone())),
    )?;
    if let Some(package) = retained_files {
        merge_files(
            &mut selected,
            package
                .iter()
                .map(|(name, raw)| (format!("{}/{name}", semantic.home), raw.clone())),
        )?;
        source_paths.extend(
            package
                .keys()
                .map(|name| format!("{}/{name}", semantic.home)),
        );
    }
    let files = selected
        .into_iter()
        .map(|(path, raw)| {
            Ok(cmd::SourceFile {
                path: RelativePath::parse(&path)
                    .map_err(|_| SourceCommandError::Invalid("public Text context path"))?,
                raw,
            })
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let context = CommandContext {
        base_revision: source_revision,
        configuration_raw: owner_raw.to_vec(),
        request_raw: request_raw.to_vec(),
        recorded_at: crate::source_serialization::instant()?,
        effective_uid: u64::from(uid),
        files,
    };
    pg::check_context_budget(&context, max_members, max_total_bytes, max_member_bytes)?;
    context.check()?;
    if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(SourceCommandError::Unsupported(
            "public Text context cancelled",
        ));
    }
    Ok((context, source_paths))
}

fn public_response(
    receipt: &JsonValue,
    bindings: &JsonValue,
    source_path: &str,
    replayed: bool,
) -> SourceCommandResult<Value> {
    Ok(json!({
        "status":if replayed {"replayed"} else {"created"},
        "source":value(cmd::field(receipt,"source")?)?,
        "source_path":source_path,
        "receipt_digest":Digest256::of_bytes(&cmd::canonical(receipt)?).to_prefixed(),
        "native_bindings":value(cmd::field(bindings,"bindings")?)?,
        "grants_admission":false,
        "external_publication_authorized":false
    }))
}

fn exact_request(request: &JsonValue, operation: &str) -> SourceCommandResult<()> {
    let keys = match operation {
        "describe" | "prepare-create" => vec!["schema_version", "operation"],
        "inspect-recovery" => vec!["schema_version", "operation", "command_id"],
        "native-text.create" => vec![
            "schema_version",
            "operation",
            "command_id",
            "expected_configuration",
            "expected_dependencies",
            "expected_source",
            "expected_revision",
        ],
        _ => {
            return Err(SourceCommandError::Unsupported(
                "public Text native operation",
            ));
        }
    };
    cmd::exact_keys(request, &keys)?;
    if cmd::text(request, "schema_version")? != "tos_local_source_command_v1"
        || cmd::text(request, "operation")? != operation
    {
        return Err(SourceCommandError::Invalid("public Text request profile"));
    }
    if matches!(operation, "inspect-recovery" | "native-text.create") {
        let command_id = cmd::text(request, "command_id")?;
        if command_id.is_empty() || command_id.chars().count() > 256 {
            return Err(SourceCommandError::Invalid("public Text command identity"));
        }
    }
    Ok(())
}

fn validate_invocation(invocation: &Value) -> SourceCommandResult<()> {
    exact(
        invocation,
        &[
            "schema_version",
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
    if text(invocation, "schema_version")? != "tos_local_pg_public_text_invocation_v1" {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL public Text invocation profile",
        ));
    }
    Ok(())
}

fn selection_from_session(
    invocation: &Value,
    session: &mut SourceRevisionReadSession<'_>,
    include_construction: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(
    PublicNativeTextSelection,
    CutWorkerSchemaExecutor,
    BTreeMap<String, Vec<u8>>,
)> {
    let schemas = schemas(session, include_construction, deadline, cancelled)?;
    let revision = session.selection().cohort.initial_revision();
    let schema_files = schemas
        .iter()
        .map(|(path, raw)| {
            Ok(cmd::SourceFile {
                path: RelativePath::parse(path)
                    .map_err(|_| SourceCommandError::Invalid("public Text schema path"))?,
                raw: raw.clone(),
            })
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let mut worker = super::revisions::selected_schema_from_context(
        invocation,
        &schema_files,
        revision,
        deadline,
        cancelled,
    )?;
    let selection = PublicNativeTextSelection::select_from_pg(
        &absolute(text(invocation, "owner_config")?)?,
        &schemas,
        &mut worker,
        deadline,
        cancelled,
    )?;
    Ok((selection, worker, schemas))
}

/// Complete one explicitly selected PG invocation for public whole-project
/// Text construction. The complete twelve-member owner package is committed
/// atomically as one family-tagged source revision.
pub(crate) fn run(
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    input: impl Read,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    validate_invocation(invocation)?;
    let source = pg::selected_source(invocation)?;
    let (max_members, max_total_bytes, max_member_bytes) = pg::invocation_budgets(invocation)?;
    let _ = absolute(text(invocation, "native_executable")?)?;
    let runtime = crate::source_serialization::observe_executable(deadline, cancelled)?;
    let executable_digest = super::digest(text(invocation, "native_executable_sha256")?)?;
    if runtime.current_digest(deadline, cancelled)? != executable_digest {
        return Err(SourceCommandError::Conflict(
            "native public Text executable identity",
        ));
    }
    let owner_path = absolute(text(invocation, "owner_config")?)?;
    let (_owner_fs, owner_raw) = CreationFilesystem::select_protected_native_owner_at_root(
        &owner_path,
        &source.store_root,
        deadline,
        cancelled,
    )?;
    let owner = cmd::parse(&owner_raw)?;
    if cmd::text(&owner, "schema_version")? != "tos_public_native_text_create_owner_v1"
        || cmd::text(&owner, "source_root")? != source.store_root.to_string_lossy()
    {
        return Err(SourceCommandError::Denied(
            "PostgreSQL public Text selected owner profile or root",
        ));
    }
    let request_raw = pg::read_request(input)?;
    let request = cmd::parse(&request_raw)?;
    let operation = cmd::text(&request, "operation")?;
    exact_request(&request, operation)?;
    if ![
        "describe",
        "prepare-create",
        "inspect-recovery",
        "native-text.create",
    ]
    .contains(&operation)
    {
        return Err(SourceCommandError::Denied("public Text PG operation"));
    }
    if max_members == 0 || max_total_bytes == 0 || max_member_bytes == 0 {
        return Err(SourceCommandError::Invalid(
            "public Text empty source budget",
        ));
    }
    let (software, components) = pg::software_selection(invocation, deadline, cancelled)?;
    let store = SegmentStore::open_existing(&source.store_root, source.store_limits)
        .map_err(|_| SourceCommandError::Conflict("selected public Text source store"))?;
    let mut db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected public Text PostgreSQL"))?,
    );
    let owner_source_path = cmd::text(&owner, "source_path")?;
    let owner_home = public_text::public_text_package_home(owner_source_path)?;
    let ((prepared, mut result), readset) = db
        .with_source_revision_session(
            &store,
            &source.domain,
            &source.job_id,
            source.job_fence,
            deadline,
            cancelled,
            |session| {
                if session.selection().contract != source.contract {
                    return Err(DurableError::Conflict(
                        "selected public Text source contract differs",
                    ));
                }
                let base_revision = session.selection().cohort.initial_revision();
                if operation == "describe" {
                    let (selection, mut worker, _) =
                        selection_from_session(invocation, session, false, deadline, cancelled)
                            .map_err(DurableError::Source)?;
                    selection
                        .verify_current_from_pg(deadline, cancelled)
                        .map_err(DurableError::Source)?;
                    finish_creation_worker(&mut worker, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                    let response = json!({
                        "status":"ready",
                        "configuration":selection.configuration_digest().to_prefixed(),
                        "source_path":owner_source_path,
                        "operation":"native-text.create",
                        "grants_admission":false,
                        "external_publication_authorized":false
                    });
                    return Ok((None, response));
                }

                if operation == "inspect-recovery" {
                    let Some(retained) = target_files(session, &owner_home, deadline, cancelled)
                        .map_err(DurableError::Source)?
                    else {
                        return Ok((None, json!({"status":"no_retained_control","grants_admission":false})));
                    };
                    let receipt = cmd::parse(&retained["source-create-receipt.json"])
                        .map_err(DurableError::Source)?;
                    if cmd::field(&receipt, "command_id").map_err(DurableError::Source)?
                        != cmd::field(&request, "command_id").map_err(DurableError::Source)?
                    {
                        return Ok((None, json!({"status":"no_retained_control","grants_admission":false})));
                    }
                    let retained_request = cmd::parse(&retained["source-create-request.json"])
                        .map_err(DurableError::Source)?;
                    exact_request(&retained_request, "native-text.create")
                        .map_err(DurableError::Source)?;
                    let mut semantic = semantic_package(
                        invocation,
                        session,
                        &software,
                        &components,
                        &runtime,
                        Some(&owner_home),
                        Some(retained.clone()),
                        max_members,
                        max_total_bytes,
                        max_member_bytes,
                        deadline,
                        cancelled,
                    )
                    .map_err(DurableError::Source)?;
                    let checked = public_text::verify_retained_package(
                        &semantic.selection,
                        &retained_request,
                        &retained,
                        &semantic.assembly,
                        &semantic.dependencies,
                        &software,
                        &components,
                        &mut semantic.worker,
                        deadline,
                        cancelled,
                    )
                    .map_err(DurableError::Source)?;
                    let _ = checked;
                    semantic.selection
                        .verify_current_from_pg(deadline, cancelled)
                        .map_err(DurableError::Source)?;
                    finish_creation_worker(&mut semantic.worker, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                    return Ok((None, json!({
                        "status":"committed",
                        "resume_operation":"native-text.create",
                        "file_count":retained.len(),
                        "grants_admission":false
                    })));
                }

                let retained = if operation == "native-text.create" {
                    target_files(session, &owner_home, deadline, cancelled)
                        .map_err(DurableError::Source)?
                } else {
                    None
                };
                if let Some(files) = &retained {
                    let retained_request = cmd::parse(&files["source-create-request.json"])
                        .map_err(DurableError::Source)?;
                    if cmd::canonical(&retained_request).map_err(DurableError::Source)?
                        != cmd::canonical(&request).map_err(DurableError::Source)?
                    {
                        return Err(DurableError::Conflict(
                            "public Text command id has different retained request",
                        ));
                    }
                }
                let mut semantic = semantic_package(
                    invocation,
                    session,
                    &software,
                    &components,
                    &runtime,
                    retained.as_ref().map(|_| owner_home.as_str()),
                    retained.clone(),
                    max_members,
                    max_total_bytes,
                    max_member_bytes,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                let selection = &semantic.selection;
                let home = semantic.home.clone();
                if operation == "prepare-create" {
                    let response = json!({
                        "configuration":selection.configuration_digest().to_prefixed(),
                        "dependencies":semantic.dependencies,
                        "source":value(&cmd::reference(&semantic.assembly.packet,"packet_id","packet_version").map_err(DurableError::Source)?).map_err(DurableError::Source)?,
                        "expected_source":null,
                        "expected_revision":null,
                        "native_bindings":value(cmd::field(&semantic.assembly.bindings,"bindings").map_err(DurableError::Source)?).map_err(DurableError::Source)?,
                        "grants_admission":false
                    });
                    selection.verify_current_from_pg(deadline, cancelled)
                        .map_err(DurableError::Source)?;
                    finish_creation_worker(&mut semantic.worker, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                    return Ok((None, response));
                }
                if operation != "native-text.create" {
                    return Err(DurableError::Refused("unsupported public Text operation"));
                }
                let (receipt, bindings, package_files, replayed) = if let Some(files) = retained {
                    let (receipt, bindings) = public_text::verify_retained_package(
                        selection,
                        &request,
                        &files,
                        &semantic.assembly,
                        &semantic.dependencies,
                        &software,
                        &components,
                        &mut semantic.worker,
                        deadline,
                        cancelled,
                    )
                    .map_err(DurableError::Source)?;
                    (receipt, bindings, files, true)
                } else {
                    public_text::check_request(&selection, &request, &semantic.dependencies)
                        .map_err(DurableError::Source)?;
                    let package = public_text::complete_package(
                        &selection,
                        &request,
                        &semantic.assembly,
                        &semantic.input_files,
                        semantic.dependencies.clone(),
                        &software,
                        &components,
                        &runtime,
                        &mut semantic.worker,
                        deadline,
                        cancelled,
                    )
                    .map_err(DurableError::Source)?;
                    (package.receipt, package.bindings, package.files, false)
                };
                let delegated = public_text::identities(&selection.config)
                    .map_err(DurableError::Source)?
                    .into_iter()
                    .collect::<BTreeSet<_>>();
                let mut package_ids = BTreeSet::new();
                for (name, raw) in &package_files {
                    if source_text_identity::is_candidate(name) {
                        package_ids.extend(
                            source_text_identity::text_identity_ids_for_member(name, raw)
                                .map_err(DurableError::Source)?,
                        );
                    }
                }
                if package_ids != delegated {
                    return Err(DurableError::Source(SourceCommandError::Conflict(
                        "public Text package identities differ from delegated point checks",
                    )));
                }
                selection.verify_current_from_pg(deadline, cancelled)
                    .map_err(DurableError::Source)?;
                semantic
                    .docs
                    .iter()
                    .try_for_each(|(path, (selected, expected))| {
                        let current = software
                            .read_selected_component(selected, path, expected.len() as u64, deadline, cancelled)
                            .map_err(|_| SourceCommandError::Conflict("public Text selected docs input changed"))?;
                        if &current != expected {
                            return Err(SourceCommandError::Conflict("public Text selected docs input changed"));
                        }
                        Ok(())
                    })
                    .map_err(DurableError::Source)?;
                finish_creation_worker(&mut semantic.worker, deadline, cancelled)
                    .map_err(DurableError::Source)?;
                let response = public_response(&receipt, &bindings, owner_source_path, replayed)
                    .map_err(DurableError::Source)?;
                let (context, source_paths) = context(
                    base_revision,
                    &owner_raw,
                    &request_raw,
                    uid,
                    &semantic,
                    if replayed { Some(&package_files) } else { None },
                    max_members,
                    max_total_bytes,
                    max_member_bytes,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                let changes = if replayed {
                    Vec::new()
                } else {
                    package_files
                        .iter()
                        .map(|(name, raw)| {
                            Ok(SourceChange {
                                path: RelativePath::parse(&format!("{home}/{name}"))
                                    .map_err(|_| SourceCommandError::Invalid("public Text output path"))?,
                                before: None,
                                after: Some(raw.clone()),
                            })
                        })
                        .collect::<SourceCommandResult<Vec<_>>>()
                        .map_err(DurableError::Source)?
                };
                let response_value = cmd::parse(
                    &serde_json::to_vec(&response)
                        .map_err(|_| DurableError::Invalid("public Text response serialization"))?,
                )
                .map_err(DurableError::Source)?;
                let proposal = context
                    .plan("public-project-native-text-create", response_value, changes, replayed)
                    .map_err(DurableError::Source)?;
                let selection = semantic.selection;
                let docs = semantic.docs;
                let implementation = semantic.software_rows;
                let runtime_digest = semantic.runtime_digest;
                let identity_generations = semantic.identity_generations;
                let prepared = PreparedPgPublicText::new(
                    context,
                    proposal,
                    PgRecordRevisionIdentity::from_selection(session.selection()),
                    selection,
                    home,
                    package_files,
                    source_paths,
                    identity_generations,
                    docs,
                    implementation,
                    runtime_digest,
                )
                .map_err(DurableError::Source)?;
                Ok((Some(prepared), response))
            },
        )
        .map_err(pg::source_error)?;

    result["source_read_work"] = json!({
        "scope":"bounded PG/STO ToS reads; selected docs/software capture is separate",
        "exact_source_members_read":readset.work.exact_source_members_read,
        "exact_source_bytes_read":readset.work.exact_source_bytes_read,
        "historical_source_members_read":readset.work.historical_source_members_read,
        "historical_source_bytes_read":readset.work.historical_source_bytes_read,
        "custody_metadata_bytes_read":readset.work.custody_metadata_bytes_read,
        "segment_bytes_read":readset.work.segment_bytes_read,
        "committed_member_rows_examined":readset.work.committed_member_rows_examined,
        "metadata_member_lookups":readset.work.metadata_member_lookups,
        "directory_index_rows_examined":readset.work.directory_index_rows_examined,
        "unique_index_lookups":readset.work.unique_index_lookups
    });
    if let Some(prepared) = prepared {
        let command_id = cmd::text(&request, "command_id")?;
        let prepare_id = prepare_id(&source.domain, command_id);
        let attempt = if prepared.proposal().replayed {
            db.recover_source_revision_attempt(
                &store,
                &source.domain,
                &prepare_id,
                command_id,
                Digest256::of_bytes(&request_raw),
                prepared.proposal().configuration_raw_sha256,
                prepared.proposal().configuration_canonical_sha256,
                prepared.family_tag(),
                prepared.legacy_family_encoding(),
                &source.job_id,
                source.job_fence,
                None,
                deadline,
                cancelled,
            )
            .map_err(pg::source_error)?
        } else {
            db.register_source_revision(
                &store,
                &prepared,
                &readset,
                &prepare_id,
                deadline,
                cancelled,
            )
            .map_err(pg::source_error)?
        };
        let owner_check = || {
            prepared
                .verify_current_owner(
                    &invocation_path,
                    invocation_raw,
                    uid,
                    &owner_path,
                    &owner_raw,
                    &source.store_root,
                    &software,
                    &components,
                    &runtime,
                    executable_digest,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)
        };
        let (receipt, _) = db
            .commit_source_revision(
                &store,
                &attempt,
                source.contract,
                &owner_check,
                deadline,
                cancelled,
            )
            .map_err(pg::source_error)?;
        result["replayed"] = json!(receipt.replayed);
        result["durable_commit"] = pg::commit_result(&receipt);
    }
    Ok(json!({
        "schema_version":"tos_local_native_source_result_v1",
        "authentication":"local-unix-account",
        "result":result,
        "grants_admission":false
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point_owner(path: &str, raw: &[u8]) -> source_text_identity::TextIdentityOwner {
        source_text_identity::TextIdentityOwner {
            path: path.to_owned(),
            member_digest: Digest256::of_bytes(raw),
        }
    }

    #[test]
    fn documented_invocation_and_create_request_match_the_closed_cli_profile() {
        let invocation: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/source-public-text-pg-v1/invocation.example.json"
        ))
        .unwrap();
        validate_invocation(&invocation).unwrap();
        exact(
            &invocation["source_selection"],
            &[
                "domain",
                "source_contract_sha256",
                "store_root",
                "store_limits",
                "job_id",
                "job_fence",
            ],
        )
        .unwrap();
        exact(
            &invocation["source_selection"]["store_limits"],
            &[
                "max_segment_bytes",
                "max_frame_bytes",
                "max_frames",
                "max_journal_bytes",
            ],
        )
        .unwrap();
        exact(
            &invocation["software_selection"],
            &[
                "source_git_commit",
                "source_git_tree",
                "capture_manifest_sha256",
            ],
        )
        .unwrap();
        exact(&invocation["schema_worker"], &["absolute_path", "sha256"]).unwrap();
        exact(
            &invocation["budgets"],
            &[
                "max_members",
                "max_total_bytes",
                "max_member_bytes",
                "max_schema_receipts",
                "max_schema_receipt_bytes",
                "worker_cpu_seconds",
                "worker_address_space_bytes",
            ],
        )
        .unwrap();

        let request: JsonValue = cmd::parse(include_bytes!(
            "../tests/fixtures/source-public-text-pg-v1/create.request.example.json"
        ))
        .unwrap();
        exact_request(&request, "native-text.create").unwrap();
    }

    #[test]
    fn public_pg_invocation_refuses_unknown_fields_and_other_profiles() {
        let raw =
            include_str!("../tests/fixtures/source-public-text-pg-v1/invocation.example.json");
        let mut invocation: Value = serde_json::from_str(raw).unwrap();
        invocation["unexpected"] = json!(true);
        assert!(validate_invocation(&invocation).is_err());

        let mut invocation: Value = serde_json::from_str(raw).unwrap();
        invocation["schema_version"] = json!("tos_local_pg_private_text_invocation_v1");
        assert!(validate_invocation(&invocation).is_err());
    }

    #[test]
    fn public_text_schema_closure_is_operation_specific_and_bounded() {
        assert_eq!(crate::source_public_text_owner::SCHEMAS.len(), 3);
        assert_eq!(PUBLIC_TEXT_CONSTRUCTION_SCHEMAS.len(), 10);
        assert!(
            PUBLIC_TEXT_CONSTRUCTION_SCHEMAS
                .iter()
                .all(|path| path.starts_with("ToS/contracts/") && path.ends_with(".schema.json"))
        );
        assert_eq!(
            PUBLIC_TEXT_CONSTRUCTION_SCHEMAS
                .iter()
                .collect::<BTreeSet<_>>()
                .len(),
            PUBLIC_TEXT_CONSTRUCTION_SCHEMAS.len(),
        );
        assert!(
            PUBLIC_TEXT_CONSTRUCTION_SCHEMAS
                .contains(&"ToS/contracts/provenance-event-v2.schema.json")
        );
    }

    #[test]
    fn public_identity_points_allow_exact_two_path_ancestry_only() {
        let token = "tos.textunit.shared".to_owned();
        let first = point_owner("ToS/source-witnesses/a/source-text-unit.v1.json", b"first");
        let second = point_owner(
            "ToS/source-witnesses/b/source-text-layer.v1.json",
            b"second",
        );
        let requested = BTreeSet::from([token.clone()]);
        let exact_owners = BTreeSet::from([first.clone(), second.clone()]);
        let observations = BTreeMap::from([(
            token.clone(),
            source_text_identity::PgSourceTextIdentityObservation {
                owners: exact_owners.clone(),
                generation: 7,
            },
        )]);
        let expected = BTreeMap::from([(token.clone(), exact_owners.clone())]);
        let ancestry = BTreeMap::from([
            (first.path.clone(), first.member_digest),
            (second.path.clone(), second.member_digest),
        ]);

        let (owners, generations) =
            checked_point_observations(&requested, &observations, &expected, &ancestry).unwrap();
        assert_eq!(owners[&token], exact_owners);
        assert_eq!(generations[&token], 7);
    }

    #[test]
    fn public_identity_points_reject_unrelated_or_changed_member_owner() {
        let token = "tos.textunit.shared".to_owned();
        let retained = point_owner(
            "ToS/source-witnesses/a/source-text-unit.v1.json",
            b"retained",
        );
        let unrelated = point_owner(
            "ToS/source-witnesses/other/source-text-unit.v1.json",
            b"other",
        );
        let requested = BTreeSet::from([token.clone()]);
        let expected = BTreeMap::from([(token.clone(), BTreeSet::from([retained.clone()]))]);
        let ancestry = BTreeMap::from([(retained.path.clone(), retained.member_digest)]);

        for owner in [unrelated, point_owner(&retained.path, b"changed")] {
            let observations = BTreeMap::from([(
                token.clone(),
                source_text_identity::PgSourceTextIdentityObservation {
                    owners: BTreeSet::from([owner]),
                    generation: 8,
                },
            )]);
            assert!(
                checked_point_observations(&requested, &observations, &expected, &ancestry,)
                    .is_err()
            );
        }
    }
}
