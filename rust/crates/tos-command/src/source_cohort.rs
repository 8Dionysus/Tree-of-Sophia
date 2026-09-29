//! Private controlled Agent creation over independently verified immutable bytes.
//! This is mechanics custody, never canonical, semantic, rights or source admission.

use super::*;
use crate::source_command::{self as cmd, CommandContext, SourceFile};
use crate::source_creation::{
    CreationFamily, CreationPackage, ManagedCreationBasis, ManagedCreationInput,
    ManagedCreationObservation, ManagedSerializedCreation, SerializedCreation,
};
use crate::source_creation_store::{CreationFilesystem, CreationOwnerFence};
use crate::{PredicateKind, PredicateRead, PredicateToken, source_claims, source_forms};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
use tos_foundation::{RelativePath, SourceRevision};
use tos_segment_store::{FrameInput, OwnerBinding};
use tos_source_store::{
    CorpusCutReader, MemberMetadata, SoftwareCaptureReader, SoftwareComponentSelectionV1,
    SourceMembershipV1,
};
use tos_validation::item_rules::ItemRefusal;
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

const ORIGINAL: &[u8] = b"tos.source-file.original-v1";
const CREATION: &[u8] = b"tos.source-file.agent-create-v1";
const OWNER: &str = "native-corpus-create:agent";
type IndexRows = BTreeSet<(String, String, String)>;
type ProjectionRows = BTreeMap<String, Vec<u8>>;

/// Exact complete compact inventory basis; not source/semantic admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ManagedAgentInventory {
    pub root: Digest256,
    pub dependencies: String,
}

/// An initial immutable revision names the bootstrap, never later content.
/// Later completeness is maintained only by this controlled atomic writer.
#[derive(Clone)]
pub struct ManagedSourceCohort {
    domain: String,
    store_id: [u8; 16],
    initial_revision: SourceRevision,
    initial_membership: SourceMembershipV1,
    epoch: u64,
    definition: Digest256,
    generation: u64,
}
impl ManagedSourceCohort {
    pub fn domain(&self) -> &str {
        &self.domain
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn initial_revision(&self) -> SourceRevision {
        self.initial_revision
    }
    pub fn initial_membership(&self) -> SourceMembershipV1 {
        self.initial_membership
    }
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
    pub fn definition_digest(&self) -> Digest256 {
        self.definition
    }
}

/// A real current address read. This file's custody revision and the managed
/// generation are distinct from an authored record version or SourceRevision.
pub struct ManagedCurrentMember {
    pub path: RelativePath,
    pub custody_revision: u64,
    pub current_generation: u64,
    pub commit_seq: u64,
    pub raw: Vec<u8>,
    pub metadata: MemberMetadata,
    pub dependency_claims: Option<Vec<RelativePath>>,
    pub placement: tos_segment_store::PlacementV1,
}

/// Verified CURRENT bodies and carrier metadata, retaining the ORIGINAL
/// bootstrap separately; no filesystem or source admission follows.
pub struct ReopenedSourceCohort {
    pub cohort: ManagedSourceCohort,
    pub files: BTreeMap<String, Vec<u8>>,
    pub selected: VerifiedSelectedGeneration,
    pub current_membership: SourceMembershipV1,
    pub indexes: Vec<(String, String, String)>,
    pub metadata: BTreeMap<String, MemberMetadata>,
    pub dependency_claims: BTreeMap<String, Option<Vec<RelativePath>>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SourceFileMetadata {
    pub mode: u32,
    pub dependencies: Option<Vec<String>>,
}
pub(super) type SourceMetadata = BTreeMap<String, SourceFileMetadata>;
fn original_metadata(cut: &CorpusCutReader) -> SourceMetadata {
    cut.current()
        .members()
        .map(|m| {
            (
                m.path.as_str().to_owned(),
                SourceFileMetadata {
                    mode: m.mode,
                    dependencies: cut
                        .current()
                        .indexed_dependencies(&m.path)
                        .map(|d| d.iter().map(|p| p.as_str().to_owned()).collect()),
                },
            )
        })
        .collect()
}
fn creation_metadata(package: CreationPackage<'_>) -> SourceMetadata {
    let dependencies = package
        .reads()
        .iter()
        .filter(|r| r.path.as_str().starts_with("ToS/"))
        .map(|r| r.path.as_str().to_owned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    package
        .changes()
        .iter()
        .map(|c| {
            (
                c.path.as_str().to_owned(),
                SourceFileMetadata {
                    // Existing CreationFilesystem publishes its original serialized files with 0644.
                    mode: 0o644,
                    dependencies: Some(dependencies.clone()),
                },
            )
        })
        .collect()
}
fn row_source_metadata(row: &postgres::Row) -> DurableResult<SourceFileMetadata> {
    let mode = row
        .get::<_, Option<i32>>("source_mode")
        .ok_or(DurableError::Corrupt("source member mode absent"))?;
    let mode = u32::try_from(mode).map_err(|_| DurableError::Corrupt("negative source mode"))?;
    if mode > 0o7777 {
        return Err(DurableError::Corrupt("source mode outside snapshot law"));
    }
    let dependencies = row.get::<_, Option<Vec<String>>>("source_dependencies");
    if let Some(paths) = &dependencies {
        for path in paths {
            RelativePath::parse(path)
                .map_err(|_| DurableError::Corrupt("source dependency path"))?;
        }
        if paths.windows(2).any(|p| p[0] >= p[1]) {
            return Err(DurableError::Corrupt(
                "source dependency claims not sorted unique",
            ));
        }
    }
    Ok(SourceFileMetadata { mode, dependencies })
}
pub(super) fn check_source_metadata(
    row: &postgres::Row,
    metadata: Option<&SourceMetadata>,
    path: &str,
) -> DurableResult<()> {
    match metadata {
        Some(m) if m.get(path) == Some(&row_source_metadata(row)?) => Ok(()),
        None if row.get::<_, Option<i32>>("source_mode").is_none()
            && row
                .get::<_, Option<Vec<String>>>("source_dependencies")
                .is_none() =>
        {
            Ok(())
        }
        _ => Err(DurableError::Conflict(
            "attached source carrier metadata differs",
        )),
    }
}
pub(super) fn hash_source_metadata(h: &mut Digest256Hasher, row: &postgres::Row) {
    if let Some(mode) = row.get::<_, Option<i32>>("source_mode") {
        part(h, b"source-carrier-metadata-v1");
        part(h, &mode.to_be_bytes());
        match row.get::<_, Option<Vec<String>>>("source_dependencies") {
            None => part(h, b"absent"),
            Some(paths) => {
                part(h, b"present");
                for p in paths {
                    part(h, p.as_bytes());
                }
            }
        }
    }
}
fn expose_metadata(
    row: &postgres::Row,
    path: &RelativePath,
) -> DurableResult<(MemberMetadata, Option<Vec<RelativePath>>)> {
    let value = row_source_metadata(row)?;
    let dependencies = value
        .dependencies
        .map(|p| {
            p.iter()
                .map(|v| {
                    RelativePath::parse(v)
                        .map_err(|_| DurableError::Corrupt("source dependency path"))
                })
                .collect::<DurableResult<Vec<_>>>()
        })
        .transpose()?;
    Ok((
        MemberMetadata {
            path: path.clone(),
            sha256: parse_hex(row.get("content_digest"))?,
            size_bytes: as_u64(row.get("content_length"))?,
            mode: value.mode,
        },
        dependencies,
    ))
}

#[derive(Clone, Copy)]
enum SourceRegistrationBasis<'a> {
    Original,
    Current(&'a crate::source_current_cut::ManagedCurrentSourceCut),
    Managed(&'a crate::source_current_cut::ManagedCurrentSourceGeneration),
    CommittedReplay,
}

struct SourceReads {
    reads: Vec<PredicateRead>,
    locators: BTreeMap<String, Digest256>,
    managed_original: Option<serde_json::Value>,
}
pub struct SourceCreationAttempt {
    domain: String,
    prepare_id: Vec<u8>,
    fence: u64,
    epoch: u64,
    definition: Digest256,
    delta: Digest256,
    reads: SourceReads,
    indexes: IndexRows,
    projections: ProjectionRows,
    continuation: Option<WarmContinuation>,
}
struct WarmContinuation {
    parent: SelectedSourceGeneration,
    fence: std::cell::Cell<u64>,
    committed: std::cell::RefCell<Option<WarmCommitted>>,
}
struct WarmCommitted {
    audit_generation: u64,
    commit_seq: u64,
    receipts: Vec<ByteDurabilityReceipt>,
}

pub(super) enum CommitMode<'a> {
    Shadow,
    Bootstrap,
    Creation {
        attempt: &'a SourceCreationAttempt,
        owner: &'a CreationOwnerFence<'a>,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    },
}

fn source_error(error: cmd::SourceCommandError) -> DurableError {
    DurableError::Source(error)
}
fn active(deadline: Instant, cancelled: &AtomicBool) -> DurableResult<()> {
    if cancelled.load(Ordering::Relaxed) || Instant::now() >= deadline {
        Err(DurableError::Refused(
            "source operation deadline or cancellation",
        ))
    } else {
        Ok(())
    }
}
fn finish_worker(
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<()> {
    active(deadline, cancelled)?;
    worker.finish(deadline, cancelled).map_err(|error| {
        source_error(match error {
            ItemRefusal::Deadline => {
                cmd::SourceCommandError::Denied("source schema operation deadline")
            }
            ItemRefusal::Budget | ItemRefusal::BudgetCheck { .. } => {
                cmd::SourceCommandError::Invalid("source schema operation budget")
            }
            ItemRefusal::Source(_) if cancelled.load(Ordering::Relaxed) => {
                cmd::SourceCommandError::Denied("source schema operation cancelled")
            }
            _ => cmd::SourceCommandError::Unsupported(
                "source schema operation finalization incomplete",
            ),
        })
    })?;
    active(deadline, cancelled)
}

fn definition() -> Digest256 {
    static DEFINITION: std::sync::OnceLock<Digest256> = std::sync::OnceLock::new();
    *DEFINITION.get_or_init(|| {
        let mut h = Digest256Hasher::new();
        part(&mut h, b"tos-managed-agent-source-definition-v1");
        for bytes in [
            include_bytes!("source_cohort.rs").as_slice(),
            include_bytes!("source_creation.rs").as_slice(),
            include_bytes!("source_claims.rs").as_slice(),
            include_bytes!("source_forms.rs").as_slice(),
        ] {
            part(&mut h, bytes);
        }
        h.finalize()
    })
}
pub(super) fn known_profile(profile: &[u8]) -> bool {
    [PROFILE_ID, ORIGINAL, CREATION].contains(&profile)
}
pub(super) fn check_source_member(
    profile: &[u8],
    member: &DurableShadowMember,
) -> DurableResult<()> {
    if ![ORIGINAL, CREATION].contains(&profile)
        || !member.subject.starts_with("ToS/")
        || RelativePath::parse(&member.subject).is_err()
        || member.expected_predecessor.is_some()
        || member.proposed_revision != 1
    {
        return Err(DurableError::Invalid(
            "source file profile or initial custody version",
        ));
    }
    // Custody version 1 is explicitly a source-file version. It never assigns
    // or changes an embedded authored record/Claim/form revision.
    Ok(())
}
pub(super) fn check_writer_profile(
    tx: &mut Transaction<'_>,
    domain: &str,
    profile: &[u8],
) -> DurableResult<()> {
    let row = tx.query_one(
        "SELECT source_revision,source_complete FROM cmd2_domain WHERE domain=$1",
        &[&domain],
    )?;
    let managed = row.get::<_, Option<String>>(0).is_some();
    if (profile == PROFILE_ID && managed)
        || (profile == ORIGINAL && (!managed || row.get::<_, bool>(1)))
        || (profile == CREATION && (!managed || !row.get::<_, bool>(1)))
        || !known_profile(profile)
    {
        return Err(DurableError::Refused(
            "writer does not own selected source cohort state",
        ));
    }
    Ok(())
}

fn index_rows(
    ctx: &CommandContext,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<IndexRows> {
    let inventory = source_claims::maintained_inventory(ctx, worker, deadline, cancelled)
        .map_err(source_error)?;
    let mut rows = BTreeMap::<(String, String), String>::new();
    let mut identity_paths = BTreeMap::<String, String>::new();
    let mut insert = |kind: &str, id: &str, path: &str| -> DurableResult<()> {
        if kind != "path" {
            if identity_paths.get(id).is_some_and(|prior| prior != path) {
                return Err(DurableError::Conflict(
                    "stable identity occurs on different source members",
                ));
            }
            identity_paths.insert(id.to_owned(), path.to_owned());
        }
        let key = (kind.to_owned(), id.to_owned());
        if rows.get(&key).is_some_and(|prior| prior != path) {
            return Err(DurableError::Conflict(
                "owner identity occurs on more than one source member",
            ));
        }
        rows.insert(key, path.to_owned());
        Ok(())
    };
    for (id, entry) in &inventory.objects {
        insert(
            "metadata",
            id,
            cmd::text(entry, "source_record_ref").map_err(source_error)?,
        )?;
    }
    for (id, entry) in &inventory.claims {
        insert(
            "claim",
            id,
            cmd::text(entry, "source_claim_file_ref").map_err(source_error)?,
        )?;
    }
    for (kind, entries) in [("event", &inventory.events), ("anchor", &inventory.anchors)] {
        for (id, entry) in entries
            .as_object()
            .ok_or(DurableError::Corrupt("owner evidence index object"))?
        {
            insert(
                kind,
                id.as_str()
                    .ok_or(DurableError::Corrupt("owner evidence identity string"))?,
                cmd::text(entry, "source_ref").map_err(source_error)?,
            )?;
        }
    }
    for file in &ctx.files {
        active(deadline, cancelled)?;
        if !file.path.as_str().starts_with("ToS/") {
            continue;
        }
        insert("path", file.path.as_str(), file.path.as_str())?;
        if file.path.as_str().starts_with("ToS/source-witnesses/")
            && file.path.as_str().ends_with(".human-forms.json")
        {
            let set = cmd::parse(&file.raw).map_err(source_error)?;
            source_forms::apply_form_changes(
                Some(&set),
                cmd::field(&set, "subject").map_err(source_error)?,
                &[],
            )
            .map_err(source_error)?;
            for section in ["forms", "prior_forms"] {
                for form in cmd::array(&set, section).map_err(source_error)? {
                    insert(
                        "form",
                        cmd::text(form, "form_id").map_err(source_error)?,
                        file.path.as_str(),
                    )?;
                }
            }
        }
    }
    Ok(rows
        .into_iter()
        .map(|((kind, id), path)| (kind, id, path))
        .collect())
}

fn verify_manifest_indexes(cut: &CorpusCutReader, rows: &IndexRows) -> DurableResult<()> {
    // Manifest index claims and the complete owner index are separate layers.
    // Every supplied claim must resolve to the exact owner-derived member;
    // owner indexes also cover identities that the manifest does not index.
    for (id, path) in cut.current().indexed_identities() {
        if !rows
            .iter()
            .any(|(kind, token, p)| kind != "path" && token == id && p == path.as_str())
        {
            return Err(DurableError::Refused(
                "manifest identity claim lacks exact maintained owner meaning",
            ));
        }
    }
    Ok(())
}

fn inventory_projections(
    ctx: &CommandContext,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<ProjectionRows> {
    inventory_projections_for(ctx, None, worker, deadline, cancelled)
}
fn inventory_projections_for(
    ctx: &CommandContext,
    paths: Option<&BTreeSet<&str>>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<ProjectionRows> {
    let inventory = source_claims::maintained_agent_inventory_from_managed(
        ctx, true, worker, deadline, cancelled,
    )
    .map_err(source_error)?;
    let grouped = source_claims::agent_inventory_members(ctx, &inventory).map_err(source_error)?;
    let mut projections = ProjectionRows::new();
    let mut projection_bytes = 0usize;
    for file in ctx.files.iter().filter(|file| {
        file.path.as_str().starts_with("ToS/")
            && paths.is_none_or(|paths| paths.contains(file.path.as_str()))
    }) {
        active(deadline, cancelled)?;
        let contribution = source_claims::agent_inventory_contribution(
            ctx, file, &inventory, &grouped, worker, deadline, cancelled,
        )
        .map_err(source_error)?;
        let raw = cmd::canonical(&contribution).map_err(source_error)?;
        // Existing cold metadata-row admission; larger contributions cannot
        // silently enter the addressed route.
        if raw.len() > 1_048_576 {
            return Err(DurableError::Refused(
                "Agent projection exceeds cold metadata bound",
            ));
        }
        projection_bytes = projection_bytes
            .checked_add(raw.len())
            .filter(|bytes| *bytes <= 64 * 1024 * 1024)
            .ok_or(DurableError::Refused(
                "Agent projection exceeds cold metadata bound",
            ))?;
        projections.insert(file.path.as_str().to_owned(), raw);
    }
    Ok(projections)
}

fn optional_agent_projections(
    ctx: &CommandContext,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<Option<ProjectionRows>> {
    match inventory_projections(ctx, worker, deadline, cancelled) {
        Ok(rows) => Ok(Some(rows)),
        Err(DurableError::Source(cmd::SourceCommandError::Unsupported(_)))
        | Err(DurableError::Refused("Agent projection exceeds cold metadata bound")) => Ok(None),
        Err(error) => Err(error),
    }
}

fn projection_root(projections: &ProjectionRows) -> Digest256 {
    let mut hash = Digest256Hasher::new();
    part(&mut hash, b"tos-managed-agent-inventory-coverage-v1");
    for (path, raw) in projections {
        part(&mut hash, path.as_bytes());
        part(&mut hash, Digest256::of_bytes(raw).as_bytes());
    }
    hash.finalize()
}

fn projections_bytes(projections: &ProjectionRows) -> DurableResult<Vec<u8>> {
    serde_json::to_vec(projections)
        .map_err(|_| DurableError::Invalid("source projections encoding"))
}

fn creation_projections(
    package: CreationPackage<'_>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<ProjectionRows> {
    let paths = package
        .changes()
        .iter()
        .map(|change| change.path.as_str())
        .collect::<BTreeSet<_>>();
    let projections = inventory_projections_for(
        &scoped_context(package),
        Some(&paths),
        worker,
        deadline,
        cancelled,
    )?;
    if projections
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != package
            .changes()
            .iter()
            .map(|change| change.path.as_str())
            .collect()
    {
        return Err(DurableError::Corrupt(
            "Agent projection omits serialized delta member",
        ));
    }
    Ok(projections)
}
fn scoped_context(package: CreationPackage<'_>) -> CommandContext {
    let mut ctx = package.prepared().context().clone();
    // The maintained producer already checks initial identity and source_refs
    // against its complete inventory. Derive only this package's new indexed
    // identities from original buffers; do not invent a second claim graph.
    ctx.files
        .retain(|f| !f.path.as_str().starts_with("ToS/source-witnesses/"));
    for (name, raw) in package.prepared().files() {
        ctx.files.push(SourceFile {
            path: RelativePath::parse(&format!("{}/{name}", package.prepared().home().as_str()))
                .unwrap(),
            raw: raw.clone(),
        });
    }
    ctx
}
fn creation_indexes(
    package: CreationPackage<'_>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<IndexRows> {
    let prepared = package.prepared();
    if !matches!(
        prepared.family(),
        CreationFamily::CorpusV1 | CreationFamily::CorpusV2
    ) || package.handler() != "native-corpus-create"
        || package.operation() != "source.create"
    {
        return Err(DurableError::Refused(
            "affected creation is limited to maintained initial native Agent",
        ));
    }
    let request = cmd::parse(&prepared.context().request_raw).map_err(source_error)?;
    let record = cmd::field(&request, "record").map_err(source_error)?;
    if cmd::text(record, "record_type").map_err(source_error)? != "agent"
        || cmd::integer(record, "record_version").map_err(source_error)? != 1
    {
        return Err(DurableError::Refused(
            "affected consumer requires actual initial Agent record",
        ));
    }
    let config = cmd::parse(&prepared.context().configuration_raw).map_err(source_error)?;
    let source = cmd::text(&config, "source_path").map_err(source_error)?;
    let raw = prepared
        .files()
        .get("agent.json")
        .ok_or(DurableError::Corrupt("serialized Agent body absent"))?;
    if source != format!("{}/agent.json", prepared.home().as_str())
        || !cmd::same(record, &cmd::parse(raw).map_err(source_error)?).map_err(source_error)?
    {
        return Err(DurableError::Conflict(
            "serialized Agent differs from actual prepared record",
        ));
    }
    if package.changes().len() != prepared.files().len() {
        return Err(DurableError::Corrupt("source package membership differs"));
    }
    for change in package.changes() {
        let name = change
            .path
            .as_str()
            .strip_prefix(&format!("{}/", prepared.home().as_str()))
            .ok_or(DurableError::Corrupt("creation change escapes source home"))?;
        if change.before.is_some() || change.after.as_ref() != prepared.files().get(name) {
            return Err(DurableError::Corrupt(
                "creation source delta differs from serialized buffers",
            ));
        }
    }
    let mut indexes = index_rows(&scoped_context(package), worker, deadline, cancelled)?;
    indexes.retain(|(_, _, path)| path.starts_with(&format!("{}/", prepared.home().as_str())));
    Ok(indexes)
}

fn predicates(indexes: &IndexRows) -> BTreeSet<(String, String, String)> {
    // Maintained creation hashes entire records/claims/evidence/form inventory.
    // Every owned creation changes that consumed collection, even without links.
    let mut keys = BTreeSet::from([("range".into(), "source-inventory".into(), "all".into())]);
    for (kind, token, path) in indexes {
        keys.insert(("unique".to_owned(), kind.clone(), token.clone()));
        // This is the maintained namespace prefix range. All controlled
        // writers derive every ancestor from their actual old/new paths.
        let mut rest = path.as_str();
        while let Some((parent, _)) = rest.rsplit_once('/') {
            keys.insert((
                "range".to_owned(),
                "source-home".to_owned(),
                parent.to_owned(),
            ));
            rest = parent;
        }
    }
    keys
}
fn indexes_bytes(rows: &IndexRows) -> DurableResult<Vec<u8>> {
    serde_json::to_vec(&rows.iter().collect::<Vec<_>>())
        .map_err(|_| DurableError::Invalid("source indexes encode"))
}
fn reads_bytes(reads: &SourceReads) -> DurableResult<Vec<u8>> {
    let values = reads
        .reads
        .iter()
        .map(|read| match read {
            PredicateRead::Exact {
                namespace,
                key,
                expected_version,
                expected_digest,
            } => serde_json::json!([
                "exact",
                namespace,
                key,
                expected_version,
                expected_digest.map(|d| d.to_hex()),
                reads.locators.get(key).map(|d| d.to_hex())
            ]),
            PredicateRead::Absent { namespace, key } => {
                serde_json::json!(["absent", namespace, key])
            }
            PredicateRead::Generation {
                predicate,
                observed_generation,
            } => serde_json::json!([
                "generation",
                predicate.kind.as_str(),
                predicate.owner,
                predicate.scope,
                predicate.token,
                predicate.definition_version,
                observed_generation
            ]),
        })
        .collect::<Vec<_>>();
    let value = match &reads.managed_original {
        None => serde_json::Value::Array(values),
        Some(original) => {
            serde_json::json!({"profile":"tos.managed-agent-original-reads-v1","reads":values,"original":original})
        }
    };
    let encoded =
        serde_json::to_vec(&value).map_err(|_| DurableError::Invalid("source reads encode"))?;
    if reads.managed_original.is_some() && encoded.len() > 1_048_576 {
        return Err(DurableError::Refused(
            "managed reads cannot fit existing cold metadata row bound",
        ));
    }
    Ok(encoded)
}
fn member_value(member: &MemberMetadata) -> serde_json::Value {
    serde_json::json!([
        member.path.as_str(),
        member.sha256.to_hex(),
        member.size_bytes,
        member.mode
    ])
}
fn software_value(components: &SoftwareComponentSelectionV1) -> serde_json::Value {
    let selected = components.capture();
    serde_json::json!([
        [
            selected.source_git_commit,
            selected.source_git_tree,
            selected.capture_manifest_sha256.to_hex()
        ],
        components.members().map(member_value).collect::<Vec<_>>()
    ])
}
fn managed_original(package: CreationPackage<'_>) -> Option<serde_json::Value> {
    let basis = package.managed_basis()?;
    let context = package.prepared().context();
    let observations = package
        .observations()
        .expect("managed owner package observations");
    let mut original = serde_json::json!({
        "basis":[basis.domain,basis.digest.to_hex(),basis.generation,basis.epoch,basis.definition.to_hex()],
        "context":[context.base_revision.0.to_hex(),context.configuration_raw,context.request_raw,context.recorded_at,context.effective_uid],
        "software":software_value(package.prepared().selected_components()),
        "software_inputs":context.files.iter().filter(|f|!f.path.as_str().starts_with("ToS/")).map(|f|serde_json::json!([f.path.as_str(),Digest256::of_bytes(&f.raw).to_hex(),f.raw.len()])).collect::<Vec<_>>(),
        "observations":observations.values().map(|o|serde_json::json!([member_value(&o.metadata),o.dependencies.as_ref().map(|paths|paths.iter().map(|p|p.as_str()).collect::<Vec<_>>()),o.custody_revision,o.commit_seq])).collect::<Vec<_>>(),
    });
    if let Some(inventory) = package.inventory() {
        original["inventory"] =
            serde_json::json!([inventory.root.to_hex(), inventory.dependencies]);
    }
    Some(original)
}
fn original_digest(value: &serde_json::Value) -> Digest256 {
    Digest256::of_bytes(&serde_json::to_vec(value).expect("private original primitives encode"))
}

// Finite Agent dependency snapshot. SQL is only an ordering/cursor carrier;
// original projection byte values are parsed/emitted by the same FND visitor.
// This performs O(N) compact passes outside the publication/commit fence.
const RETAINED_PROJECTIONS: &str = "SELECT DISTINCT ON (subject) subject,content_digest,CASE WHEN octet_length(inventory_projection)<=1048576 THEN inventory_projection ELSE NULL END AS inventory_projection FROM cmd2_history WHERE domain=$1 AND commit_seq<=$2 ORDER BY subject,revision DESC";

struct AgentDigest {
    hash: Digest256Hasher,
    bytes: usize,
    visits: usize,
    limits: tos_foundation::JsonLimits,
}
impl AgentDigest {
    fn new() -> Self {
        Self {
            hash: Digest256Hasher::new(),
            bytes: 0,
            visits: 0,
            limits: tos_foundation::JsonLimits {
                max_bytes: 8_388_608,
                ..Default::default()
            },
        }
    }
    fn framing(&mut self, bytes: &[u8]) -> DurableResult<()> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .filter(|n| *n <= self.limits.max_bytes)
            .ok_or(DurableError::Refused(
                "Agent dependency canonical byte bound",
            ))?;
        self.hash.update(bytes);
        Ok(())
    }
    fn container(&mut self, byte: u8) -> DurableResult<()> {
        self.visits = self
            .visits
            .checked_add(1)
            .filter(|n| *n <= self.limits.max_visits)
            .ok_or(DurableError::Refused(
                "Agent dependency canonical node bound",
            ))?;
        self.framing(&[byte])
    }
    fn key(&mut self, key: &str) -> DurableResult<()> {
        self.framing(&cmd::canonical(&cmd::string(key)).map_err(source_error)?)?;
        self.framing(b":")
    }
    fn value(&mut self, value: &tos_foundation::JsonValue, depth: usize) -> DurableResult<()> {
        tos_foundation::canonical_feed_digest_v1(
            value,
            tos_foundation::CanonicalProfile::SourceRecordDigestV1,
            self.limits,
            &mut self.hash,
            &mut self.bytes,
            &mut self.visits,
            depth,
        )
        .map_err(|_| DurableError::Refused("Agent dependency canonical structural bound"))
    }
}
fn projection_value(row: &postgres::Row) -> DurableResult<tos_foundation::JsonValue> {
    let raw = row
        .get::<_, Option<Vec<u8>>>("inventory_projection")
        .ok_or(DurableError::Refused(
            "managed inventory projection incomplete; FullOnly required",
        ))?;
    if raw.len() > 1_048_576 {
        return Err(DurableError::Refused(
            "Agent projection exceeds cold metadata bound",
        ));
    }
    let value = cmd::parse(&raw).map_err(source_error)?;
    cmd::exact_keys(
        &value,
        &[
            "schema_version",
            "path",
            "raw_sha256",
            "records",
            "source_profiles",
            "events",
            "anchors",
            "form",
        ],
    )
    .map_err(source_error)?;
    if cmd::text(&value, "schema_version").map_err(source_error)?
        != "tos_managed_agent_inventory_member_v1"
        || cmd::text(&value, "path").map_err(source_error)? != row.get::<_, String>("subject")
        || cmd::text(&value, "raw_sha256").map_err(source_error)?
            != row.get::<_, String>("content_digest")
        || cmd::canonical(&value).map_err(source_error)? != raw
    {
        return Err(DurableError::Corrupt(
            "Agent projection raw/classification binding differs",
        ));
    }
    Ok(value)
}
fn retained_projection_root(
    tx: &mut Transaction<'_>,
    domain: &str,
    generation: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<(Digest256, tos_foundation::JsonValue, u64)> {
    active(deadline, cancelled)?;
    let admission = format!(
        "WITH members AS ({RETAINED_PROJECTIONS}) SELECT count(*),coalesce(sum(octet_length(inventory_projection)),0),count(*) FILTER(WHERE inventory_projection IS NULL) FROM members"
    );
    let admitted = tx.query_one(&admission, &[&domain, &as_i64(generation)?])?;
    if as_u64(admitted.get(0))? > MAX_CUT
        || as_u64(admitted.get(1))? > 64 * 1024 * 1024
        || admitted.get::<_, i64>(2) != 0
    {
        return Err(DurableError::Refused(
            "Agent projection exceeds existing cold metadata envelope or is incomplete; FullOnly required",
        ));
    }
    let mut root = Digest256Hasher::new();
    part(&mut root, b"tos-managed-agent-inventory-coverage-v1");
    let mut profiles = cmd::object(vec![]);
    let mut cursor = String::new();
    let mut count = 0u64;
    active(deadline, cancelled)?;
    let sql = format!(
        "WITH members AS ({RETAINED_PROJECTIONS}) SELECT * FROM members ORDER BY subject COLLATE \"C\""
    );
    let sequence = as_i64(generation)?;
    let mut rows = tx.query_raw(
        &sql,
        [&domain as &(dyn postgres::types::ToSql + Sync), &sequence],
    )?;
    while let Some(row) = rows.next()? {
        active(deadline, cancelled)?;
        let value = projection_value(&row)?;
        let path: String = row.get("subject");
        if path <= cursor || !path.starts_with("ToS/") {
            return Err(DurableError::Corrupt(
                "Agent projection path coverage/order differs",
            ));
        }
        let raw: Vec<u8> = row
            .get::<_, Option<Vec<u8>>>("inventory_projection")
            .unwrap();
        part(&mut root, path.as_bytes());
        part(&mut root, Digest256::of_bytes(&raw).as_bytes());
        for (key, digest) in cmd::field(&value, "source_profiles")
            .map_err(source_error)?
            .as_object()
            .ok_or(DurableError::Corrupt("Agent source profile map"))?
        {
            let key = key
                .as_str()
                .ok_or(DurableError::Corrupt("Agent profile locator"))?;
            if profiles
                .object_get(key)
                .is_some_and(|prior| prior != digest)
            {
                return Err(DurableError::Corrupt(
                    "Agent source profile contributions disagree",
                ));
            }
            cmd::set(&mut profiles, key, digest.clone()).map_err(source_error)?;
        }
        // EVERY member is represented, including non-catalogue paths.
        // Non-Agent/native/Claim scopes cannot obtain these contributions.
        count = count
            .checked_add(1)
            .ok_or(DurableError::Corrupt("Agent projection coverage overflow"))?;
        cursor = path;
    }
    Ok((root.finalize(), profiles, count))
}

fn visit_projection_section(
    tx: &mut Transaction<'_>,
    domain: &str,
    generation: u64,
    section: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
    mut visit: impl FnMut(&str, &tos_foundation::JsonValue) -> DurableResult<()>,
) -> DurableResult<()> {
    // The already verified owner index supplies only stable string ordering
    // keys. Its controlled writer is initial-only: later identities always
    // have new paths, so the retained member join excludes future additions.
    // No PostgreSQL JSON codec touches authored values or number spelling.
    let expansion = match section {
        "records" => {
            "SELECT m.*,i.token AS entry_key FROM members m JOIN cmd2_source_index i ON i.domain=$1 AND i.path=m.subject AND i.kind='metadata' AND i.definition_digest=$3"
        }
        "events" => {
            "SELECT m.*,i.token AS entry_key FROM members m JOIN cmd2_source_index i ON i.domain=$1 AND i.path=m.subject AND i.kind='event' AND i.definition_digest=$3"
        }
        "anchors" => {
            "SELECT m.*,i.token AS entry_key FROM members m JOIN cmd2_source_index i ON i.domain=$1 AND i.path=m.subject AND i.kind='anchor' AND i.definition_digest=$3"
        }
        "forms" => {
            "SELECT m.*,m.subject AS entry_key FROM members m WHERE right(m.subject,length('.human-forms.json'))='.human-forms.json' AND EXISTS(SELECT 1 FROM members p JOIN cmd2_source_index i ON i.domain=$1 AND i.path=p.subject AND i.kind='metadata' AND i.definition_digest=$3 WHERE p.subject=left(m.subject,length(m.subject)-length('.human-forms.json'))||'.json')"
        }
        _ => return Err(DurableError::Invalid("Agent dependency section")),
    };
    let mut cursor = String::new();
    active(deadline, cancelled)?;
    let sql = format!(
        "WITH members AS ({RETAINED_PROJECTIONS}), entries AS ({expansion}) SELECT * FROM entries ORDER BY entry_key COLLATE \"C\""
    );
    let sequence = as_i64(generation)?;
    let definition = definition().to_hex();
    // Keep fixed positional SQL binding; ordering no longer repeats the full
    // retained scan for each page. RowIter releases each bounded row at once.
    let mut rows = tx.query_raw(
        &sql,
        [
            &domain as &(dyn postgres::types::ToSql + Sync),
            &sequence,
            &definition,
        ],
    )?;
    while let Some(row) = rows.next()? {
        active(deadline, cancelled)?;
        let key: String = row.get("entry_key");
        if key <= cursor {
            return Err(DurableError::Corrupt(
                "Agent dependency identity ordering/uniqueness differs",
            ));
        }
        let value = projection_value(&row)?;
        let entry = match section {
            "records" => cmd::array(
                cmd::field(&value, "records").map_err(source_error)?,
                "agent",
            )
            .map_err(source_error)?
            .iter()
            .find(|entry| cmd::text(entry, "record_id").ok() == Some(key.as_str()))
            .ok_or(DurableError::Corrupt("Agent catalogue entry absent"))?,
            "forms" => cmd::field(
                cmd::field(&value, "form").map_err(source_error)?,
                "raw_sha256",
            )
            .map_err(source_error)?,
            _ => cmd::field(cmd::field(&value, section).map_err(source_error)?, &key)
                .map_err(source_error)?,
        };
        visit(&key, entry)?;
        cursor = key;
    }
    Ok(())
}
fn stream_projection_section(
    tx: &mut Transaction<'_>,
    domain: &str,
    generation: u64,
    section: &str,
    output: &mut AgentDigest,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<()> {
    let mut first = true;
    visit_projection_section(
        tx,
        domain,
        generation,
        section,
        deadline,
        cancelled,
        |key, entry| {
            if !first {
                output.framing(b",")?;
            }
            first = false;
            if section != "records" {
                output.key(key)?;
            }
            output.value(entry, if section == "records" { 3 } else { 2 })
        },
    )
}

fn retained_agent_inventory(
    tx: &mut Transaction<'_>,
    domain: &str,
    generation: u64,
    ctx: &CommandContext,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<(ManagedAgentInventory, u64)> {
    let (root, profiles, count) =
        retained_projection_root(tx, domain, generation, deadline, cancelled)?;
    // Resource digests from retained owner contributions must name the exact
    // independently selected schema bytes read by this operation.
    for (path, sha) in profiles
        .as_object()
        .ok_or(DurableError::Corrupt("Agent profile contribution object"))?
    {
        let path = RelativePath::parse(
            path.as_str()
                .ok_or(DurableError::Corrupt("Agent profile path"))?,
        )
        .map_err(|_| DurableError::Corrupt("Agent profile path"))?;
        let raw = ctx
            .file(&path)
            .map_err(source_error)?
            .ok_or(DurableError::Conflict(
                "Agent profile resource not selected",
            ))?;
        if sha.as_str() != Some(Digest256::of_bytes(raw).to_hex().as_str()) {
            return Err(DurableError::Conflict(
                "Agent profile resource digest differs",
            ));
        }
    }
    let template =
        crate::source_creation::agent_dependency_template(ctx, profiles).map_err(source_error)?;
    let mut fields = template
        .as_object()
        .ok_or(DurableError::Corrupt("Agent dependency template"))?
        .iter()
        .collect::<Vec<_>>();
    fields.sort_by(|(a, _), (b, _)| a.as_str().cmp(&b.as_str()));
    let mut output = AgentDigest::new();
    output.container(b'{')?;
    for (index, (key, value)) in fields.into_iter().enumerate() {
        if index > 0 {
            output.framing(b",")?;
        }
        let key = key
            .as_str()
            .ok_or(DurableError::Corrupt("Agent dependency key"))?;
        output.key(key)?;
        match key {
            "records" => {
                output.container(b'{')?;
                let mut kinds = source_claims::NATIVE_CATALOG_KINDS
                    .iter()
                    .copied()
                    .collect::<Vec<_>>();
                kinds.sort_unstable();
                for (i, kind) in kinds.into_iter().enumerate() {
                    if i > 0 {
                        output.framing(b",")?;
                    }
                    output.key(kind)?;
                    output.container(b'[')?;
                    if kind == "agent" {
                        stream_projection_section(
                            tx,
                            domain,
                            generation,
                            "records",
                            &mut output,
                            deadline,
                            cancelled,
                        )?;
                    }
                    output.framing(b"]")?;
                }
                output.framing(b"}")?;
            }
            "events" | "anchors" | "forms" => {
                output.container(b'{')?;
                stream_projection_section(
                    tx,
                    domain,
                    generation,
                    key,
                    &mut output,
                    deadline,
                    cancelled,
                )?;
                output.framing(b"}")?;
            }
            _ => output.value(value, 1)?,
        }
    }
    output.framing(b"}")?;
    Ok((
        ManagedAgentInventory {
            root,
            dependencies: output.hash.finalize().to_prefixed(),
        },
        count,
    ))
}

fn source_delta(package: CreationPackage<'_>) -> Digest256 {
    let mut h = Digest256Hasher::new();
    if let Some(revision) = package.v1_revision() {
        part(&mut h, revision.0.as_bytes());
    } else {
        let basis = package
            .managed_basis()
            .expect("managed concrete package owns its basis");
        part(&mut h, b"tos-managed-source-generation-basis-v1");
        part(&mut h, basis.domain.as_bytes());
        part(&mut h, basis.digest.as_bytes());
        part(&mut h, &basis.generation.to_be_bytes());
        part(&mut h, &basis.epoch.to_be_bytes());
        part(&mut h, basis.definition.as_bytes());
        part(
            &mut h,
            package.prepared().context().base_revision.0.as_bytes(),
        );
    }
    if let Some(observations) = package.observations() {
        for (path, observed) in observations {
            part(&mut h, path.as_bytes());
            part(&mut h, &observed.metadata.mode.to_be_bytes());
            part(&mut h, &observed.metadata.size_bytes.to_be_bytes());
            part(&mut h, observed.metadata.sha256.as_bytes());
            part(&mut h, &observed.custody_revision.to_be_bytes());
            part(&mut h, &observed.commit_seq.to_be_bytes());
            match &observed.dependencies {
                None => part(&mut h, b"none"),
                Some(paths) => {
                    part(&mut h, b"some");
                    for path in paths {
                        part(&mut h, path.as_str().as_bytes());
                    }
                }
            }
        }
    }
    part(&mut h, b"tos-managed-agent-create-delta-v1");
    part(&mut h, package.configuration_digest().as_bytes());
    part(&mut h, package.prepared().dependencies().as_bytes());
    for change in package.changes() {
        part(&mut h, change.path.as_str().as_bytes());
        part(
            &mut h,
            Digest256::of_bytes(change.after.as_ref().unwrap()).as_bytes(),
        );
        part(
            &mut h,
            &(change.after.as_ref().unwrap().len() as u64).to_be_bytes(),
        );
    }
    for (path, metadata) in creation_metadata(package) {
        part(&mut h, path.as_bytes());
        part(&mut h, &metadata.mode.to_be_bytes());
        for dependency in metadata.dependencies.unwrap() {
            part(&mut h, dependency.as_bytes());
        }
    }
    if let Some(original) = managed_original(package) {
        part(&mut h, b"tos-managed-agent-original-companion-v1");
        part(&mut h, original_digest(&original).as_bytes());
    }
    h.finalize()
}

// The managed owner records its entire derived read closure in the durable delta.
// Legacy v1 retains its original byte encoding and delta law.
fn registered_source_delta(
    package: CreationPackage<'_>,
    reads: &SourceReads,
    projections: &ProjectionRows,
) -> DurableResult<Digest256> {
    if reads.managed_original != managed_original(package) {
        return Err(DurableError::Conflict(
            "registered original managed input differs",
        ));
    }
    let delta = source_delta(package);
    if package.managed_basis().is_none() {
        return Ok(delta);
    }
    let mut h = Digest256Hasher::new();
    part(&mut h, b"tos-managed-agent-registered-read-closure-v1");
    part(&mut h, delta.as_bytes());
    part(&mut h, Digest256::of_bytes(&reads_bytes(reads)?).as_bytes());
    part(
        &mut h,
        Digest256::of_bytes(&projections_bytes(projections)?).as_bytes(),
    );
    Ok(h.finalize())
}

fn cohort_matches(
    row: &postgres::Row,
    cohort: &ManagedSourceCohort,
    require_complete: bool,
) -> DurableResult<()> {
    if row.get::<_, Option<String>>("source_revision") != Some(cohort.initial_revision.0.to_hex())
        || row.get::<_, Option<String>>("source_membership_digest")
            != Some(cohort.initial_membership.digest.to_hex())
        || row.get::<_, Option<i64>>("source_membership_count")
            != Some(as_i64(cohort.initial_membership.count)?)
        || row.get::<_, Option<i64>>("source_epoch") != Some(as_i64(cohort.epoch)?)
        || row.get::<_, Option<String>>("source_definition_digest")
            != Some(cohort.definition.to_hex())
        || cohort.definition != definition()
        || (require_complete && !row.get::<_, bool>("source_complete"))
    {
        return Err(DurableError::Conflict(
            "selected source cohort completeness/epoch/definition changed",
        ));
    }
    Ok(())
}

// A tentative metadata identity only. Full projection/member coverage and
// catalogue EOF are still verified by the actual visitor before issuance.
fn managed_model_projection_identity(
    tx: &mut Transaction<'_>,
    generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
) -> DurableResult<Digest256> {
    let domain = generation.cohort().domain();
    let row = tx.query_one("SELECT d.*,f.generation AS audit_generation,f.maintenance_state FROM cmd2_domain d JOIN cmd2_audit_fence f USING(domain) WHERE d.domain=$1", &[&domain])?;
    cohort_matches(&row, generation.cohort(), true)?;
    if as_u64(row.get("audit_generation"))? != generation.audit_generation()
        || row.get::<_, String>("maintenance_state") != "normal"
        || !row.get::<_, bool>("rights_allowed")
        || as_u64(row.get("head_seq"))? != generation.commit_seq()
        || row.get::<_, Option<i64>>("source_generation") != Some(as_i64(generation.commit_seq())?)
        || row.get::<_, Option<String>>("selected_generation_digest")
            != Some(generation.digest().to_hex())
    {
        return Err(DurableError::Conflict(
            "managed model catalogue source changed",
        ));
    }
    let stored = row
        .get::<_, Option<String>>("source_projection_digest")
        .ok_or(DurableError::Refused(
            "managed model projection identity unavailable",
        ))?;
    Digest256::from_hex(&stored)
        .map_err(|_| DurableError::Corrupt("managed model projection identity invalid"))
}

fn finite_warm_parent_rows(
    parent: &MembershipInstallation<'_>,
    namespace: GenerationNamespaceV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<Vec<PlacementGenerationRowV1>> {
    let count = parent.count(namespace);
    if count > MAX_CUT {
        return Err(DurableError::Refused(
            "warm parent exceeds finite membership bound",
        ));
    }
    let mut cursor = parent.cursor(namespace)?;
    let mut rows = Vec::new();
    let mut key_bytes = 0usize;
    while let Some(row) = cursor.next_row()? {
        active(deadline, cancelled)?;
        key_bytes = key_bytes
            .checked_add(row.key.len())
            .filter(|bytes| *bytes <= MAX_TOTAL_MEMBERSHIP_KEY_BYTES)
            .ok_or(DurableError::Refused("warm parent key byte bound exceeded"))?;
        if rows.len() as u64 >= count
            || rows
                .last()
                .is_some_and(|previous: &PlacementGenerationRowV1| previous.key >= row.key)
        {
            return Err(DurableError::Corrupt("warm parent order/count differs"));
        }
        rows.push(row);
    }
    cursor.finish()?;
    let (tag, expected_root) = match namespace {
        GenerationNamespaceV1::History => (
            HISTORY_KEY_TAG,
            parent.descriptor_cut.history_membership_root,
        ),
        GenerationNamespaceV1::Current => (
            CURRENT_KEY_TAG,
            parent.descriptor_cut.current_membership_root,
        ),
    };
    if rows.len() as u64 != count || logical_membership_root(tag, &rows) != expected_root {
        return Err(DurableError::Corrupt("warm parent EOF/root differs"));
    }
    Ok(rows)
}

fn held_generation_metadata(
    tx: &mut Transaction<'_>,
    store: &SegmentStore,
    generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
) -> DurableResult<()> {
    let selection = generation.selected().view();
    selection.audited_root.require_store(store)?;
    let domain = generation.cohort().domain();
    let fence = tx.query_one(
        "SELECT generation,maintenance_state FROM cmd2_audit_fence WHERE domain=$1 FOR SHARE",
        &[&domain],
    )?;
    let row = tx.query_one(
        "SELECT * FROM cmd2_domain WHERE domain=$1 FOR SHARE",
        &[&domain],
    )?;
    cohort_matches(&row, generation.cohort(), true)?;
    if as_u64(fence.get(0))? != generation.audit_generation()
        || fence.get::<_, String>(1) != "normal"
        || as_u64(row.get("head_seq"))? != generation.commit_seq()
        || row.get::<_, Option<i64>>("source_generation") != Some(as_i64(generation.commit_seq())?)
        || row.get::<_, Option<String>>("selected_generation_digest")
            != Some(generation.digest().to_hex())
        || row.get::<_, Option<String>>("schema_profile_digest")
            != Some(selection.descriptor_cut.schema_profile_digest.to_hex())
        || database_oid(tx)? != selection.descriptor_cut.database_oid
        || !row.get::<_, bool>("rights_allowed")
    {
        return Err(DurableError::Conflict(
            "selected metadata generation/policy changed",
        ));
    }
    Ok(())
}

fn selected_source_metadata(
    row: &postgres::Row,
    selected: &PlacementGenerationRowV1,
    domain: &str,
    path: &RelativePath,
) -> DurableResult<MemberMetadata> {
    let placement = selected.placement;
    let coordinate = placement.coordinate();
    if selected.key != membership_key(CURRENT_KEY_TAG, domain, path.as_str(), None)?
        || selected.logical_digest != coordinate.sha256
        || selected.logical_length != coordinate.size_bytes
        || row.get::<_, String>("domain") != domain
        || row.get::<_, String>("subject") != path.as_str()
        || row.get::<_, String>("content_digest") != selected.logical_digest.to_hex()
        || as_u64(row.get("content_length"))? != selected.logical_length
        || row.get::<_, Vec<u8>>("store_id") != placement.store_id()
        || row.get::<_, String>("custody_domain_digest") != placement.domain_digest().to_hex()
        || row.get::<_, Vec<u8>>("custody_domain") != domain.as_bytes()
        || row.get::<_, Vec<u8>>("pin_id") != placement.pin_id()
        || as_u64(row.get("pin_fence"))? != placement.fence_epoch()
        || row.get::<_, String>("sto_receipt_id") != placement.receipt_id().to_hex()
        || row.get::<_, String>("segment_digest") != placement.segment_digest().to_hex()
        || as_u64(row.get("segment_size"))? != placement.segment_size()
        || row.get::<_, i32>("frame_index") != placement.frame_index() as i32
        || as_u64(row.get("frame_header_offset"))? != coordinate.header_offset
        || row.get::<_, String>("frame_digest") != coordinate.sha256.to_hex()
        || as_u64(row.get("frame_length"))? != coordinate.size_bytes
        || ![
            "tos.source-file.original-v1",
            "tos.source-file.agent-create-v1",
        ]
        .contains(&row.get::<_, String>("profile_id").as_str())
    {
        return Err(DurableError::Corrupt("selected metadata placement differs"));
    }
    expose_metadata(row, path).map(|(metadata, _)| metadata)
}

fn path_from_current_key(domain: &str, key: &[u8]) -> DurableResult<RelativePath> {
    let start = CURRENT_KEY_TAG.len() + 4 + domain.len();
    let length_bytes: [u8; 4] = key
        .get(start..start + 4)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(DurableError::Corrupt("selected source key length absent"))?;
    let length = u32::from_be_bytes(length_bytes) as usize;
    let name = key
        .get(start + 4..)
        .filter(|name| name.len() == length)
        .and_then(|name| std::str::from_utf8(name).ok())
        .ok_or(DurableError::Corrupt("selected source key path absent"))?;
    let path = RelativePath::parse(name)
        .map_err(|_| DurableError::Corrupt("selected source key path invalid"))?;
    if key != membership_key(CURRENT_KEY_TAG, domain, name, None)? {
        return Err(DurableError::Corrupt("selected source key domain differs"));
    }
    Ok(path)
}

impl DurablePgCoordinator {
    pub(crate) fn managed_model_delta_from_commit(
        &mut self,
        store: &SegmentStore,
        parent: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        current: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        prepare_id: &[u8],
        package: &ManagedSerializedCreation,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<DurableCommitReceipt> {
        active(deadline, cancelled)?;
        parent.selected().view().audited_root.require_store(store)?;
        current
            .selected()
            .view()
            .audited_root
            .require_store(store)?;
        let view = CreationPackage::Managed(package);
        if package.basis != ManagedCreationBasis::from_generation(parent)
            || parent.commit_seq().checked_add(1) != Some(current.commit_seq())
            || parent.cohort().domain() != current.cohort().domain()
            || parent.epoch() != current.epoch()
            || parent.agent_definition_digest() != current.agent_definition_digest()
        {
            return Err(DurableError::Conflict(
                "managed model parent/commit basis differs",
            ));
        }
        let domain = current.cohort().domain();
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ; SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        held_generation_metadata(&mut tx, store, current)?;
        let size = tx.query_one("SELECT octet_length(source_reads),octet_length(source_projections) FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2 AND state='committed'", &[&domain, &prepare_id])?;
        for column in 0..2 {
            if size
                .get::<_, Option<i32>>(column)
                .is_none_or(|n| n <= 0 || n > 1_048_576)
            {
                return Err(DurableError::Refused(
                    "managed model original attempt exceeds cold row envelope",
                ));
            }
        }
        let attempt = tx.query_one(
            "SELECT * FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2 AND state='committed'",
            &[&domain, &prepare_id],
        )?;
        let reads = decode_reads(&attempt.get::<_, Option<Vec<u8>>>("source_reads").unwrap())?;
        let projection_raw: Vec<u8> = attempt
            .get::<_, Option<Vec<u8>>>("source_projections")
            .unwrap();
        cmd::parse(&projection_raw).map_err(source_error)?;
        let projections: ProjectionRows = serde_json::from_slice(&projection_raw)
            .map_err(|_| DurableError::Corrupt("managed model registered projections"))?;
        if projections_bytes(&projections)? != projection_raw
            || projections
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
                != view
                    .changes()
                    .iter()
                    .map(|change| change.path.as_str())
                    .collect()
        {
            return Err(DurableError::Conflict(
                "managed model projection delta coverage differs",
            ));
        }
        let receipt = receipt_from_committed_attempt(&mut tx, &attempt)?;
        let request = cmd::parse(&view.prepared().context().request_raw).map_err(source_error)?;
        if receipt.commit_seq != current.commit_seq()
            || receipt.delta_digest != registered_source_delta(view, &reads, &projections)?
            || receipt.command_id != cmd::text(&request, "command_id").map_err(source_error)?
            || receipt.raw_request_digest
                != Digest256::of_bytes(&view.prepared().context().request_raw)
            || attempt.get::<_, Option<i64>>("source_epoch") != Some(as_i64(current.epoch())?)
        {
            return Err(DurableError::Conflict(
                "managed model actual committed package differs",
            ));
        }
        let members = tx.query("SELECT m.member_slot,m.subject,m.proposed_revision,c.* FROM cmd2_member m JOIN cmd2_current c ON c.domain=m.domain AND c.subject=m.subject WHERE m.domain=$1 AND m.prepare_id=$2 ORDER BY m.member_slot", &[&domain, &prepare_id])?;
        if members.len() != view.changes().len() {
            return Err(DurableError::Conflict(
                "managed model committed members differ",
            ));
        }
        let expected_metadata = creation_metadata(view);
        for (slot, (member, change)) in members.iter().zip(view.changes()).enumerate() {
            active(deadline, cancelled)?;
            let raw = change.after.as_ref().ok_or(DurableError::Refused(
                "managed model supports initial Agent only",
            ))?;
            let selected = current
                .selected()
                .lookup_current(domain, &change.path, deadline, cancelled)?
                .ok_or(DurableError::Conflict(
                    "managed model changed member absent",
                ))?;
            let metadata = selected_source_metadata(member, &selected, domain, &change.path)?;
            if member.get::<_, i32>("member_slot") as usize != slot
                || member.get::<_, String>("subject") != change.path.as_str()
                || member.get::<_, i64>("proposed_revision") != 1
                || member.get::<_, i64>("revision") != 1
                || as_u64(member.get("commit_seq"))? != current.commit_seq()
                || member.get::<_, Vec<u8>>("prepare_id") != prepare_id
                || metadata.sha256 != Digest256::of_bytes(raw)
                || metadata.size_bytes != raw.len() as u64
                || row_source_metadata(member)? != expected_metadata[change.path.as_str()]
                || projection_value(member)?.object_get("raw_sha256")
                    != Some(&cmd::string(&metadata.sha256.to_hex()))
            {
                return Err(DurableError::Conflict(
                    "managed model changed body/custody differs",
                ));
            }
        }
        tx.commit()?;
        active(deadline, cancelled)?;
        Ok(receipt)
    }

    pub(crate) fn managed_model_generation_description(
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        projection: Digest256,
    ) -> tos_compiler::managed_source::ManagedSourceGenerationV1 {
        let installation = generation.selected().view();
        let cut = installation.descriptor_cut;
        let cohort = generation.cohort();
        tos_compiler::managed_source::ManagedSourceGenerationV1 {
            domain: cohort.domain().to_owned(),
            store_id: cut.store_id,
            installed_generation_sha256: generation.digest().to_hex(),
            through_commit_seq: generation.commit_seq(),
            selected_audit_generation: generation.audit_generation(),
            epoch: cohort.epoch(),
            definition_sha256: cohort.definition_digest().to_hex(),
            bootstrap_source_revision: cohort.initial_revision().0.to_hex(),
            bootstrap_membership_sha256: cohort.initial_membership().digest.to_hex(),
            bootstrap_members: cohort.initial_membership().count,
            domain_sha256: cut.domain_digest.to_hex(),
            database_oid: cut.database_oid,
            schema_profile_sha256: cut.schema_profile_digest.to_hex(),
            state_sha256: cut.state_digest.to_hex(),
            log_sha256: cut.log_digest.to_hex(),
            current_membership_sha256: cut.current_membership_root.to_hex(),
            current_members: cut.current_members,
            history_membership_sha256: cut.history_membership_root.to_hex(),
            history_members: cut.historical_members,
            inventory_projection_sha256: projection.to_hex(),
        }
    }

    /// Only an optimistic identity for producer setup, never coverage or a
    /// selected read grant. The actual single full visitor must prove it.
    pub(crate) fn managed_model_projection_identity(
        &mut self,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Digest256> {
        active(deadline, cancelled)?;
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY; SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let identity = managed_model_projection_identity(&mut tx, generation)?;
        tx.commit()?;
        active(deadline, cancelled)?;
        Ok(identity)
    }

    // The full selected producer consumes these actual retained catalogue
    // contributions, not a second authored-body inventory. This optimistic
    // read phase holds no publication locks; selection must recheck currentness.
    pub(crate) fn visit_managed_model_catalogue(
        &mut self,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        deadline: Instant,
        cancelled: &AtomicBool,
        mut visit: impl FnMut(&tos_foundation::JsonValue) -> DurableResult<()>,
    ) -> DurableResult<Digest256> {
        active(deadline, cancelled)?;
        let domain = generation.cohort().domain();
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY; SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let expected_projection = managed_model_projection_identity(&mut tx, generation)?;
        let (root, _, count) = retained_projection_root(
            &mut tx,
            domain,
            generation.commit_seq(),
            deadline,
            cancelled,
        )?;
        if count != generation.member_count() || root != expected_projection {
            return Err(DurableError::Refused(
                "managed model catalogue coverage unproved; FullOnly required",
            ));
        }
        visit_projection_section(
            &mut tx,
            domain,
            generation.commit_seq(),
            "records",
            deadline,
            cancelled,
            |_, entry| visit(entry),
        )?;
        tx.commit()?;
        active(deadline, cancelled)?;
        Ok(root)
    }

    // Native selected-model disclosure uses the same real source/owner fences
    // as creation. A serialized model companion cannot issue this boundary.
    // Keep both owner and database locks across the actual bounded model seek.
    pub(crate) fn with_current_managed_model<T>(
        &mut self,
        store: &SegmentStore,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        proof: &tos_compiler::managed_source::ManagedSourceProofV1,
        filesystem: &CreationFilesystem,
        package: CreationPackage<'_>,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
        seek: impl for<'seek> FnOnce(
            &crate::source_managed_selection::ManagedCurrentModelLease<'seek>,
        ) -> T,
    ) -> DurableResult<T> {
        active(deadline, cancelled)?;
        let projection = Digest256::from_hex(&proof.generation.inventory_projection_sha256)
            .map_err(|_| DurableError::Invalid("managed model projection digest"))?;
        if proof.generation != Self::managed_model_generation_description(generation, projection) {
            return Err(DurableError::Conflict(
                "managed model retained generation differs",
            ));
        }
        let installation = generation.selected().view();
        installation.audited_root.require_store(store)?;
        if store.store_id() != installation.descriptor_cut.store_id
            || store.domain_digest() != installation.descriptor_cut.domain_digest
            || installation.domain != generation.cohort().domain()
        {
            return Err(DurableError::Conflict("managed model source store differs"));
        }
        let owner = filesystem
            .hold_creation_owner(package, deadline, cancelled)
            .map_err(source_error)?;
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let domain = generation.cohort().domain();
        let audit = tx.query_one(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        if as_u64(audit.get(0))? != generation.audit_generation()
            || audit.get::<_, String>(1) != "normal"
        {
            return Err(DurableError::Conflict("managed model source audit changed"));
        }
        let row = tx.query_one(
            "SELECT * FROM cmd2_domain WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        cohort_matches(&row, generation.cohort(), true)?;
        if as_u64(row.get("head_seq"))? != generation.commit_seq()
            || row.get::<_, Option<i64>>("source_generation")
                != Some(as_i64(generation.commit_seq())?)
            || row.get::<_, Option<String>>("selected_generation_digest")
                != Some(generation.digest().to_hex())
            || row.get::<_, Option<String>>("source_projection_digest") != Some(projection.to_hex())
            || row.get::<_, Option<String>>("schema_profile_digest")
                != Some(schema_profile_digest().to_hex())
            || database_oid(&mut tx)? != installation.descriptor_cut.database_oid
            || as_u64(row.get("rule_version"))? != rule_version
            || row.get::<_, String>("contract_digest") != contract.to_hex()
        {
            return Err(DurableError::Conflict(
                "managed model selected source/contract changed",
            ));
        }
        if !row.get::<_, bool>("rights_allowed")
            || as_u64(row.get("rights_version"))? != rights_version
        {
            return Err(DurableError::Refused(
                "managed model current rights changed",
            ));
        }
        let lease = tx.query_opt(
            "SELECT fence_epoch FROM cmd2_job WHERE domain=$1 AND job_id=$2 FOR SHARE",
            &[&domain, &job_id],
        )?;
        if lease.map(|row| row.get::<_, i64>(0)) != Some(as_i64(job_fence)?) {
            return Err(DurableError::Refused("managed model current job changed"));
        }
        owner
            .verify_current(deadline, cancelled)
            .map_err(source_error)?;
        active(deadline, cancelled)?;
        let lease = crate::source_managed_selection::ManagedCurrentModelLease::under_held_fences(
            proof,
            &owner,
            installation.audited_root,
            store,
            deadline,
            cancelled,
        );
        lease.require_managed_basis(proof).map_err(source_error)?;
        let result = seek(&lease);
        lease.require_managed_basis(proof).map_err(source_error)?;
        active(deadline, cancelled)?;
        owner
            .verify_current(deadline, cancelled)
            .map_err(source_error)?;
        installation.audited_root.require_store(store)?;
        tx.commit()?;
        Ok(result)
    }

    pub(crate) fn select_agent_inventory(
        &mut self,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        ctx: &CommandContext,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<ManagedAgentInventory> {
        active(deadline, cancelled)?;
        let cohort = generation.cohort();
        let domain = cohort.domain.as_str();
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY; SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let row = tx.query_one("SELECT * FROM cmd2_domain WHERE domain=$1", &[&domain])?;
        cohort_matches(&row, cohort, true)?;
        let audit = tx.query_one(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence WHERE domain=$1",
            &[&domain],
        )?;
        if as_u64(audit.get(0))? != generation.audit_generation()
            || audit.get::<_, String>(1) != "normal"
        {
            return Err(DurableError::Conflict(
                "managed inventory audit fence differs",
            ));
        }
        if !row.get::<_, bool>("rights_allowed")
            || as_u64(row.get("head_seq"))? != generation.commit_seq()
            || row.get::<_, Option<String>>("selected_generation_digest")
                != Some(generation.digest().to_hex())
            || tx
                .query_one(
                    "SELECT maintenance_state FROM cmd2_audit_fence WHERE domain=$1",
                    &[&domain],
                )?
                .get::<_, String>(0)
                != "normal"
        {
            return Err(DurableError::Conflict(
                "managed inventory current fence differs",
            ));
        }
        let (inventory, count) = retained_agent_inventory(
            &mut tx,
            domain,
            generation.commit_seq(),
            ctx,
            deadline,
            cancelled,
        )?;
        if row.get::<_, Option<String>>("source_projection_digest") != Some(inventory.root.to_hex())
            || count != generation.member_count()
        {
            return Err(DurableError::Refused(
                "managed projection lacks verified complete coverage; FullOnly required",
            ));
        }
        let config = cmd::parse(&ctx.configuration_raw).map_err(source_error)?;
        let request = cmd::parse(&ctx.request_raw).map_err(source_error)?;
        let source_path = cmd::text(&config, "source_path").map_err(source_error)?;
        let home = source_path
            .strip_suffix("/agent.json")
            .ok_or(DurableError::Refused(
                "managed projection consumes Agent home only",
            ))?;
        if tx.query_opt("SELECT 1 FROM cmd2_current WHERE domain=$1 AND (subject=$2 OR left(subject,length($2)+1)=$2||'/') LIMIT 1", &[&domain,&home])?.is_some() {
            return Err(DurableError::Conflict("creation source home already occupied"));
        }
        let mut ids = vec![
            cmd::text(
                cmd::field(&request, "record").map_err(source_error)?,
                "record_id",
            )
            .map_err(source_error)?,
            cmd::text(&config, "provenance_event_id").map_err(source_error)?,
        ];
        for form in cmd::array(&request, "forms").map_err(source_error)? {
            ids.push(cmd::text(form, "form_id").map_err(source_error)?);
        }
        if tx.query_opt("SELECT 1 FROM cmd2_source_index WHERE domain=$1 AND kind<>'path' AND token=ANY($2) LIMIT 1", &[&domain,&ids])?.is_some() {
            return Err(DurableError::Conflict("managed identity already allocated"));
        }
        tx.commit()?;
        active(deadline, cancelled)?;
        let fresh=self.client.query_one("SELECT d.head_seq,d.source_epoch,d.source_projection_digest,d.selected_generation_digest,d.source_complete,d.rights_allowed,f.generation,f.maintenance_state FROM cmd2_domain d JOIN cmd2_audit_fence f USING(domain) WHERE d.domain=$1", &[&domain])?;
        if as_u64(fresh.get(0))? != generation.commit_seq()
            || fresh.get::<_, Option<i64>>(1) != Some(as_i64(cohort.epoch)?)
            || fresh.get::<_, Option<String>>(2) != Some(inventory.root.to_hex())
            || fresh.get::<_, Option<String>>(3) != Some(generation.digest().to_hex())
            || !fresh.get::<_, bool>(4)
            || !fresh.get::<_, bool>(5)
            || as_u64(fresh.get(6))? != generation.audit_generation()
            || fresh.get::<_, String>(7) != "normal"
        {
            return Err(DurableError::Conflict(
                "managed inventory changed during selection",
            ));
        }
        Ok(inventory)
    }

    pub fn read_current_source_member(
        &mut self,
        store: &SegmentStore,
        cohort: &ManagedSourceCohort,
        path: &RelativePath,
        max_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<ManagedCurrentMember> {
        self.read_source_member_bound(store, cohort, None, path, max_bytes, deadline, cancelled)
    }

    pub(crate) fn read_generation_source_metadata(
        &mut self,
        store: &SegmentStore,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        path: &RelativePath,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Option<MemberMetadata>> {
        active(deadline, cancelled)?;
        let selection = generation.selected();
        selection.view().audited_root.require_store(store)?;
        let domain = generation.cohort().domain();
        let selected = selection.lookup_current(domain, path, deadline, cancelled)?;
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        held_generation_metadata(&mut tx, store, generation)?;
        let row = tx.query_opt(
            "SELECT * FROM cmd2_current WHERE domain=$1 AND subject=$2",
            &[&domain, &path.as_str()],
        )?;
        let result = match (selected, row) {
            (None, None) => None,
            (Some(selected), Some(row)) => {
                Some(selected_source_metadata(&row, &selected, domain, path)?)
            }
            _ => return Err(DurableError::Conflict("selected source presence differs")),
        };
        active(deadline, cancelled)?;
        tx.commit()?;
        Ok(result)
    }

    pub(crate) fn visit_generation_source_metadata(
        &mut self,
        store: &SegmentStore,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        deadline: Instant,
        cancelled: &AtomicBool,
        mut visit: impl FnMut(MemberMetadata) -> cmd::SourceCommandResult<()>,
    ) -> DurableResult<GenerationCoverageV1> {
        active(deadline, cancelled)?;
        let mut stream = generation.selected().current_stream()?;
        let domain = generation.cohort().domain();
        let count = generation.member_count();
        let mut root = membership_root_start(CURRENT_KEY_TAG, count);
        let mut observed = 0u64;
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        held_generation_metadata(&mut tx, store, generation)?;
        while let Some(selected) = stream.next_row(deadline, cancelled)? {
            active(deadline, cancelled)?;
            let path = path_from_current_key(domain, &selected.key)?;
            let row = tx
                .query_opt(
                    "SELECT * FROM cmd2_current WHERE domain=$1 AND subject=$2",
                    &[&domain, &path.as_str()],
                )?
                .ok_or(DurableError::Conflict(
                    "selected metadata stream member absent",
                ))?;
            let metadata = selected_source_metadata(&row, &selected, domain, &path)?;
            membership_root_row(&mut root, &selected);
            observed += 1;
            if observed > count {
                return Err(DurableError::Corrupt(
                    "selected metadata stream count exceeded",
                ));
            }
            visit(metadata).map_err(source_error)?;
        }
        let coverage = stream
            .coverage()
            .ok_or(DurableError::Corrupt("selected metadata stream lacks EOF"))?;
        if observed != count
            || coverage.rows != count
            || coverage.descriptor_digest != generation.digest()
            || root.finalize()
                != generation
                    .selected()
                    .view()
                    .descriptor_cut
                    .current_membership_root
        {
            return Err(DurableError::Corrupt(
                "selected metadata EOF/count/root differs",
            ));
        }
        active(deadline, cancelled)?;
        tx.commit()?;
        Ok(coverage)
    }

    pub(crate) fn read_generation_source_member(
        &mut self,
        store: &SegmentStore,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        path: &RelativePath,
        max_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<ManagedCurrentMember> {
        self.read_source_member_bound(
            store,
            generation.cohort(),
            Some(generation),
            path,
            max_bytes,
            deadline,
            cancelled,
        )
    }

    fn read_source_member_bound(
        &mut self,
        store: &SegmentStore,
        cohort: &ManagedSourceCohort,
        generation: Option<&crate::source_current_cut::ManagedCurrentSourceGeneration>,
        path: &RelativePath,
        max_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<ManagedCurrentMember> {
        active(deadline, cancelled)?;
        if max_bytes == 0 || max_bytes > 8_388_608 {
            return Err(DurableError::Refused(
                "current source read exceeds existing member cap",
            ));
        }
        if store.store_id() != cohort.store_id || store.custody_domain() != cohort.domain.as_bytes()
        {
            return Err(DurableError::Conflict(
                "current source custody domain differs",
            ));
        }
        let mut observed_tx = self.client.transaction()?;
        observed_tx
            .batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'")?;
        let observed = observed_tx
            .query_opt(
                "SELECT revision FROM cmd2_current WHERE domain=$1 AND subject=$2",
                &[&cohort.domain, &path.as_str()],
            )?
            .ok_or(DurableError::Conflict("current source member absent"))?;
        observed_tx.commit()?;
        let revision = as_u64(observed.get(0))?;
        let recovered = self.cold_recover_exact(store, &cohort.domain, path.as_str(), revision)?;
        // Retain the global STO pin guard through the actual post-lock current
        // check and disclosure, independent of the filesystem owner mutex.
        let _custody = store.hold_audit_root()?;
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'")?;
        let fence = tx.query_one(
            "SELECT maintenance_state,generation FROM cmd2_audit_fence WHERE domain=$1 FOR SHARE",
            &[&cohort.domain],
        )?;
        if fence.get::<_, String>(0) != "normal" {
            return Err(DurableError::Refused("current source maintenance active"));
        }
        let domain = tx.query_one(
            "SELECT * FROM cmd2_domain WHERE domain=$1 FOR SHARE",
            &[&cohort.domain],
        )?;
        cohort_matches(&domain, cohort, true)?;
        if let Some(selected) = generation {
            let selected_seq = as_i64(selected.commit_seq())?;
            if as_u64(fence.get(1))? != selected.audit_generation()
                || domain.get::<_, i64>("head_seq") != selected_seq
                || domain.get::<_, Option<i64>>("source_generation") != Some(selected_seq)
                || domain.get::<_, Option<String>>("selected_generation_digest")
                    != Some(selected.digest().to_hex())
            {
                return Err(DurableError::Conflict(
                    "selected current source generation changed",
                ));
            }
        }
        if !domain.get::<_, bool>("rights_allowed") {
            return Err(DurableError::Refused("current source rights revoked"));
        }
        let row = tx
            .query_opt(
                "SELECT * FROM cmd2_current WHERE domain=$1 AND subject=$2",
                &[&cohort.domain, &path.as_str()],
            )?
            .ok_or(DurableError::Conflict("current source member disappeared"))?;
        check_history_locator(
            &row,
            &recovered.receipt,
            &cohort.domain,
            path.as_str(),
            revision,
        )?;
        if let Some(generation) = generation {
            let selected = generation
                .selected()
                .lookup_current(&cohort.domain, path, deadline, cancelled)?
                .ok_or(DurableError::Conflict("selected current frame absent"))?;
            selected_source_metadata(&row, &selected, &cohort.domain, path)?;
            if selected.placement != recovered.receipt.placement() {
                return Err(DurableError::Conflict(
                    "selected recovered current frame differs",
                ));
            }
        }
        let mut raw = Vec::new();
        store.read_selected(&recovered.receipt, max_bytes, &mut raw)?;
        active(deadline, cancelled)?;
        let (metadata, dependency_claims) = expose_metadata(&row, path)?;
        let result = ManagedCurrentMember {
            path: path.clone(),
            custody_revision: revision,
            current_generation: as_u64(domain.get("head_seq"))?,
            commit_seq: as_u64(row.get("commit_seq"))?,
            raw,
            metadata,
            dependency_claims,
            placement: recovered.receipt.placement(),
        };
        tx.commit()?;
        Ok(result)
    }
    /// Whole used producer/consumer: maintained initial Agent preparation and
    /// actual serialization, registered affected reads, then STO/PG commit.
    /// A later changed-source reprepare requires a real exported source cut;
    /// this explicit optimistic route retains its original immutable proposal.
    pub fn execute_optimistic_agent_creation_from_captures(
        &mut self,
        store: &SegmentStore,
        cohort: &ManagedSourceCohort,
        prepare_id: &[u8],
        filesystem: &CreationFilesystem,
        context: &CommandContext,
        original: &CorpusCutReader,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        worker: &mut CutWorkerSchemaExecutor,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<(SerializedCreation, DurableCommitReceipt, DurableTiming)> {
        let prepared = crate::source_creation::prepare_source_creation_from_captures(
            context, original, software, components, worker, deadline, cancelled,
        )
        .map_err(source_error)?;
        let serialized = prepared
            .serialize(software, components, worker, deadline, cancelled)
            .map_err(source_error)?;
        let attempt = self.register_source_creation(
            store,
            cohort,
            prepare_id,
            &serialized,
            worker,
            deadline,
            cancelled,
        )?;
        finish_worker(worker, deadline, cancelled)?;
        let (receipt, timing) = self.commit_source_creation(
            store,
            &attempt,
            &serialized,
            filesystem,
            contract,
            rule_version,
            rights_version,
            job_id,
            job_fence,
            deadline,
            cancelled,
        )?;
        Ok((serialized, receipt, timing))
    }
    /// Prepare and serialize against the real current cut before durable
    /// registration; no initial SourceRevision is reused for changed content.
    pub fn execute_current_agent_creation_from_captures(
        &mut self,
        store: &SegmentStore,
        current: &crate::source_current_cut::ManagedCurrentSourceCut,
        prepare_id: &[u8],
        filesystem: &CreationFilesystem,
        context: &CommandContext,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        worker: &mut CutWorkerSchemaExecutor,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<(SerializedCreation, DurableCommitReceipt, DurableTiming)> {
        let prepared = crate::source_creation::prepare_source_creation_from_captures(
            context,
            current.cut(),
            software,
            components,
            worker,
            deadline,
            cancelled,
        )
        .map_err(source_error)?;
        let serialized = prepared
            .serialize(software, components, worker, deadline, cancelled)
            .map_err(source_error)?;
        let attempt = self.register_source_creation_from_current(
            store,
            current,
            prepare_id,
            &serialized,
            worker,
            deadline,
            cancelled,
        )?;
        finish_worker(worker, deadline, cancelled)?;
        let (receipt, timing) = self.commit_source_creation(
            store,
            &attempt,
            &serialized,
            filesystem,
            contract,
            rule_version,
            rights_version,
            job_id,
            job_fence,
            deadline,
            cancelled,
        )?;
        Ok((serialized, receipt, timing))
    }

    /// Same actual creation owner, with authored generation separate from schemas.
    pub fn execute_managed_agent_creation_from_captures(
        &mut self,
        store: &SegmentStore,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        schema_cut: &CorpusCutReader,
        prepare_id: &[u8],
        filesystem: &CreationFilesystem,
        context: &CommandContext,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        worker: &mut CutWorkerSchemaExecutor,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<(
        ManagedSerializedCreation,
        DurableCommitReceipt,
        DurableTiming,
        crate::source_current_cut::ManagedCurrentSourceGeneration,
    )> {
        let input = crate::source_creation::select_managed_agent_creation_input(
            self, store, generation, context, schema_cut, software, components, deadline, cancelled,
        )
        .map_err(source_error)?;
        let prepared = crate::source_creation::prepare_managed_agent_creation(
            &input, schema_cut, software, components, worker, deadline, cancelled,
        )
        .map_err(source_error)?;
        let serialized = prepared
            .serialize(software, components, worker, deadline, cancelled)
            .map_err(source_error)?;
        let attempt = self.register_managed_source_creation(
            store,
            generation,
            prepare_id,
            &serialized,
            worker,
            deadline,
            cancelled,
        )?;
        finish_worker(worker, deadline, cancelled)?;
        let (receipt, timing) = self.commit_managed_source_creation(
            store,
            &attempt,
            &serialized,
            filesystem,
            contract,
            rule_version,
            rights_version,
            job_id,
            job_fence,
            deadline,
            cancelled,
        )?;
        let successor = self.continue_managed_creation(
            store,
            generation.cohort(),
            &attempt,
            CreationPackage::Managed(&serialized),
            filesystem,
            deadline,
            cancelled,
        )?;
        Ok((serialized, receipt, timing, successor))
    }

    // Continue only the uninterrupted owner-issued register/attach/commit chain.
    // This does not reopen a lost process or turn a caller descriptor into a cut.
    fn continue_managed_creation(
        &mut self,
        store: &SegmentStore,
        cohort: &ManagedSourceCohort,
        attempt: &SourceCreationAttempt,
        package: CreationPackage<'_>,
        filesystem: &CreationFilesystem,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<crate::source_current_cut::ManagedCurrentSourceGeneration> {
        active(deadline, cancelled)?;
        let continuation = attempt.continuation.as_ref().ok_or(DurableError::Refused(
            "warm source chain absent; explicit cold reopen required",
        ))?;
        let committed = continuation.committed.borrow();
        let committed = committed.as_ref().ok_or(DurableError::Refused(
            "warm source commit not observed; explicit cold reopen required",
        ))?;
        let parent = continuation.parent.view();
        parent.audited_root.require_store(store)?;
        if attempt.domain != cohort.domain
            || attempt.epoch != cohort.epoch
            || attempt.definition != cohort.definition
            || committed.commit_seq
                != parent
                    .descriptor_cut
                    .through_seq
                    .checked_add(1)
                    .ok_or(DurableError::Corrupt("warm head overflow"))?
            || registered_source_delta(package, &attempt.reads, &attempt.projections)?
                != attempt.delta
        {
            return Err(DurableError::Conflict("warm source original basis changed"));
        }
        let owner = filesystem
            .hold_creation_owner(package, deadline, cancelled)
            .map_err(source_error)?;
        owner
            .verify_current(deadline, cancelled)
            .map_err(source_error)?;
        drop(owner);
        let domain = &attempt.domain;
        let head = committed.commit_seq;
        let started = Instant::now();
        let requested = Some((deadline, cancelled));
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        tx.batch_execute("SET LOCAL statement_timeout='60s'; SET LOCAL work_mem='4MB'")?;
        let row = tx.query_one("SELECT d.*,f.generation AS audit_generation,f.maintenance_state FROM cmd2_domain d JOIN cmd2_audit_fence f USING(domain) WHERE d.domain=$1", &[&domain])?;
        cohort_matches(&row, cohort, true)?;
        if as_u64(row.get("audit_generation"))? != committed.audit_generation
            || as_u64(row.get("head_seq"))? != head
            || row.get::<_, Option<i64>>("source_generation") != Some(as_i64(head)?)
            || row.get::<_, String>("maintenance_state") != "normal"
            || !row.get::<_, bool>("rights_allowed")
            || row.get::<_, Option<String>>("schema_profile_digest")
                != Some(schema_profile_digest().to_hex())
            || database_oid(&mut tx)? != parent.descriptor_cut.database_oid
        {
            return Err(DurableError::Conflict(
                "warm metadata chain lost; explicit cold reopen required",
            ));
        }
        admit_private_metadata(&mut tx, domain, started, requested, None)?;
        let registered = tx.query_one(
            "SELECT * FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2",
            &[&domain, &attempt.prepare_id],
        )?;
        let receipt = receipt_from_committed_attempt(&mut tx, &registered)?;
        if receipt.commit_seq != head
            || receipt.delta_digest != attempt.delta
            || registered.get::<_, Option<Vec<u8>>>("source_reads")
                != Some(reads_bytes(&attempt.reads)?)
            || registered.get::<_, Option<Vec<u8>>>("source_indexes")
                != Some(indexes_bytes(&attempt.indexes)?)
            || registered.get::<_, Option<Vec<u8>>>("source_projections")
                != Some(projections_bytes(&attempt.projections)?)
        {
            return Err(DurableError::Corrupt("warm committed attempt differs"));
        }
        let members = tx.query(
            "SELECT * FROM cmd2_member WHERE domain=$1 AND prepare_id=$2 ORDER BY member_slot",
            &[&domain, &attempt.prepare_id],
        )?;
        if members.len() != committed.receipts.len()
            || members.len() != package.changes().len()
            || members.is_empty()
            || members.len() > MAX_MEMBERS
        {
            return Err(DurableError::Corrupt(
                "warm committed member coverage differs",
            ));
        }
        let mut history_rows =
            finite_warm_parent_rows(&parent, GenerationNamespaceV1::History, deadline, cancelled)?;
        let mut current_rows =
            finite_warm_parent_rows(&parent, GenerationNamespaceV1::Current, deadline, cancelled)?;
        let mut member_root = Digest256Hasher::new();
        part(&mut member_root, b"cmd2-member-root-v1");
        part(&mut member_root, &(members.len() as u64).to_be_bytes());
        let carriers = creation_metadata(package);
        for (member, change) in members.iter().zip(package.changes()) {
            active(deadline, cancelled)?;
            let subject = change.path.as_str();
            let selected = committed
                .receipts
                .iter()
                .find(|r| r.binding().member_slot == member.get::<_, i32>("member_slot") as u32)
                .ok_or(DurableError::Corrupt("warm verified frame absent"))?;
            check_member_row(member, selected, subject, 1)?;
            if row_source_metadata(member)? != carriers[subject]
                || continuation
                    .parent
                    .lookup_current(domain, &change.path, deadline, cancelled)?
                    .is_some()
                || selected.binding().profile_id != CREATION
                || selected.coordinate().sha256
                    != Digest256::of_bytes(change.after.as_ref().unwrap())
            {
                return Err(DurableError::Corrupt(
                    "warm initial creation frame/metadata differs",
                ));
            }
            update_member_root(&mut member_root, member);
            let coordinate = selected.coordinate();
            history_rows.push(PlacementGenerationRowV1 {
                key: membership_key(HISTORY_KEY_TAG, domain, subject, Some(1))?,
                logical_digest: coordinate.sha256,
                logical_length: coordinate.size_bytes,
                placement: selected.placement(),
            });
            current_rows.push(PlacementGenerationRowV1 {
                key: membership_key(CURRENT_KEY_TAG, domain, subject, None)?,
                logical_digest: coordinate.sha256,
                logical_length: coordinate.size_bytes,
                placement: selected.placement(),
            });
        }
        if member_root.finalize() != receipt.member_root {
            return Err(DurableError::Corrupt("warm member root differs"));
        }
        sort_complete_membership(&mut history_rows)?;
        sort_complete_membership(&mut current_rows)?;
        let keys: usize = history_rows
            .iter()
            .chain(&current_rows)
            .map(|r| r.key.len())
            .sum();
        if history_rows.len() > MAX_CUT as usize
            || current_rows.len() > MAX_CUT as usize
            || keys > MAX_TOTAL_MEMBERSHIP_KEY_BYTES
        {
            return Err(DurableError::Refused(
                "warm membership exceeds existing complete generation bounds",
            ));
        }
        // Metadata/log scans are explicit O(N), without re-reading old STO bodies.
        let mut log_hash = Digest256Hasher::new();
        part(&mut log_hash, b"cmd2-cold-cut-v1");
        let mut logs=tx.query_raw("SELECT commit_seq,event_kind,command_id,delta_digest,members_root FROM cmd2_log WHERE domain=$1 ORDER BY commit_seq", &[&domain])?;
        let mut next = 1u64;
        while let Some(log) = logs.next()? {
            active(deadline, cancelled)?;
            let seq = as_u64(log.get(0))?;
            if seq != next || seq > head {
                return Err(DurableError::Corrupt("warm log coverage differs"));
            }
            let kind: String = log.get(1);
            let command: String = log.get(2);
            let delta: String = log.get(3);
            let root: String = log.get(4);
            if seq == head
                && (kind != "command"
                    || command != receipt.command_id
                    || delta != receipt.delta_digest.to_hex()
                    || root != receipt.member_root.to_hex())
            {
                return Err(DurableError::Corrupt("warm final log differs"));
            }
            for value in [
                &as_i64(seq)?.to_be_bytes()[..],
                kind.as_bytes(),
                command.as_bytes(),
                delta.as_bytes(),
                root.as_bytes(),
            ] {
                part(&mut log_hash, value);
            }
            if seq == parent.descriptor_cut.through_seq
                && log_hash.clone().finalize() != parent.descriptor_cut.log_digest
            {
                return Err(DurableError::Corrupt("warm verified log prefix differs"));
            }
            next += 1;
        }
        drop(logs);
        if next != head + 1 {
            return Err(DurableError::Corrupt("warm log EOF differs"));
        }
        let mut state = Digest256Hasher::new();
        part(&mut state, COLD_AUDIT_PROFILE);
        part(&mut state, domain.as_bytes());
        part(&mut state, &head.to_be_bytes());
        part(
            &mut state,
            &parent.descriptor_cut.database_oid.to_be_bytes(),
        );
        part(&mut state, schema_profile_digest().to_hex().as_bytes());
        let expected = history_rows
            .iter()
            .map(|r| (r.key.as_slice(), r))
            .collect::<BTreeMap<_, _>>();
        let mut seen = BTreeSet::new();
        let mut latest = BTreeMap::new();
        let mut histories = tx.query_raw(
            "SELECT * FROM cmd2_history WHERE domain=$1 ORDER BY commit_seq,member_slot",
            &[&domain],
        )?;
        while let Some(h) = histories.next()? {
            active(deadline, cancelled)?;
            let subject: String = h.get("subject");
            let revision = as_u64(h.get("revision"))?;
            let key = membership_key(HISTORY_KEY_TAG, domain, &subject, Some(revision))?;
            let selected = expected
                .get(key.as_slice())
                .ok_or(DurableError::Corrupt("warm extra history row"))?;
            if !seen.insert(key)
                || as_u64(h.get("commit_seq"))? > head
                || h.get::<_, String>("content_digest") != selected.logical_digest.to_hex()
                || as_u64(h.get("content_length"))? != selected.logical_length
            {
                return Err(DurableError::Corrupt("warm history row binding differs"));
            }
            if as_u64(h.get("commit_seq"))? == head {
                let frame = committed
                    .receipts
                    .iter()
                    .find(|r| r.receipt_id().to_hex() == h.get::<_, String>("sto_receipt_id"))
                    .ok_or(DurableError::Corrupt("warm history frame absent"))?;
                check_history_locator(&h, frame, domain, &subject, revision)?;
                if row_source_metadata(&h)? != carriers[subject.as_str()]
                    || h.get::<_, Option<Vec<u8>>>("inventory_projection").as_ref()
                        != attempt.projections.get(&subject)
                {
                    return Err(DurableError::Corrupt("warm appended projection differs"));
                }
            }
            part(&mut state, &selected.placement.encode());
            if latest
                .insert(subject, (revision, metadata_locator_digest(&h)))
                .is_some_and(|prior| prior.0 >= revision)
            {
                return Err(DurableError::Corrupt("warm revision order differs"));
            }
        }
        drop(histories);
        if seen.len() != history_rows.len() {
            return Err(DurableError::Corrupt("warm history EOF differs"));
        }
        let mut currents =
            tx.query_raw("SELECT * FROM cmd2_current WHERE domain=$1", &[&domain])?;
        let mut seen_current = BTreeSet::new();
        while let Some(c) = currents.next()? {
            active(deadline, cancelled)?;
            let subject: String = c.get("subject");
            let path =
                RelativePath::parse(&subject).map_err(|_| DurableError::Corrupt("warm path"))?;
            let key = membership_key(CURRENT_KEY_TAG, domain, &subject, None)?;
            let position = current_rows
                .binary_search_by(|member| member.key.cmp(&key))
                .map_err(|_| DurableError::Corrupt("warm current selected member absent"))?;
            selected_source_metadata(&c, &current_rows[position], domain, &path)?;
            if !seen_current.insert(subject.clone())
                || latest.get(&subject)
                    != Some(&(as_u64(c.get("revision"))?, metadata_locator_digest(&c)))
            {
                return Err(DurableError::Corrupt(
                    "warm current metadata/history differs",
                ));
            }
        }
        drop(currents);
        if seen_current.len() != current_rows.len() {
            return Err(DurableError::Corrupt("warm current EOF differs"));
        }
        let (projection, _, count) =
            retained_projection_root(&mut tx, domain, head, deadline, cancelled)?;
        if count != current_rows.len() as u64 {
            return Err(DurableError::Corrupt("warm projection coverage differs"));
        }
        append_private_metadata(&mut tx, domain, &mut state, started, requested, None)?;
        let cut = WarmSuccessorCut {
            audited_root: parent.audited_root.clone(),
            domain: domain.clone(),
            descriptor_cut: GenerationCutV1 {
                store_id: store.store_id(),
                domain_digest: store.domain_digest(),
                through_seq: head,
                audit_generation: committed.audit_generation,
                database_oid: parent.descriptor_cut.database_oid,
                schema_profile_digest: schema_profile_digest(),
                state_digest: state.finalize(),
                log_digest: log_hash.finalize(),
                historical_members: history_rows.len() as u64,
                current_members: current_rows.len() as u64,
                history_membership_root: logical_membership_root(HISTORY_KEY_TAG, &history_rows),
                current_membership_root: logical_membership_root(CURRENT_KEY_TAG, &current_rows),
            },
            history_rows,
            current_rows,
        };
        tx.commit()?;
        let (installed, history_coverage, current_coverage) =
            install_verified_membership(store, cut.installation(), deadline, cancelled)?;
        if history_coverage.descriptor_digest != installed.digest()
            || current_coverage.descriptor_digest != installed.digest()
            || history_coverage.rows != cut.descriptor_cut.historical_members
            || current_coverage.rows != cut.descriptor_cut.current_members
        {
            return Err(DurableError::Corrupt("warm installation coverage differs"));
        }
        let owner = filesystem
            .hold_creation_owner(package, deadline, cancelled)
            .map_err(source_error)?;
        owner
            .verify_current(deadline, cancelled)
            .map_err(source_error)?;
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let audit = lock_audit_fence(&mut tx, domain)?;
        let row = tx.query_one(
            "SELECT * FROM cmd2_domain WHERE domain=$1 FOR UPDATE",
            &[&domain],
        )?;
        cohort_matches(&row, cohort, true)?;
        if audit != committed.audit_generation
            || as_u64(row.get("head_seq"))? != head
            || !row.get::<_, bool>("rights_allowed")
            || database_oid(&mut tx)? != cut.descriptor_cut.database_oid
            || row.get::<_, Option<String>>("schema_profile_digest")
                != Some(schema_profile_digest().to_hex())
        {
            return Err(DurableError::Conflict(
                "warm installation raced metadata; explicit cold reopen required",
            ));
        }
        owner
            .verify_current(deadline, cancelled)
            .map_err(source_error)?;
        cut.audited_root.require_installed(&installed)?;
        // The anticipated value is only DB bookkeeping. Authority is issued
        // from the actual post-trigger observation below, never counter+N.
        let bookkeeping = audit
            .checked_add(1)
            .ok_or(DurableError::Corrupt("warm audit overflow"))?;
        tx.execute("UPDATE cmd2_domain SET published_seq=$2,complete_cut_digest=$3,complete_cut_generation=$4,selected_generation_digest=$5,source_projection_digest=$6 WHERE domain=$1", &[&domain,&as_i64(head)?,&cut.descriptor_cut.state_digest.to_hex(),&as_i64(bookkeeping)?,&installed.digest().to_hex(),&projection.to_hex()])?;
        let selected_audit_generation = lock_audit_fence(&mut tx, domain)?;
        if selected_audit_generation != bookkeeping {
            return Err(DurableError::Corrupt(
                "warm publication trigger observation differs",
            ));
        }
        owner
            .verify_current(deadline, cancelled)
            .map_err(source_error)?;
        tx.commit()?;
        let mut successor_cohort = cohort.clone();
        successor_cohort.generation = head;
        Ok(
            crate::source_current_cut::ManagedCurrentSourceGeneration::from_verified_successor(
                store,
                successor_cohort,
                VerifiedWarmGeneration {
                    cut,
                    installed,
                    selected_audit_generation,
                },
            ),
        )
    }

    /// Bootstrap a fresh private domain; neither a caller SQL complete flag nor
    /// an authored mutable directory can construct the returned source basis.
    pub fn bootstrap_source_cohort(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        cut: &CorpusCutReader,
        selected_revision: SourceRevision,
        selected_membership: SourceMembershipV1,
        context: &CommandContext,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        worker: &mut CutWorkerSchemaExecutor,
        contract: Digest256,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<ReopenedSourceCohort> {
        active(deadline, cancelled)?;
        if store.custody_domain() != domain.as_bytes()
            || cut.current().revision() != selected_revision
            || context.base_revision != selected_revision
        {
            return Err(DurableError::Conflict(
                "independent source selection differs",
            ));
        }
        context
            .check_from_selected_captures(cut, software, components, deadline, cancelled)
            .map_err(source_error)?;
        let mut stream = cut
            .stream(selected_revision)
            .map_err(|_| DurableError::Refused("source stream unavailable"))?;
        if stream.expectation() != selected_membership {
            return Err(DurableError::Conflict(
                "independent membership selection differs",
            ));
        }
        let mut ctx = context.clone();
        ctx.files.retain(|f| !f.path.as_str().starts_with("ToS/"));
        let mut total_bytes = ctx.files.iter().map(|f| f.raw.len()).sum::<usize>();
        let planned_bytes = cut
            .current()
            .members()
            .try_fold(total_bytes as u64, |n, m| n.checked_add(m.size_bytes));
        if ctx
            .files
            .len()
            .checked_add(cut.current().member_count())
            .is_none_or(|n| n > 4096)
            || planned_bytes.is_none_or(|n| n > 33_554_432)
            || cut.current().members().any(|m| m.size_bytes > 8_388_608)
        {
            return Err(DurableError::Refused(
                "source bootstrap preadmission exceeds existing command bounds",
            ));
        }
        while let Some(member) = stream
            .next_member(deadline, cancelled)
            .map_err(|_| DurableError::Refused("source EOF/fixity coverage failed"))?
        {
            // Preserve the existing CommandContext count/aggregate/member bounds.
            if ctx.files.len() >= 4096
                || member.raw.len() > 8_388_608
                || total_bytes
                    .checked_add(member.raw.len())
                    .is_none_or(|n| n > 33_554_432)
            {
                return Err(DurableError::Refused(
                    "source bootstrap exceeds existing selected command bounds",
                ));
            }
            total_bytes += member.raw.len();
            ctx.files.push(SourceFile {
                path: member.path,
                raw: member.raw,
            });
        }
        if stream.coverage() != Some(selected_membership) {
            return Err(DurableError::Refused(
                "source stream lacks complete EOF coverage",
            ));
        }
        ctx.check().map_err(source_error)?;
        let indexes = index_rows(&ctx, worker, deadline, cancelled)?;
        let projections = optional_agent_projections(&ctx, worker, deadline, cancelled)?;
        verify_manifest_indexes(cut, &indexes)?;
        let def = definition();
        self.create_domain(domain, contract)?;
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'")?;
        lock_audit_fence(&mut tx, domain)?;
        let row = tx.query_one(
            "SELECT * FROM cmd2_domain WHERE domain=$1 FOR UPDATE",
            &[&domain],
        )?;
        let counts = tx.query_one("SELECT (SELECT count(*) FROM cmd2_attempt WHERE domain=$1)+(SELECT count(*) FROM cmd2_current WHERE domain=$1)", &[&domain])?;
        if row.get::<_, i64>("head_seq") != 0
            || row.get::<_, Option<String>>("source_revision").is_some()
            || counts.get::<_, i64>(0) != 0
        {
            return Err(DurableError::Refused(
                "bootstrap requires an unused isolated durable domain",
            ));
        }
        tx.execute("UPDATE cmd2_domain SET source_revision=$2,source_membership_digest=$3,source_membership_count=$4,source_epoch=1,source_generation=0,source_complete=false,source_definition_digest=$5 WHERE domain=$1",
            &[&domain,&selected_revision.0.to_hex(),&selected_membership.digest.to_hex(),&as_i64(selected_membership.count)?,&def.to_hex()])?;
        tx.commit()?;
        self.set_job_epoch(domain, "source-bootstrap", 1)?;
        let source_metadata = original_metadata(cut);
        let originals = ctx
            .files
            .iter()
            .filter(|f| f.path.as_str().starts_with("ToS/"))
            .collect::<Vec<_>>();
        for (batch, files) in originals.chunks(MAX_MEMBERS).enumerate() {
            active(deadline, cancelled)?;
            let prepare = format!("source-bootstrap:{batch}").into_bytes();
            let identities = files
                .iter()
                .enumerate()
                .map(|(slot, f)| ShadowWriteIdentity {
                    member_slot: slot as u32,
                    subject: f.path.as_str(),
                    expected_predecessor: None,
                    proposed_revision: 1,
                    exact_bytes: &f.raw,
                })
                .collect::<Vec<_>>();
            let fence = self.register_attempt(&RegisterShadowAttempt {
                domain,
                prepare_id: &prepare,
                command_id: std::str::from_utf8(&prepare).unwrap(),
                raw_request_digest: selected_revision.0,
                delta_digest: durable_shadow_delta_prepared(&identities),
            })?;
            let members = seal_members(store, &prepare, fence, ORIGINAL, &identities)?;
            self.attach_ready_profile(
                store,
                domain,
                &prepare,
                fence,
                &members,
                ORIGINAL,
                None,
                Some(&source_metadata),
            )?;
            let receipts = members
                .iter()
                .map(|m| m.receipt.clone())
                .collect::<Vec<_>>();
            let mut head_tx = self.client.transaction()?;
            head_tx.batch_execute(
                "SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'",
            )?;
            let head = as_u64(
                head_tx
                    .query_one(
                        "SELECT head_seq FROM cmd2_domain WHERE domain=$1",
                        &[&domain],
                    )?
                    .get(0),
            )?;
            head_tx.commit()?;
            self.commit_durable(
                store,
                &CommitShadowAttempt {
                    domain,
                    prepare_id: &prepare,
                    attempt_fence: fence,
                    receipts: &receipts,
                    expected_contract_digest: contract,
                    expected_rule_version: 0,
                    expected_rights_version: 0,
                    job_id: "source-bootstrap",
                    job_fence: 1,
                    full_base_seq: head,
                },
                CommitMode::Bootstrap,
            )?;
        }
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'")?;
        lock_audit_fence(&mut tx, domain)?;
        for (kind, token, path) in &indexes {
            tx.execute("INSERT INTO cmd2_source_index(domain,kind,token,path,definition_digest) VALUES($1,$2,$3,$4,$5)", &[&domain,kind,token,path,&def.to_hex()])?;
        }
        if let Some(projections) = &projections {
            for (path, projection) in projections {
                let current = tx.execute("UPDATE cmd2_current SET inventory_projection=$3 WHERE domain=$1 AND subject=$2", &[&domain,path,projection])?;
                let history = tx.execute("UPDATE cmd2_history SET inventory_projection=$3 WHERE domain=$1 AND subject=$2 AND revision=1", &[&domain,path,projection])?;
                let indexed = tx.execute("UPDATE cmd2_source_index SET inventory_projection=$3 WHERE domain=$1 AND kind='path' AND path=$2", &[&domain,path,projection])?;
                if current != 1 || history != 1 || indexed != 1 {
                    return Err(DurableError::Corrupt(
                        "bootstrap projection membership differs",
                    ));
                }
            }
        }
        for (kind, scope, token) in predicates(&indexes) {
            tx.execute("INSERT INTO cmd2_predicate(domain,kind,owner,scope,token,definition_version,generation,complete) VALUES($1,$2,$3,$4,$5,$6,0,false)", &[&domain,&kind,&OWNER,&scope,&token,&def.to_hex()])?;
        }
        tx.commit()?;
        self.cold_reopen_source_cohort(
            store,
            domain,
            cut,
            selected_revision,
            selected_membership,
            context,
            software,
            components,
            worker,
            deadline,
            cancelled,
        )
    }

    /// Composable registration: its highest caller must finish the schema
    /// operation after all registrations and before commit_source_creation.
    pub fn register_source_creation(
        &mut self,
        store: &SegmentStore,
        cohort: &ManagedSourceCohort,
        prepare_id: &[u8],
        package: &SerializedCreation,
        worker: &mut CutWorkerSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<SourceCreationAttempt> {
        if package.command().base_revision != cohort.initial_revision {
            return Err(DurableError::Conflict(
                "original proposal is not based on bootstrap revision",
            ));
        }
        self.register_source_creation_bound(
            store,
            cohort,
            prepare_id,
            CreationPackage::V1(package),
            SourceRegistrationBasis::Original,
            worker,
            deadline,
            cancelled,
        )
    }

    /// A subsequent maintained creation consumes the actual parent-owned,
    /// independently reopened current source cut, including earlier bodies.
    /// Its highest caller finishes the schema operation before durable commit.
    pub fn register_source_creation_from_current(
        &mut self,
        store: &SegmentStore,
        current: &crate::source_current_cut::ManagedCurrentSourceCut,
        prepare_id: &[u8],
        package: &SerializedCreation,
        worker: &mut CutWorkerSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<SourceCreationAttempt> {
        if package.command().base_revision != current.cut().current().revision() {
            return Err(DurableError::Conflict(
                "proposal does not consume actual current source cut",
            ));
        }
        let package_members = package
            .command()
            .reads
            .iter()
            .filter(|r| r.path.as_str().starts_with("ToS/"))
            .map(|r| (r.path.as_str(), r.raw_sha256))
            .collect::<BTreeMap<_, _>>();
        if package_members.len() != current.files().len()
            || current
                .files()
                .iter()
                .any(|(p, raw)| package_members.get(p.as_str()) != Some(&Digest256::of_bytes(raw)))
            || membership_of_files(current.files()) != current.membership()
        {
            return Err(DurableError::Conflict(
                "current cut package and verified raw membership differ",
            ));
        }
        self.register_source_creation_bound(
            store,
            current.cohort(),
            prepare_id,
            CreationPackage::V1(package),
            SourceRegistrationBasis::Current(current),
            worker,
            deadline,
            cancelled,
        )
    }

    pub fn register_managed_source_creation(
        &mut self,
        store: &SegmentStore,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        prepare_id: &[u8],
        package: &ManagedSerializedCreation,
        worker: &mut CutWorkerSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<SourceCreationAttempt> {
        let view = CreationPackage::Managed(package);
        if view.managed_basis() != Some(&ManagedCreationBasis::from_generation(generation)) {
            return Err(DurableError::Conflict("managed proposal selection differs"));
        }
        let members = view
            .reads()
            .iter()
            .filter(|read| read.path.as_str().starts_with("ToS/"))
            .map(|read| (read.path.as_str(), read.raw_sha256))
            .collect::<BTreeMap<_, _>>();
        if view.inventory().is_none() && members.len() as u64 != generation.member_count() {
            return Err(DurableError::Conflict(
                "managed proposal complete inventory differs",
            ));
        }
        let observations = view
            .observations()
            .ok_or(DurableError::Conflict("managed observations absent"))?;
        if observations.len() != members.len()
            || observations.iter().any(|(path, observed)| {
                members.get(path.as_str()) != Some(&observed.metadata.sha256)
            })
        {
            return Err(DurableError::Conflict(
                "managed observed membership differs",
            ));
        }
        self.register_source_creation_bound(
            store,
            generation.cohort(),
            prepare_id,
            view,
            SourceRegistrationBasis::Managed(generation),
            worker,
            deadline,
            cancelled,
        )
    }
    /// Process-loss recovery owns the original managed input and exact output
    /// buffers through retained history. No caller package can issue a write.
    pub fn recover_committed_managed_agent_creation(
        &mut self,
        store: &SegmentStore,
        cohort: &ManagedSourceCohort,
        prepare_id: &[u8],
        filesystem: &CreationFilesystem,
        schema_cut: &CorpusCutReader,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        worker: &mut CutWorkerSchemaExecutor,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<(DurableCommitReceipt, DurableTiming)> {
        active(deadline, cancelled)?;
        if store.store_id() != cohort.store_id || store.custody_domain() != cohort.domain.as_bytes()
        {
            return Err(DurableError::Conflict("recovery custody domain differs"));
        }
        // Global retained-pin custody spans all original observation and output reads.
        let custody = store.hold_audit_root()?;
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'")?;
        lock_audit_fence(&mut tx, &cohort.domain)?;
        let domain = tx.query_one(
            "SELECT * FROM cmd2_domain WHERE domain=$1 FOR SHARE",
            &[&cohort.domain],
        )?;
        cohort_matches(&domain, cohort, true)?;
        if !domain.get::<_, bool>("rights_allowed") {
            return Err(DurableError::Refused(
                "recovery current source rights revoked",
            ));
        }
        let gate=tx.query_opt("SELECT state,octet_length(row_to_json(a)::text) FROM cmd2_attempt a WHERE domain=$1 AND prepare_id=$2 FOR SHARE",&[&cohort.domain,&prepare_id])?.ok_or(DurableError::Refused("recovery attempt absent"))?;
        if gate.get::<_, String>(0) != "committed" {
            return Err(DurableError::Refused("recovery requires committed attempt"));
        }
        if gate
            .get::<_, Option<i32>>(1)
            .is_none_or(|n| n < 0 || n > 1_048_576)
        {
            return Err(DurableError::Refused(
                "recovery attempt exceeds existing cold metadata row bound",
            ));
        }
        let attempt = tx.query_one(
            "SELECT * FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2",
            &[&cohort.domain, &prepare_id],
        )?;
        let committed_seq = as_u64(
            attempt
                .get::<_, Option<i64>>("commit_seq")
                .ok_or(DurableError::Corrupt("recovery commit sequence absent"))?,
        )?;
        let reads = decode_reads(
            &attempt
                .get::<_, Option<Vec<u8>>>("source_reads")
                .ok_or(DurableError::Corrupt("recovery original reads absent"))?,
        )?;
        let original = reads
            .managed_original
            .as_ref()
            .ok_or(DurableError::Refused(
                "attempt has no original managed recovery basis",
            ))?;
        let (mut input, software_inputs) = decode_managed_original(
            original,
            cohort,
            committed_seq,
            schema_cut,
            software,
            components,
            worker,
        )?;
        if attempt.get::<_, Option<i64>>("source_epoch") != Some(as_i64(input.basis.epoch)?)
            || Digest256::of_bytes(&input.context.request_raw).to_hex()
                != attempt.get::<_, String>("raw_request_digest")
        {
            return Err(DurableError::Corrupt(
                "original managed request/epoch binding differs",
            ));
        }
        let exact_reads = reads
            .reads
            .iter()
            .filter_map(|r| match r {
                PredicateRead::Exact {
                    namespace,
                    key,
                    expected_version,
                    expected_digest,
                } => Some((key, (namespace, expected_version, expected_digest))),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        if exact_reads.len() != input.observations.len()
            || input.observations.iter().any(|(path, o)| {
                exact_reads.get(path).is_none_or(|(ns, version, digest)| {
                    ns.as_str() != "source"
                        || **version != Some(o.custody_revision)
                        || **digest != Some(o.metadata.sha256)
                })
            })
        {
            return Err(DurableError::Corrupt(
                "original managed observation/read closure differs",
            ));
        }
        let paths = input.observations.keys().collect::<Vec<_>>();
        let revisions = input
            .observations
            .values()
            .map(|o| as_i64(o.custody_revision))
            .collect::<DurableResult<Vec<_>>>()?;
        let history=tx.query("SELECT h.* FROM cmd2_history h JOIN unnest($2::text[],$3::bigint[]) AS wanted(subject,revision) ON h.subject=wanted.subject AND h.revision=wanted.revision WHERE h.domain=$1 ORDER BY h.subject,h.revision",&[&cohort.domain,&paths,&revisions])?.into_iter().map(|r|((r.get::<_,String>("subject"),r.get::<_,i64>("revision")),r)).collect::<BTreeMap<_,_>>();
        let outputs=tx.query("SELECT * FROM cmd2_history WHERE domain=$1 AND prepare_id=$2 AND commit_seq=$3 ORDER BY subject",&[&cohort.domain,&prepare_id,&as_i64(committed_seq)?])?;
        tx.commit()?;
        let mut total = 0usize;
        for (path, expected_sha, expected_size) in software_inputs {
            let raw = software
                .read_selected_component(components, &path, 8_388_608, deadline, cancelled)
                .map_err(|_| {
                    DurableError::Refused("recovery original software bytes unavailable")
                })?;
            if Digest256::of_bytes(&raw) != expected_sha || raw.len() as u64 != expected_size {
                return Err(DurableError::Corrupt(
                    "recovery original software input differs",
                ));
            }
            retain_context_file(&mut input.context, &mut total, path, raw)?;
        }
        for (path, observed) in &input.observations {
            let row = history
                .get(&(path.clone(), as_i64(observed.custody_revision)?))
                .ok_or(DurableError::Corrupt(
                    "recovery retained observation absent",
                ))?;
            let (metadata, dependencies) = expose_metadata(row, &observed.metadata.path)?;
            if metadata != observed.metadata
                || dependencies != observed.dependencies
                || as_u64(row.get("commit_seq"))? != observed.commit_seq
                || reads.locators.get(path) != Some(&metadata_locator_digest(row))
            {
                return Err(DurableError::Corrupt(
                    "recovery original metadata/locator differs",
                ));
            }
            let raw = self.read_retained_source_row(store, cohort, row, deadline, cancelled)?;
            retain_context_file(
                &mut input.context,
                &mut total,
                observed.metadata.path.clone(),
                raw,
            )?;
        }
        input.context.files.sort_by(|a, b| a.path.cmp(&b.path));
        input.context.check().map_err(source_error)?;
        if let Some(expected) = &input.inventory {
            let mut projection_tx = self.client.transaction()?;
            projection_tx.batch_execute("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY; SET LOCAL statement_timeout='15s'")?;
            let (actual, _) = retained_agent_inventory(
                &mut projection_tx,
                &cohort.domain,
                input.basis.generation,
                &input.context,
                deadline,
                cancelled,
            )?;
            if &actual != expected {
                return Err(DurableError::Corrupt(
                    "original retained Agent projection differs",
                ));
            }
            projection_tx.commit()?;
        }
        let config = cmd::parse(&input.context.configuration_raw).map_err(source_error)?;
        let source_path = cmd::text(&config, "source_path").map_err(source_error)?;
        let home = source_path
            .strip_suffix("/agent.json")
            .ok_or(DurableError::Refused(
                "recovery is limited to actual Agent home",
            ))?;
        RelativePath::parse(home).map_err(|_| DurableError::Corrupt("original Agent home path"))?;
        let absent = reads
            .reads
            .iter()
            .filter_map(|r| match r {
                PredicateRead::Absent { namespace, key } if namespace == "source" => {
                    Some(key.as_str())
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        if outputs.is_empty() || outputs.len() > MAX_MEMBERS || absent.len() != outputs.len() {
            return Err(DurableError::Corrupt(
                "recovery complete output membership differs",
            ));
        }
        let mut original_files = BTreeMap::new();
        let mut output_bytes = 0usize;
        for row in outputs {
            let path: String = row.get("subject");
            if !absent.contains(path.as_str())
                || row.get::<_, String>("profile_id").as_bytes() != CREATION
                || as_u64(row.get("revision"))? != 1
            {
                return Err(DurableError::Corrupt(
                    "recovery output outside registered creation delta",
                ));
            }
            let name = path
                .strip_prefix(&format!("{home}/"))
                .ok_or(DurableError::Corrupt("recovery output escapes Agent home"))?
                .to_owned();
            RelativePath::parse(&name)
                .map_err(|_| DurableError::Corrupt("recovery output relative path"))?;
            let raw = self.read_retained_source_row(store, cohort, &row, deadline, cancelled)?;
            output_bytes = output_bytes
                .checked_add(raw.len())
                .filter(|n| *n <= cmd::SELECTED_SOURCE_MAX_BYTES)
                .ok_or(DurableError::Refused(
                    "recovery outputs exceed existing byte bound",
                ))?;
            if original_files.insert(name, raw).is_some() {
                return Err(DurableError::Corrupt("recovery duplicate output path"));
            }
        }
        let input = ManagedCreationInput {
            context: input.context,
            basis: input.basis,
            observations: input.observations,
            components: components.clone(),
            inventory: input.inventory,
        };
        let package = crate::source_creation::reprepare_managed_agent_creation(
            input,
            schema_cut,
            software,
            components,
            original_files,
            worker,
            deadline,
            cancelled,
        )
        .map_err(source_error)?;
        // Same producer and registered delta/index/original companion reconcile.
        let restored = self.reopen_committed_managed_creation_attempt(
            store, cohort, prepare_id, &package, worker, deadline, cancelled,
        )?;
        finish_worker(worker, deadline, cancelled)?;
        drop(custody);
        self.commit_managed_source_creation(
            store,
            &restored,
            &package,
            filesystem,
            contract,
            rule_version,
            rights_version,
            job_id,
            job_fence,
            deadline,
            cancelled,
        )
    }

    fn read_retained_source_row(
        &mut self,
        store: &SegmentStore,
        cohort: &ManagedSourceCohort,
        expected: &postgres::Row,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Vec<u8>> {
        active(deadline, cancelled)?;
        let path: String = expected.get("subject");
        let revision = as_u64(expected.get("revision"))?;
        if as_u64(expected.get("content_length"))? > 8_388_608 {
            return Err(DurableError::Refused(
                "retained source member exceeds existing cap",
            ));
        }
        let recovered = self.cold_recover_exact(store, &cohort.domain, &path, revision)?;
        let custody = store.hold_audit_root()?;
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'")?;
        let fence = tx.query_one(
            "SELECT maintenance_state FROM cmd2_audit_fence WHERE domain=$1 FOR SHARE",
            &[&cohort.domain],
        )?;
        if fence.get::<_, String>(0) != "normal" {
            return Err(DurableError::Refused("retained source maintenance active"));
        }
        let domain = tx.query_one(
            "SELECT * FROM cmd2_domain WHERE domain=$1 FOR SHARE",
            &[&cohort.domain],
        )?;
        cohort_matches(&domain, cohort, true)?;
        if !domain.get::<_, bool>("rights_allowed") {
            return Err(DurableError::Refused(
                "retained source current rights revoked",
            ));
        }
        let row = tx.query_one(
            "SELECT * FROM cmd2_history WHERE domain=$1 AND subject=$2 AND revision=$3",
            &[&cohort.domain, &path, &as_i64(revision)?],
        )?;
        if metadata_locator_digest(&row) != metadata_locator_digest(expected) {
            return Err(DurableError::Conflict("retained source locator changed"));
        }
        check_history_locator(&row, &recovered.receipt, &cohort.domain, &path, revision)?;
        let mut raw = Vec::new();
        store.read_selected(&recovered.receipt, 8_388_608, &mut raw)?;
        active(deadline, cancelled)?;
        tx.commit()?;
        drop(custody);
        Ok(raw)
    }

    pub fn reopen_committed_managed_creation_attempt(
        &mut self,
        store: &SegmentStore,
        cohort: &ManagedSourceCohort,
        prepare_id: &[u8],
        package: &ManagedSerializedCreation,
        worker: &mut CutWorkerSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<SourceCreationAttempt> {
        self.register_source_creation_bound(
            store,
            cohort,
            prepare_id,
            CreationPackage::Managed(package),
            SourceRegistrationBasis::CommittedReplay,
            worker,
            deadline,
            cancelled,
        )
    }

    /// Recover only an already committed exact package, including one prepared
    /// from a retained later source revision. Current owner/rule/rights/lease
    /// checks still run at replay; this route cannot register a new write.
    /// Its highest caller finishes the schema operation before replay commit.
    pub fn reopen_committed_source_creation_attempt(
        &mut self,
        store: &SegmentStore,
        cohort: &ManagedSourceCohort,
        prepare_id: &[u8],
        package: &SerializedCreation,
        worker: &mut CutWorkerSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<SourceCreationAttempt> {
        self.register_source_creation_bound(
            store,
            cohort,
            prepare_id,
            CreationPackage::V1(package),
            SourceRegistrationBasis::CommittedReplay,
            worker,
            deadline,
            cancelled,
        )
    }

    fn register_source_creation_bound(
        &mut self,
        store: &SegmentStore,
        cohort: &ManagedSourceCohort,
        prepare_id: &[u8],
        package: CreationPackage<'_>,
        basis: SourceRegistrationBasis<'_>,
        worker: &mut CutWorkerSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<SourceCreationAttempt> {
        active(deadline, cancelled)?;
        if store.custody_domain() != cohort.domain.as_bytes() || store.store_id() != cohort.store_id
        {
            return Err(DurableError::Conflict("source creation carrier differs"));
        }
        if let SourceRegistrationBasis::Managed(generation) = &basis {
            generation
                .selected()
                .view()
                .audited_root
                .require_store(store)?;
        }
        let indexes = creation_indexes(package, worker, deadline, cancelled)?;
        let projections = creation_projections(package, worker, deadline, cancelled)?;
        let registered_original = managed_original(package);
        let request =
            cmd::parse(&package.prepared().context().request_raw).map_err(source_error)?;
        let command_id = cmd::text(&request, "command_id").map_err(source_error)?;
        let raw_request_digest = Digest256::of_bytes(&package.prepared().context().request_raw);
        let domain = cohort.domain.as_str();
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'")?;
        let audit_generation = lock_audit_fence(&mut tx, domain)?;
        let row = tx.query_one("SELECT * FROM cmd2_domain WHERE domain=$1", &[&domain])?;
        cohort_matches(&row, cohort, true)?;
        if let SourceRegistrationBasis::Current(current) = &basis {
            if as_u64(row.get("head_seq"))? != cohort.generation
                || row.get::<_, Option<i64>>("source_generation")
                    != Some(as_i64(cohort.generation)?)
                || row.get::<_, Option<String>>("selected_generation_digest")
                    != Some(current.selected_digest().to_hex())
            {
                return Err(DurableError::Conflict(
                    "selected current source generation changed",
                ));
            }
        }
        if let SourceRegistrationBasis::Managed(generation) = &basis {
            if package.managed_basis() != Some(&ManagedCreationBasis::from_generation(generation))
                || as_u64(row.get("head_seq"))? != generation.commit_seq()
                || row.get::<_, Option<i64>>("source_generation")
                    != Some(as_i64(generation.commit_seq())?)
                || row.get::<_, Option<String>>("selected_generation_digest")
                    != Some(generation.digest().to_hex())
                || package.inventory().is_some_and(|inventory| {
                    row.get::<_, Option<String>>("source_projection_digest")
                        != Some(inventory.root.to_hex())
                })
            {
                return Err(DurableError::Conflict(
                    "managed proposal generation changed",
                ));
            }
        }
        let existing = tx.query_opt(
            "SELECT * FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2 FOR UPDATE",
            &[&domain, &prepare_id],
        )?;
        if let Some(existing) = existing {
            if matches!(basis, SourceRegistrationBasis::CommittedReplay)
                && existing.get::<_, String>("state") != "committed"
            {
                return Err(DurableError::Refused(
                    "source replay requires committed exact attempt",
                ));
            }
            let encoded = existing
                .get::<_, Option<Vec<u8>>>("source_reads")
                .ok_or(DurableError::Corrupt("source attempt read binding absent"))?;
            let reads = decode_reads(&encoded)?;
            let delta = registered_source_delta(package, &reads, &projections)?;
            if existing.get::<_, String>("command_id") != command_id
                || existing.get::<_, String>("raw_request_digest") != raw_request_digest.to_hex()
                || existing.get::<_, String>("delta_digest") != delta.to_hex()
                || existing.get::<_, Option<Vec<u8>>>("source_indexes")
                    != Some(indexes_bytes(&indexes)?)
                || existing.get::<_, Option<Vec<u8>>>("source_projections")
                    != Some(projections_bytes(&projections)?)
                || (existing.get::<_, String>("state") != "committed"
                    && existing.get::<_, Option<i64>>("source_epoch")
                        != Some(as_i64(cohort.epoch)?))
                || existing.get::<_, String>("state") == "aborted"
            {
                return Err(DurableError::Conflict(
                    "source attempt exact identity differs",
                ));
            }
            let fence = as_u64(existing.get("attempt_fence"))?;
            let epoch = as_u64(
                existing
                    .get::<_, Option<i64>>("source_epoch")
                    .ok_or(DurableError::Corrupt("source attempt epoch absent"))?,
            )?;
            tx.commit()?;
            return Ok(SourceCreationAttempt {
                domain: domain.into(),
                prepare_id: prepare_id.into(),
                fence,
                epoch,
                definition: cohort.definition,
                delta,
                reads,
                indexes,
                projections,
                continuation: None,
            });
        }
        if matches!(basis, SourceRegistrationBasis::CommittedReplay) {
            return Err(DurableError::Refused(
                "source replay attempt does not exist",
            ));
        }
        if let SourceRegistrationBasis::Managed(generation) = &basis {
            if audit_generation != generation.audit_generation() {
                return Err(DurableError::Conflict(
                    "managed inventory audit fence changed before registration",
                ));
            }
        }
        let mut reads = SourceReads {
            reads: Vec::new(),
            locators: BTreeMap::new(),
            managed_original: registered_original,
        };
        let dependency_paths = package
            .reads()
            .iter()
            .filter(|r| r.path.as_str().starts_with("ToS/"))
            .map(|r| r.path.as_str())
            .collect::<Vec<_>>();
        let current_dependencies = tx.query(
            "SELECT * FROM cmd2_current WHERE domain=$1 AND subject=ANY($2)",
            &[&domain, &dependency_paths],
        )?;
        let current_dependencies = current_dependencies
            .into_iter()
            .map(|r| (r.get::<_, String>("subject"), r))
            .collect::<BTreeMap<_, _>>();
        let whole_membership_differs = if package.inventory().is_none() {
            let count: i64 = tx
                .query_one(
                    "SELECT count(*) FROM cmd2_current WHERE domain=$1",
                    &[&domain],
                )?
                .get(0);
            as_u64(count)? != dependency_paths.len() as u64
        } else {
            false
        };
        if current_dependencies.len() != dependency_paths.len() || whole_membership_differs {
            return Err(DurableError::Conflict(
                "maintained complete source inventory changed since preparation",
            ));
        }
        for dependency in package.reads() {
            if !dependency.path.as_str().starts_with("ToS/") {
                continue;
            }
            let key = dependency.path.as_str();
            let current = current_dependencies
                .get(key)
                .ok_or(DurableError::Conflict("exact source dependency missing"))?;
            if current.get::<_, String>("content_digest") != dependency.raw_sha256.to_hex() {
                return Err(DurableError::Conflict("exact source dependency changed"));
            }
            if let Some(observations) = package.observations() {
                let observed = observations.get(key).ok_or(DurableError::Conflict(
                    "managed dependency observation missing",
                ))?;
                if let SourceRegistrationBasis::Managed(generation) = &basis {
                    let selected = generation
                        .selected()
                        .lookup_current(domain, &observed.metadata.path, deadline, cancelled)?
                        .ok_or(DurableError::Conflict("managed selected dependency absent"))?;
                    if selected_source_metadata(
                        current,
                        &selected,
                        domain,
                        &observed.metadata.path,
                    )? != observed.metadata
                    {
                        return Err(DurableError::Conflict(
                            "managed selected dependency differs",
                        ));
                    }
                }
                let actual_metadata = row_source_metadata(current)?;
                let dependencies = observed.dependencies.as_ref().map(|paths| {
                    paths
                        .iter()
                        .map(|path| path.as_str().to_owned())
                        .collect::<Vec<_>>()
                });
                if actual_metadata.mode != observed.metadata.mode
                    || actual_metadata.dependencies != dependencies
                    || as_u64(current.get("revision"))? != observed.custody_revision
                    || as_u64(current.get("commit_seq"))? != observed.commit_seq
                {
                    return Err(DurableError::Conflict(
                        "managed dependency metadata changed",
                    ));
                }
            }
            reads
                .locators
                .insert(key.into(), metadata_locator_digest(current));
            reads.reads.push(PredicateRead::Exact {
                namespace: "source".into(),
                key: key.into(),
                expected_version: Some(as_u64(current.get("revision"))?),
                expected_digest: Some(dependency.raw_sha256),
            });
        }
        for change in package.changes() {
            if tx
                .query_opt(
                    "SELECT 1 FROM cmd2_current WHERE domain=$1 AND subject=$2",
                    &[&domain, &change.path.as_str()],
                )?
                .is_some()
            {
                return Err(DurableError::Conflict(
                    "creation source member already exists",
                ));
            }
            reads.reads.push(PredicateRead::Absent {
                namespace: "source".into(),
                key: change.path.as_str().into(),
            });
        }
        let home = package.prepared().home().as_str();
        if tx.query_opt("SELECT 1 FROM cmd2_current WHERE domain=$1 AND (subject=$2 OR left(subject,length($2)+1)=$2||'/') LIMIT 1", &[&domain,&home])?.is_some() { return Err(DurableError::Conflict("complete source home range is occupied")); }
        let mut observed = BTreeSet::from([(
            "range".to_owned(),
            "source-inventory".to_owned(),
            "all".to_owned(),
        )]);
        observed.insert((
            "range".to_owned(),
            "source-home".to_owned(),
            home.to_owned(),
        ));
        for (kind, token, _) in &indexes {
            if kind != "path" && tx.query_opt("SELECT 1 FROM cmd2_source_index WHERE domain=$1 AND kind<>'path' AND token=$2 LIMIT 1", &[&domain,token])?.is_some() {
                return Err(DurableError::Conflict("stable source identity already exists"));
            }
            if tx
                .query_opt(
                    "SELECT 1 FROM cmd2_source_index WHERE domain=$1 AND kind=$2 AND token=$3",
                    &[&domain, kind, token],
                )?
                .is_some()
            {
                return Err(DurableError::Conflict(
                    "complete owner identity index is occupied",
                ));
            }
            observed.insert(("unique".to_owned(), kind.clone(), token.clone()));
        }
        for (kind, scope, token) in observed {
            tx.execute("INSERT INTO cmd2_predicate(domain,kind,owner,scope,token,definition_version,generation,complete) VALUES($1,$2,$3,$4,$5,$6,0,true) ON CONFLICT DO NOTHING", &[&domain,&kind,&OWNER,&scope,&token,&cohort.definition.to_hex()])?;
            let row = tx.query_one("SELECT generation,complete,definition_version FROM cmd2_predicate WHERE domain=$1 AND kind=$2 AND owner=$3 AND scope=$4 AND token=$5", &[&domain,&kind,&OWNER,&scope,&token])?;
            if !row.get::<_, bool>(1) || row.get::<_, String>(2) != cohort.definition.to_hex() {
                return Err(DurableError::Refused(
                    "source predicate lacks exact complete definition",
                ));
            }
            reads.reads.push(PredicateRead::Generation {
                predicate: PredicateToken {
                    kind: if kind == "range" {
                        PredicateKind::Range
                    } else {
                        PredicateKind::Unique
                    },
                    owner: OWNER.into(),
                    scope,
                    token,
                    definition_version: cohort.definition.to_hex(),
                },
                observed_generation: as_u64(row.get(0))?,
            });
        }
        let delta = registered_source_delta(package, &reads, &projections)?;
        tx.execute("INSERT INTO cmd2_attempt(domain,prepare_id,command_id,raw_request_digest,delta_digest,state,attempt_fence,source_reads,source_indexes,source_epoch,source_projections) VALUES($1,$2,$3,$4,$5,'registered',1,$6,$7,$8,$9)",
            &[&domain,&prepare_id,&command_id,&raw_request_digest.to_hex(),&delta.to_hex(),&reads_bytes(&reads)?,&indexes_bytes(&indexes)?,&as_i64(cohort.epoch)?,&projections_bytes(&projections)?])?;
        if reads.managed_original.is_some() {
            let measured:i32=tx.query_one("SELECT octet_length(row_to_json(a)::text) FROM cmd2_attempt a WHERE domain=$1 AND prepare_id=$2", &[&domain,&prepare_id])?.get(0);
            if measured > 1_048_576 {
                return Err(DurableError::Refused(
                    "managed attempt exceeds existing cold metadata row bound",
                ));
            }
        }
        let continuation = match &basis {
            SourceRegistrationBasis::Managed(generation) if package.inventory().is_some() => {
                Some(WarmContinuation {
                    parent: generation.selected().clone(),
                    fence: std::cell::Cell::new(lock_audit_fence(&mut tx, domain)?),
                    committed: std::cell::RefCell::new(None),
                })
            }
            _ => None,
        };
        tx.commit()?;
        Ok(SourceCreationAttempt {
            domain: domain.into(),
            prepare_id: prepare_id.into(),
            fence: 1,
            epoch: cohort.epoch,
            definition: cohort.definition,
            delta,
            reads,
            indexes,
            projections,
            continuation,
        })
    }

    /// Consume the actual maintained serialized Agent package. Proposals stay
    /// bound to their immutable original cut; current absence/uniqueness and
    /// dependencies come only from the verified managed CURRENT generation.
    pub fn commit_source_creation(
        &mut self,
        store: &SegmentStore,
        attempt: &SourceCreationAttempt,
        package: &SerializedCreation,
        filesystem: &CreationFilesystem,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<(DurableCommitReceipt, DurableTiming)> {
        self.commit_creation_package(
            store,
            attempt,
            CreationPackage::V1(package),
            filesystem,
            contract,
            rule_version,
            rights_version,
            job_id,
            job_fence,
            deadline,
            cancelled,
        )
    }
    pub fn commit_managed_source_creation(
        &mut self,
        store: &SegmentStore,
        attempt: &SourceCreationAttempt,
        package: &ManagedSerializedCreation,
        filesystem: &CreationFilesystem,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<(DurableCommitReceipt, DurableTiming)> {
        self.commit_creation_package(
            store,
            attempt,
            CreationPackage::Managed(package),
            filesystem,
            contract,
            rule_version,
            rights_version,
            job_id,
            job_fence,
            deadline,
            cancelled,
        )
    }
    fn commit_creation_package(
        &mut self,
        store: &SegmentStore,
        attempt: &SourceCreationAttempt,
        package: CreationPackage<'_>,
        filesystem: &CreationFilesystem,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<(DurableCommitReceipt, DurableTiming)> {
        active(deadline, cancelled)?;
        if attempt.delta != registered_source_delta(package, &attempt.reads, &attempt.projections)?
            || attempt.definition != definition()
            || store.custody_domain() != attempt.domain.as_bytes()
        {
            return Err(DurableError::Conflict("registered source package changed"));
        }
        if let Some(continuation) = &attempt.continuation {
            continuation
                .parent
                .view()
                .audited_root
                .require_store(store)?;
        }
        // Acquire the real maintained owner mutex before STO and PG locks.
        let owner = filesystem
            .hold_creation_owner(package, deadline, cancelled)
            .map_err(source_error)?;
        let identities = package
            .changes()
            .iter()
            .enumerate()
            .map(|(slot, c)| ShadowWriteIdentity {
                member_slot: slot as u32,
                subject: c.path.as_str(),
                expected_predecessor: None,
                proposed_revision: 1,
                exact_bytes: c.after.as_ref().unwrap(),
            })
            .collect::<Vec<_>>();
        let state = self.resolve_attempt(&attempt.domain, &attempt.prepare_id)?;
        let receipts = match state {
            AttemptResolution::Registered => {
                let members =
                    match store.recover_attempt_fenced(&attempt.prepare_id, attempt.fence, 0)? {
                        None => seal_members(
                            store,
                            &attempt.prepare_id,
                            attempt.fence,
                            CREATION,
                            &identities,
                        )?,
                        Some(AttemptRecovery::Sealed { receipts }) => {
                            members_from_receipts(&identities, receipts)?
                        }
                        _ => {
                            return Err(DurableError::Indeterminate(
                                "source seal retains uncertain custody",
                            ));
                        }
                    };
                let audit = self.attach_ready_profile_bound(
                    store,
                    &attempt.domain,
                    &attempt.prepare_id,
                    attempt.fence,
                    &members,
                    CREATION,
                    Some(attempt.delta),
                    Some(&creation_metadata(package)),
                    attempt
                        .continuation
                        .as_ref()
                        .map(|continuation| continuation.fence.get()),
                )?;
                if let Some(continuation) = &attempt.continuation {
                    continuation.fence.set(audit);
                }
                members.into_iter().map(|m| m.receipt).collect::<Vec<_>>()
            }
            AttemptResolution::Ready | AttemptResolution::Committed(_) => {
                match store.recover_attempt_fenced(&attempt.prepare_id, attempt.fence, 0)? {
                    Some(AttemptRecovery::Sealed { receipts }) => {
                        let members = members_from_receipts(&identities, receipts)?;
                        for member in &members {
                            check_durable_member(
                                &attempt.domain,
                                &attempt.prepare_id,
                                member,
                                CREATION,
                            )?;
                        }
                        members.into_iter().map(|m| m.receipt).collect()
                    }
                    _ => {
                        return Err(DurableError::Corrupt(
                            "source attempt exact intent/bytes absent",
                        ));
                    }
                }
            }
            AttemptResolution::Aborted => {
                return Err(DurableError::Refused("source attempt is aborted"));
            }
        };
        self.commit_durable(
            store,
            &CommitShadowAttempt {
                domain: &attempt.domain,
                prepare_id: &attempt.prepare_id,
                attempt_fence: attempt.fence,
                receipts: &receipts,
                expected_contract_digest: contract,
                expected_rule_version: rule_version,
                expected_rights_version: rights_version,
                job_id,
                job_fence,
                full_base_seq: 0,
            },
            CommitMode::Creation {
                attempt,
                owner: &owner,
                deadline,
                cancelled,
            },
        )
    }

    /// Reverify the ORIGINAL bootstrap plus exact CURRENT bodies/indexes and
    /// all retained CMD/STO history before selecting an affected basis again.
    /// Finalizes the highest schema operation before completeness publication;
    /// callers must not finalize this worker again.
    /// Returns a managed generation and current membership, never labels the
    /// initial SourceRevision as the current contents or as a new source cut.
    pub fn cold_reopen_source_cohort(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        original: &CorpusCutReader,
        initial_revision: SourceRevision,
        initial_membership: SourceMembershipV1,
        context: &CommandContext,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        worker: &mut CutWorkerSchemaExecutor,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<ReopenedSourceCohort> {
        self.cold_reopen_source_cohort_inner(
            store,
            domain,
            original,
            initial_revision,
            initial_membership,
            context,
            software,
            components,
            worker,
            None,
            deadline,
            cancelled,
        )
    }

    /// Opt-in external-sort custody at the real source-cohort generation
    /// boundary. The upstream authored-file/owner-index validation remains
    /// finite and is not a billion-record capacity claim.
    pub fn cold_reopen_source_cohort_streamed(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        original: &CorpusCutReader,
        initial_revision: SourceRevision,
        initial_membership: SourceMembershipV1,
        context: &CommandContext,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        worker: &mut CutWorkerSchemaExecutor,
        workspace: &super::PrivateGenerationWorkspace,
        profile: super::StreamedGenerationProfile,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<ReopenedSourceCohort> {
        let profile = profile.validate(workspace)?;
        self.cold_reopen_source_cohort_inner(
            store,
            domain,
            original,
            initial_revision,
            initial_membership,
            context,
            software,
            components,
            worker,
            Some((workspace, profile)),
            deadline,
            cancelled,
        )
    }

    fn cold_reopen_source_cohort_inner(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        original: &CorpusCutReader,
        initial_revision: SourceRevision,
        initial_membership: SourceMembershipV1,
        context: &CommandContext,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        worker: &mut CutWorkerSchemaExecutor,
        streamed: Option<(
            &super::PrivateGenerationWorkspace,
            super::StreamedGenerationProfile,
        )>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<ReopenedSourceCohort> {
        active(deadline, cancelled)?;
        if original.current().revision() != initial_revision
            || context.base_revision != initial_revision
            || store.custody_domain() != domain.as_bytes()
        {
            return Err(DurableError::Conflict(
                "cold original source selection differs",
            ));
        }
        context
            .check_from_selected_captures(original, software, components, deadline, cancelled)
            .map_err(source_error)?;
        let mut original_stream = original
            .stream(initial_revision)
            .map_err(|_| DurableError::Refused("original bootstrap stream"))?;
        if original_stream.expectation() != initial_membership {
            return Err(DurableError::Conflict("cold original membership differs"));
        }
        let mut expected = BTreeMap::new();
        let original_total = original
            .current()
            .members()
            .try_fold(0u64, |n, m| n.checked_add(m.size_bytes));
        if original.current().member_count() > 4096 || original_total.is_none_or(|n| n > 33_554_432)
        {
            return Err(DurableError::Refused(
                "cold original membership exceeds existing source bounds",
            ));
        }
        let before = match streamed {
            Some((workspace, profile)) => self
                .cold_verify_cut_streamed(store, domain, workspace, profile, deadline, cancelled)?,
            None => self.cold_verify_cut_with_budget(store, domain, Some((deadline, cancelled)))?,
        };
        let mut metadata_tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        metadata_tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '60s'; SET LOCAL work_mem = '4MB'")?;
        if let Some((_, profile)) = streamed {
            metadata_tx.query_one(
                "SELECT set_config('temp_file_limit',$1,true)",
                &[&format!("{}kB", profile.max_pg_temp_bytes / 1024)],
            )?;
            metadata_tx.query_one(
                "SELECT set_config('statement_timeout',$1,true)",
                &[&format!("{}ms", profile.max_sql_statement_ms)],
            )?;
        }
        let domain_row = metadata_tx.query_one(
            "SELECT d.*,f.generation AS audit_generation,f.maintenance_state
             FROM cmd2_domain d JOIN cmd2_audit_fence f USING(domain) WHERE d.domain=$1",
            &[&domain],
        )?;
        if as_u64(domain_row.get("head_seq"))? != before.through_commit_seq
            || as_u64(domain_row.get("audit_generation"))? != before.audit_generation
            || domain_row.get::<_, String>("maintenance_state") != "normal"
        {
            return Err(DurableError::Conflict(
                "cold source metadata snapshot changed",
            ));
        }
        // The returned maps and CommandContext remain finite. Refuse the
        // complete current selection before allocating rows or reading bodies.
        let software_count = context
            .files
            .iter()
            .filter(|f| !f.path.as_str().starts_with("ToS/"))
            .count();
        let software_bytes = context
            .files
            .iter()
            .filter(|f| !f.path.as_str().starts_with("ToS/"))
            .try_fold(0u64, |n, f| n.checked_add(f.raw.len() as u64))
            .ok_or(DurableError::Refused(
                "cold software selection byte overflow",
            ))?;
        let admitted = metadata_tx.query_one(
            "SELECT count(*),coalesce(sum(content_length),0)::bigint,
                    coalesce(max(content_length),0)
             FROM cmd2_current WHERE domain=$1",
            &[&domain],
        )?;
        let current_count = as_u64(admitted.get(0))?;
        if current_count != before.current_members
            || current_count
                .checked_add(software_count as u64)
                .is_none_or(|n| n > cmd::SELECTED_SOURCE_MAX_FILES as u64)
            || as_u64(admitted.get(2))? > 8_388_608
            || as_u64(admitted.get(1))?
                .checked_add(software_bytes)
                .is_none_or(|n| n > cmd::SELECTED_SOURCE_MAX_BYTES as u64)
        {
            return Err(DurableError::Refused(
                "current source exceeds existing command selection bounds",
            ));
        }
        let counts = admit_cold_source_metadata(
            &mut metadata_tx,
            domain,
            streamed.map(|(_, p)| p),
            deadline,
            cancelled,
        )?;
        let rows =
            cold_source_current_rows(&mut metadata_tx, domain, counts[0], deadline, cancelled)?;
        let mut actual = IndexRows::new();
        let mut indexed_projections = ProjectionRows::new();
        let mut indexed_projection_count = 0u64;
        cold_source_key_rows(
            &mut metadata_tx,
            domain,
            false,
            counts[1],
            deadline,
            cancelled,
            |row| {
                if row.get::<_, String>("definition_digest") != definition().to_hex() {
                    return Err(DurableError::Corrupt(
                        "current owner index definition differs",
                    ));
                }
                let kind: String = row.get("kind");
                let path: String = row.get("path");
                if kind == "path" {
                    indexed_projection_count += 1;
                    if let Some(projection) = row.get::<_, Option<Vec<u8>>>("inventory_projection")
                    {
                        indexed_projections.insert(path.clone(), projection);
                    }
                }
                if !actual.insert((kind, row.get("token"), path)) {
                    return Err(DurableError::Corrupt("duplicate cold owner index"));
                }
                Ok(())
            },
        )?;
        let mut predicate_keys = BTreeSet::new();
        cold_source_key_rows(
            &mut metadata_tx,
            domain,
            true,
            counts[2],
            deadline,
            cancelled,
            |row| {
                let kind: String = row.get("kind");
                let scope: String = row.get("scope");
                let token: String = row.get("token");
                if row.get::<_, String>("definition_version") != definition().to_hex()
                    || !["unique", "range"].contains(&kind.as_str())
                    || (kind == "range"
                        && !["source-home", "source-inventory"].contains(&scope.as_str()))
                    || (scope == "source-inventory" && token != "all")
                    || (kind == "unique"
                        && !["metadata", "claim", "event", "anchor", "form", "path"]
                            .contains(&scope.as_str()))
                    || !predicate_keys.insert((kind, scope, token))
                {
                    return Err(DurableError::Corrupt(
                        "stored source predicate owner definition differs",
                    ));
                }
                Ok(())
            },
        )?;
        metadata_tx.commit()?;
        while let Some(member) = original_stream
            .next_member(deadline, cancelled)
            .map_err(|_| DurableError::Refused("original cold EOF/fixity failure"))?
        {
            expected.insert(member.path.as_str().to_owned(), member.raw);
        }
        if original_stream.coverage() != Some(initial_membership) {
            return Err(DurableError::Refused("original cold membership incomplete"));
        }
        let cohort = ManagedSourceCohort {
            domain: domain.into(),
            store_id: store.store_id(),
            initial_revision,
            initial_membership,
            epoch: as_u64(
                domain_row
                    .get::<_, Option<i64>>("source_epoch")
                    .ok_or(DurableError::Corrupt("source epoch absent"))?,
            )?,
            definition: definition(),
            generation: before.through_commit_seq,
        };
        cohort_matches(&domain_row, &cohort, false)?;
        let mut ctx = context.clone();
        ctx.files.retain(|f| !f.path.as_str().starts_with("ToS/"));
        let mut total_bytes = ctx.files.iter().map(|f| f.raw.len() as u64).sum::<u64>();
        let mut files = BTreeMap::new();
        let mut metadata = BTreeMap::new();
        let mut dependency_claims = BTreeMap::new();
        let source_metadata = original_metadata(original);
        if rows.len() != before.current_members as usize {
            return Err(DurableError::Conflict(
                "cold current membership changed during verification",
            ));
        }
        let mut last_prepare = Vec::new();
        let mut sealed = Vec::new();
        let mut current_placements = Vec::new();
        for row in &rows {
            active(deadline, cancelled)?;
            let path: String = row.get("subject");
            let size = as_u64(row.get("content_length"))?;
            if ctx.files.len() >= 4096
                || size > 8_388_608
                || total_bytes.checked_add(size).is_none_or(|n| n > 33_554_432)
            {
                return Err(DurableError::Refused(
                    "current source exceeds existing command selection bounds",
                ));
            }
            total_bytes += size;
            let prepare: Vec<u8> = row.get("prepare_id");
            if prepare != last_prepare {
                sealed = match store.recover_attempt_fenced(
                    &prepare,
                    as_u64(row.get("source_attempt_fence"))?,
                    0,
                )? {
                    Some(AttemptRecovery::Sealed { receipts }) => receipts,
                    _ => return Err(DurableError::Corrupt("cold source intent absent")),
                };
                last_prepare = prepare;
            }
            let receipt = sealed
                .iter()
                .find(|r| r.receipt_id().to_hex() == row.get::<_, String>("sto_receipt_id"))
                .ok_or(DurableError::Corrupt("cold current receipt absent"))?;
            check_history_locator(row, receipt, domain, &path, as_u64(row.get("revision"))?)?;
            current_placements.push(PlacementGenerationRowV1 {
                key: membership_key(CURRENT_KEY_TAG, domain, &path, None)?,
                logical_digest: receipt.placement().coordinate().sha256,
                logical_length: receipt.placement().coordinate().size_bytes,
                placement: receipt.placement(),
            });
            let mut raw = Vec::new();
            // Rights are checked before disclosure even for this private full
            // cold operation, and held through each exact selected frame read.
            let mut rights_tx = self.client.transaction()?;
            rights_tx.batch_execute(
                "SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'",
            )?;
            if !rights_tx
                .query_one(
                    "SELECT rights_allowed FROM cmd2_domain WHERE domain=$1 FOR SHARE",
                    &[&domain],
                )?
                .get::<_, bool>(0)
            {
                return Err(DurableError::Refused("cold current rights revoked"));
            }
            store.read_selected(receipt, 8_388_608, &mut raw)?;
            rights_tx.commit()?;
            let carrier = row_source_metadata(row)?;
            if let Some(original_raw) = expected.get(&path) {
                if source_metadata.get(&path) != Some(&carrier) {
                    return Err(DurableError::Corrupt(
                        "original source carrier metadata changed",
                    ));
                }
                if original_raw != &raw || receipt.binding().profile_id != ORIGINAL {
                    return Err(DurableError::Corrupt(
                        "original source bytes/profile changed",
                    ));
                }
            } else if receipt.binding().profile_id != CREATION
                || carrier.mode != 0o644
                || carrier.dependencies.is_none()
            {
                return Err(DurableError::Corrupt(
                    "extra current member is outside controlled Agent writer",
                ));
            }
            let member_path = RelativePath::parse(&path)
                .map_err(|_| DurableError::Corrupt("current source path"))?;
            let (member_metadata, member_dependencies) = expose_metadata(row, &member_path)?;
            metadata.insert(path.clone(), member_metadata);
            dependency_claims.insert(path.clone(), member_dependencies);
            ctx.files.push(SourceFile {
                path: member_path,
                raw: raw.clone(),
            });
            files.insert(path, raw);
        }
        sort_complete_membership(&mut current_placements)?;
        if logical_membership_root(CURRENT_KEY_TAG, &current_placements)
            != before.current_membership_root
        {
            return Err(DurableError::Conflict(
                "cold source current membership root changed",
            ));
        }
        if !expected.keys().all(|p| files.contains_key(p)) {
            return Err(DurableError::Corrupt(
                "original bootstrap membership missing",
            ));
        }
        for paths in dependency_claims.values().flatten() {
            if paths.iter().any(|p| !files.contains_key(p.as_str())) {
                return Err(DurableError::Corrupt(
                    "current dependency claim points outside actual source membership",
                ));
            }
        }
        ctx.check().map_err(source_error)?;
        let indexes = index_rows(&ctx, worker, deadline, cancelled)?;
        let projections = optional_agent_projections(&ctx, worker, deadline, cancelled)?;
        if let Some(projections) = &projections {
            for row in &rows {
                let path: String = row.get("subject");
                if row
                    .get::<_, Option<Vec<u8>>>("inventory_projection")
                    .as_ref()
                    != projections.get(&path)
                {
                    return Err(DurableError::Corrupt(
                        "current Agent projection differs from original body",
                    ));
                }
            }
            if rows.len() != projections.len()
                || rows.iter().any(|row| {
                    row.get::<_, Option<Vec<u8>>>("retained_inventory_projection")
                        .as_ref()
                        != projections.get(&row.get::<_, String>("subject"))
                })
            {
                return Err(DurableError::Corrupt(
                    "retained Agent projection membership differs",
                ));
            }
            if indexed_projection_count != projections.len() as u64
                || &indexed_projections != projections
            {
                return Err(DurableError::Corrupt(
                    "Agent index projection coverage differs",
                ));
            }
        }
        verify_manifest_indexes(original, &indexes)?;
        if indexes != actual {
            return Err(DurableError::Corrupt(
                "current original bodies and complete owner indexes differ",
            ));
        }
        if !predicates(&indexes).is_subset(&predicate_keys) {
            return Err(DurableError::Corrupt(
                "complete source invalidation membership is missing",
            ));
        }
        // Highest schema owner: verification is complete. Final child custody
        // must succeed before any completeness enablement or cut selection.
        finish_worker(worker, deadline, cancelled)?;
        let current_membership = membership_of_files(&files);
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'")?;
        if lock_audit_fence(&mut tx, domain)? != before.audit_generation {
            return Err(DurableError::Conflict(
                "source verification raced metadata mutation",
            ));
        }
        let row = tx.query_one(
            "SELECT * FROM cmd2_domain WHERE domain=$1 FOR UPDATE",
            &[&domain],
        )?;
        cohort_matches(&row, &cohort, false)?;
        if as_u64(row.get("head_seq"))? != before.through_commit_seq {
            return Err(DurableError::Conflict("source cold head changed"));
        }
        // Existing empty/absent predicates must be verified and re-enabled too.
        tx.execute("UPDATE cmd2_predicate SET complete=true WHERE domain=$1 AND owner=$2 AND definition_version=$3", &[&domain,&OWNER,&cohort.definition.to_hex()])?;
        let projection_digest = projections
            .as_ref()
            .map(|rows| projection_root(rows).to_hex());
        tx.execute("UPDATE cmd2_domain SET source_complete=true,source_generation=head_seq,source_projection_digest=$2,selected_generation_digest=NULL,complete_cut_digest=NULL,complete_cut_generation=NULL WHERE domain=$1", &[&domain,&projection_digest])?;
        tx.commit()?;
        let verified = match streamed {
            Some((workspace, profile)) => self
                .cold_verify_cut_streamed(store, domain, workspace, profile, deadline, cancelled)?,
            None => self.cold_verify_cut_with_budget(store, domain, Some((deadline, cancelled)))?,
        };
        let generation = self.build_complete_generation(store, &verified, deadline, cancelled)?;
        self.select_complete_generation(&generation)?;
        let selected = match streamed {
            Some((workspace, profile)) => self.cold_open_selected_generation_streamed(
                store, domain, workspace, profile, deadline, cancelled,
            )?,
            None => self.cold_open_selected_generation(store, domain, deadline, cancelled)?,
        };
        Ok(ReopenedSourceCohort {
            cohort,
            files,
            selected,
            current_membership,
            indexes: indexes.into_iter().collect(),
            metadata,
            dependency_claims,
        })
    }
}

// Small PG pages are separate from the finite complete maps returned to the
// creation owner. Neither a wider generation profile nor streaming custody
// raises the existing command input or finite owner-metadata ceilings.
const COLD_SOURCE_PAGE_ROWS: i64 = 8;
fn admit_cold_source_metadata(
    tx: &mut Transaction<'_>,
    domain: &str,
    profile: Option<StreamedGenerationProfile>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<[u64; 3]> {
    let mut counts = [0; 3];
    let mut rows = 0u64;
    let mut bytes = 0u64;
    for (i, table) in ["cmd2_current", "cmd2_source_index", "cmd2_predicate"]
        .into_iter()
        .enumerate()
    {
        active(deadline, cancelled)?;
        let query = format!(
            "SELECT count(*),coalesce(max(octet_length(row_to_json(t)::text)),0),
                    coalesce(sum(octet_length(row_to_json(t)::text)),0)::bigint
             FROM {table} t WHERE domain=$1"
        );
        let admitted = tx.query_one(&query, &[&domain])?;
        counts[i] = as_u64(admitted.get(0))?;
        rows = rows
            .checked_add(counts[i])
            .ok_or(DurableError::Refused("cold source metadata row overflow"))?;
        bytes = bytes
            .checked_add(as_u64(admitted.get(2))?)
            .ok_or(DurableError::Refused("cold source metadata byte overflow"))?;
        if rows > profile.map_or(100_000, |p| p.max_metadata_rows.min(100_000)) as u64
            || bytes
                > profile.map_or(64 * 1024 * 1024, |p| {
                    p.max_metadata_bytes.min(64 * 1024 * 1024)
                }) as u64
            || admitted.get::<_, i32>(1) > 1_048_576
        {
            return Err(DurableError::Refused(
                "cold source metadata preadmission exceeded",
            ));
        }
    }
    // Predicates belonging to another owner are included in the resource
    // accounting above, but only this owner's complete keys are consumed.
    counts[2] = as_u64(
        tx.query_one(
            "SELECT count(*) FROM cmd2_predicate WHERE domain=$1 AND owner=$2",
            &[&domain, &OWNER],
        )?
        .get(0),
    )?;
    Ok(counts)
}

fn cold_source_current_rows(
    tx: &mut Transaction<'_>,
    domain: &str,
    count: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<Vec<postgres::Row>> {
    let mut rows = Vec::new();
    let mut after = (Vec::<u8>::new(), -1i32, String::new());
    loop {
        active(deadline, cancelled)?;
        let page = tx.query(
            "SELECT c.*,a.attempt_fence AS source_attempt_fence,
                    h.inventory_projection AS retained_inventory_projection
             FROM cmd2_current c JOIN cmd2_attempt a USING(domain,prepare_id)
             LEFT JOIN cmd2_history h USING(domain,subject,revision)
             WHERE c.domain=$1 AND
               (c.prepare_id,c.member_slot,c.subject COLLATE \"C\") > ($2,$3,$4 COLLATE \"C\")
             ORDER BY c.prepare_id,c.member_slot,c.subject COLLATE \"C\" LIMIT $5",
            &[
                &domain,
                &after.0,
                &after.1,
                &after.2,
                &COLD_SOURCE_PAGE_ROWS,
            ],
        )?;
        if page.is_empty() {
            break;
        }
        for row in page {
            active(deadline, cancelled)?;
            let key = (
                row.get::<_, Vec<u8>>("prepare_id"),
                row.get::<_, i32>("member_slot"),
                row.get::<_, String>("subject"),
            );
            if key <= after || rows.len() as u64 >= count {
                return Err(DurableError::Corrupt(
                    "cold current keyset order/count differs",
                ));
            }
            after = key;
            rows.push(row);
        }
    }
    if rows.len() as u64 != count {
        return Err(DurableError::Corrupt("cold current EOF count differs"));
    }
    Ok(rows)
}

fn cold_source_key_rows(
    tx: &mut Transaction<'_>,
    domain: &str,
    predicate: bool,
    count: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
    mut consume: impl FnMut(&postgres::Row) -> DurableResult<()>,
) -> DurableResult<()> {
    let mut after: Option<(String, String, String)> = None;
    let mut seen = 0u64;
    loop {
        active(deadline, cancelled)?;
        let page = match (predicate, after.as_ref()) {
            (false, None) => tx.query(
                "SELECT * FROM cmd2_source_index WHERE domain=$1
                 ORDER BY kind COLLATE \"C\",token COLLATE \"C\",path COLLATE \"C\" LIMIT $2",
                &[&domain, &COLD_SOURCE_PAGE_ROWS],
            )?,
            (false, Some(key)) => tx.query(
                "SELECT * FROM cmd2_source_index WHERE domain=$1 AND
                   (kind COLLATE \"C\",token COLLATE \"C\",path COLLATE \"C\") >
                   ($2 COLLATE \"C\",$3 COLLATE \"C\",$4 COLLATE \"C\")
                 ORDER BY kind COLLATE \"C\",token COLLATE \"C\",path COLLATE \"C\" LIMIT $5",
                &[&domain, &key.0, &key.1, &key.2, &COLD_SOURCE_PAGE_ROWS],
            )?,
            (true, None) => tx.query(
                "SELECT * FROM cmd2_predicate WHERE domain=$1 AND owner=$2
                 ORDER BY kind COLLATE \"C\",scope COLLATE \"C\",token COLLATE \"C\" LIMIT $3",
                &[&domain, &OWNER, &COLD_SOURCE_PAGE_ROWS],
            )?,
            (true, Some(key)) => tx.query(
                "SELECT * FROM cmd2_predicate WHERE domain=$1 AND owner=$2 AND
                   (kind COLLATE \"C\",scope COLLATE \"C\",token COLLATE \"C\") >
                   ($3 COLLATE \"C\",$4 COLLATE \"C\",$5 COLLATE \"C\")
                 ORDER BY kind COLLATE \"C\",scope COLLATE \"C\",token COLLATE \"C\" LIMIT $6",
                &[
                    &domain,
                    &OWNER,
                    &key.0,
                    &key.1,
                    &key.2,
                    &COLD_SOURCE_PAGE_ROWS,
                ],
            )?,
        };
        if page.is_empty() {
            break;
        }
        for row in page {
            active(deadline, cancelled)?;
            let key = if predicate {
                (row.get("kind"), row.get("scope"), row.get("token"))
            } else {
                (row.get("kind"), row.get("token"), row.get("path"))
            };
            if after.as_ref().is_some_and(|previous| previous >= &key) || seen >= count {
                return Err(DurableError::Corrupt(
                    "cold source keyset order/count differs",
                ));
            }
            consume(&row)?;
            after = Some(key);
            seen += 1;
        }
    }
    if seen != count {
        return Err(DurableError::Corrupt(
            "cold source keyset EOF count differs",
        ));
    }
    Ok(())
}

fn membership_of_files(files: &BTreeMap<String, Vec<u8>>) -> SourceMembershipV1 {
    let mut h = Digest256Hasher::new();
    h.update(b"tos-val-full-membership-v1\0");
    for (path, raw) in files {
        h.update(&(path.len() as u64).to_be_bytes());
        h.update(path.as_bytes());
        h.update(&(raw.len() as u64).to_be_bytes());
        h.update(Digest256::of_bytes(raw).as_bytes());
    }
    SourceMembershipV1 {
        count: files.len() as u64,
        digest: h.finalize(),
    }
}

fn members_from_receipts(
    identities: &[ShadowWriteIdentity<'_>],
    receipts: Vec<ByteDurabilityReceipt>,
) -> DurableResult<Vec<DurableShadowMember>> {
    if receipts.len() != identities.len() {
        return Err(DurableError::Corrupt("source receipt count differs"));
    }
    identities
        .iter()
        .map(|m| {
            let receipt = receipts
                .iter()
                .find(|r| r.binding().member_slot == m.member_slot)
                .ok_or(DurableError::Corrupt("source receipt slot absent"))?
                .clone();
            Ok(DurableShadowMember {
                member_slot: m.member_slot,
                subject: m.subject.into(),
                expected_predecessor: m.expected_predecessor,
                proposed_revision: m.proposed_revision,
                exact_bytes: m.exact_bytes.into(),
                receipt,
            })
        })
        .collect()
}

pub(super) fn check_commit_owner(
    tx: &mut Transaction<'_>,
    request: &CommitShadowAttempt<'_>,
    registered: &postgres::Row,
    members: &[postgres::Row],
    mode: &CommitMode<'_>,
    replayed: bool,
) -> DurableResult<()> {
    match mode {
        CommitMode::Shadow => check_writer_profile(tx, request.domain, PROFILE_ID),
        CommitMode::Bootstrap => {
            check_writer_profile(tx, request.domain, ORIGINAL)?;
            if members
                .iter()
                .any(|m| m.get::<_, String>("profile_id").as_bytes() != ORIGINAL)
            {
                return Err(DurableError::Refused(
                    "bootstrap accepts only exact original source bytes",
                ));
            }
            Ok(())
        }
        CommitMode::Creation {
            attempt,
            owner,
            deadline,
            cancelled,
        } => {
            owner
                .verify_current(*deadline, cancelled)
                .map_err(source_error)?;
            let domain = tx.query_one(
                "SELECT * FROM cmd2_domain WHERE domain=$1",
                &[&request.domain],
            )?;
            if attempt.domain != request.domain
                || attempt.prepare_id != request.prepare_id
                || attempt.fence != request.attempt_fence
                || attempt.definition != definition()
                || (!replayed
                    && domain.get::<_, Option<i64>>("source_epoch") != Some(as_i64(attempt.epoch)?))
                || domain.get::<_, Option<String>>("source_definition_digest")
                    != Some(attempt.definition.to_hex())
                || !domain.get::<_, bool>("source_complete")
                || registered.get::<_, Option<i64>>("source_epoch") != Some(as_i64(attempt.epoch)?)
                || registered.get::<_, String>("delta_digest") != attempt.delta.to_hex()
                || registered.get::<_, Option<Vec<u8>>>("source_reads")
                    != Some(reads_bytes(&attempt.reads)?)
                || registered.get::<_, Option<Vec<u8>>>("source_indexes")
                    != Some(indexes_bytes(&attempt.indexes)?)
                || registered.get::<_, Option<Vec<u8>>>("source_projections")
                    != Some(projections_bytes(&attempt.projections)?)
                || members
                    .iter()
                    .any(|m| m.get::<_, String>("profile_id").as_bytes() != CREATION)
            {
                return Err(DurableError::Conflict(
                    "source attempt/cohort exact registered binding changed",
                ));
            }
            let dependencies = attempt
                .reads
                .reads
                .iter()
                .filter_map(|r| match r {
                    PredicateRead::Exact { key, .. } => Some(key.clone()),
                    _ => None,
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            for member in members {
                let carrier = row_source_metadata(member)?;
                if carrier.mode != 0o644 || carrier.dependencies != Some(dependencies.clone()) {
                    return Err(DurableError::Conflict(
                        "source creation carrier metadata differs from registered reads",
                    ));
                }
            }
            // Current rule/schema/lease still govern original-package replay.
            if as_u64(domain.get("rule_version"))? != request.expected_rule_version
                || domain.get::<_, String>("contract_digest")
                    != request.expected_contract_digest.to_hex()
                || domain.get::<_, Option<String>>("schema_profile_digest")
                    != Some(schema_profile_digest().to_hex())
            {
                return Err(DurableError::Conflict(
                    "source replay/current schema rule contract changed",
                ));
            }
            let job = tx.query_opt(
                "SELECT fence_epoch FROM cmd2_job WHERE domain=$1 AND job_id=$2 FOR UPDATE",
                &[&request.domain, &request.job_id],
            )?;
            if job.map(|r| r.get::<_, i64>(0)) != Some(as_i64(request.job_fence)?) {
                return Err(DurableError::Refused("source current job lease changed"));
            }
            if replayed {
                return Ok(());
            }
            let mut keys = predicates(&attempt.indexes);
            let observed = attempt
                .reads
                .reads
                .iter()
                .filter_map(|r| match r {
                    PredicateRead::Generation {
                        predicate,
                        observed_generation,
                    } => Some((
                        (
                            predicate.kind.as_str().to_owned(),
                            predicate.scope.clone(),
                            predicate.token.clone(),
                        ),
                        *observed_generation,
                    )),
                    _ => None,
                })
                .collect::<BTreeMap<_, _>>();
            keys.extend(observed.keys().cloned());
            // Lock both observed and invalidated keys in one deterministic
            // order, including previously absent namespace ancestors.
            for (kind, scope, token) in keys {
                tx.execute("INSERT INTO cmd2_predicate(domain,kind,owner,scope,token,definition_version,generation,complete) VALUES($1,$2,$3,$4,$5,$6,0,true) ON CONFLICT DO NOTHING", &[&request.domain,&kind,&OWNER,&scope,&token,&attempt.definition.to_hex()])?;
                let row=tx.query_one("SELECT generation,complete,definition_version FROM cmd2_predicate WHERE domain=$1 AND kind=$2 AND owner=$3 AND scope=$4 AND token=$5 FOR UPDATE", &[&request.domain,&kind,&OWNER,&scope,&token])?;
                if !row.get::<_, bool>(1) || row.get::<_, String>(2) != attempt.definition.to_hex()
                {
                    return Err(DurableError::Refused(
                        "source predicate definition/completeness invalidated",
                    ));
                }
                if observed.get(&(kind, scope, token)).is_some_and(|expected| {
                    as_u64(row.get(0)).map_or(true, |actual| actual != *expected)
                }) {
                    return Err(DurableError::Conflict("affected source predicate changed"));
                }
            }
            let exact_paths = attempt.reads.locators.keys().collect::<Vec<_>>();
            let exact_current = tx
                .query(
                    "SELECT * FROM cmd2_current WHERE domain=$1 AND subject=ANY($2)",
                    &[&request.domain, &exact_paths],
                )?
                .into_iter()
                .map(|r| (r.get::<_, String>("subject"), r))
                .collect::<BTreeMap<_, _>>();
            for read in &attempt.reads.reads {
                match read {
                    PredicateRead::Exact {
                        key,
                        expected_version,
                        expected_digest,
                        ..
                    } => {
                        let row = exact_current
                            .get(key)
                            .ok_or(DurableError::Conflict("exact source read disappeared"))?;
                        if Some(as_u64(row.get("revision"))?) != *expected_version
                            || Some(parse_hex(row.get("content_digest"))?) != *expected_digest
                            || attempt.reads.locators.get(key)
                                != Some(&metadata_locator_digest(row))
                        {
                            return Err(DurableError::Conflict(
                                "exact source body/version/locator read changed",
                            ));
                        }
                    }
                    PredicateRead::Absent { key, .. } => {
                        if tx
                            .query_opt(
                                "SELECT 1 FROM cmd2_current WHERE domain=$1 AND subject=$2",
                                &[&request.domain, key],
                            )?
                            .is_some()
                        {
                            return Err(DurableError::Conflict("complete source absence changed"));
                        }
                    }
                    PredicateRead::Generation { .. } => {}
                }
            }
            Ok(())
        }
    }
}

pub(super) fn check_warm_continuation(
    mode: &CommitMode<'_>,
    audit: u64,
    replayed: bool,
) -> DurableResult<()> {
    if let CommitMode::Creation { attempt, .. } = mode {
        if let Some(continuation) = &attempt.continuation {
            if !replayed
                && (continuation.fence.get() != audit || continuation.committed.borrow().is_some())
            {
                return Err(DurableError::Conflict(
                    "warm source continuation lost; cold reopen required",
                ));
            }
        }
    }
    Ok(())
}
pub(super) fn record_warm_commit(
    mode: &CommitMode<'_>,
    audit: u64,
    commit_seq: u64,
    receipts: &[ByteDurabilityReceipt],
) {
    if let CommitMode::Creation { attempt, .. } = mode {
        if let Some(continuation) = &attempt.continuation {
            *continuation.committed.borrow_mut() = Some(WarmCommitted {
                audit_generation: audit,
                commit_seq,
                receipts: receipts.to_vec(),
            });
        }
    }
}

pub(super) fn apply_source_change(
    tx: &mut Transaction<'_>,
    request: &CommitShadowAttempt<'_>,
    mode: &CommitMode<'_>,
    seq: u64,
) -> DurableResult<()> {
    match mode {
        CommitMode::Shadow => Ok(()),
        CommitMode::Bootstrap => {
            tx.execute(
                "UPDATE cmd2_domain SET source_generation=$2 WHERE domain=$1",
                &[&request.domain, &as_i64(seq)?],
            )?;
            Ok(())
        }
        CommitMode::Creation { attempt, .. } => {
            for (kind, token, path) in &attempt.indexes {
                tx.execute("INSERT INTO cmd2_source_index(domain,kind,token,path,definition_digest) VALUES($1,$2,$3,$4,$5)", &[&request.domain,kind,token,path,&attempt.definition.to_hex()])?;
            }
            for (path, projection) in &attempt.projections {
                let current = tx.execute("UPDATE cmd2_current SET inventory_projection=$3 WHERE domain=$1 AND subject=$2 AND prepare_id=$4", &[&request.domain,path,projection,&request.prepare_id])?;
                let history = tx.execute("UPDATE cmd2_history SET inventory_projection=$3 WHERE domain=$1 AND subject=$2 AND prepare_id=$4 AND commit_seq=$5", &[&request.domain,path,projection,&request.prepare_id,&as_i64(seq)?])?;
                let indexed = tx.execute("UPDATE cmd2_source_index SET inventory_projection=$3 WHERE domain=$1 AND kind='path' AND path=$2 AND definition_digest=$4", &[&request.domain,path,projection,&attempt.definition.to_hex()])?;
                if current != 1 || history != 1 || indexed != 1 {
                    return Err(DurableError::Corrupt(
                        "atomic Agent projection membership differs",
                    ));
                }
            }
            for (kind, scope, token) in predicates(&attempt.indexes) {
                let changed=tx.execute("UPDATE cmd2_predicate SET generation=generation+1 WHERE domain=$1 AND kind=$2 AND owner=$3 AND scope=$4 AND token=$5 AND complete AND definition_version=$6 AND generation<9223372036854775807", &[&request.domain,&kind,&OWNER,&scope,&token,&attempt.definition.to_hex()])?;
                if changed != 1 {
                    return Err(DurableError::Refused(
                        "source invalidation exhausted or lost complete definition",
                    ));
                }
            }
            tx.execute("UPDATE cmd2_domain SET source_generation=$2,source_projection_digest=NULL,selected_generation_digest=NULL,complete_cut_digest=NULL,complete_cut_generation=NULL WHERE domain=$1 AND source_complete", &[&request.domain,&as_i64(seq)?])?;
            Ok(())
        }
    }
}

pub(super) fn verify_owner_before_outcome(mode: &CommitMode<'_>) -> DurableResult<()> {
    if let CommitMode::Creation {
        owner,
        deadline,
        cancelled,
        ..
    } = mode
    {
        owner
            .verify_current(*deadline, cancelled)
            .map_err(source_error)?;
    }
    Ok(())
}

fn seal_members(
    store: &SegmentStore,
    prepare: &[u8],
    fence: u64,
    profile: &[u8],
    identities: &[ShadowWriteIdentity<'_>],
) -> DurableResult<Vec<DurableShadowMember>> {
    let mut readers = identities
        .iter()
        .map(|m| Cursor::new(m.exact_bytes))
        .collect::<Vec<_>>();
    let mut frames = identities
        .iter()
        .zip(readers.iter_mut())
        .map(|(m, reader)| FrameInput {
            binding: OwnerBinding {
                profile_id: profile.into(),
                profile_version: PROFILE_VERSION.into(),
                subject_key: m.subject.as_bytes().into(),
                member_slot: m.member_slot,
            },
            declared_size: m.exact_bytes.len() as u64,
            declared_sha256: Digest256::of_bytes(m.exact_bytes),
            reader,
        })
        .collect::<Vec<_>>();
    let receipts = store.seal_segment_fenced(prepare, fence, 0, &mut frames)?;
    if receipts.len() != identities.len() {
        return Err(DurableError::Corrupt("sealed source membership differs"));
    }
    Ok(identities
        .iter()
        .zip(receipts)
        .map(|(m, receipt)| DurableShadowMember {
            member_slot: m.member_slot,
            subject: m.subject.into(),
            expected_predecessor: m.expected_predecessor,
            proposed_revision: m.proposed_revision,
            exact_bytes: m.exact_bytes.into(),
            receipt,
        })
        .collect())
}

fn retained_tuple(value: &serde_json::Value, length: usize) -> DurableResult<&[serde_json::Value]> {
    value
        .as_array()
        .filter(|v| v.len() == length)
        .map(Vec::as_slice)
        .ok_or(DurableError::Corrupt("original companion tuple shape"))
}
fn retained_text(value: &serde_json::Value) -> DurableResult<&str> {
    value
        .as_str()
        .ok_or(DurableError::Corrupt("original companion text"))
}
fn retained_u64(value: &serde_json::Value) -> DurableResult<u64> {
    value
        .as_u64()
        .ok_or(DurableError::Corrupt("original companion integer"))
}
fn retained_member(value: &serde_json::Value) -> DurableResult<MemberMetadata> {
    let m = retained_tuple(value, 4)?;
    let path = RelativePath::parse(retained_text(&m[0])?)
        .map_err(|_| DurableError::Corrupt("original member path"))?;
    let size = retained_u64(&m[2])?;
    let mode = u32::try_from(retained_u64(&m[3])?)
        .map_err(|_| DurableError::Corrupt("original member mode"))?;
    if size > 8_388_608 || ![0o644, 0o755].contains(&mode) {
        return Err(DurableError::Refused(
            "original member outside existing source profile",
        ));
    }
    Ok(MemberMetadata {
        path,
        sha256: parse_hex(retained_text(&m[1])?.into())?,
        size_bytes: size,
        mode,
    })
}
fn retain_context_file(
    context: &mut CommandContext,
    total: &mut usize,
    path: RelativePath,
    raw: Vec<u8>,
) -> DurableResult<()> {
    if context.files.len() >= cmd::SELECTED_SOURCE_MAX_FILES || raw.len() > 8_388_608 {
        return Err(DurableError::Refused(
            "recovery input exceeds existing file/member bound",
        ));
    }
    *total = total
        .checked_add(raw.len())
        .filter(|n| *n <= cmd::SELECTED_SOURCE_MAX_BYTES)
        .ok_or(DurableError::Refused(
            "recovery input exceeds existing aggregate byte bound",
        ))?;
    context.files.push(SourceFile { path, raw });
    Ok(())
}
struct RetainedManagedOriginal {
    context: CommandContext,
    basis: ManagedCreationBasis,
    observations: BTreeMap<String, ManagedCreationObservation>,
    inventory: Option<ManagedAgentInventory>,
}

fn decode_managed_original(
    value: &serde_json::Value,
    cohort: &ManagedSourceCohort,
    committed_seq: u64,
    schema_cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &CutWorkerSchemaExecutor,
) -> DurableResult<(RetainedManagedOriginal, Vec<(RelativePath, Digest256, u64)>)> {
    let basis = retained_tuple(
        value
            .get("basis")
            .ok_or(DurableError::Corrupt("original basis absent"))?,
        5,
    )?;
    let basis = ManagedCreationBasis {
        domain: retained_text(&basis[0])?.into(),
        digest: parse_hex(retained_text(&basis[1])?.into())?,
        generation: retained_u64(&basis[2])?,
        epoch: retained_u64(&basis[3])?,
        definition: parse_hex(retained_text(&basis[4])?.into())?,
    };
    if basis.domain != cohort.domain
        || basis.definition != cohort.definition
        || basis.epoch == 0
        || basis.epoch > cohort.epoch
        || basis.generation >= committed_seq
        || committed_seq > cohort.generation
    {
        return Err(DurableError::Corrupt(
            "original managed basis outside retained cohort",
        ));
    }
    let inventory = match value.get("inventory") {
        None | Some(serde_json::Value::Null) => None,
        Some(value) => {
            let fields = retained_tuple(value, 2)?;
            let dependencies = retained_text(&fields[1])?.to_owned();
            if !dependencies.starts_with("sha256:")
                || Digest256::from_prefixed(&dependencies).is_err()
            {
                return Err(DurableError::Corrupt("original Agent dependency digest"));
            }
            Some(ManagedAgentInventory {
                root: parse_hex(retained_text(&fields[0])?.into())?,
                dependencies,
            })
        }
    };
    let ctx = retained_tuple(
        value
            .get("context")
            .ok_or(DurableError::Corrupt("original context absent"))?,
        5,
    )?;
    let revision = SourceRevision(parse_hex(retained_text(&ctx[0])?.into())?);
    if schema_cut.current().revision() != revision
        || worker.source_revision() != revision
        || software.selection() != components.capture()
        || value.get("software") != Some(&software_value(components))
    {
        return Err(DurableError::Conflict(
            "original independent schema/software selection differs",
        ));
    }
    let context = CommandContext {
        base_revision: revision,
        configuration_raw: serde_json::from_value(ctx[1].clone())
            .map_err(|_| DurableError::Corrupt("original configuration bytes"))?,
        request_raw: serde_json::from_value(ctx[2].clone())
            .map_err(|_| DurableError::Corrupt("original request bytes"))?,
        recorded_at: retained_text(&ctx[3])?.into(),
        effective_uid: retained_u64(&ctx[4])?,
        files: Vec::new(),
    };
    context.check().map_err(source_error)?;
    let mut observations = BTreeMap::new();
    for observed in value
        .get("observations")
        .and_then(serde_json::Value::as_array)
        .filter(|a| a.len() <= cmd::SELECTED_SOURCE_MAX_FILES)
        .ok_or(DurableError::Refused(
            "original observations outside existing input bound",
        ))?
    {
        let o = retained_tuple(observed, 4)?;
        let metadata = retained_member(&o[0])?;
        if !metadata.path.as_str().starts_with("ToS/") {
            return Err(DurableError::Corrupt(
                "original authored observation namespace",
            ));
        }
        let dependencies = if o[1].is_null() {
            None
        } else {
            Some(
                o[1].as_array()
                    .ok_or(DurableError::Corrupt("original dependency claims array"))?
                    .iter()
                    .map(|p| {
                        RelativePath::parse(retained_text(p)?)
                            .map_err(|_| DurableError::Corrupt("original dependency path"))
                    })
                    .collect::<DurableResult<Vec<_>>>()?,
            )
        };
        let custody_revision = retained_u64(&o[2])?;
        let commit_seq = retained_u64(&o[3])?;
        if custody_revision == 0
            || commit_seq == 0
            || commit_seq > basis.generation
            || dependencies
                .as_ref()
                .is_some_and(|p| p.windows(2).any(|v| v[0] >= v[1]))
        {
            return Err(DurableError::Corrupt(
                "original source observation custody/dependencies",
            ));
        }
        let path = metadata.path.as_str().to_owned();
        if observations
            .insert(
                path,
                ManagedCreationObservation {
                    metadata,
                    dependencies,
                    custody_revision,
                    commit_seq,
                },
            )
            .is_some()
        {
            return Err(DurableError::Corrupt("original duplicate observation"));
        }
    }
    if observations
        .values()
        .filter_map(|o| o.dependencies.as_ref())
        .flatten()
        .any(|p| !observations.contains_key(p.as_str()))
    {
        return Err(DurableError::Corrupt(
            "original dependency outside observation membership",
        ));
    }
    let mut software_inputs = Vec::new();
    let mut previous: Option<RelativePath> = None;
    for member in value
        .get("software_inputs")
        .and_then(serde_json::Value::as_array)
        .ok_or(DurableError::Corrupt("original software inputs absent"))?
    {
        let row = retained_tuple(member, 3)?;
        let path = RelativePath::parse(retained_text(&row[0])?)
            .map_err(|_| DurableError::Corrupt("original software path"))?;
        let sha = parse_hex(retained_text(&row[1])?.into())?;
        let size = retained_u64(&row[2])?;
        if path.as_str().starts_with("ToS/")
            || previous.as_ref().is_some_and(|p| p >= &path)
            || components
                .member(&path)
                .is_none_or(|m| m.sha256 != sha || m.size_bytes != size)
        {
            return Err(DurableError::Corrupt(
                "original software input selection differs",
            ));
        }
        previous = Some(path.clone());
        software_inputs.push((path, sha, size));
    }
    if observations
        .len()
        .checked_add(software_inputs.len())
        .is_none_or(|n| n > cmd::SELECTED_SOURCE_MAX_FILES)
    {
        return Err(DurableError::Refused(
            "original total input count exceeds existing bound",
        ));
    }
    Ok((
        RetainedManagedOriginal {
            context,
            basis,
            observations,
            inventory,
        },
        software_inputs,
    ))
}

fn decode_reads(raw: &[u8]) -> DurableResult<SourceReads> {
    let value: serde_json::Value = serde_json::from_slice(raw)
        .map_err(|_| DurableError::Corrupt("persisted source reads JSON"))?;
    let mut reads = SourceReads {
        reads: Vec::new(),
        locators: BTreeMap::new(),
        managed_original: None,
    };
    let tuples = if value.is_array() {
        &value
    } else {
        if value.get("profile").and_then(serde_json::Value::as_str)
            != Some("tos.managed-agent-original-reads-v1")
        {
            return Err(DurableError::Corrupt("persisted managed reads profile"));
        }
        reads.managed_original = Some(
            value
                .get("original")
                .filter(|v| v.is_object())
                .ok_or(DurableError::Corrupt(
                    "persisted managed original companion",
                ))?
                .clone(),
        );
        value
            .get("reads")
            .ok_or(DurableError::Corrupt("persisted managed read tuples"))?
    };
    for row in tuples
        .as_array()
        .ok_or(DurableError::Corrupt("persisted source reads array"))?
    {
        let row = row
            .as_array()
            .ok_or(DurableError::Corrupt("persisted source read tuple"))?;
        let string = |i: usize| {
            row.get(i)
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .ok_or(DurableError::Corrupt("persisted read string"))
        };
        match string(0)?.as_str() {
            "exact" if row.len() == 6 => {
                let key = string(2)?;
                reads.locators.insert(key.clone(), parse_hex(string(5)?)?);
                reads.reads.push(PredicateRead::Exact {
                    namespace: string(1)?,
                    key,
                    expected_version: Some(
                        row[3]
                            .as_u64()
                            .ok_or(DurableError::Corrupt("persisted exact version"))?,
                    ),
                    expected_digest: Some(parse_hex(string(4)?)?),
                });
            }
            "absent" if row.len() == 3 => reads.reads.push(PredicateRead::Absent {
                namespace: string(1)?,
                key: string(2)?,
            }),
            "generation" if row.len() == 7 => reads.reads.push(PredicateRead::Generation {
                predicate: PredicateToken {
                    kind: match string(1)?.as_str() {
                        "range" => PredicateKind::Range,
                        "unique" => PredicateKind::Unique,
                        _ => return Err(DurableError::Corrupt("persisted predicate kind")),
                    },
                    owner: string(2)?,
                    scope: string(3)?,
                    token: string(4)?,
                    definition_version: string(5)?,
                },
                observed_generation: row[6]
                    .as_u64()
                    .ok_or(DurableError::Corrupt("persisted generation"))?,
            }),
            _ => return Err(DurableError::Corrupt("persisted source read shape")),
        }
    }
    if reads_bytes(&reads)? != raw {
        return Err(DurableError::Corrupt(
            "persisted source reads canonical binding",
        ));
    }
    Ok(reads)
}
