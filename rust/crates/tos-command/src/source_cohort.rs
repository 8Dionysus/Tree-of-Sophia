//! Private controlled Agent creation over independently verified immutable bytes.
//! This is mechanics custody, never canonical, semantic, rights or source admission.

use super::*;
use crate::source_command::{self as cmd, CommandContext, SourceFile};
use crate::source_creation::{CreationFamily, SerializedCreation};
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

/// An initial immutable revision names the bootstrap, never later content.
/// Later completeness is maintained only by this controlled atomic writer.
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
fn creation_metadata(package: &SerializedCreation) -> SourceMetadata {
    let dependencies = package
        .command()
        .reads
        .iter()
        .filter(|r| r.path.as_str().starts_with("ToS/"))
        .map(|r| r.path.as_str().to_owned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    package
        .command()
        .changes
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
    CommittedReplay,
}

struct SourceReads {
    reads: Vec<PredicateRead>,
    locators: BTreeMap<String, Digest256>,
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
            ItemRefusal::Budget => {
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
fn scoped_context(package: &SerializedCreation) -> CommandContext {
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
    package: &SerializedCreation,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<IndexRows> {
    let prepared = package.prepared();
    if !matches!(
        prepared.family(),
        CreationFamily::CorpusV1 | CreationFamily::CorpusV2
    ) || package.command().handler_id != "native-corpus-create"
        || package.command().operation != "source.create"
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
    if package.command().changes.len() != prepared.files().len() {
        return Err(DurableError::Corrupt("source package membership differs"));
    }
    for change in &package.command().changes {
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
    serde_json::to_vec(&values).map_err(|_| DurableError::Invalid("source reads encode"))
}
fn source_delta(package: &SerializedCreation) -> Digest256 {
    let mut h = Digest256Hasher::new();
    part(&mut h, package.command().base_revision.0.as_bytes());
    part(&mut h, b"tos-managed-agent-create-delta-v1");
    part(
        &mut h,
        package.command().configuration_raw_sha256.as_bytes(),
    );
    part(&mut h, package.prepared().dependencies().as_bytes());
    for change in &package.command().changes {
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
    h.finalize()
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

impl DurablePgCoordinator {
    pub fn read_current_source_member(
        &mut self,
        store: &SegmentStore,
        cohort: &ManagedSourceCohort,
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
            "SELECT maintenance_state FROM cmd2_audit_fence WHERE domain=$1 FOR SHARE",
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
            package,
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
            package,
            SourceRegistrationBasis::Current(current),
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
            package,
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
        package: &SerializedCreation,
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
        let indexes = creation_indexes(package, worker, deadline, cancelled)?;
        let delta = source_delta(package);
        let request =
            cmd::parse(&package.prepared().context().request_raw).map_err(source_error)?;
        let command_id = cmd::text(&request, "command_id").map_err(source_error)?;
        let raw_request_digest = Digest256::of_bytes(&package.prepared().context().request_raw);
        let domain = cohort.domain.as_str();
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'")?;
        lock_audit_fence(&mut tx, domain)?;
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
            if existing.get::<_, String>("command_id") != command_id
                || existing.get::<_, String>("raw_request_digest") != raw_request_digest.to_hex()
                || existing.get::<_, String>("delta_digest") != delta.to_hex()
                || existing.get::<_, Option<Vec<u8>>>("source_indexes")
                    != Some(indexes_bytes(&indexes)?)
                || (existing.get::<_, String>("state") != "committed"
                    && existing.get::<_, Option<i64>>("source_epoch")
                        != Some(as_i64(cohort.epoch)?))
                || existing.get::<_, String>("state") == "aborted"
            {
                return Err(DurableError::Conflict(
                    "source attempt exact identity differs",
                ));
            }
            let encoded = existing
                .get::<_, Option<Vec<u8>>>("source_reads")
                .ok_or(DurableError::Corrupt("source attempt read binding absent"))?;
            let reads = decode_reads(&encoded)?;
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
            });
        }
        if matches!(basis, SourceRegistrationBasis::CommittedReplay) {
            return Err(DurableError::Refused(
                "source replay attempt does not exist",
            ));
        }
        let mut reads = SourceReads {
            reads: Vec::new(),
            locators: BTreeMap::new(),
        };
        let dependency_paths = package
            .command()
            .reads
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
        let membership_count: i64 = tx
            .query_one(
                "SELECT count(*) FROM cmd2_current WHERE domain=$1",
                &[&domain],
            )?
            .get(0);
        if current_dependencies.len() != dependency_paths.len()
            || as_u64(membership_count)? != dependency_paths.len() as u64
        {
            return Err(DurableError::Conflict(
                "maintained complete source inventory changed since preparation",
            ));
        }
        for dependency in &package.command().reads {
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
        for change in &package.command().changes {
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
        tx.execute("INSERT INTO cmd2_attempt(domain,prepare_id,command_id,raw_request_digest,delta_digest,state,attempt_fence,source_reads,source_indexes,source_epoch) VALUES($1,$2,$3,$4,$5,'registered',1,$6,$7,$8)",
            &[&domain,&prepare_id,&command_id,&raw_request_digest.to_hex(),&delta.to_hex(),&reads_bytes(&reads)?,&indexes_bytes(&indexes)?,&as_i64(cohort.epoch)?])?;
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
        active(deadline, cancelled)?;
        if attempt.delta != source_delta(package)
            || attempt.definition != definition()
            || store.custody_domain() != attempt.domain.as_bytes()
        {
            return Err(DurableError::Conflict("registered source package changed"));
        }
        // Acquire the real maintained owner mutex before STO and PG locks.
        let owner = filesystem
            .hold_current_owner(package, deadline, cancelled)
            .map_err(source_error)?;
        let identities = package
            .command()
            .changes
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
                self.attach_ready_profile(
                    store,
                    &attempt.domain,
                    &attempt.prepare_id,
                    attempt.fence,
                    &members,
                    CREATION,
                    Some(attempt.delta),
                    Some(&creation_metadata(package)),
                )?;
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
        while let Some(member) = original_stream
            .next_member(deadline, cancelled)
            .map_err(|_| DurableError::Refused("original cold EOF/fixity failure"))?
        {
            expected.insert(member.path.as_str().to_owned(), member.raw);
        }
        if original_stream.coverage() != Some(initial_membership) {
            return Err(DurableError::Refused("original cold membership incomplete"));
        }
        let before =
            self.cold_verify_cut_with_budget(store, domain, Some((deadline, cancelled)))?;
        let mut metadata_tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        metadata_tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '60s'; SET LOCAL work_mem = '4MB'")?;
        let domain_row =
            metadata_tx.query_one("SELECT * FROM cmd2_domain WHERE domain=$1", &[&domain])?;
        let rows=metadata_tx.query("SELECT c.*,a.attempt_fence AS source_attempt_fence FROM cmd2_current c JOIN cmd2_attempt a USING(domain,prepare_id) WHERE c.domain=$1 ORDER BY c.prepare_id,c.member_slot", &[&domain])?;
        let stored=metadata_tx.query("SELECT kind,token,path,definition_digest FROM cmd2_source_index WHERE domain=$1 ORDER BY kind,token,path", &[&domain])?;
        let persisted_predicates=metadata_tx.query("SELECT kind,scope,token,definition_version FROM cmd2_predicate WHERE domain=$1 AND owner=$2", &[&domain,&OWNER])?;
        metadata_tx.commit()?;
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
        verify_manifest_indexes(original, &indexes)?;
        let mut actual = IndexRows::new();
        for row in stored {
            if row.get::<_, String>(3) != cohort.definition.to_hex() {
                return Err(DurableError::Corrupt(
                    "current owner index definition differs",
                ));
            }
            actual.insert((row.get(0), row.get(1), row.get(2)));
        }
        if indexes != actual {
            return Err(DurableError::Corrupt(
                "current original bodies and complete owner indexes differ",
            ));
        }
        let mut predicate_keys = BTreeSet::new();
        for row in persisted_predicates {
            let kind: String = row.get(0);
            let scope: String = row.get(1);
            if row.get::<_, String>(3) != cohort.definition.to_hex()
                || !["unique", "range"].contains(&kind.as_str())
                || (kind == "range"
                    && !["source-home", "source-inventory"].contains(&scope.as_str()))
                || (scope == "source-inventory" && row.get::<_, String>(2) != "all")
                || (kind == "unique"
                    && !["metadata", "claim", "event", "anchor", "form", "path"]
                        .contains(&scope.as_str()))
            {
                return Err(DurableError::Corrupt(
                    "stored source predicate owner definition differs",
                ));
            }
            predicate_keys.insert((kind, scope, row.get::<_, String>(2)));
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
        tx.execute("UPDATE cmd2_domain SET source_complete=true,source_generation=head_seq,selected_generation_digest=NULL,complete_cut_digest=NULL,complete_cut_generation=NULL WHERE domain=$1", &[&domain])?;
        tx.commit()?;
        let verified =
            self.cold_verify_cut_with_budget(store, domain, Some((deadline, cancelled)))?;
        let generation = self.build_complete_generation(store, &verified, deadline, cancelled)?;
        self.select_complete_generation(&generation)?;
        let selected = self.cold_open_selected_generation(store, domain, deadline, cancelled)?;
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
            for (kind, scope, token) in predicates(&attempt.indexes) {
                let changed=tx.execute("UPDATE cmd2_predicate SET generation=generation+1 WHERE domain=$1 AND kind=$2 AND owner=$3 AND scope=$4 AND token=$5 AND complete AND definition_version=$6 AND generation<9223372036854775807", &[&request.domain,&kind,&OWNER,&scope,&token,&attempt.definition.to_hex()])?;
                if changed != 1 {
                    return Err(DurableError::Refused(
                        "source invalidation exhausted or lost complete definition",
                    ));
                }
            }
            tx.execute("UPDATE cmd2_domain SET source_generation=$2,selected_generation_digest=NULL,complete_cut_digest=NULL,complete_cut_generation=NULL WHERE domain=$1 AND source_complete", &[&request.domain,&as_i64(seq)?])?;
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

fn decode_reads(raw: &[u8]) -> DurableResult<SourceReads> {
    let value: serde_json::Value = serde_json::from_slice(raw)
        .map_err(|_| DurableError::Corrupt("persisted source reads JSON"))?;
    let mut reads = SourceReads {
        reads: Vec::new(),
        locators: BTreeMap::new(),
    };
    for row in value
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
