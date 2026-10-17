//! Private controlled Agent creation over independently verified immutable bytes.
//! This is mechanics custody, never canonical, semantic, rights or source admission.

use super::*;
#[path = "addressed_inventory.rs"]
mod addressed_inventory;
#[path = "addressed_successor.rs"]
mod addressed_successor;
#[path = "source_cohort_bootstrap_streamed.rs"]
mod source_cohort_bootstrap_streamed;
#[path = "source_cohort_export.rs"]
mod source_cohort_export;
#[path = "source_cohort_stream_index.rs"]
mod source_cohort_stream_index;
#[path = "source_cohort_streamed.rs"]
mod source_cohort_streamed;
use crate::source_command::{self as cmd, CommandContext, SourceFile};
use crate::source_creation::{
    CreationFamily, CreationPackage, ManagedCreationBasis, ManagedCreationInput,
    ManagedCreationObservation, ManagedSerializedCreation, SerializedCreation,
};
use crate::source_creation_store::{CreationFilesystem, CreationOwnerFence};
use crate::{
    PredicateKind, PredicateRead, PredicateToken, source_claims, source_forms, source_revisions,
};
pub(crate) use addressed_successor::VerifiedAddressedGeneration;
pub use source_cohort_stream_index::SourceAssessmentWork as StreamedSourceIndexWorkV1;
pub use source_cohort_streamed::{StreamedColdSourceLimitsV1, StreamedColdSourceWorkV1};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;
use tos_compiler::managed_source::ManagedProducerProof;
use tos_foundation::{Digest256, RelativePath, SourceRevision};
use tos_segment_store::{FrameInput, OwnerBinding};
use tos_source_store::{
    CorpusCutReader, MemberMetadata, SoftwareCaptureReader, SoftwareComponentSelectionV1,
    SourceMembershipV1,
};
use tos_validation::item_rules::ItemRefusal;
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

/// The source-file carrier used by the bounded PG revision owner. It is
/// separate from ORIGINAL/CREATION so a later authored revision cannot be
/// mistaken for an initial source member or a maintained Agent creation.
pub(super) const REVISION: &[u8] = b"tos.source-file.revision-v1";

#[path = "source_cohort_claims.rs"]
pub(crate) mod claims;
#[cfg(all(test, feature = "postgres-lab"))]
#[path = "source_cohort_concurrency_tests.rs"]
mod concurrency_tests;
#[path = "source_cohort_creation.rs"]
pub(crate) mod creation;
#[path = "source_cohort_recovery.rs"]
mod recovery;
#[path = "source_cohort_revision.rs"]
pub(crate) mod revision;

const ORIGINAL: &[u8] = b"tos.source-file.original-v1";
const CREATION: &[u8] = b"tos.source-file.agent-create-v1";
pub(crate) const OWNER: &str = "native-corpus-create:agent";

/// Recognized source-cohort predicate contracts across bootstrap, streamed
/// cold verification, and bounded PG creation. Hash-shaped tokens stay
/// owner-specific: navigation uses prefixed digests while PG Work revisions
/// use bare lowercase hex digests.
pub(crate) fn valid_owned_predicate_shape(
    owner: &str,
    kind: &str,
    scope: &str,
    token: &str,
) -> bool {
    fn lower_hex(bytes: &[u8]) -> bool {
        bytes
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    }

    fn prefixed_digest(token: &str) -> bool {
        token.len() == 71
            && token.starts_with("sha256:")
            && lower_hex(&token.as_bytes()[7..])
            && Digest256::from_hex(&token[7..]).is_ok()
    }

    fn bare_digest(token: &str) -> bool {
        token.len() == 64 && lower_hex(token.as_bytes()) && Digest256::from_hex(token).is_ok()
    }

    if owner != OWNER {
        return false;
    }
    match (kind, scope) {
        ("range", "source-home") => {
            (token == "ToS" || token.starts_with("ToS/"))
                && !token.ends_with('/')
                && RelativePath::parse(token).is_ok()
        }
        ("range", "source-inventory") => token == "all",
        ("range", creation::DEPENDENCY_SCOPE) => token == creation::COMMITMENT_VERSION.to_string(),
        ("range", "claim-dependency-v1")
        | ("range", "item-edition-v1")
        | ("range", "item-file-v1")
        | ("range", "item-rights-v1") => prefixed_digest(token),
        ("range", crate::source_claim_dependency_index::NAVIGATION_REFERENCE_SCOPE) => {
            token.starts_with("sha256:")
                && token.len() == 71
                && lower_hex(&token.as_bytes()[7..])
                && Digest256::from_prefixed(token).is_ok()
        }
        ("range", crate::source_pg_work_revision::PREDICATE_SCOPE) => bare_digest(token),
        ("unique", "metadata" | "claim" | "event" | "anchor" | "form") => !token.is_empty(),
        ("unique", "path") => {
            token.starts_with("ToS/") && !token.ends_with('/') && RelativePath::parse(token).is_ok()
        }
        ("unique", "sign-candidate") => {
            token.len() <= 4096 && crate::source_revisions::valid_id(token, "tos.claim.", false)
        }
        ("unique", "text-identity" | "semantic-annotation-entity") => {
            (1..=4096).contains(&token.len())
        }
        ("unique", "source-publication") => token == "current",
        _ => false,
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ManagedManifestSelectionV2 {
    pub model_version: u64,
    pub manifest_digest: Digest256,
}

pub(crate) type IndexRows = BTreeSet<(String, String, String)>;
type ProjectionRows = BTreeMap<String, Vec<u8>>;

/// Exact complete compact inventory basis; not source/semantic admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ManagedAgentInventory {
    pub commitment_version: u16,
    pub root: Digest256,
    pub dependencies: String,
}

/// An initial immutable revision names the bootstrap, never later content.
/// Later completeness is maintained only by this controlled atomic writer.
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
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

/// Whether a fresh bootstrap may create the selected initial job row. The
/// initialization case accepts only an absent row or the exact initial fence;
/// it never advances an existing lease.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapJobMode {
    Initialize,
    RequireExisting,
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

fn revision_package(
    proposal: &cmd::PreparedCommand,
    readset: &revision::SourceRevisionReadSet,
    family_tag: &str,
) -> DurableResult<(
    SourceReads,
    Vec<SourceRevisionWrite>,
    SourceMetadata,
    BTreeSet<(String, String, String)>,
)> {
    let record_revision = proposal.operation == "record.revise";
    let claim_command = family_tag.starts_with("tos-pg-global-claims-v1:")
        && ["claims.create", "claim.revise"].contains(&proposal.operation.as_str());
    let growth_family = pg_growth_family(family_tag);
    let growth_command = growth_family.is_some_and(|family| {
        proposal.operation == family.operation() && proposal.handler_id == family.handler()
    });
    let forms_historical_family = pg_forms_historical_family(family_tag);
    let forms_historical_command = forms_historical_family.is_some_and(|(schema, operation)| {
        proposal.operation == operation
            && proposal.handler_id == pg_forms_historical_handler(schema, operation)
    });
    let object_link_creation = family_tag
        == crate::source_creation_store::OBJECT_LINK_PG_FAMILY_TAG
        && proposal.operation == "object.link.create"
        && proposal.handler_id == "native-object-link-create";
    let item_adoption = family_tag == crate::source_creation_store::PG_ITEM_ADOPTION_FAMILY_TAG
        && proposal.operation == "item.adopt"
        && proposal.handler_id == "native-item-adoption";
    let public_text = family_tag == crate::source_public_text_entry::PG_FAMILY_TAG
        && proposal.operation == "native-text.create"
        && proposal.handler_id == "public-project-native-text-create";
    if (!record_revision
        && !claim_command
        && !growth_command
        && !forms_historical_command
        && !item_adoption
        && !public_text)
        || proposal.changes.is_empty()
    {
        return Err(DurableError::Refused(
            "bounded source revision requires a non-empty supported owner delta",
        ));
    }
    if object_link_creation
        && proposal
            .changes
            .iter()
            .any(|change| change.before.is_some())
    {
        return Err(DurableError::Refused(
            "PG ObjectLink creation only admits new source members",
        ));
    }
    if proposal.changes.len() > MAX_MEMBERS {
        return Err(DurableError::Refused(
            "bounded source revision member budget",
        ));
    }
    let reads = SourceReads {
        reads: readset.reads.reads.clone(),
        locators: readset.reads.locators.clone(),
        managed_original: None,
        revision: None,
    };
    let dependencies = reads
        .reads
        .iter()
        .filter_map(|read| match read {
            PredicateRead::Exact { key, .. } => Some(key.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut changes = proposal.changes.clone();
    changes.sort_by(|left, right| left.path.cmp(&right.path));
    if changes.windows(2).any(|pair| pair[0].path == pair[1].path) {
        return Err(DurableError::Invalid(
            "source revision writes contain duplicate paths",
        ));
    }
    let mut writes = Vec::with_capacity(changes.len());
    let mut metadata = SourceMetadata::new();
    for change in changes {
        let subject = change.path.as_str().to_owned();
        if !subject.starts_with("ToS/")
            || subject.contains("/payload/")
            || subject.contains("/owner-local/")
            || subject.ends_with("/.metadata-publication.json")
        {
            return Err(DurableError::Refused(
                "bounded source revision path outside authored metadata",
            ));
        }
        let exact = readset.observations.get(&subject);
        let expected_predecessor = match (change.before, exact) {
            (Some(before), Some(observed)) if observed.member.metadata.sha256 == before => {
                Some((observed.member.custody_revision, before))
            }
            (Some(_), _) => {
                return Err(DurableError::Conflict(
                    "source revision predecessor body differs from readset",
                ));
            }
            (None, Some(_)) => {
                return Err(DurableError::Conflict(
                    "source revision insertion target was read as present",
                ));
            }
            (None, None) => None,
        };
        let exact_bytes = change.after.ok_or(DurableError::Refused(
            "bounded source revision does not admit member deletion",
        ))?;
        let proposed_revision = expected_predecessor
            .map(|(revision, _)| {
                revision
                    .checked_add(1)
                    .ok_or(DurableError::Corrupt("source revision custody overflow"))
            })
            .unwrap_or(Ok(1))?;
        writes.push(SourceRevisionWrite {
            subject: subject.clone(),
            expected_predecessor,
            proposed_revision,
            exact_bytes,
        });
        metadata.insert(
            subject,
            SourceFileMetadata {
                mode: 0o644,
                dependencies: Some(dependencies.clone()),
            },
        );
    }
    if object_link_creation {
        // This owner supplies a complete new-only Object/Link package. Reuse
        // its exact sealed-package validator here so registration and recovery
        // derive the same path and semantic-index rows from authored bytes.
        crate::source_creation_store::expected_object_link_revision_indexes(&writes)?;
    }
    let predicate_keys = reads
        .reads
        .iter()
        .filter_map(|read| match read {
            PredicateRead::Generation { predicate, .. }
                if predicate.owner == OWNER
                    && !(predicate.scope == "source-cohort" && predicate.token == "generation") =>
            {
                Some((
                    predicate.kind.as_str().to_owned(),
                    predicate.scope.clone(),
                    predicate.token.clone(),
                ))
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    Ok((reads, writes, metadata, predicate_keys))
}

fn pg_growth_family(family_tag: &str) -> Option<crate::source_native_cli::PgGrowthFamily> {
    use crate::source_native_cli::PgGrowthFamily;
    [
        PgGrowthFamily::ExpressionEdition,
        PgGrowthFamily::ExpressionResponsibility,
        PgGrowthFamily::CollectionMembership,
        PgGrowthFamily::WorkExpression,
        PgGrowthFamily::ObjectLink,
        PgGrowthFamily::SignPromotion,
    ]
    .into_iter()
    .find(|family| family.family_tag() == family_tag)
}

fn pg_forms_historical_family(family_tag: &str) -> Option<(&'static str, &'static str)> {
    match family_tag {
        "tos-pg-human-forms-v1:tos_local_source_command_owner_v1:apply" => {
            Some(("tos_local_source_command_owner_v1", "apply"))
        }
        "tos-pg-human-forms-v1:tos_local_canonical_form_owner_v1:apply" => {
            Some(("tos_local_canonical_form_owner_v1", "apply"))
        }
        "tos-pg-human-forms-v1:tos_local_claim_form_owner_v1:apply" => {
            Some(("tos_local_claim_form_owner_v1", "apply"))
        }
        "tos-pg-human-forms-v1:tos_local_claim_form_owner_v2:apply" => {
            Some(("tos_local_claim_form_owner_v2", "apply"))
        }
        "tos-pg-historical-claim-v1:tos_local_historical_claim_form_owner_v1:apply" => {
            Some(("tos_local_historical_claim_form_owner_v1", "apply"))
        }
        "tos-pg-historical-claim-v1:tos_local_historical_claim_revision_owner_v1:claim.revise" => {
            Some((
                "tos_local_historical_claim_revision_owner_v1",
                "claim.revise",
            ))
        }
        _ => None,
    }
}

fn pg_forms_historical_handler(schema: &str, operation: &str) -> &'static str {
    match (schema, operation) {
        ("tos_local_source_command_owner_v1", "apply")
        | ("tos_local_canonical_form_owner_v1", "apply")
        | ("tos_local_claim_form_owner_v1", "apply")
        | ("tos_local_claim_form_owner_v2", "apply") => "public-source-forms",
        ("tos_local_historical_claim_form_owner_v1", "apply") => "legacy-historical-claim-forms",
        ("tos_local_historical_claim_revision_owner_v1", "claim.revise") => {
            "legacy-historical-claim-revision"
        }
        _ => "unsupported-pg-forms-handler",
    }
}

fn revision_delta(
    proposal: &cmd::PreparedCommand,
    selection: &revision::SourceRevisionSelection,
    reads: &SourceReads,
    writes: &[SourceRevisionWrite],
    metadata: &SourceMetadata,
    projections: &ProjectionRows,
    publication_package: Option<&crate::source_work_transaction::SelectedPublicationPackage>,
    work_revision: Option<&crate::source_pg_work_revision::CommitEvidence>,
) -> DurableResult<Digest256> {
    let mut hasher = Digest256Hasher::new();
    if let Some(revision) = &reads.revision {
        part(
            &mut hasher,
            b"tos-family-tagged-source-record-revision-delta-v1",
        );
        part(&mut hasher, revision.family_tag.as_bytes());
    } else {
        // Preserve the established flat/Work delta for exact retries of
        // attempts written before family-tagged PG admission.
        part(&mut hasher, b"tos-corpus-flat-record-revision-delta-v1");
    }
    part(&mut hasher, selection.cohort.domain.as_bytes());
    part(&mut hasher, selection.contract.as_bytes());
    part(&mut hasher, &selection.epoch.to_be_bytes());
    part(&mut hasher, &selection.source_generation.to_be_bytes());
    part(&mut hasher, &selection.head_seq.to_be_bytes());
    part(&mut hasher, proposal.handler_id.as_bytes());
    part(&mut hasher, proposal.operation.as_bytes());
    part(&mut hasher, proposal.base_revision.0.as_bytes());
    part(&mut hasher, proposal.request_canonical_sha256.as_bytes());
    part(&mut hasher, proposal.configuration_raw_sha256.as_bytes());
    part(
        &mut hasher,
        proposal.configuration_canonical_sha256.as_bytes(),
    );
    part(
        &mut hasher,
        Digest256::of_bytes(&reads_bytes(reads)?).as_bytes(),
    );
    if let Some(evidence) = work_revision {
        part(&mut hasher, b"pg-work-revision-v1");
        part(
            &mut hasher,
            crate::source_pg_work_revision::commit_delta_digest(evidence).as_bytes(),
        );
    }
    if let Some(package) = publication_package {
        part(&mut hasher, b"tos-pg-selected-publication-package-v1");
        part(&mut hasher, package.sha256.as_bytes());
        part(
            &mut hasher,
            &(package.canonical_bytes.len() as u64).to_be_bytes(),
        );
    } else {
        part(&mut hasher, b"no-selected-publication-package");
    }
    if !projections.is_empty() {
        part(&mut hasher, b"tos-source-revision-owner-projections-v1");
        part(
            &mut hasher,
            Digest256::of_bytes(&projections_bytes(projections)?).as_bytes(),
        );
    }
    for write in writes {
        part(&mut hasher, write.subject.as_bytes());
        match write.expected_predecessor {
            None => part(&mut hasher, b"absent"),
            Some((revision, digest)) => {
                part(&mut hasher, b"present");
                part(&mut hasher, &revision.to_be_bytes());
                part(&mut hasher, digest.as_bytes());
            }
        }
        part(&mut hasher, &write.proposed_revision.to_be_bytes());
        part(
            &mut hasher,
            Digest256::of_bytes(&write.exact_bytes).as_bytes(),
        );
        part(&mut hasher, &(write.exact_bytes.len() as u64).to_be_bytes());
        if let Some(file) = metadata.get(&write.subject) {
            part(&mut hasher, &file.mode.to_be_bytes());
            for dependency in file.dependencies.as_deref().unwrap_or(&[]) {
                part(&mut hasher, dependency.as_bytes());
            }
        }
    }
    Ok(hasher.finalize())
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

fn work_revision_recorded_at(
    writes: &[SourceRevisionWrite],
    binding: &WorkRevisionRecoveryBinding,
) -> DurableResult<String> {
    let names = source_revisions::names(&binding.source_path).map_err(source_error)?;
    let (parent, _) = binding
        .source_path
        .rsplit_once('/')
        .ok_or(DurableError::Corrupt("PG Work recovery source parent"))?;
    let expected = BTreeSet::from([
        binding.source_path.clone(),
        format!("{parent}/{}", names[1]),
        format!("{parent}/{}", source_revisions::HISTORY),
    ]);
    let actual = writes
        .iter()
        .map(|write| write.subject.clone())
        .collect::<BTreeSet<_>>();
    if expected.len() != 3 || actual.len() != writes.len() || !expected.is_subset(&actual) {
        return Err(DurableError::Corrupt(
            "ready PG Work recovery write scope differs",
        ));
    }
    let record_raw = writes
        .iter()
        .find(|write| write.subject == binding.source_path)
        .ok_or(DurableError::Corrupt(
            "ready PG Work recovery record absent",
        ))?;
    let record = cmd::parse(&record_raw.exact_bytes).map_err(source_error)?;
    let subject = source_forms::metadata_subject(&record).map_err(source_error)?;
    let family_parts = binding.family_tag.split(':').collect::<Vec<_>>();
    if family_parts.len() != 3 {
        return Err(DurableError::Corrupt(
            "ready PG selected recovery family tag",
        ));
    }
    let family = source_revisions::RevisionFamily::parse(family_parts[0]).map_err(source_error)?;
    let record_schema = cmd::text(&record, "schema_version").map_err(source_error)?;
    let record_type = match record_schema {
        "tos_artifact_source_witness_v1" | "tos_artifact_source_witness_v2" => "artifact",
        "tos_scholarly_composite_witness_v1" => "composite",
        _ => cmd::text(&record, "record_type").map_err(source_error)?,
    };
    let schema_matches = if family == source_revisions::RevisionFamily::NativeSelected {
        family_parts[2] == record_schema
    } else {
        family_parts[2].is_empty()
    };
    if !family.selected()
        || family_parts[1] != record_type
        || !schema_matches
        || cmd::text(&subject, "id").map_err(source_error)? != binding.record_id
    {
        return Err(DurableError::Conflict(
            "ready PG selected recovery record identity differs",
        ));
    }
    let mut package = source_revisions::Package::new();
    for name in &names {
        let path = format!("{parent}/{name}");
        let write =
            writes
                .iter()
                .find(|write| write.subject == path)
                .ok_or(DurableError::Corrupt(
                    "ready PG Work recovery member absent",
                ))?;
        package.insert(name.clone(), write.exact_bytes.clone());
    }
    let history = source_revisions::history(&package, &record).map_err(source_error)?;
    let latest = cmd::array(&history, "receipts")
        .map_err(source_error)?
        .last()
        .ok_or(DurableError::Corrupt(
            "ready PG Work recovery receipt absent",
        ))?;
    if cmd::text(latest, "command_id").map_err(source_error)? != binding.command_id
        || cmd::text(latest, "request_digest").map_err(source_error)?
            != binding.request_canonical_digest.to_prefixed()
        || cmd::text(latest, "owner_configuration").map_err(source_error)?
            != binding.owner_configuration_canonical_digest.to_prefixed()
        || cmd::text(
            cmd::field(latest, "publication").map_err(source_error)?,
            "transaction_id",
        )
        .map_err(source_error)?
            != binding.transaction_id
    {
        return Err(DurableError::Conflict(
            "ready PG Work recovery receipt binding differs",
        ));
    }
    let recorded_at = cmd::text(latest, "recorded_at")
        .map_err(source_error)?
        .to_owned();
    cmd::validate_instant(&recorded_at).map_err(source_error)?;

    // A selected revision also writes the immutable archive for the predecessor
    // package. Admit only its exact owner-derived manifest and the blobs bound
    // by that manifest; arbitrary extra writes are not part of ready recovery.
    // The owner may already have retained these exact bytes, in which case its
    // ordinary reprepare validates the existing archive and correctly emits no
    // archive writes. The caller always performs that reprepare before commit.
    if actual == expected {
        return Ok(recorded_at);
    }
    let archive = source_revisions::archive_path(
        &cmd::object(vec![("record_id", cmd::string(&binding.record_id))]),
        cmd::text(latest, "previous_revision").map_err(source_error)?,
    )
    .map_err(source_error)?;
    let manifest_path = format!("{archive}/manifest.json");
    let manifest_write = writes
        .iter()
        .find(|write| write.subject == manifest_path)
        .ok_or(DurableError::Corrupt(
            "ready PG Work predecessor archive manifest absent",
        ))?;
    let manifest = cmd::parse(&manifest_write.exact_bytes).map_err(source_error)?;
    cmd::exact_keys(
        &manifest,
        &[
            "schema_version",
            "source_path",
            "source",
            "revision",
            "files",
            "publication_protocol",
        ],
    )
    .map_err(source_error)?;
    if cmd::text(&manifest, "schema_version").map_err(source_error)?
        != "tos_source_package_archive_v2"
        || cmd::text(&manifest, "source_path").map_err(source_error)? != binding.source_path
        || !cmd::same(
            cmd::field(&manifest, "source").map_err(source_error)?,
            cmd::field(latest, "previous_source").map_err(source_error)?,
        )
        .map_err(source_error)?
        || cmd::text(&manifest, "revision").map_err(source_error)?
            != cmd::text(latest, "previous_revision").map_err(source_error)?
        || cmd::text(&manifest, "publication_protocol").map_err(source_error)?
            != "tos_selected_source_metadata_v1"
    {
        return Err(DurableError::Conflict(
            "ready PG Work predecessor archive binding differs",
        ));
    }
    let archive_files = cmd::field(&manifest, "files")
        .map_err(source_error)?
        .as_object()
        .ok_or(DurableError::Corrupt(
            "ready PG Work predecessor archive files absent",
        ))?;
    if archive_files.len() != names.len()
        || archive_files.iter().any(|(name, _)| {
            name.as_str()
                .is_none_or(|name| !names.iter().any(|expected| expected == name))
        })
    {
        return Err(DurableError::Conflict(
            "ready PG Work predecessor archive package differs",
        ));
    }
    let mut expected_writes = expected;
    expected_writes.insert(manifest_path);
    let mut archive_package = source_revisions::Package::new();
    let mut archive_package_bytes = 0usize;
    for (name, archive_binding) in archive_files {
        let name = name
            .as_str()
            .ok_or(DurableError::Corrupt("ready PG Work archive filename"))?;
        cmd::exact_keys(archive_binding, &["blob", "sha256", "bytes"]).map_err(source_error)?;
        let digest = cmd::text(archive_binding, "sha256").map_err(source_error)?;
        let blob = cmd::text(archive_binding, "blob").map_err(source_error)?;
        let blob_digest = digest
            .strip_prefix("sha256:")
            .ok_or(DurableError::Corrupt("ready PG Work archive blob digest"))?;
        if blob != format!("{blob_digest}.blob") {
            return Err(DurableError::Conflict(
                "ready PG Work archive blob locator differs",
            ));
        }
        let blob_path = format!("{archive}/{blob}");
        let blob_write = writes
            .iter()
            .find(|write| write.subject == blob_path)
            .ok_or(DurableError::Corrupt(
                "ready PG Work predecessor archive blob absent",
            ))?;
        archive_package_bytes = archive_package_bytes
            .checked_add(blob_write.exact_bytes.len())
            .filter(|bytes| blob_write.exact_bytes.len() <= 2_097_152 && *bytes <= 8_388_608)
            .ok_or(DurableError::Refused(
                "ready PG Work predecessor archive byte budget",
            ))?;
        if Digest256::of_bytes(&blob_write.exact_bytes).to_prefixed() != digest
            || u64::try_from(blob_write.exact_bytes.len()).ok()
                != Some(cmd::integer(archive_binding, "bytes").map_err(source_error)?)
        {
            return Err(DurableError::Conflict(
                "ready PG Work predecessor archive blob differs",
            ));
        }
        archive_package.insert(name.to_owned(), blob_write.exact_bytes.clone());
        expected_writes.insert(blob_path);
    }
    if actual != expected_writes {
        return Err(DurableError::Corrupt(
            "ready PG Work recovery write scope differs",
        ));
    }
    let previous_revision = cmd::text(latest, "previous_revision").map_err(source_error)?;
    if source_revisions::revision(&archive_package).map_err(source_error)? != previous_revision {
        return Err(DurableError::Conflict(
            "ready PG Work predecessor archive revision differs",
        ));
    }
    let archived_record = cmd::parse(archive_package.get(&names[0]).ok_or(
        DurableError::Corrupt("ready PG Work predecessor archive record absent"),
    )?)
    .map_err(source_error)?;
    if !cmd::same(
        &source_forms::metadata_subject(&archived_record).map_err(source_error)?,
        cmd::field(latest, "previous_source").map_err(source_error)?,
    )
    .map_err(source_error)?
    {
        return Err(DurableError::Conflict(
            "ready PG Work predecessor archive source differs",
        ));
    }
    Ok(recorded_at)
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
    Managed(&'a crate::source_current_cut::ManagedCurrentSourceGeneration),
    CommittedReplay,
}

#[derive(Clone)]
pub(crate) struct SourceReads {
    pub(crate) reads: Vec<PredicateRead>,
    pub(crate) locators: BTreeMap<String, Digest256>,
    pub(crate) managed_original: Option<serde_json::Value>,
    pub(crate) revision: Option<SourceRevisionReadBinding>,
}

/// Persisted identity for a family-tagged authored PG revision. This travels
/// inside the existing canonical source-read envelope, so replay recovers the
/// exact event cut without a schema migration or an untyped side channel.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceRevisionReadBinding {
    pub(crate) family_tag: String,
    pub(crate) source_head_sequence: u64,
    pub(crate) predecessor_source_revision: SourceRevision,
    /// Canonical semantic request identity retained independently from the
    /// raw stdin digest on `cmd2_attempt`. Owner receipts may bind this value
    /// while replay still requires the exact raw request bytes.
    pub(crate) request_canonical_digest: Option<Digest256>,
    /// Exact bounded owner configuration needed to rederive historical Claim
    /// projections after a process restart. It is retained only for the
    /// dedicated Forms/Historical route; outer attempt columns bind both
    /// digests and the original command still requires its exact raw retry.
    pub(crate) owner_configuration_raw: Option<Vec<u8>>,
    pub(crate) owner_configuration_raw_digest: Option<Digest256>,
    pub(crate) owner_configuration_canonical_digest: Option<Digest256>,
}
/// Caller-owned logical source work for the maintained Agent operation.
/// It is descriptive accounting, never a source/custody authority. Gate and
/// control queries, PG protocol/page I/O, and failed STO calls without a work
/// receipt are excluded. Completed packed STO work includes active-buffer
/// reads; it must not be labeled physical disk I/O.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ManagedSourceWorkV1 {
    pub addressed_continuations_attempted: u64,
    pub legacy_continuations_attempted: u64,
    pub audit: audit_delta::AuditDeltaWork,
    pub tree: tos_segment_store::AuthenticatedTreeWorkV1,
    pub projection_admission_rows_witnessed: u64,
    pub projection_admission_bytes_witnessed: u64,
    pub projection_rows_returned: u64,
    pub projection_payload_bytes_returned: u64,
    pub projection_subject_bytes_returned: u64,
    pub catalogue_section_rows_returned: u64,
    pub catalogue_projection_bytes_returned: u64,
    pub catalogue_entries_delivered: u64,
    pub point_projection_rows_returned: u64,
    pub point_projection_bytes_returned: u64,
}
impl ManagedSourceWorkV1 {
    pub(crate) fn charge(counter: &mut u64, value: u64) -> DurableResult<()> {
        *counter = counter.checked_add(value).ok_or(DurableError::Invalid(
            "managed source work counter overflow",
        ))?;
        Ok(())
    }
    pub(crate) fn charge_tree(
        &mut self,
        work: tos_segment_store::AuthenticatedTreeWorkV1,
    ) -> DurableResult<()> {
        Self::charge(&mut self.tree.read_nodes, work.read_nodes)?;
        Self::charge(&mut self.tree.read_bytes, work.read_bytes)?;
        Self::charge(&mut self.tree.written_nodes, work.written_nodes)?;
        Self::charge(&mut self.tree.written_bytes, work.written_bytes)
    }
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
    writes: Vec<SourceRevisionWrite>,
    continuation: Option<WarmContinuation>,
}

/// A bounded authored revision registered against the real current PG/STO
/// source. Its exact family and read closure are retained in
/// `cmd2_attempt.source_reads`; the in-memory copy is the typed commit view.
pub(crate) struct SourceRevisionAttempt {
    pub(crate) domain: String,
    pub(crate) store_id: [u8; 16],
    pub(crate) prepare_id: Vec<u8>,
    pub(crate) fence: u64,
    pub(crate) command_id: String,
    pub(crate) raw_request_digest: Digest256,
    pub(crate) configuration_raw_digest: Digest256,
    pub(crate) configuration_canonical_digest: Digest256,
    pub(crate) epoch: u64,
    pub(crate) source_generation: u64,
    pub(crate) audit_generation: u64,
    pub(crate) rights_version: u64,
    pub(crate) rule_version: u64,
    pub(crate) job_id: String,
    pub(crate) job_fence: u64,
    pub(crate) contract: Digest256,
    pub(crate) definition: Digest256,
    pub(crate) delta: Digest256,
    /// Exact maintained owner family and type/schema tag used for admission.
    pub(crate) family_tag: String,
    /// Source head before this command was registered. Legacy attempts may
    /// lack the original selection sequence and expose `None` on recovery.
    pub(crate) source_head_sequence: Option<u64>,
    pub(crate) predecessor_source_revision: SourceRevision,
    pub(crate) reads: SourceReads,
    pub(in crate::durable_adapter) metadata: SourceMetadata,
    /// Only structural source-index rows created by this revision are bound
    /// here. Existing semantic rows remain in place after an index-preserving
    /// update; newly inserted archive members still need their path rows for
    /// the complete cold verifier.
    pub(crate) indexes: IndexRows,
    pub(crate) predicate_keys: BTreeSet<(String, String, String)>,
    pub(crate) writes: Vec<SourceRevisionWrite>,
    /// Canonical owner-specific inventory projections sealed with the source
    /// writes. These bytes are retained through attempt recovery and attached
    /// to current/history rows in the same commit transaction.
    pub(crate) projections: ProjectionRows,
    /// Metadata-only sidecar package for the last selected source publication
    /// transition. The sidecar files and transaction blobs never become
    /// authored STO members; the typed package is persisted on the attempt.
    pub(crate) publication_package:
        Option<crate::source_work_transaction::SelectedPublicationPackage>,
    pub(crate) work_revision: Option<crate::source_pg_work_revision::CommitEvidence>,
    pub(crate) recovered_work_recorded_at: Option<String>,
}

/// Typed, owner-prepared package entering the common STO/PG source revision
/// transaction. Each owner keeps its semantic preparation and index law; the
/// shared writer owns only readset fencing, sealing, recovery and commit.
pub(crate) trait PreparedPgSourceRevision {
    fn context(&self) -> &CommandContext;
    fn proposal(&self) -> &cmd::PreparedCommand;
    fn identity(&self) -> revision::PgRecordRevisionIdentity;
    fn family_tag(&self) -> &str;
    fn legacy_family_encoding(&self) -> bool;
    /// Extract the one complete publication metadata package from the owner's
    /// generated transaction changes and return only authored source writes.
    /// Owners keep constructing the selected transaction; the common writer
    /// owns its PG representation and admission boundary.
    fn publication_package_and_source_changes(
        &self,
    ) -> DurableResult<(
        Option<crate::source_work_transaction::SelectedPublicationPackage>,
        Vec<cmd::SourceChange>,
    )> {
        crate::source_work_transaction::extract_selected_publication_package(
            &self.proposal().changes,
        )
        .map_err(DurableError::Source)
    }
    fn validate_readset(&self, readset: &revision::SourceRevisionReadSet) -> DurableResult<()>;
    fn work_revision_evidence(
        &self,
        writes: &[SourceRevisionWrite],
    ) -> DurableResult<Option<crate::source_pg_work_revision::CommitEvidence>>;
    fn check_indexes(
        &self,
        tx: &mut Transaction<'_>,
        domain: &str,
        writes: &[SourceRevisionWrite],
    ) -> DurableResult<IndexRows>;

    /// Return one owner-specific mixed-inventory contribution for each
    /// authored write that this family can project. The common writer binds
    /// each contribution to its exact path and successor bytes, then stores
    /// the canonical mixed-v2 envelope. Other families retain the empty
    /// default until they own an exact contribution contract.
    fn inventory_owner_contributions(
        &self,
        _writes: &[SourceRevisionWrite],
    ) -> DurableResult<BTreeMap<String, tos_foundation::JsonValue>> {
        Ok(BTreeMap::new())
    }
}

impl PreparedPgSourceRevision for crate::source_native_cli::revisions::PreparedPgRecordRevision {
    fn context(&self) -> &CommandContext {
        crate::source_native_cli::revisions::PreparedPgRecordRevision::context(self)
    }

    fn proposal(&self) -> &cmd::PreparedCommand {
        crate::source_native_cli::revisions::PreparedPgRecordRevision::proposal(self)
    }

    fn identity(&self) -> revision::PgRecordRevisionIdentity {
        crate::source_native_cli::revisions::PreparedPgRecordRevision::identity(self)
    }

    fn family_tag(&self) -> &str {
        crate::source_native_cli::revisions::PreparedPgRecordRevision::family_tag(self)
    }

    fn legacy_family_encoding(&self) -> bool {
        crate::source_native_cli::revisions::PreparedPgRecordRevision::legacy_family_encoding(self)
    }

    fn validate_readset(&self, readset: &revision::SourceRevisionReadSet) -> DurableResult<()> {
        let context = self.context();
        let proposal = self.proposal();
        let (owner_configuration, owner_family) =
            source_revisions::configuration(context).map_err(source_error)?;
        let selected_family_tag = owner_family
            .pg_family_tag(&owner_configuration)
            .map_err(source_error)?;
        let selected_legacy_encoding = owner_family
            .pg_legacy_compatible(&owner_configuration)
            .map_err(source_error)?;
        if self.identity() != revision::PgRecordRevisionIdentity::from_selection(&readset.selection)
            || proposal.operation != "record.revise"
            || proposal.base_revision != readset.selection.cohort.initial_revision
            || context.base_revision != readset.selection.cohort.initial_revision
            || self.family_tag() != selected_family_tag
            || self.legacy_family_encoding() != selected_legacy_encoding
        {
            return Err(DurableError::Conflict(
                "source revision proposal base, family or operation differs",
            ));
        }
        // This typed adapter owns record-revision schema admission. Other
        // prepared families retain their own semantic admission and must not
        // be parsed as record-revision owners by the common transaction.
        validate_selected_record_revision_admission(context, proposal, self.family_tag())
    }

    fn work_revision_evidence(
        &self,
        writes: &[SourceRevisionWrite],
    ) -> DurableResult<Option<crate::source_pg_work_revision::CommitEvidence>> {
        self.work_revision()
            .map(|observation| {
                crate::source_pg_work_revision::make_commit_evidence(
                    self.context(),
                    self.proposal(),
                    observation,
                    self.publication_observation_digest()
                        .ok_or(DurableError::Corrupt(
                            "PG Work publication observation digest absent",
                        ))?,
                    writes,
                )
            })
            .transpose()
    }

    fn check_indexes(
        &self,
        tx: &mut Transaction<'_>,
        domain: &str,
        writes: &[SourceRevisionWrite],
    ) -> DurableResult<IndexRows> {
        check_revision_path_indexes(tx, domain, writes)
    }

    fn inventory_owner_contributions(
        &self,
        _writes: &[SourceRevisionWrite],
    ) -> DurableResult<BTreeMap<String, tos_foundation::JsonValue>> {
        if self
            .family_tag()
            .starts_with("tos_local_native_metadata_revision_owner_v1:")
        {
            return source_revisions::pg_native_metadata_owner_contributions(
                self.context(),
                self.proposal(),
            )
            .map_err(source_error);
        }
        Ok(BTreeMap::new())
    }
}

pub(crate) struct WorkRevisionRecoveryBinding {
    pub(crate) domain: String,
    pub(crate) prepare_id: Vec<u8>,
    pub(crate) raw_request_digest: Digest256,
    pub(crate) owner_configuration_raw_digest: Digest256,
    pub(crate) source_path: String,
    pub(crate) record_id: String,
    pub(crate) command_id: String,
    pub(crate) transaction_id: String,
    pub(crate) request_canonical_digest: Digest256,
    pub(crate) owner_configuration_canonical_digest: Digest256,
    pub(crate) job_id: String,
    pub(crate) job_fence: u64,
    pub(crate) family_tag: String,
}

#[derive(Clone)]
pub(crate) struct SourceRevisionWrite {
    pub(crate) subject: String,
    pub(crate) expected_predecessor: Option<(u64, Digest256)>,
    pub(crate) proposed_revision: u64,
    pub(crate) exact_bytes: Vec<u8>,
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
    Bootstrap {
        owner_check: &'a dyn Fn() -> DurableResult<()>,
    },
    Creation {
        attempt: &'a SourceCreationAttempt,
        owner: &'a CreationOwnerFence<'a>,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    },
    Revision {
        attempt: &'a SourceRevisionAttempt,
        owner_check: &'a dyn Fn() -> DurableResult<()>,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    },
    PgCreation {
        attempt: &'a creation::PgAgentCreationAttempt,
        owner_check: &'a dyn Fn() -> DurableResult<()>,
        deadline: Instant,
        cancelled: &'a AtomicBool,
    },
}

fn source_error(error: cmd::SourceCommandError) -> DurableError {
    DurableError::Source(error)
}

/// Keep the authored-source refusal intact while naming the narrow owner
/// phase that produced the otherwise context-free form/schema error.  The
/// native bootstrap adapter preserves `Source` errors, so this context remains
/// visible to the bounded caller without turning a malformed member into a
/// successful optional projection.
fn source_error_with_phase(error: cmd::SourceCommandError, phase: &'static str) -> DurableError {
    match error {
        cmd::SourceCommandError::Invalid("missing object field") => {
            source_error(cmd::SourceCommandError::Invalid(phase))
        }
        error => source_error(error),
    }
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
        part(
            &mut h,
            super::durable_compatibility::SOURCE_COMPATIBILITY_PROFILE_V8,
        );
        h.finalize()
    })
}

fn previous_mixed_definition_v7() -> Digest256 {
    static DEFINITION: std::sync::OnceLock<Digest256> = std::sync::OnceLock::new();
    *DEFINITION.get_or_init(|| {
        let mut h = Digest256Hasher::new();
        part(
            &mut h,
            super::durable_compatibility::SOURCE_COMPATIBILITY_PROFILE_V7,
        );
        h.finalize()
    })
}
fn legacy_agent_definition() -> Digest256 {
    static DEFINITION: std::sync::OnceLock<Digest256> = std::sync::OnceLock::new();
    *DEFINITION.get_or_init(|| {
        let mut h = Digest256Hasher::new();
        part(
            &mut h,
            super::durable_compatibility::SOURCE_COMPATIBILITY_PROFILE_V1,
        );
        h.finalize()
    })
}
fn legacy_mixed_definition() -> Digest256 {
    static DEFINITION: std::sync::OnceLock<Digest256> = std::sync::OnceLock::new();
    *DEFINITION.get_or_init(|| {
        let mut h = Digest256Hasher::new();
        part(
            &mut h,
            super::durable_compatibility::SOURCE_COMPATIBILITY_PROFILE_V2,
        );
        h.finalize()
    })
}

fn previous_mixed_definition_v3() -> Digest256 {
    static DEFINITION: std::sync::OnceLock<Digest256> = std::sync::OnceLock::new();
    *DEFINITION.get_or_init(|| {
        let mut h = Digest256Hasher::new();
        part(
            &mut h,
            super::durable_compatibility::SOURCE_COMPATIBILITY_PROFILE_V3,
        );
        h.finalize()
    })
}

fn previous_mixed_definition_v4() -> Digest256 {
    static DEFINITION: std::sync::OnceLock<Digest256> = std::sync::OnceLock::new();
    *DEFINITION.get_or_init(|| {
        let mut h = Digest256Hasher::new();
        part(
            &mut h,
            super::durable_compatibility::SOURCE_COMPATIBILITY_PROFILE_V4,
        );
        h.finalize()
    })
}

fn previous_mixed_definition_v5() -> Digest256 {
    static DEFINITION: std::sync::OnceLock<Digest256> = std::sync::OnceLock::new();
    *DEFINITION.get_or_init(|| {
        let mut h = Digest256Hasher::new();
        part(
            &mut h,
            super::durable_compatibility::SOURCE_COMPATIBILITY_PROFILE_V5,
        );
        h.finalize()
    })
}

fn previous_mixed_definition_v6() -> Digest256 {
    static DEFINITION: std::sync::OnceLock<Digest256> = std::sync::OnceLock::new();
    *DEFINITION.get_or_init(|| {
        let mut h = Digest256Hasher::new();
        part(
            &mut h,
            super::durable_compatibility::SOURCE_COMPATIBILITY_PROFILE_V6,
        );
        h.finalize()
    })
}

/// Fail with an actionable rebootstrap refusal for the old Agent-only domain
/// profile. Other unknown profile digests remain ordinary definition drift.
pub(super) fn require_current_source_definition(actual: Digest256) -> DurableResult<()> {
    if actual == legacy_agent_definition() {
        return Err(DurableError::Refused(
            "legacy Agent-only source profile requires fresh mixed-source bootstrap",
        ));
    }
    if actual == legacy_mixed_definition() {
        return Err(DurableError::Refused(
            "mixed v1 source projections require fresh owner-contribution bootstrap",
        ));
    }
    if actual == previous_mixed_definition_v3() {
        return Err(DurableError::Refused(
            "mixed v3 source profile requires fresh Claim dependency bootstrap",
        ));
    }
    if actual == previous_mixed_definition_v4() {
        return Err(DurableError::Refused(
            "mixed v4 source profile requires fresh Item File assignment bootstrap",
        ));
    }
    if actual == previous_mixed_definition_v5() {
        return Err(DurableError::Refused(
            "mixed v5 source profile requires fresh Sign candidate index bootstrap",
        ));
    }
    if actual == previous_mixed_definition_v6() {
        return Err(DurableError::Refused(
            "mixed v6 source profile requires fresh Forms/Historical owner projection bootstrap",
        ));
    }
    if actual == previous_mixed_definition_v7() {
        return Err(DurableError::Refused(
            "mixed v7 source profile requires Claim navigation interval bootstrap",
        ));
    }
    if actual != definition() {
        return Err(DurableError::Conflict(
            "mixed public-metadata source definition differs",
        ));
    }
    Ok(())
}
pub(super) fn known_profile(profile: &[u8]) -> bool {
    [
        PROFILE_ID,
        ORIGINAL,
        CREATION,
        REVISION,
        creation::PG_CREATION,
    ]
    .contains(&profile)
}
pub(super) fn check_source_member(
    profile: &[u8],
    member: &DurableShadowMember,
) -> DurableResult<()> {
    let valid_path =
        member.subject.starts_with("ToS/") && RelativePath::parse(&member.subject).is_ok();
    let initial = [ORIGINAL, CREATION, creation::PG_CREATION].contains(&profile)
        && member.expected_predecessor.is_none()
        && member.proposed_revision == 1;
    let successor = profile == REVISION
        && match member.expected_predecessor {
            Some((revision, _digest)) => revision.checked_add(1) == Some(member.proposed_revision),
            None => member.proposed_revision == 1,
        };
    if !(known_profile(profile) && valid_path && (initial || successor)) {
        return Err(DurableError::Invalid(
            "source file profile or custody version",
        ));
    }
    // Custody revisions are carrier versions. They never assign or change an
    // embedded authored record/Claim/form revision.
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
        || (profile == creation::PG_CREATION && (!managed || !row.get::<_, bool>(1)))
        || (profile == REVISION && (!managed || !row.get::<_, bool>(1)))
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
    let inventory =
        source_claims::maintained_inventory(ctx, worker, deadline, cancelled).map_err(|error| {
            source_error_with_phase(error, "missing object field in maintained source index")
        })?;
    let mut rows = BTreeMap::<(String, String), String>::new();
    let mut identity_paths = BTreeMap::<String, String>::new();
    fn insert(
        rows: &mut BTreeMap<(String, String), String>,
        identity_paths: &mut BTreeMap<String, String>,
        kind: &str,
        id: &str,
        path: &str,
    ) -> DurableResult<()> {
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
    }
    for (id, entry) in &inventory.objects {
        insert(
            &mut rows,
            &mut identity_paths,
            "metadata",
            id,
            cmd::text(entry, "source_record_ref").map_err(|error| {
                source_error_with_phase(error, "missing object field in metadata index row")
            })?,
        )?;
    }
    for (id, entry) in &inventory.claims {
        insert(
            &mut rows,
            &mut identity_paths,
            "claim",
            id,
            cmd::text(entry, "source_claim_file_ref").map_err(|error| {
                source_error_with_phase(error, "missing object field in claim index row")
            })?,
        )?;
    }
    for (kind, entries) in [("event", &inventory.events), ("anchor", &inventory.anchors)] {
        for (id, entry) in entries
            .as_object()
            .ok_or(DurableError::Corrupt("owner evidence index object"))?
        {
            insert(
                &mut rows,
                &mut identity_paths,
                kind,
                id.as_str()
                    .ok_or(DurableError::Corrupt("owner evidence identity string"))?,
                cmd::text(entry, "source_ref").map_err(|error| {
                    source_error_with_phase(error, "missing object field in evidence index row")
                })?,
            )?;
        }
    }
    for file in &ctx.files {
        active(deadline, cancelled)?;
        if !file.path.as_str().starts_with("ToS/") {
            continue;
        }
        insert(
            &mut rows,
            &mut identity_paths,
            "path",
            file.path.as_str(),
            file.path.as_str(),
        )?;
        if file.path.as_str().starts_with("ToS/source-witnesses/")
            && file.path.as_str().ends_with(".human-forms.json")
        {
            let set = cmd::parse(&file.raw).map_err(|error| {
                source_error_with_phase(error, "missing object field in HumanForm source set")
            })?;
            source_forms::apply_form_changes(
                Some(&set),
                cmd::field(&set, "subject").map_err(|error| {
                    source_error_with_phase(error, "missing object field in HumanForm subject")
                })?,
                &[],
            )
            .map_err(|error| {
                source_error_with_phase(error, "missing object field in HumanForm history")
            })?;
            for section in ["forms", "prior_forms"] {
                for form in cmd::array(&set, section).map_err(|error| {
                    source_error_with_phase(error, "missing object field in HumanForm section")
                })? {
                    insert(
                        &mut rows,
                        &mut identity_paths,
                        "form",
                        cmd::text(form, "form_id").map_err(|error| {
                            source_error_with_phase(
                                error,
                                "missing object field in HumanForm identity",
                            )
                        })?,
                        file.path.as_str(),
                    )?;
                }
            }
        }
        if file.path.as_str().starts_with("ToS/source-witnesses/")
            && file.path.as_str().ends_with("/sign.json")
        {
            let candidate = crate::source_sign::pg::candidate_id_from_current_bytes(
                file.path.as_str(),
                &file.raw,
            )
            .map_err(source_error)?;
            let key = ("sign-candidate".to_owned(), candidate);
            if rows
                .get(&key)
                .is_some_and(|prior| prior != file.path.as_str())
            {
                return Err(DurableError::Conflict(
                    "Sign candidate already issued in selected source cohort",
                ));
            }
            // A promotion candidate Claim ID is intentionally in its own
            // uniqueness namespace. It necessarily equals a `claim` token,
            // so it must not participate in the generic cross-kind identity
            // collision check above.
            rows.insert(key, file.path.as_str().to_owned());
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
    let authored_members = ctx
        .files
        .iter()
        .filter(|file| file.path.as_str().starts_with("ToS/"))
        .cloned()
        .collect::<Vec<_>>();
    let has_global_claims = authored_members.iter().try_fold(false, |found, file| {
        Ok::<_, DurableError>(found || is_global_claim_stream(ctx, file.path.as_str())?)
    })?;
    let mut growth_claim_contributions = BTreeMap::new();
    for file in &authored_members {
        let location = file.path.as_str();
        if location.rsplit('/').next() != Some(source_claims::CLAIM_STREAM) {
            continue;
        }
        if let Some(contributions) =
            crate::source_creation_store::current_pg_growth_owner_contributions(ctx, location)?
        {
            for (path, contribution) in contributions {
                if path == location
                    || path.rsplit('/').next().is_some_and(|name| {
                        name.starts_with("source-claims.") && name.ends_with(".human-forms.json")
                    })
                {
                    if growth_claim_contributions
                        .insert(path, contribution)
                        .is_some()
                    {
                        return Err(DurableError::Corrupt(
                            "PG Growth current Claim contribution repeats",
                        ));
                    }
                }
            }
        }
    }
    let has_imported_legacy_claims = authored_members.iter().any(|file| {
        source_claims::is_imported_legacy_claim_path(file.path.as_str())
            || source_claims::nonprojecting_custody_kind(file.path.as_str()).is_some()
    });
    let has_historical_claims = authored_members
        .iter()
        .any(|file| source_claims::is_historical_claim_stream_path(file.path.as_str()));
    let has_item_claim_streams = authored_members
        .iter()
        .any(|file| is_item_claim_stream(file.path.as_str()));
    let has_managed_claim_owners = has_global_claims
        || has_historical_claims
        || has_item_claim_streams
        || !growth_claim_contributions.is_empty();
    let inventory = if has_imported_legacy_claims || has_historical_claims {
        source_claims::maintained_inventory_from_members(
            ctx,
            &authored_members,
            true,
            worker,
            deadline,
            cancelled,
        )
    } else if has_managed_claim_owners {
        source_claims::maintained_global_claim_inventory_from_members(
            ctx,
            &authored_members,
            worker,
            deadline,
            cancelled,
        )
    } else {
        source_claims::maintained_agent_inventory_from_managed(
            ctx, true, worker, deadline, cancelled,
        )
    }
    .map_err(|error| {
        source_error_with_phase(
            error,
            "missing object field in maintained Agent projection inventory",
        )
    })?;
    let grouped =
        source_claims::agent_inventory_members(ctx, &inventory, worker, deadline, cancelled)
            .map_err(|error| {
                source_error_with_phase(error, "missing object field in Agent projection grouping")
            })?;
    let mut historical_claim_owners = BTreeMap::new();
    for file in authored_members
        .iter()
        .filter(|file| source_claims::is_historical_claim_stream_path(file.path.as_str()))
    {
        let owner = source_claims::pg_inventory::historical_claim_owner_contribution(
            ctx, file, &grouped, worker, deadline, cancelled,
        )
        .map_err(|error| {
            source_error_with_phase(
                error,
                "historical Claim owner projection differs from exact source package",
            )
        })?;
        if historical_claim_owners
            .insert(file.path.as_str().to_owned(), owner)
            .is_some()
        {
            return Err(DurableError::Corrupt(
                "historical Claim owner projection path repeats",
            ));
        }
    }
    let mut projections = ProjectionRows::new();
    let mut projection_bytes = 0usize;
    for file in ctx.files.iter().filter(|file| {
        file.path.as_str().starts_with("ToS/")
            && paths.is_none_or(|paths| paths.contains(file.path.as_str()))
    }) {
        active(deadline, cancelled)?;
        let location = file.path.as_str();
        let basename = location.rsplit('/').next().unwrap_or(location);
        let form_owner_stream = location
            .rsplit_once('/')
            .map(|(parent, _)| format!("{parent}/{}", source_claims::CLAIM_STREAM));
        let historical_owner_stream = if source_claims::is_historical_claim_stream_path(location) {
            Some(location.to_owned())
        } else {
            source_claims::historical_claim_form_stream_path(location)
        };
        let is_global_claim_form = match form_owner_stream.as_deref() {
            Some(path) if ctx.files.iter().any(|member| member.path.as_str() == path) => {
                is_global_claim_stream(ctx, path)?
            }
            _ => false,
        };
        let is_item_claim_form = item_claim_form_owner_stream(ctx, location).is_some();
        let contribution = if let Some(contribution) = growth_claim_contributions.get(location) {
            contribution.clone()
        } else if let Some(owner_stream) = historical_owner_stream.as_deref() {
            let owner = historical_claim_owners.get(owner_stream);
            source_claims::pg_inventory::historical_claim_inventory_contribution(
                file, &grouped, owner, worker, deadline, cancelled,
            )
            .map_err(|error| {
                source_error_with_phase(
                    error,
                    "historical Claim member projection differs from exact source package",
                )
            })?
        } else if has_global_claims
            && basename == source_claims::CLAIM_STREAM
            && is_global_claim_stream(ctx, location)?
        {
            source_claims::pg_inventory::claim_inventory_contribution(
                ctx, file, &grouped, worker, deadline, cancelled,
            )
            .map_err(|error| {
                source_error_with_phase(
                    error,
                    "missing object field in Agent projection member or forms",
                )
            })?
        } else if basename.ends_with(".human-forms.json")
            && (is_global_claim_form || is_item_claim_form)
        {
            if is_item_claim_form {
                let owner_stream = item_claim_form_owner_stream(ctx, location)
                    .ok_or(DurableError::Corrupt("Item Claim form owner stream absent"))?;
                item_claim_owner_contribution(ctx, &owner_stream, deadline, cancelled)?;
            }
            source_claims::pg_inventory::claim_form_inventory_contribution(
                file, &grouped, worker, deadline, cancelled,
            )
            .map_err(|error| {
                source_error_with_phase(
                    error,
                    "missing object field in Agent projection member or forms",
                )
            })?
        } else if is_item_claim_stream(location) {
            let owner = item_claim_owner_contribution(ctx, location, deadline, cancelled)?;
            let mut contribution = source_claims::agent_inventory_contribution(
                ctx, file, &inventory, &grouped, worker, deadline, cancelled,
            )
            .map_err(source_error)?;
            cmd::set(&mut contribution, "claim_owner", owner).map_err(source_error)?;
            contribution
        } else {
            source_claims::agent_inventory_contribution(
                ctx, file, &inventory, &grouped, worker, deadline, cancelled,
            )
            .map_err(|error| {
                source_error_with_phase(
                    error,
                    "missing object field in Agent projection member or forms",
                )
            })?
        };
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

/// Bind a current Item Claim form route to the exact selected Item stream and
/// form bytes. Both full and streamed projection dispatchers use this gate.
fn item_claim_form_owner_stream(ctx: &CommandContext, location: &str) -> Option<String> {
    let stream = source_claims::pg_inventory::claim_form_stream_path(location)?;
    if !is_item_claim_stream(&stream)
        || ctx
            .files
            .iter()
            .filter(|member| member.path.as_str() == location)
            .count()
            != 1
        || ctx
            .files
            .iter()
            .filter(|member| member.path.as_str() == stream)
            .count()
            != 1
    {
        return None;
    }
    Some(stream)
}

fn is_item_claim_stream(path: &str) -> bool {
    let stream_suffix = format!("/{}", source_claims::CLAIM_STREAM);
    if !path.ends_with(&stream_suffix) || RelativePath::parse(path).is_err() {
        return false;
    }
    let Some(relative) = path.strip_prefix("ToS/source-witnesses/") else {
        return false;
    };
    let components = relative.split('/').collect::<Vec<_>>();
    // Keep the established direct Item route, whose contract is exactly one
    // Item home under `items/<slug>`.
    if components.len() == 3
        && components[0] == "items"
        && !components[1].is_empty()
        && components[2] == source_claims::CLAIM_STREAM
    {
        return true;
    }
    if components.len() < 6 || components.last().copied() != Some(source_claims::CLAIM_STREAM) {
        return false;
    }
    let item_home_index = components.len() - 3;
    if components[item_home_index] != "items" || components[item_home_index + 1].is_empty() {
        return false;
    }
    components[..item_home_index]
        .iter()
        .enumerate()
        .any(|(index, component)| *component == "editions" && index + 1 < item_home_index)
}

fn is_global_claim_stream(ctx: &CommandContext, path: &str) -> DurableResult<bool> {
    if !path.starts_with("ToS/source-witnesses/relations/")
        || !path.ends_with(&format!("/{}", source_claims::CLAIM_STREAM))
        || path.split('/').count() != 5
    {
        return Ok(false);
    }
    let home = path
        .strip_suffix(&format!("/{}", source_claims::CLAIM_STREAM))
        .ok_or(DurableError::Corrupt("Global Claim stream path"))?;
    let request_path = format!("{home}/source-create-request.json");
    let mut matching = ctx
        .files
        .iter()
        .filter(|file| file.path.as_str() == request_path);
    let Some(request_file) = matching.next() else {
        return Ok(false);
    };
    if matching.next().is_some() {
        return Err(DurableError::Corrupt(
            "duplicate Global Claim creation request path",
        ));
    }
    let request = cmd::parse(&request_file.raw).map_err(source_error)?;
    Ok(matches!(
        cmd::text(&request, "operation").map_err(source_error)?,
        "claims.create" | "claim.revise"
    ))
}

fn selected_direct_member_map(
    ctx: &CommandContext,
    home: &str,
) -> DurableResult<BTreeMap<String, Vec<u8>>> {
    let prefix = format!("{home}/");
    let mut files = BTreeMap::new();
    for file in &ctx.files {
        let path = file.path.as_str();
        let Some(name) = path.strip_prefix(&prefix) else {
            continue;
        };
        if name.is_empty() || name.contains('/') {
            continue;
        }
        if files.insert(name.to_owned(), file.raw.clone()).is_some() {
            return Err(DurableError::Corrupt(
                "duplicate selected Item package member",
            ));
        }
    }
    Ok(files)
}

/// Recreate the exact Item Claim owner envelope from its current 3+15
/// Edition/Item package and the retained predecessor archive. This uses the
/// same owner verifier as proposal preparation; the mixed projection is not
/// reconstructed from path naming alone.
fn item_claim_transaction_owner_contribution(
    ctx: &CommandContext,
    stream_path: &str,
) -> DurableResult<tos_foundation::JsonValue> {
    use tos_validation::native_compound::EditionItemBytes;

    if !is_item_claim_stream(stream_path) {
        return Err(DurableError::Refused(
            "Item Claim owner is outside the maintained Item namespace",
        ));
    }
    let item_home = stream_path
        .strip_suffix(&format!("/{}", source_claims::CLAIM_STREAM))
        .ok_or(DurableError::Corrupt("Item Claim stream path"))?;
    let (edition_home, _) = item_home
        .rsplit_once("/items/")
        .ok_or(DurableError::Corrupt("Item parent Edition home"))?;
    if !edition_home.starts_with("ToS/source-witnesses/") {
        return Err(DurableError::Corrupt("Item parent Edition namespace"));
    }
    let parent = selected_direct_member_map(ctx, edition_home)?;
    let child = selected_direct_member_map(ctx, item_home)?;
    let receipt_raw = child
        .get("edition-item-receipt.json")
        .ok_or(DurableError::Corrupt("Item compound receipt absent"))?;
    let receipt = cmd::parse(receipt_raw).map_err(source_error)?;
    let parent_history_raw = parent
        .get(source_revisions::HISTORY)
        .ok_or(DurableError::Corrupt("Item parent Edition history absent"))?;
    let parent_history = cmd::parse(parent_history_raw).map_err(source_error)?;
    let parent_receipt = cmd::array(&parent_history, "receipts")
        .map_err(source_error)?
        .last()
        .cloned()
        .ok_or(DurableError::Corrupt(
            "Item parent Edition transition receipt absent",
        ))?;
    let edition_path = format!("{edition_home}/edition.json");
    let edition_raw = parent
        .get("edition.json")
        .ok_or(DurableError::Corrupt("Item parent Edition record absent"))?;
    let edition = cmd::parse(edition_raw).map_err(source_error)?;
    let archive_config = cmd::object(vec![
        (
            "record_id",
            cmd::field(&edition, "record_id")
                .map_err(source_error)?
                .clone(),
        ),
        ("source_path", cmd::string(&edition_path)),
    ]);
    if cmd::text(&receipt, "parent_archive_ref").map_err(source_error)?
        != cmd::text(&parent_receipt, "archive_path").map_err(source_error)?
    {
        return Err(DurableError::Conflict(
            "Item parent archive differs from Edition transition",
        ));
    }
    let (predecessor, _) = source_revisions::read_archive(ctx, &archive_config, &parent_receipt)
        .map_err(source_error)?;
    let bytes = EditionItemBytes {
        parent,
        child,
        parent_receipt: serde_json::from_slice(
            &cmd::canonical(&parent_receipt).map_err(source_error)?,
        )
        .map_err(|_| DurableError::Corrupt("Item parent receipt JSON conversion"))?,
        receipt: serde_json::from_slice(&cmd::canonical(&receipt).map_err(source_error)?)
            .map_err(|_| DurableError::Corrupt("Item receipt JSON conversion"))?,
        transaction_id: cmd::text(&receipt, "transaction_id")
            .map_err(source_error)?
            .to_owned(),
        archive_path: cmd::text(&receipt, "parent_archive_ref")
            .map_err(source_error)?
            .to_owned(),
        reads: Vec::new(),
        bytes_read: 0,
    };
    crate::source_creation_store::pg_claim_owner_contribution(
        &bytes,
        edition_home,
        item_home,
        &predecessor,
    )
    .map_err(source_error)
}

/// Reconstruct the immutable adoption transaction from the retained record
/// archives, then bind it separately from the current Edition and Item tips.
/// The historical V1 owner remains strict about its original 3+15 package.
fn item_claim_owner_contribution(
    ctx: &CommandContext,
    stream_path: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<tos_foundation::JsonValue> {
    use tos_validation::native_compound::EditionItemBytes;

    if !is_item_claim_stream(stream_path) {
        return Err(DurableError::Refused(
            "Item Claim owner is outside the maintained Item namespace",
        ));
    }
    let item_home = stream_path
        .strip_suffix(&format!("/{}", source_claims::CLAIM_STREAM))
        .ok_or(DurableError::Corrupt("Item Claim stream path"))?;
    let (edition_home, _) = item_home
        .rsplit_once("/items/")
        .ok_or(DurableError::Corrupt("Item parent Edition home"))?;
    if !edition_home.starts_with("ToS/source-witnesses/") {
        return Err(DurableError::Corrupt("Item parent Edition namespace"));
    }

    let current_parent = selected_direct_member_map(ctx, edition_home)?;
    let current_child = selected_direct_member_map(ctx, item_home)?;
    let edition_path = format!("{edition_home}/edition.json");
    let item_path = format!("{item_home}/item.json");
    let edition_record = cmd::parse(
        current_parent
            .get("edition.json")
            .ok_or(DurableError::Corrupt("current Edition record absent"))?,
    )
    .map_err(source_error)?;
    let item_record = cmd::parse(
        current_child
            .get("item.json")
            .ok_or(DurableError::Corrupt("current Item record absent"))?,
    )
    .map_err(source_error)?;
    let edition_archive_config = cmd::object(vec![
        (
            "record_id",
            cmd::field(&edition_record, "record_id")
                .map_err(source_error)?
                .clone(),
        ),
        ("source_path", cmd::string(&edition_path)),
    ]);
    let item_archive_config = cmd::object(vec![
        (
            "record_id",
            cmd::field(&item_record, "record_id")
                .map_err(source_error)?
                .clone(),
        ),
        ("source_path", cmd::string(&item_path)),
    ]);

    // This checks current history semantics, every archive's bytes and
    // manifest, and each archived predecessor's exact history prefix.
    let edition_history = source_revisions::verify_owner_history_archives(
        ctx,
        &edition_archive_config,
        &current_parent,
        &edition_record,
        deadline,
        cancelled,
    )
    .map_err(source_error)?;
    let item_history = source_revisions::verify_owner_history_archives(
        ctx,
        &item_archive_config,
        &current_child,
        &item_record,
        deadline,
        cancelled,
    )
    .map_err(source_error)?;
    let edition_receipts = cmd::array(&edition_history, "receipts").map_err(source_error)?;
    let item_receipts = cmd::array(&item_history, "receipts").map_err(source_error)?;
    let receipt = cmd::parse(
        current_child
            .get("edition-item-receipt.json")
            .ok_or(DurableError::Corrupt("Item compound receipt absent"))?,
    )
    .map_err(source_error)?;
    let request_raw = current_child
        .get("source-create-request.json")
        .ok_or(DurableError::Corrupt("Item creation request absent"))?;
    let request = cmd::parse(request_raw).map_err(source_error)?;
    let mut canonical_request = cmd::canonical(&request).map_err(source_error)?;
    canonical_request.push(b'\n');
    if canonical_request.as_slice() != request_raw.as_slice()
        || cmd::record_digest(&request)
            .map_err(source_error)?
            .to_prefixed()
            != cmd::text(&receipt, "request_digest").map_err(source_error)?
    {
        return Err(DurableError::Conflict(
            "retained Item creation request differs from its native receipt",
        ));
    }
    let principal_id = cmd::text(&receipt, "principal_id")
        .map_err(source_error)?
        .to_owned();
    let transition_sha = cmd::text(&receipt, "parent_transition_sha256").map_err(source_error)?;
    let transition_archive = cmd::text(&receipt, "parent_archive_ref").map_err(source_error)?;
    let parent_before_ref = cmd::field(&receipt, "parent_before").map_err(source_error)?;
    let parent_after_ref = cmd::field(&receipt, "parent_after").map_err(source_error)?;

    let mut adoption_indexes = Vec::new();
    for (index, transition) in edition_receipts.iter().enumerate() {
        let request = cmd::field(transition, "request").map_err(source_error)?;
        if cmd::text(request, "operation").map_err(source_error)? == "item.adopt"
            && cmd::record_digest(transition)
                .map_err(source_error)?
                .to_prefixed()
                == transition_sha
            && cmd::text(transition, "archive_path").map_err(source_error)? == transition_archive
            && cmd::same(
                cmd::field(transition, "previous_source").map_err(source_error)?,
                parent_before_ref,
            )
            .map_err(source_error)?
            && cmd::same(
                cmd::field(transition, "source").map_err(source_error)?,
                parent_after_ref,
            )
            .map_err(source_error)?
        {
            adoption_indexes.push(index);
        }
    }
    if adoption_indexes.len() != 1 {
        return Err(DurableError::Conflict(
            "Item adoption transition is not uniquely retained",
        ));
    }
    let adoption_index = adoption_indexes[0];
    let parent_transition = &edition_receipts[adoption_index];
    let (predecessor_archive, _) =
        source_revisions::read_archive(ctx, &edition_archive_config, parent_transition)
            .map_err(source_error)?;
    let adopted_parent_archive =
        if let Some(next_transition) = edition_receipts.get(adoption_index + 1) {
            if !cmd::same(
                cmd::field(next_transition, "previous_source").map_err(source_error)?,
                parent_after_ref,
            )
            .map_err(source_error)?
            {
                return Err(DurableError::Conflict(
                    "next Edition archive does not begin at Item adoption output",
                ));
            }
            source_revisions::read_archive(ctx, &edition_archive_config, next_transition)
                .map_err(source_error)?
                .0
        } else {
            let current_ref = cmd::reference(&edition_record, "record_id", "record_version")
                .map_err(source_error)?;
            if !cmd::same(&current_ref, parent_after_ref).map_err(source_error)? {
                return Err(DurableError::Conflict(
                    "current Edition tip differs from Item adoption",
                ));
            }
            current_parent.clone()
        };
    let predecessor = history_core_package(&predecessor_archive, "edition.json", false)?;
    let mut adopted_parent = history_core_package(&adopted_parent_archive, "edition.json", true)?;
    let adopted_parent_record = cmd::parse(
        adopted_parent
            .get("edition.json")
            .ok_or(DurableError::Corrupt("retained adopted Edition absent"))?,
    )
    .map_err(source_error)?;
    if !cmd::same(
        &cmd::reference(&adopted_parent_record, "record_id", "record_version")
            .map_err(source_error)?,
        parent_after_ref,
    )
    .map_err(source_error)?
    {
        return Err(DurableError::Conflict(
            "retained adopted Edition package differs from transition",
        ));
    }
    let parent_form_set = reconstruct_item_adoption_source_form_set(
        &adopted_parent_record,
        predecessor
            .get("edition.human-forms.json")
            .map(Vec::as_slice),
        &request,
        "forms",
        &principal_id,
        "name",
    )?;
    adopted_parent.insert("edition.human-forms.json".to_owned(), parent_form_set);

    let item_history_present = current_child.contains_key(source_revisions::HISTORY);
    let mut adopted_child = if let Some(first_transition) = item_receipts.first() {
        let item_id = cmd::text(&item_record, "record_id").map_err(source_error)?;
        if item_receipts.iter().any(|transition| {
            transition
                .object_get("request")
                .and_then(|request| request.object_get("operation"))
                .and_then(tos_foundation::JsonValue::as_str)
                != Some("record.revise")
        }) {
            return Err(source_error(cmd::SourceCommandError::Unsupported(
                "Item history contains a non-record revision",
            )));
        }
        let initial = source_revisions::read_archive(ctx, &item_archive_config, first_transition)
            .map_err(source_error)?
            .0;
        let initial_record = cmd::parse(initial.get("item.json").ok_or(DurableError::Corrupt(
            "Item adoption record absent from archive",
        ))?)
        .map_err(source_error)?;
        let initial_ref =
            cmd::reference(&initial_record, "record_id", "record_version").map_err(source_error)?;
        if cmd::text(&initial_record, "record_id").map_err(source_error)? != item_id
            || cmd::integer(&initial_record, "record_version").map_err(source_error)? != 1
            || !cmd::same(
                cmd::field(first_transition, "previous_source").map_err(source_error)?,
                &initial_ref,
            )
            .map_err(source_error)?
        {
            return Err(DurableError::Conflict(
                "Item history does not begin at adopted Item v1",
            ));
        }
        let initial_record_raw = initial
            .get("item.json")
            .ok_or(DurableError::Corrupt("Item v1 record absent from archive"))?;
        let mut package = current_child
            .iter()
            .filter(|(name, _)| name.as_str() != source_revisions::HISTORY)
            .map(|(name, raw)| (name.clone(), raw.clone()))
            .collect::<std::collections::BTreeMap<_, _>>();
        package.insert("item.json".to_owned(), initial_record_raw.clone());
        package
    } else {
        if item_history_present
            || cmd::integer(&item_record, "record_version").map_err(source_error)? != 1
        {
            return Err(DurableError::Conflict(
                "current Item lacks its required revision history",
            ));
        }
        current_child.clone()
    };
    let adopted_child_record = cmd::parse(
        adopted_child
            .get("item.json")
            .ok_or(DurableError::Corrupt("adopted Item record absent"))?,
    )
    .map_err(source_error)?;
    let request_record = cmd::field(&request, "record").map_err(source_error)?;
    let adopted_child_record_raw = adopted_child
        .get("item.json")
        .ok_or(DurableError::Corrupt("adopted Item record absent"))?;
    if !cmd::same(request_record, &adopted_child_record).map_err(source_error)?
        || cmd::published(request_record)
            .map_err(source_error)?
            .as_slice()
            != adopted_child_record_raw.as_slice()
    {
        return Err(DurableError::Conflict(
            "retained Item creation request differs from adopted Item v1",
        ));
    }
    adopted_child.insert(
        "item.human-forms.json".to_owned(),
        reconstruct_item_adoption_source_form_set(
            request_record,
            None,
            &request,
            "item_forms",
            &principal_id,
            "name",
        )?,
    );
    let claim = cmd::field(&request, "claim").map_err(source_error)?;
    let claim_id = cmd::text(claim, "claim_id").map_err(source_error)?;
    let claim_stream = adopted_child
        .get("source-claims.jsonl")
        .ok_or(DurableError::Corrupt("Item Claim stream absent"))?;
    let claim_lines = claim_stream
        .split(|byte| *byte == b'\n' || *byte == b'\r')
        .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
        .collect::<Vec<_>>();
    let mut expected_claim_stream = cmd::canonical(claim).map_err(source_error)?;
    expected_claim_stream.push(b'\n');
    if claim_lines.len() != 1 || claim_stream.as_slice() != expected_claim_stream.as_slice() {
        return Err(DurableError::Conflict(
            "retained Item Claim stream differs from its creation request",
        ));
    }
    let claim_forms_name = format!(
        "source-claims.{}.human-forms.json",
        Digest256::of_bytes(claim_id.as_bytes()).to_hex()
    );
    adopted_child.insert(
        claim_forms_name,
        reconstruct_item_adoption_source_form_set(
            claim,
            None,
            &request,
            "claim_forms",
            &principal_id,
            "statement",
        )?,
    );
    if adopted_child.len() != 15
        || adopted_child
            .keys()
            .any(|name| source_item_adoption_role(name).is_none())
    {
        return Err(DurableError::Conflict(
            "adopted Item package membership differs from original v1",
        ));
    }
    let item_id = cmd::text(&item_record, "record_id").map_err(source_error)?;
    let adopted_child_record = cmd::parse(
        adopted_child
            .get("item.json")
            .ok_or(DurableError::Corrupt("adopted Item record absent"))?,
    )
    .map_err(source_error)?;
    if cmd::text(&adopted_child_record, "record_id").map_err(source_error)? != item_id
        || cmd::integer(&adopted_child_record, "record_version").map_err(source_error)? != 1
    {
        return Err(DurableError::Conflict(
            "retained adopted Item package is not Item v1",
        ));
    }
    let adoption_item_source = cmd::reference(&adopted_child_record, "record_id", "record_version")
        .map_err(source_error)?;

    let historical_parent = EditionItemBytes {
        parent: adopted_parent,
        child: adopted_child,
        parent_receipt: serde_json::from_slice(
            &cmd::canonical(parent_transition).map_err(source_error)?,
        )
        .map_err(|_| DurableError::Corrupt("Item parent receipt conversion"))?,
        receipt: serde_json::from_slice(&cmd::canonical(&receipt).map_err(source_error)?)
            .map_err(|_| DurableError::Corrupt("Item receipt conversion"))?,
        transaction_id: cmd::text(&receipt, "transaction_id")
            .map_err(source_error)?
            .to_owned(),
        archive_path: transition_archive.to_owned(),
        reads: Vec::new(),
        bytes_read: 0,
    };
    let adoption_v1 = crate::source_creation_store::pg_claim_owner_contribution(
        &historical_parent,
        edition_home,
        item_home,
        &predecessor,
    )
    .map_err(source_error)?;
    let adoption_lineage = cmd::field(&adoption_v1, "lineage").map_err(source_error)?;
    let adoption_native_receipt =
        cmd::field(adoption_lineage, "native_receipt").map_err(source_error)?;
    let adoption_scope = cmd::field(adoption_native_receipt, "scope").map_err(source_error)?;
    if cmd::text(&adoption_v1, "package_home").map_err(source_error)? != item_home
        || cmd::text(&adoption_v1, "source_path").map_err(source_error)? != stream_path
        || cmd::text(adoption_scope, "item_source_path").map_err(source_error)? != item_path
    {
        return Err(DurableError::Conflict(
            "historical adoption contribution points at another Item",
        ));
    }

    let current_stream_raw = current_child
        .get("source-claims.jsonl")
        .ok_or(DurableError::Corrupt("current Item Claim stream absent"))?;
    let current_claim_lines = current_stream_raw
        .split(|byte| *byte == b'\n' || *byte == b'\r')
        .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
        .collect::<Vec<_>>();
    if current_claim_lines.len() != 1 {
        return Err(DurableError::Conflict(
            "current Item Claim stream is not singular",
        ));
    }
    let current_claim = cmd::parse(current_claim_lines[0]).map_err(source_error)?;
    let current_claim_id = cmd::text(&current_claim, "claim_id").map_err(source_error)?;
    let current_stream_sha256 = Digest256::of_bytes(current_stream_raw).to_prefixed();

    let mut edition_members = Vec::with_capacity(current_parent.len());
    for (name, raw) in &current_parent {
        let role = current_edition_role(name)
            .ok_or(DurableError::Corrupt("current Edition member role"))?;
        edition_members.push(current_member_row(
            &format!("{edition_home}/{name}"),
            role,
            raw,
        ));
    }
    edition_members.sort_by(|a, b| {
        a.object_get("ref")
            .and_then(tos_foundation::JsonValue::as_str)
            .cmp(
                &b.object_get("ref")
                    .and_then(tos_foundation::JsonValue::as_str),
            )
    });
    let mut item_members = Vec::with_capacity(current_child.len());
    for (name, raw) in &current_child {
        let role = if name == source_revisions::HISTORY {
            Some("revision-history")
        } else {
            source_item_adoption_role(name)
        }
        .ok_or(DurableError::Corrupt("current Item member role"))?;
        item_members.push(current_member_row(
            &format!("{item_home}/{name}"),
            role,
            raw,
        ));
    }
    item_members.sort_by(|a, b| {
        a.object_get("ref")
            .and_then(tos_foundation::JsonValue::as_str)
            .cmp(
                &b.object_get("ref")
                    .and_then(tos_foundation::JsonValue::as_str),
            )
    });

    let edition_ref =
        cmd::reference(&edition_record, "record_id", "record_version").map_err(source_error)?;
    let item_ref =
        cmd::reference(&item_record, "record_id", "record_version").map_err(source_error)?;
    let edition_chain = owner_history_chain(edition_receipts)?;
    let item_chain = owner_history_chain(item_receipts)?;
    let edition_tip = edition_receipts
        .last()
        .ok_or(DurableError::Corrupt("current Edition history is empty"))?;
    let edition_value = cmd::object(vec![
        ("source_path", cmd::string(&edition_path)),
        ("source", edition_ref),
        (
            "history_ref",
            cmd::string(&format!("{edition_home}/{}", source_revisions::HISTORY)),
        ),
        (
            "history_sha256",
            cmd::string(
                &Digest256::of_bytes(current_parent[source_revisions::HISTORY].as_slice())
                    .to_prefixed(),
            ),
        ),
        (
            "history_head_source",
            cmd::field(edition_tip, "source")
                .map_err(source_error)?
                .clone(),
        ),
        (
            "history_head_sha256",
            cmd::string(
                &cmd::record_digest(edition_tip)
                    .map_err(source_error)?
                    .to_prefixed(),
            ),
        ),
        (
            "history_chain",
            tos_foundation::JsonValue::Array(edition_chain),
        ),
        (
            "direct_members",
            tos_foundation::JsonValue::Array(edition_members),
        ),
    ]);
    let mut item_fields = vec![
        ("source_path", cmd::string(&item_path)),
        ("source", item_ref),
        ("claim_id", cmd::string(current_claim_id)),
        ("stream_sha256", cmd::string(&current_stream_sha256)),
        (
            "direct_members",
            tos_foundation::JsonValue::Array(item_members),
        ),
    ];
    if let Some(history_raw) = current_child.get(source_revisions::HISTORY) {
        let item_tip = item_receipts
            .last()
            .ok_or(DurableError::Corrupt("current Item history is empty"))?;
        item_fields.extend([
            (
                "history_ref",
                cmd::string(&format!("{item_home}/{}", source_revisions::HISTORY)),
            ),
            (
                "history_sha256",
                cmd::string(&Digest256::of_bytes(history_raw).to_prefixed()),
            ),
            (
                "history_head_source",
                cmd::field(item_tip, "source")
                    .map_err(source_error)?
                    .clone(),
            ),
            (
                "history_head_sha256",
                cmd::string(
                    &cmd::record_digest(item_tip)
                        .map_err(source_error)?
                        .to_prefixed(),
                ),
            ),
        ]);
    } else {
        item_fields.extend([
            ("history_ref", tos_foundation::JsonValue::Null),
            ("history_sha256", tos_foundation::JsonValue::Null),
            ("history_head_source", tos_foundation::JsonValue::Null),
            ("history_head_sha256", tos_foundation::JsonValue::Null),
        ]);
    }
    item_fields.push((
        "history_chain",
        tos_foundation::JsonValue::Array(item_chain),
    ));
    let current = cmd::object(vec![
        ("edition", edition_value),
        ("item", cmd::object(item_fields)),
    ]);
    let mut owner = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_managed_item_adoption_claim_owner_inventory_v2"),
        ),
        ("package_home", cmd::string(item_home)),
        ("source_path", cmd::string(stream_path)),
        ("adoption_v1", adoption_v1),
        ("adoption_item_source", adoption_item_source),
        ("current", current),
    ]);
    let closure = cmd::record_digest(&owner)
        .map_err(source_error)?
        .to_prefixed();
    cmd::set(&mut owner, "closure_sha256", cmd::string(&closure)).map_err(source_error)?;
    Ok(owner)
}

fn owner_history_chain(
    receipts: &[tos_foundation::JsonValue],
) -> DurableResult<Vec<tos_foundation::JsonValue>> {
    receipts
        .iter()
        .map(|receipt| {
            let request = cmd::field(receipt, "request").map_err(source_error)?;
            Ok(cmd::object(vec![
                (
                    "operation",
                    cmd::field(request, "operation")
                        .map_err(source_error)?
                        .clone(),
                ),
                (
                    "archive_path",
                    cmd::field(receipt, "archive_path")
                        .map_err(source_error)?
                        .clone(),
                ),
                (
                    "receipt_sha256",
                    cmd::string(
                        &cmd::record_digest(receipt)
                            .map_err(source_error)?
                            .to_prefixed(),
                    ),
                ),
                (
                    "previous_source",
                    cmd::field(receipt, "previous_source")
                        .map_err(source_error)?
                        .clone(),
                ),
                (
                    "source",
                    cmd::field(receipt, "source").map_err(source_error)?.clone(),
                ),
            ]))
        })
        .collect()
}

fn history_core_package(
    package: &std::collections::BTreeMap<String, Vec<u8>>,
    record_name: &str,
    require_history: bool,
) -> DurableResult<std::collections::BTreeMap<String, Vec<u8>>> {
    let forms_name = record_name
        .strip_suffix(".json")
        .map(|stem| format!("{stem}.human-forms.json"))
        .ok_or(DurableError::Corrupt("record history core basename"))?;
    let mut names = vec![record_name, forms_name.as_str()];
    if require_history || package.contains_key(source_revisions::HISTORY) {
        names.push(source_revisions::HISTORY);
    }
    names
        .into_iter()
        .map(|name| {
            package
                .get(name)
                .cloned()
                .map(|raw| (name.to_owned(), raw))
                .ok_or(DurableError::Corrupt("record history core member absent"))
        })
        .collect()
}

/// Rebuild the exact initial form-set bytes through the shared maintained
/// source-copy kernel. The receipt-bound creation request and predecessor
/// forms are the original inputs; later form-set growth can make a subsequent
/// record-revision archive a different historical state.
fn reconstruct_item_adoption_source_form_set(
    source: &tos_foundation::JsonValue,
    previous_forms_raw: Option<&[u8]>,
    request: &tos_foundation::JsonValue,
    selection_key: &str,
    principal: &str,
    required_role: &str,
) -> DurableResult<Vec<u8>> {
    let previous = previous_forms_raw
        .map(|raw| cmd::parse(raw).map_err(source_error))
        .transpose()?;
    let selections = cmd::array(request, selection_key).map_err(source_error)?;
    if let Some(previous) = previous.as_ref() {
        for form in cmd::array(previous, "forms").map_err(source_error)? {
            let form_id = form
                .object_get("form_id")
                .and_then(tos_foundation::JsonValue::as_str)
                .ok_or(DurableError::Corrupt("prior Item source-copy form id"))?;
            let source_copy = form
                .object_get("content")
                .and_then(|content| content.object_get("kind"))
                .and_then(tos_foundation::JsonValue::as_str)
                == Some("source-copy");
            if !source_copy
                || !selections.iter().any(|selection| {
                    selection
                        .object_get("form_id")
                        .and_then(tos_foundation::JsonValue::as_str)
                        == Some(form_id)
                })
            {
                return Err(DurableError::Conflict(
                    "Item adoption does not explicitly preserve prior source-copy forms",
                ));
            }
        }
    }
    let subject = source_forms::metadata_subject(source).map_err(source_error)?;
    let mut changes = Vec::with_capacity(selections.len());
    for selection in selections {
        let form_id = cmd::text(selection, "form_id").map_err(source_error)?;
        let field_id = cmd::text(selection, "field_id").map_err(source_error)?;
        changes.push(
            source_forms::prepare_form_change(
                source,
                previous.as_ref(),
                principal,
                form_id,
                field_id,
            )
            .map_err(source_error)?,
        );
    }
    let set = source_forms::apply_form_changes(previous.as_ref(), &subject, &changes)
        .map_err(source_error)?;
    let views = source_forms::materialize_source_forms(source, &set).map_err(source_error)?;
    if !views.iter().all(|view| {
        view.object_get("state")
            .and_then(tos_foundation::JsonValue::as_str)
            == Some("ready")
    }) || !views.iter().any(|view| {
        view.object_get("role")
            .and_then(tos_foundation::JsonValue::as_str)
            == Some(required_role)
    }) {
        return Err(DurableError::Conflict(
            "Item adoption source-copy form reconstruction is not ready",
        ));
    }
    cmd::published(&set).map_err(source_error)
}

fn source_item_adoption_role(name: &str) -> Option<&'static str> {
    Some(match name {
        "item.json" => "item-record",
        "item.human-forms.json" => "item-form-set",
        "source-claims.jsonl" => "claim-stream",
        "source-create-request.json" => "creation-request",
        "source-create-environment.json" => "creation-environment",
        "source-create-provenance.jsonl" => "creation-event",
        "edition-item-receipt.json" => "compound-receipt",
        "item.manifest.json" => "file-manifest",
        "rights.json" => "rights-record",
        "resource-inventory.json" => "resource-inventory",
        "fixity.sha256" => "fixity",
        "forensic-report.md" => "forensic-report",
        "provenance.jsonl" => "provenance",
        "item-deposit-receipt.json" => "deposit-receipt",
        name if name.starts_with("source-claims.") && name.ends_with(".human-forms.json") => {
            "claim-form-set"
        }
        _ => return None,
    })
}

fn current_edition_role(name: &str) -> Option<&'static str> {
    Some(match name {
        "edition.json" => "edition-record",
        "edition.human-forms.json" => "edition-form-set",
        "source-revision-history.json" => "revision-history",
        "expression-edition-receipt.json" => "compound-receipt",
        "source-claims.jsonl" => "claim-stream",
        "source-create-request.json" => "creation-request",
        "source-create-environment.json" => "creation-environment",
        "source-create-provenance.jsonl" => "creation-event",
        name if name.starts_with("source-claims.") && name.ends_with(".human-forms.json") => {
            "claim-form-set"
        }
        _ => return None,
    })
}

fn current_member_row(path: &str, role: &str, raw: &[u8]) -> tos_foundation::JsonValue {
    cmd::object(vec![
        ("ref", cmd::string(path)),
        ("role", cmd::string(role)),
        ("bytes", cmd::number(raw.len() as u64)),
        (
            "sha256",
            cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
        ),
    ])
}

pub(super) fn item_claim_current_inventory_contribution(
    owner: tos_foundation::JsonValue,
    profile: &source_claims::AgentInventoryAssessmentProfile,
) -> DurableResult<tos_foundation::JsonValue> {
    let records = cmd::object(
        source_claims::MIXED_INVENTORY_RECORD_KINDS
            .iter()
            .map(|kind| (*kind, tos_foundation::JsonValue::Array(Vec::new())))
            .collect(),
    );
    let source_profiles =
        source_claims::pg_inventory::claim_profile_sources_from_validated_profile(profile)
            .map_err(source_error)?;
    Ok(cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_pg_source_inventory_member_contribution_v1"),
        ),
        ("records", records),
        ("source_profiles", source_profiles),
        ("events", cmd::object(Vec::new())),
        ("anchors", cmd::object(Vec::new())),
        ("form", tos_foundation::JsonValue::Null),
        ("claim_owner", owner),
    ]))
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
    part(&mut hash, b"tos-managed-mixed-source-inventory-coverage-v1");
    for (path, raw) in projections {
        part(&mut hash, path.as_bytes());
        part(&mut hash, Digest256::of_bytes(raw).as_bytes());
    }
    hash.finalize()
}

/// A complete Public Text package whose owner receipts came from one committed
/// typed source-revision attempt. Filenames alone never select this family.
#[derive(Clone, Debug)]
pub(super) struct CurrentPublicTextPackageV1 {
    pub(super) package_home: String,
    pub(super) committed_owners: BTreeMap<String, tos_foundation::JsonValue>,
}

pub(super) fn public_text_owner_contributions_match(
    left: &tos_foundation::JsonValue,
    right: &tos_foundation::JsonValue,
) -> DurableResult<bool> {
    Ok(cmd::canonical(left).map_err(source_error)?
        == cmd::canonical(right).map_err(source_error)?)
}

/// Recover the exact typed Public Text owner receipts for current members.
/// The source-read family tag, committed write set, current rows, and attempt
/// projections must all describe the same 12-member package.
pub(super) fn current_public_text_packages(
    tx: &mut Transaction<'_>,
    domain: &str,
    current_count: u64,
    max_statement_ms: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<Vec<CurrentPublicTextPackageV1>> {
    const FAMILY_PREFIX: &[u8] = b"tos-pg-public-native-text-v1:";
    const MAX_RETAINED_RECEIPT_BYTES: u64 = 67_108_864;

    source_cohort_streamed::set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
    let candidates = tx.query(
        "SELECT DISTINCT a.prepare_id,octet_length(a.source_reads)
         FROM cmd2_current c JOIN cmd2_attempt a USING(domain,prepare_id)
         WHERE c.domain=$1 AND a.source_reads IS NOT NULL
           AND position($2::bytea in a.source_reads)>0
         ORDER BY a.prepare_id LIMIT $3",
        &[&domain, &FAMILY_PREFIX, &as_i64(current_count)?],
    )?;
    if candidates.len() as u64 > current_count {
        return Err(DurableError::Corrupt(
            "Public Text current attempt candidate count differs",
        ));
    }
    let mut retained_bytes = 0u64;
    for candidate in &candidates {
        active(deadline, cancelled)?;
        let length = candidate
            .get::<_, Option<i32>>(1)
            .filter(|length| *length > 0 && *length as usize <= 1_048_576)
            .ok_or(DurableError::Refused(
                "Public Text retained source reads exceed row bound",
            ))? as u64;
        retained_bytes = retained_bytes
            .checked_add(length)
            .filter(|bytes| *bytes <= MAX_RETAINED_RECEIPT_BYTES)
            .ok_or(DurableError::Refused(
                "Public Text retained owner receipts exceed package bound",
            ))?;
    }

    let mut packages = Vec::new();
    let mut seen_homes = BTreeSet::new();
    for candidate in candidates {
        active(deadline, cancelled)?;
        let prepare_id: Vec<u8> = candidate.get(0);
        source_cohort_streamed::set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
        let attempt = tx.query_one(
            "SELECT state,commit_seq,source_reads
             FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2",
            &[&domain, &prepare_id],
        )?;
        if attempt.get::<_, String>("state") != "committed" {
            return Err(DurableError::Corrupt(
                "Public Text current source attempt is not committed",
            ));
        }
        let reads_raw =
            attempt
                .get::<_, Option<Vec<u8>>>("source_reads")
                .ok_or(DurableError::Corrupt(
                    "Public Text committed source reads are absent",
                ))?;
        let reads = decode_reads(&reads_raw)?;
        let Some(binding) = reads.revision.as_ref() else {
            continue;
        };
        if !binding
            .family_tag
            .starts_with("tos-pg-public-native-text-v1:")
        {
            continue;
        }
        if !binding
            .family_tag
            .strip_prefix("tos-pg-public-native-text-v1:")
            .is_some_and(|suffix| !suffix.is_empty())
        {
            return Err(DurableError::Corrupt(
                "Public Text source family marker is malformed",
            ));
        }
        let commit_seq = as_u64(attempt.get::<_, Option<i64>>("commit_seq").ok_or(
            DurableError::Corrupt("Public Text committed sequence is absent"),
        )?)?;
        if binding.source_head_sequence.checked_add(1) != Some(commit_seq) {
            return Err(DurableError::Corrupt(
                "Public Text source attempt head binding differs",
            ));
        }

        source_cohort_streamed::set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
        let projection_length: Option<i32> = tx
            .query_one(
                "SELECT octet_length(source_projections) FROM cmd2_attempt
                 WHERE domain=$1 AND prepare_id=$2",
                &[&domain, &prepare_id],
            )?
            .get(0);
        let projection_length = projection_length
            .filter(|length| *length > 0 && *length as usize <= 1_048_576)
            .ok_or(DurableError::Refused(
                "Public Text retained owner projection exceeds row bound",
            ))? as u64;
        retained_bytes = retained_bytes
            .checked_add(projection_length)
            .filter(|bytes| *bytes <= MAX_RETAINED_RECEIPT_BYTES)
            .ok_or(DurableError::Refused(
                "Public Text retained owner receipts exceed package bound",
            ))?;

        let member_row_limit = (crate::source_public_text_entry::FILES.len() + 1) as i64;
        source_cohort_streamed::set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
        let member_rows = tx.query(
            "SELECT m.subject,m.proposed_revision,m.content_digest,m.content_length,
                    c.prepare_id AS current_prepare_id,c.revision AS current_revision,
                    c.commit_seq AS current_commit_seq,c.content_digest AS current_digest,
                    c.content_length AS current_length
             FROM cmd2_member m LEFT JOIN cmd2_current c
               ON c.domain=m.domain AND c.subject=m.subject
             WHERE m.domain=$1 AND m.prepare_id=$2
             ORDER BY m.subject COLLATE \"C\" LIMIT $3",
            &[&domain, &prepare_id, &member_row_limit],
        )?;
        if member_rows.len() != crate::source_public_text_entry::FILES.len() {
            return Err(DurableError::Corrupt(
                "Public Text committed member closure differs",
            ));
        }
        let packet_paths = member_rows
            .iter()
            .map(|row| row.get::<_, String>("subject"))
            .filter(|path| path.ends_with("/source-text-unit.v1.json"))
            .collect::<Vec<_>>();
        if packet_paths.len() != 1 {
            return Err(DurableError::Corrupt(
                "Public Text committed package packet path differs",
            ));
        }
        let package_home =
            crate::source_public_text_entry::public_text_package_home(&packet_paths[0])
                .map_err(source_error)?;
        let expected_paths = crate::source_public_text_entry::FILES
            .iter()
            .map(|name| format!("{package_home}/{name}"))
            .collect::<BTreeSet<_>>();
        if !seen_homes.insert(package_home.clone())
            || member_rows
                .iter()
                .map(|row| row.get::<_, String>("subject"))
                .collect::<BTreeSet<_>>()
                != expected_paths
        {
            return Err(DurableError::Corrupt(
                "Public Text committed package paths are incomplete or duplicated",
            ));
        }
        for row in &member_rows {
            let subject: String = row.get("subject");
            let proposed_revision = as_u64(row.get("proposed_revision"))?;
            let member_digest: String = row.get("content_digest");
            let member_length = as_u64(row.get("content_length"))?;
            if row
                .get::<_, Option<Vec<u8>>>("current_prepare_id")
                .as_deref()
                != Some(prepare_id.as_slice())
                || row.get::<_, Option<i64>>("current_revision") != Some(as_i64(proposed_revision)?)
                || row.get::<_, Option<i64>>("current_commit_seq") != Some(as_i64(commit_seq)?)
                || row.get::<_, Option<String>>("current_digest").as_deref()
                    != Some(member_digest.as_str())
                || row.get::<_, Option<i64>>("current_length") != Some(as_i64(member_length)?)
            {
                return Err(DurableError::Corrupt(
                    "Public Text committed members are not the complete current revision",
                ));
            }
            if !expected_paths.contains(&subject) {
                return Err(DurableError::Corrupt(
                    "Public Text committed member path differs",
                ));
            }
        }
        source_cohort_streamed::set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
        let current_count_for_attempt: i64 = tx
            .query_one(
                "SELECT count(*) FROM cmd2_current WHERE domain=$1 AND prepare_id=$2",
                &[&domain, &prepare_id],
            )?
            .get(0);
        if current_count_for_attempt != member_rows.len() as i64 {
            return Err(DurableError::Corrupt(
                "Public Text current member count differs from committed write set",
            ));
        }

        source_cohort_streamed::set_streamed_pg_limits(tx, max_statement_ms, deadline, cancelled)?;
        let projections_raw = tx
            .query_one(
                "SELECT source_projections FROM cmd2_attempt
                 WHERE domain=$1 AND prepare_id=$2",
                &[&domain, &prepare_id],
            )?
            .get::<_, Option<Vec<u8>>>(0)
            .ok_or(DurableError::Corrupt(
                "Public Text committed owner projections are absent",
            ))?;
        if projections_raw.len() != projection_length as usize {
            return Err(DurableError::Corrupt(
                "Public Text retained owner projection length differs",
            ));
        }
        let projections: ProjectionRows = serde_json::from_slice(&projections_raw)
            .map_err(|_| DurableError::Corrupt("Public Text committed owner projections"))?;
        if projections_bytes(&projections)? != projections_raw
            || projections.keys().cloned().collect::<BTreeSet<_>>() != expected_paths
        {
            return Err(DurableError::Corrupt(
                "Public Text committed projection closure differs",
            ));
        }
        let member_digests = member_rows
            .iter()
            .map(|row| {
                (
                    row.get::<_, String>("subject"),
                    row.get::<_, String>("content_digest"),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut committed_owners = BTreeMap::new();
        for (path, raw) in projections {
            let content_digest = member_digests.get(&path).ok_or(DurableError::Corrupt(
                "Public Text projection member absent",
            ))?;
            let projection = addressed_inventory::AddressedMixedInventoryV1::checked_projection(
                &path,
                &raw,
                content_digest,
            )?;
            let owner = cmd::field(&projection, "public_text_owner").map_err(source_error)?;
            if owner == &tos_foundation::JsonValue::Null
                || committed_owners.insert(path, owner.clone()).is_some()
            {
                return Err(DurableError::Corrupt(
                    "Public Text committed typed owner projection is absent or duplicated",
                ));
            }
        }
        packages.push(CurrentPublicTextPackageV1 {
            package_home,
            committed_owners,
        });
    }
    Ok(packages)
}

/// Regenerate typed Public Text contributions from the exact current source
/// bodies, then overlay only those proved paths on the generic inventory.
pub(super) fn regenerate_current_public_text_contributions(
    files: &BTreeMap<String, Vec<u8>>,
    packages: &[CurrentPublicTextPackageV1],
) -> DurableResult<BTreeMap<String, tos_foundation::JsonValue>> {
    let mut contributions = BTreeMap::new();
    let mut package_homes = BTreeSet::new();
    for package in packages {
        let expected_paths = crate::source_public_text_entry::FILES
            .iter()
            .map(|name| format!("{}/{name}", package.package_home))
            .collect::<BTreeSet<_>>();
        if !package_homes.insert(package.package_home.clone())
            || package
                .committed_owners
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>()
                != expected_paths
        {
            return Err(DurableError::Corrupt(
                "Public Text typed receipt paths differ",
            ));
        }
        let mut package_files = BTreeMap::new();
        for name in crate::source_public_text_entry::FILES {
            let path = format!("{}/{name}", package.package_home);
            let raw = files.get(&path).ok_or(DurableError::Corrupt(
                "Public Text current package member absent",
            ))?;
            package_files.insert(name.to_owned(), raw.clone());
        }
        for name in crate::source_public_text_entry::FILES {
            let path = format!("{}/{name}", package.package_home);
            let owner = crate::source_public_text_entry::public_text_inventory_projection(
                &package.package_home,
                &path,
                &package_files,
            )
            .map_err(source_error)?;
            let committed_owner =
                package
                    .committed_owners
                    .get(&path)
                    .ok_or(DurableError::Corrupt(
                        "Public Text committed owner receipt absent",
                    ))?;
            if !public_text_owner_contributions_match(committed_owner, &owner)? {
                return Err(DurableError::Corrupt(
                    "Public Text committed owner receipt differs from current bytes",
                ));
            }
            if contributions.insert(path.clone(), owner).is_some() {
                return Err(DurableError::Corrupt(
                    "duplicate typed Public Text current package path",
                ));
            }
        }
    }
    Ok(contributions)
}

pub(super) fn overlay_current_public_text_contributions(
    files: &BTreeMap<String, Vec<u8>>,
    packages: &[CurrentPublicTextPackageV1],
    projections: &mut ProjectionRows,
) -> DurableResult<()> {
    for (path, owner) in regenerate_current_public_text_contributions(files, packages)? {
        let source = files
            .get(&path)
            .ok_or(DurableError::Corrupt("Public Text current member absent"))?;
        let raw = projections.get(&path).ok_or(DurableError::Corrupt(
            "Public Text generic projection absent",
        ))?;
        let mut projection = addressed_inventory::AddressedMixedInventoryV1::checked_projection(
            &path,
            raw,
            &Digest256::of_bytes(source).to_hex(),
        )?;
        let existing = projection
            .object_get("public_text_owner")
            .ok_or(DurableError::Corrupt("Public Text owner field absent"))?;
        if existing != &tos_foundation::JsonValue::Null
            && !public_text_owner_contributions_match(existing, &owner)?
        {
            return Err(DurableError::Corrupt(
                "Public Text generic owner contribution conflicts",
            ));
        }
        cmd::set(&mut projection, "public_text_owner", owner).map_err(source_error)?;
        let raw = cmd::canonical(&projection).map_err(source_error)?;
        addressed_inventory::AddressedMixedInventoryV1::checked_projection(
            &path,
            &raw,
            &Digest256::of_bytes(source).to_hex(),
        )?;
        projections.insert(path, raw);
    }
    Ok(())
}

fn projections_bytes(projections: &ProjectionRows) -> DurableResult<Vec<u8>> {
    serde_json::to_vec(projections)
        .map_err(|_| DurableError::Invalid("source projections encoding"))
}

fn mixed_projection_envelope(
    path: &str,
    source_raw: &[u8],
    records: tos_foundation::JsonValue,
    source_profiles: tos_foundation::JsonValue,
    events: tos_foundation::JsonValue,
    anchors: tos_foundation::JsonValue,
    form: tos_foundation::JsonValue,
    claim_owner: tos_foundation::JsonValue,
    public_text_owner: tos_foundation::JsonValue,
) -> DurableResult<Vec<u8>> {
    let source_digest = Digest256::of_bytes(source_raw).to_hex();
    let envelope = cmd::object(vec![
        (
            "schema_version",
            cmd::string(tos_compiler::managed_source::MANAGED_MIXED_INVENTORY_MEMBER_SCHEMA_V2),
        ),
        ("path", cmd::string(path)),
        ("raw_sha256", cmd::string(&source_digest)),
        ("records", records),
        ("source_profiles", source_profiles),
        ("events", events),
        ("anchors", anchors),
        ("form", form),
        ("claim_owner", claim_owner),
        ("public_text_owner", public_text_owner),
    ]);
    let raw = cmd::canonical(&envelope).map_err(source_error)?;
    addressed_inventory::AddressedMixedInventoryV1::checked_projection(path, &raw, &source_digest)?;
    Ok(raw)
}

fn revision_owner_projections(
    family_tag: &str,
    writes: &[SourceRevisionWrite],
    contributions: BTreeMap<String, tos_foundation::JsonValue>,
) -> DurableResult<ProjectionRows> {
    if contributions.is_empty() {
        if family_tag.starts_with("tos-pg-public-native-text-v1:") {
            return Err(DurableError::Conflict(
                "public Text revision owner contribution is absent",
            ));
        }
        if family_tag.starts_with("tos-pg-global-claims-v1:")
            && writes
                .iter()
                .any(|write| claim_projection_requires_owner(&write.subject))
        {
            return Err(DurableError::Conflict(
                "Global Claims revision owner contribution is absent",
            ));
        }
        if pg_forms_historical_family(family_tag).is_some() && !writes.is_empty() {
            return Err(DurableError::Conflict(
                "PG Forms/Historical owner contribution is absent",
            ));
        }
        return Ok(ProjectionRows::new());
    }
    let writes_by_path = writes
        .iter()
        .map(|write| (write.subject.as_str(), write))
        .collect::<BTreeMap<_, _>>();
    if writes_by_path.len() != writes.len()
        || contributions
            .keys()
            .any(|path| !path.starts_with("ToS/") || !writes_by_path.contains_key(path.as_str()))
    {
        return Err(DurableError::Conflict(
            "source revision owner contribution path differs from writes",
        ));
    }
    let is_public_text = family_tag.starts_with("tos-pg-public-native-text-v1:");
    let is_global_claim = family_tag.starts_with("tos-pg-global-claims-v1:");
    let is_forms_historical = pg_forms_historical_family(family_tag).is_some();
    let is_owner_inventory = pg_family_uses_mixed_inventory(family_tag);
    if !is_public_text && !is_global_claim && !is_forms_historical && !is_owner_inventory {
        return Err(DurableError::Refused(
            "source revision owner contribution family is unsupported",
        ));
    }
    if (is_public_text || is_forms_historical) && contributions.len() != writes.len() {
        return Err(DurableError::Conflict(
            "typed source package projection coverage differs",
        ));
    }
    if is_global_claim
        && writes
            .iter()
            .filter(|write| claim_projection_requires_owner(&write.subject))
            .any(|write| !contributions.contains_key(&write.subject))
    {
        return Err(DurableError::Conflict(
            "Global Claims stream/form projection contribution is absent",
        ));
    }
    if is_owner_inventory && contributions.len() != writes.len() {
        return Err(DurableError::Conflict(
            "maintained owner inventory projection coverage differs",
        ));
    }

    let mut projections = ProjectionRows::new();
    let mut projection_bytes = 0usize;
    for (path, owner) in contributions {
        let write = writes_by_path
            .get(path.as_str())
            .copied()
            .ok_or(DurableError::Corrupt(
                "source revision owner projection write absent",
            ))?;
        let schema = cmd::text(&owner, "schema_version").map_err(source_error)?;
        let (claim_owner, public_text_owner, form, source_profiles, events, anchors) = match schema
        {
            "tos_pg_global_claim_projection_contribution_v1" if is_global_claim => {
                cmd::exact_keys(
                    &owner,
                    &[
                        "schema_version",
                        "claim_owner",
                        "source_profiles",
                        "events",
                        "anchors",
                    ],
                )
                .map_err(source_error)?;
                (
                    cmd::field(&owner, "claim_owner")
                        .map_err(source_error)?
                        .clone(),
                    tos_foundation::JsonValue::Null,
                    tos_foundation::JsonValue::Null,
                    cmd::field(&owner, "source_profiles")
                        .map_err(source_error)?
                        .clone(),
                    cmd::field(&owner, "events").map_err(source_error)?.clone(),
                    cmd::field(&owner, "anchors").map_err(source_error)?.clone(),
                )
            }
            "tos_pg_global_claim_form_projection_contribution_v1" if is_global_claim => {
                cmd::exact_keys(
                    &owner,
                    &[
                        "schema_version",
                        "form",
                        "source_profiles",
                        "events",
                        "anchors",
                    ],
                )
                .map_err(source_error)?;
                (
                    tos_foundation::JsonValue::Null,
                    tos_foundation::JsonValue::Null,
                    cmd::field(&owner, "form").map_err(source_error)?.clone(),
                    cmd::field(&owner, "source_profiles")
                        .map_err(source_error)?
                        .clone(),
                    cmd::field(&owner, "events").map_err(source_error)?.clone(),
                    cmd::field(&owner, "anchors").map_err(source_error)?.clone(),
                )
            }
            "tos_pg_historical_claim_projection_contribution_v1" if is_forms_historical => {
                cmd::exact_keys(
                    &owner,
                    &[
                        "schema_version",
                        "claim_owner",
                        "source_profiles",
                        "events",
                        "anchors",
                    ],
                )
                .map_err(source_error)?;
                (
                    cmd::field(&owner, "claim_owner")
                        .map_err(source_error)?
                        .clone(),
                    tos_foundation::JsonValue::Null,
                    tos_foundation::JsonValue::Null,
                    cmd::field(&owner, "source_profiles")
                        .map_err(source_error)?
                        .clone(),
                    cmd::field(&owner, "events").map_err(source_error)?.clone(),
                    cmd::field(&owner, "anchors").map_err(source_error)?.clone(),
                )
            }
            "tos_pg_human_form_projection_contribution_v1" if is_forms_historical => {
                cmd::exact_keys(
                    &owner,
                    &[
                        "schema_version",
                        "form",
                        "source_profiles",
                        "events",
                        "anchors",
                    ],
                )
                .map_err(source_error)?;
                (
                    tos_foundation::JsonValue::Null,
                    tos_foundation::JsonValue::Null,
                    cmd::field(&owner, "form").map_err(source_error)?.clone(),
                    cmd::field(&owner, "source_profiles")
                        .map_err(source_error)?
                        .clone(),
                    cmd::field(&owner, "events").map_err(source_error)?.clone(),
                    cmd::field(&owner, "anchors").map_err(source_error)?.clone(),
                )
            }
            "tos_pg_historical_package_member_contribution_v1" if is_forms_historical => {
                cmd::exact_keys(
                    &owner,
                    &["schema_version", "path", "raw_sha256", "source_profiles"],
                )
                .map_err(source_error)?;
                if cmd::text(&owner, "path").map_err(source_error)? != path
                    || cmd::text(&owner, "raw_sha256").map_err(source_error)?
                        != Digest256::of_bytes(&write.exact_bytes).to_prefixed()
                {
                    return Err(DurableError::Conflict(
                        "historical package owner contribution binding differs",
                    ));
                }
                (
                    tos_foundation::JsonValue::Null,
                    tos_foundation::JsonValue::Null,
                    tos_foundation::JsonValue::Null,
                    cmd::field(&owner, "source_profiles")
                        .map_err(source_error)?
                        .clone(),
                    cmd::object(Vec::new()),
                    cmd::object(Vec::new()),
                )
            }
            "tos_managed_public_text_inventory_member_v1" if is_public_text => (
                tos_foundation::JsonValue::Null,
                owner.clone(),
                tos_foundation::JsonValue::Null,
                cmd::object(Vec::new()),
                cmd::object(Vec::new()),
                cmd::object(Vec::new()),
            ),
            "tos_pg_source_inventory_member_contribution_v1" if is_owner_inventory => {
                cmd::exact_keys(
                    &owner,
                    &[
                        "schema_version",
                        "records",
                        "source_profiles",
                        "events",
                        "anchors",
                        "form",
                        "claim_owner",
                    ],
                )
                .map_err(source_error)?;
                (
                    cmd::field(&owner, "claim_owner")
                        .map_err(source_error)?
                        .clone(),
                    tos_foundation::JsonValue::Null,
                    cmd::field(&owner, "form").map_err(source_error)?.clone(),
                    cmd::field(&owner, "source_profiles")
                        .map_err(source_error)?
                        .clone(),
                    cmd::field(&owner, "events").map_err(source_error)?.clone(),
                    cmd::field(&owner, "anchors").map_err(source_error)?.clone(),
                )
            }
            _ => {
                return Err(DurableError::Conflict(
                    "source revision owner projection schema differs",
                ));
            }
        };
        let records = if schema == "tos_pg_source_inventory_member_contribution_v1" {
            // The shared checked_projection call below owns the built-in and
            // registered-profile row grammar for both cold and changed members.
            cmd::field(&owner, "records").map_err(source_error)?.clone()
        } else {
            cmd::object(
                source_claims::MIXED_INVENTORY_RECORD_KINDS
                    .iter()
                    .map(|kind| (*kind, tos_foundation::JsonValue::Array(Vec::new())))
                    .collect(),
            )
        };
        let raw = mixed_projection_envelope(
            &path,
            &write.exact_bytes,
            records,
            source_profiles,
            events,
            anchors,
            form,
            claim_owner,
            public_text_owner,
        )?;
        if raw.len() > 1_048_576 {
            return Err(DurableError::Refused(
                "source revision inventory projection exceeds member bound",
            ));
        }
        projection_bytes = projection_bytes
            .checked_add(raw.len())
            .filter(|bytes| *bytes <= 64 * 1024 * 1024)
            .ok_or(DurableError::Refused(
                "source revision inventory projections exceed package bound",
            ))?;
        projections.insert(path, raw);
    }
    validate_revision_owner_projections(family_tag, writes, &projections)?;
    Ok(projections)
}

fn validate_revision_owner_projections(
    family_tag: &str,
    writes: &[SourceRevisionWrite],
    projections: &ProjectionRows,
) -> DurableResult<()> {
    let writes_by_path = writes
        .iter()
        .map(|write| (write.subject.as_str(), write))
        .collect::<BTreeMap<_, _>>();
    if writes_by_path.len() != writes.len()
        || projections
            .keys()
            .any(|path| !writes_by_path.contains_key(path.as_str()))
    {
        return Err(DurableError::Corrupt(
            "source revision projection path coverage differs",
        ));
    }
    let is_public_text = family_tag.starts_with("tos-pg-public-native-text-v1:");
    let is_global_claim = family_tag.starts_with("tos-pg-global-claims-v1:");
    let is_owner_inventory = pg_family_uses_mixed_inventory(family_tag);
    let is_item_adoption = family_tag == crate::source_creation_store::PG_ITEM_ADOPTION_FAMILY_TAG;
    let is_forms_historical = pg_forms_historical_family(family_tag).is_some();
    if ((is_public_text || is_forms_historical) && projections.len() != writes.len())
        || (is_owner_inventory && projections.len() != writes.len())
        || (is_global_claim
            && writes.iter().any(|write| {
                claim_projection_requires_owner(&write.subject)
                    && !projections.contains_key(&write.subject)
            }))
    {
        return Err(DurableError::Corrupt(
            "source revision required owner projection absent",
        ));
    }
    if is_item_adoption {
        let item_claim_paths = writes
            .iter()
            .filter(|write| is_item_claim_stream(&write.subject))
            .map(|write| write.subject.as_str())
            .collect::<Vec<_>>();
        if item_claim_paths.len() != 1 {
            return Err(DurableError::Corrupt(
                "Item adoption Claim projection path count differs",
            ));
        }
        let item_claim_path = item_claim_paths[0];
        let item_claim_projection =
            projections
                .get(item_claim_path)
                .ok_or(DurableError::Corrupt(
                    "Item adoption Claim projection absent",
                ))?;
        let item_claim_write = writes_by_path
            .get(item_claim_path)
            .copied()
            .ok_or(DurableError::Corrupt("Item adoption Claim write absent"))?;
        let item_claim_digest = Digest256::of_bytes(&item_claim_write.exact_bytes).to_hex();
        let item_claim_projection =
            addressed_inventory::AddressedMixedInventoryV1::checked_projection(
                item_claim_path,
                item_claim_projection,
                &item_claim_digest,
            )?;
        let stored_claim_owner =
            cmd::field(&item_claim_projection, "claim_owner").map_err(source_error)?;
        let expected_claim_owner =
            item_claim_owner_contribution_from_writes(writes, item_claim_path)?;
        if !cmd::same(stored_claim_owner, &expected_claim_owner).map_err(source_error)? {
            return Err(DurableError::Corrupt(
                "Item adoption Claim owner projection differs from retained package",
            ));
        }
    }
    for (path, raw) in projections {
        let write = writes_by_path
            .get(path.as_str())
            .copied()
            .ok_or(DurableError::Corrupt(
                "source revision projection write absent",
            ))?;
        let digest = Digest256::of_bytes(&write.exact_bytes).to_hex();
        let value =
            addressed_inventory::AddressedMixedInventoryV1::checked_projection(path, raw, &digest)?;
        let claim_owner = cmd::field(&value, "claim_owner").map_err(source_error)?;
        let public_text_owner = cmd::field(&value, "public_text_owner").map_err(source_error)?;
        let form = cmd::field(&value, "form").map_err(source_error)?;
        if is_global_claim {
            let subject = path.as_str();
            let claim_stream = subject.starts_with("ToS/source-witnesses/relations/")
                && subject.ends_with(&format!("/{}", source_claims::CLAIM_STREAM));
            let claim_form = claim_form_projection_path(subject);
            if public_text_owner != &tos_foundation::JsonValue::Null
                || (claim_stream
                    && (claim_owner == &tos_foundation::JsonValue::Null
                        || form != &tos_foundation::JsonValue::Null))
                || (claim_form
                    && (form == &tos_foundation::JsonValue::Null
                        || claim_owner != &tos_foundation::JsonValue::Null))
                || (!claim_stream
                    && !claim_form
                    && (claim_owner != &tos_foundation::JsonValue::Null
                        || form != &tos_foundation::JsonValue::Null))
            {
                return Err(DurableError::Corrupt(
                    "source revision Claim owner/form projection differs",
                ));
            }
        } else if is_forms_historical {
            let basename = path.rsplit('/').next().unwrap_or(path);
            let stream = basename == "historical-claims.jsonl";
            let human_form = basename.starts_with("historical-claims.")
                && basename.ends_with(".human-forms.json");
            if public_text_owner != &tos_foundation::JsonValue::Null
                || (stream
                    && (claim_owner == &tos_foundation::JsonValue::Null
                        || form != &tos_foundation::JsonValue::Null))
                || (human_form
                    && (form == &tos_foundation::JsonValue::Null
                        || claim_owner != &tos_foundation::JsonValue::Null))
                || (!stream
                    && !human_form
                    && (claim_owner != &tos_foundation::JsonValue::Null
                        || form != &tos_foundation::JsonValue::Null))
            {
                return Err(DurableError::Corrupt(
                    "source revision Historical owner/form projection differs",
                ));
            }
        } else if is_public_text {
            if public_text_owner == &tos_foundation::JsonValue::Null
                || claim_owner != &tos_foundation::JsonValue::Null
            {
                return Err(DurableError::Corrupt(
                    "source revision public Text owner projection differs",
                ));
            }
        } else if is_owner_inventory {
            if public_text_owner != &tos_foundation::JsonValue::Null
                || (is_item_adoption
                    && ((is_item_claim_stream(path)
                        && claim_owner == &tos_foundation::JsonValue::Null)
                        || (!is_item_claim_stream(path)
                            && claim_owner != &tos_foundation::JsonValue::Null)))
            {
                return Err(DurableError::Corrupt(
                    "source revision owner projection has a public Text contribution",
                ));
            }
        } else {
            return Err(DurableError::Refused(
                "source revision projection family is unsupported",
            ));
        }
    }
    Ok(())
}

fn item_claim_owner_contribution_from_writes(
    writes: &[SourceRevisionWrite],
    stream_path: &str,
) -> DurableResult<tos_foundation::JsonValue> {
    let files = writes
        .iter()
        .map(|write| {
            Ok(SourceFile {
                path: RelativePath::parse(&write.subject)
                    .map_err(|_| DurableError::Corrupt("Item recovery source path"))?,
                raw: write.exact_bytes.clone(),
            })
        })
        .collect::<DurableResult<Vec<_>>>()?;
    let ctx = CommandContext {
        base_revision: SourceRevision(Digest256::of_bytes(b"tos-item-recovery-projection-context")),
        configuration_raw: Vec::new(),
        request_raw: Vec::new(),
        recorded_at: String::new(),
        effective_uid: 0,
        files,
    };
    item_claim_transaction_owner_contribution(&ctx, stream_path)
}

fn pg_family_uses_mixed_inventory(family_tag: &str) -> bool {
    if pg_growth_family(family_tag).is_some() {
        return true;
    }
    family_tag == crate::source_creation_store::PG_ITEM_ADOPTION_FAMILY_TAG
        || family_tag.starts_with("tos_local_native_metadata_revision_owner_v1:")
}

fn claim_projection_requires_owner(subject: &str) -> bool {
    subject.starts_with("ToS/source-witnesses/relations/")
        && (subject.ends_with(&format!("/{}", source_claims::CLAIM_STREAM))
            || claim_form_projection_path(subject))
}

fn claim_form_projection_path(subject: &str) -> bool {
    let basename = subject.rsplit('/').next().unwrap_or(subject);
    subject.starts_with("ToS/source-witnesses/relations/")
        && basename.starts_with("source-claims.")
        && basename.ends_with(".human-forms.json")
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

/// A bounded revision invalidates only structural rows that it actually adds.
/// An empty set is meaningful for an index-preserving update: the readset still
/// protects the exact predecessor and semantic owner checks, while shared
/// inventory/source-home ranges are not spuriously advanced by every byte
/// revision.
fn revision_mutation_predicates(indexes: &IndexRows) -> BTreeSet<(String, String, String)> {
    if indexes.is_empty() {
        BTreeSet::new()
    } else {
        predicates(indexes)
    }
}

// A completed import keeps the source cut at which it actually entered PG.
// Reassessment verifies the same exact legacy bytes after later native commits;
// it must not manufacture a new import event at the current source head.
fn publication_import_origin_head(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    current_head: u64,
) -> DurableResult<u64> {
    let row = tx.query_opt(
        "SELECT import_source_head FROM cmd2_source_publication_import WHERE domain=$1 FOR SHARE",
        &[&domain],
    )?;
    let head = match row {
        Some(row) => as_u64(row.get("import_source_head"))?,
        None => current_head,
    };
    if head > current_head {
        return Err(DurableError::Corrupt(
            "publication import is ahead of source cut",
        ));
    }
    // ensure_source_publication_bootstrap_state subsequently checks this head
    // against the exact stored receipt and verifies every other binding.
    Ok(head)
}

/// Build an ordered legacy publication import from the exact complete source
/// cut. Only the selected control and transaction-directory members enter the
/// codec; unrelated source members never become import evidence.
pub(super) fn legacy_selected_publication_import_from_members(
    tx: &mut postgres::Transaction<'_>,
    members: &BTreeMap<String, Vec<u8>>,
    domain: &str,
    store_id: [u8; 16],
    bootstrap_revision: SourceRevision,
    bootstrap_membership: SourceMembershipV1,
    source_head: u64,
) -> DurableResult<Option<crate::source_work_transaction::LegacySelectedPublicationImportV1>> {
    const CONTROL: &str = "ToS/source-witnesses/.metadata-publication.json";
    const TRANSACTION_PREFIX: &str = "ToS/source-witnesses/.metadata-transactions/";
    let exact_members = members
        .iter()
        .filter(|(path, _)| path.as_str() == CONTROL || path.starts_with(TRANSACTION_PREFIX))
        .map(|(path, raw)| (path.clone(), raw.clone()))
        .collect::<BTreeMap<_, _>>();
    if exact_members.is_empty() {
        return Ok(None);
    }
    let source_head = publication_import_origin_head(tx, domain, source_head)?;
    let control = exact_members
        .get(CONTROL)
        .ok_or(DurableError::Corrupt("legacy publication control absent"))?;
    let transitions = crate::source_work_transaction::
        legacy_selected_publication_transition_chain_from_exact_members(&exact_members)
        .map_err(source_error)?;
    let import = crate::source_work_transaction::legacy_selected_publication_import_from_chain(
        domain,
        store_id,
        bootstrap_revision,
        bootstrap_membership,
        source_head,
        control,
        transitions,
    )
    .map_err(source_error)?;
    crate::source_work_transaction::validate_legacy_selected_publication_import(&import, control)
        .map_err(source_error)?;
    if import.binding.domain != domain
        || import.binding.store_id != store_id
        || import.binding.bootstrap_source_revision != bootstrap_revision
        || import.binding.bootstrap_membership != bootstrap_membership
        || import.binding.through_seq != source_head
    {
        return Err(DurableError::Conflict(
            "legacy publication import bootstrap binding differs",
        ));
    }
    Ok(Some(import))
}

/// Ensure the publication pointer and any pre-PG legacy chain are installed
/// before publishing a complete source cut. Legacy generations remain import
/// history under the real bootstrap-completion event, never synthetic PG rows.
pub(super) fn ensure_source_publication_bootstrap_state(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    store_id: [u8; 16],
    initial_revision: SourceRevision,
    initial_membership: SourceMembershipV1,
    source_head: u64,
    definition: Digest256,
    legacy_import: Option<&crate::source_work_transaction::LegacySelectedPublicationImportV1>,
) -> DurableResult<()> {
    let bootstrap_revision = initial_revision.0.to_hex();
    let bootstrap_membership = initial_membership.digest.to_hex();
    let bootstrap_count = as_i64(initial_membership.count)?;
    let current = tx.query_opt(
        "SELECT state,publication_generation,origin_kind,source_commit_seq,
                import_event_id,import_transition_index,import_receipt_digest,
                store_id,bootstrap_source_revision,
                bootstrap_membership_digest,bootstrap_membership_count
         FROM cmd2_source_publication_current WHERE domain=$1 FOR UPDATE",
        &[&domain],
    )?;
    let history_count: i64 = tx
        .query_one(
            "SELECT count(*) FROM cmd2_source_publication_history WHERE domain=$1",
            &[&domain],
        )?
        .get(0);
    let predicate = tx.query_opt(
        "SELECT generation,complete,definition_version FROM cmd2_predicate
         WHERE domain=$1 AND kind='unique' AND owner=$2
           AND scope='source-publication' AND token='current' FOR UPDATE",
        &[&domain, &OWNER],
    )?;
    let Some(current) = current else {
        if history_count != 0 {
            return Err(DurableError::Corrupt(
                "source publication history exists without a current pointer",
            ));
        }
        // This query is over the complete bootstrapped current membership.
        // Any member below the transaction directory, including an orphaned
        // blob, proves that import must preserve a legacy chain explicitly.
        let legacy_material: bool = tx
            .query_one(
                "SELECT EXISTS(
                 SELECT 1 FROM (
                   SELECT subject FROM cmd2_current WHERE domain=$1
                   UNION ALL
                   SELECT subject FROM cmd2_history WHERE domain=$1
                 ) source_member
                 WHERE subject=$2 OR subject LIKE $3
             )",
                &[
                    &domain,
                    &"ToS/source-witnesses/.metadata-publication.json",
                    &"ToS/source-witnesses/.metadata-transactions/%",
                ],
            )?
            .get(0);
        if legacy_material && legacy_import.is_none() {
            return Err(DurableError::Refused(
                "legacy publication chain requires exact ordered import",
            ));
        }
        if predicate.as_ref().is_some_and(|row| {
            row.get::<_, i64>(0) != 0 || row.get::<_, String>(2) != definition.to_hex()
        }) {
            return Err(DurableError::Conflict(
                "source publication genesis predicate differs",
            ));
        }
        let terminal_generation = if let Some(import) = legacy_import {
            let binding = &import.binding;
            if binding.domain != domain
                || binding.store_id != store_id
                || binding.bootstrap_source_revision != initial_revision
                || binding.bootstrap_membership != initial_membership
                || binding.through_seq != source_head
                || import.receipt_sha256 != Digest256::of_bytes(&import.receipt_bytes)
                || import.receipt_sha256 != binding.receipt_sha256
                || import.packages.is_empty()
                || import.packages.len() as u64 != binding.transition_count
            {
                return Err(DurableError::Conflict(
                    "legacy publication import bootstrap identity differs",
                ));
            }
            let terminal = import.packages.last().ok_or(DurableError::Corrupt(
                "legacy publication import terminal absent",
            ))?;
            if terminal.generation != binding.head_generation
                || terminal.token != binding.head_token
                || terminal.control_sha256 != binding.head_control_sha256
            {
                return Err(DurableError::Corrupt(
                    "legacy publication import terminal differs",
                ));
            }
            let import_source_head = as_i64(binding.through_seq)?;
            let chain_root = binding.chain_root_sha256.to_hex();
            let head_control_digest = binding.head_control_sha256.to_hex();
            let terminal_generation = as_i64(binding.head_generation)?;
            let receipt_digest = import.receipt_sha256.to_hex();
            tx.execute(
                "INSERT INTO cmd2_source_publication_import(
                    domain,origin_kind,import_event_id,receipt_digest,receipt_bytes,store_id,
                    bootstrap_source_revision,bootstrap_membership_digest,bootstrap_membership_count,
                    import_source_head,chain_root_digest,transition_count,head_control_digest,
                    terminal_generation,terminal_token)
                 VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)",
                &[
                    &domain,
                    &binding.origin_kind,
                    &binding.import_event_id,
                    &receipt_digest,
                    &import.receipt_bytes,
                    &&store_id[..],
                    &bootstrap_revision,
                    &bootstrap_membership,
                    &bootstrap_count,
                    &import_source_head,
                    &chain_root,
                    &as_i64(binding.transition_count)?,
                    &head_control_digest,
                    &terminal_generation,
                    &binding.head_token,
                ],
            )?;
            tx.execute(
                "INSERT INTO cmd2_source_publication_history(
                    domain,publication_generation,state,token,base_token,base_generation,
                    transaction_id,package_digest,package_bytes,source_commit_seq,origin_kind,
                    import_event_id,import_transition_index,import_receipt_digest,store_id,
                    bootstrap_source_revision,bootstrap_membership_digest,bootstrap_membership_count)
                 VALUES($1,0,'absent',NULL,NULL,0,NULL,NULL,NULL,0,'bootstrap_absent',
                    NULL,NULL,NULL,$2,$3,$4,$5)",
                &[
                    &domain,
                    &&store_id[..],
                    &bootstrap_revision,
                    &bootstrap_membership,
                    &bootstrap_count,
                ],
            )?;
            let no_pg_seq: Option<i64> = None;
            for package in &import.packages {
                tx.execute(
                    "INSERT INTO cmd2_source_publication_history(
                        domain,publication_generation,state,token,base_token,base_generation,
                        transaction_id,package_digest,package_bytes,source_commit_seq,origin_kind,
                        import_event_id,import_transition_index,import_receipt_digest,store_id,
                        bootstrap_source_revision,bootstrap_membership_digest,bootstrap_membership_count)
                     VALUES($1,$2,'present',$3,$4,$5,$6,$7,$8,$9,'legacy_sto_import',
                        $10,$11,$12,$13,$14,$15,$16)",
                    &[
                        &domain,
                        &as_i64(package.generation)?,
                        &package.token,
                        &package.base_publication_token,
                        &as_i64(package.base_publication_generation)?,
                        &package.transaction_id,
                        &package.sha256.to_hex(),
                        &package.canonical_bytes,
                        &no_pg_seq,
                        &binding.import_event_id,
                        &as_i64(package.transition_index)?,
                        &receipt_digest,
                        &&store_id[..],
                        &bootstrap_revision,
                        &bootstrap_membership,
                        &bootstrap_count,
                    ],
                )?;
            }
            tx.execute(
                "INSERT INTO cmd2_source_publication_current(
                    domain,state,publication_generation,token,base_token,base_generation,
                    transaction_id,package_digest,package_bytes,source_commit_seq,origin_kind,
                    import_event_id,import_transition_index,import_receipt_digest,store_id,
                    bootstrap_source_revision,bootstrap_membership_digest,bootstrap_membership_count)
                 VALUES($1,'present',$2,$3,$4,$5,$6,$7,$8,$9,'legacy_sto_import',
                    $10,$11,$12,$13,$14,$15,$16)",
                &[
                    &domain,
                    &as_i64(terminal.generation)?,
                    &terminal.token,
                    &terminal.base_publication_token,
                    &as_i64(terminal.base_publication_generation)?,
                    &terminal.transaction_id,
                    &terminal.sha256.to_hex(),
                    &terminal.canonical_bytes,
                    &no_pg_seq,
                    &binding.import_event_id,
                    &as_i64(terminal.transition_index)?,
                    &receipt_digest,
                    &&store_id[..],
                    &bootstrap_revision,
                    &bootstrap_membership,
                    &bootstrap_count,
                ],
            )?;
            binding.head_generation
        } else {
            tx.execute(
                "INSERT INTO cmd2_source_publication_current(
                    domain,state,publication_generation,token,base_token,base_generation,
                    transaction_id,package_digest,package_bytes,source_commit_seq,origin_kind,
                    import_event_id,import_transition_index,import_receipt_digest,store_id,
                    bootstrap_source_revision,bootstrap_membership_digest,bootstrap_membership_count)
                 VALUES($1,'absent',0,NULL,NULL,0,NULL,NULL,NULL,0,'bootstrap_absent',
                    NULL,NULL,NULL,$2,$3,$4,$5)",
                &[
                    &domain,
                    &&store_id[..],
                    &bootstrap_revision,
                    &bootstrap_membership,
                    &bootstrap_count,
                ],
            )?;
            tx.execute(
                "INSERT INTO cmd2_source_publication_history(
                    domain,publication_generation,state,token,base_token,base_generation,
                    transaction_id,package_digest,package_bytes,source_commit_seq,origin_kind,
                    import_event_id,import_transition_index,import_receipt_digest,store_id,
                    bootstrap_source_revision,bootstrap_membership_digest,bootstrap_membership_count)
                 VALUES($1,0,'absent',NULL,NULL,0,NULL,NULL,NULL,0,'bootstrap_absent',
                    NULL,NULL,NULL,$2,$3,$4,$5)",
                &[
                    &domain,
                    &&store_id[..],
                    &bootstrap_revision,
                    &bootstrap_membership,
                    &bootstrap_count,
                ],
            )?;
            0
        };
        if let Some(predicate) = &predicate {
            if predicate.get::<_, i64>(0) != 0
                || predicate.get::<_, String>(2) != definition.to_hex()
            {
                return Err(DurableError::Conflict(
                    "source publication genesis predicate differs",
                ));
            }
            tx.execute(
                "UPDATE cmd2_predicate SET generation=$4,complete=true
                 WHERE domain=$1 AND kind='unique' AND owner=$2
                   AND scope='source-publication' AND token='current'
                   AND generation=0 AND definition_version=$3",
                &[
                    &domain,
                    &OWNER,
                    &definition.to_hex(),
                    &as_i64(terminal_generation)?,
                ],
            )?;
        } else {
            tx.execute(
                "INSERT INTO cmd2_predicate(domain,kind,owner,scope,token,definition_version,generation,complete)
                 VALUES($1,'unique',$2,'source-publication','current',$3,$4,true)",
                &[
                    &domain,
                    &OWNER,
                    &definition.to_hex(),
                    &as_i64(terminal_generation)?,
                ],
            )?;
        }
        let seeded = tx.query_one(
            "SELECT generation,complete,definition_version FROM cmd2_predicate
             WHERE domain=$1 AND kind='unique' AND owner=$2
               AND scope='source-publication' AND token='current' FOR UPDATE",
            &[&domain, &OWNER],
        )?;
        if seeded.get::<_, i64>(0) != as_i64(terminal_generation)?
            || !seeded.get::<_, bool>(1)
            || seeded.get::<_, String>(2) != definition.to_hex()
        {
            return Err(DurableError::Conflict(
                "source publication genesis predicate changed",
            ));
        }
        tx.execute(
            "UPDATE cmd2_predicate SET complete=true
             WHERE domain=$1 AND kind='unique' AND owner=$2
               AND scope='source-publication' AND token='current'
               AND generation=$3 AND definition_version=$4",
            &[
                &domain,
                &OWNER,
                &as_i64(terminal_generation)?,
                &definition.to_hex(),
            ],
        )?;
        return Ok(());
    };

    let generation: i64 = current.get("publication_generation");
    if generation < 0
        || generation % 2 != 0
        || current.get::<_, Vec<u8>>("store_id") != store_id
        || current.get::<_, String>("bootstrap_source_revision") != bootstrap_revision
        || current.get::<_, String>("bootstrap_membership_digest") != bootstrap_membership
        || current.get::<_, i64>("bootstrap_membership_count") != bootstrap_count
    {
        return Err(DurableError::Conflict(
            "source publication bootstrap identity differs",
        ));
    }
    let persisted_import = tx.query_opt(
        "SELECT origin_kind,import_event_id,receipt_digest,receipt_bytes,store_id,
                bootstrap_source_revision,bootstrap_membership_digest,bootstrap_membership_count,
                import_source_head,chain_root_digest,transition_count,head_control_digest,
                terminal_generation,terminal_token
         FROM cmd2_source_publication_import WHERE domain=$1 FOR UPDATE",
        &[&domain],
    )?;
    match (persisted_import.as_ref(), legacy_import) {
        (None, Some(_)) => {
            return Err(DurableError::Corrupt(
                "source publication import receipt is absent",
            ));
        }
        (Some(row), supplied) => {
            let receipt_bytes: Vec<u8> = row.get("receipt_bytes");
            let (binding, receipt_digest) =
                crate::source_work_transaction::parse_legacy_selected_publication_import_receipt(
                    &receipt_bytes,
                )
                .map_err(source_error)?;
            let expected_digest = parse_hex(row.get::<_, String>("receipt_digest"))?;
            if row.get::<_, String>("origin_kind") != "legacy_sto_import"
                || row.get::<_, String>("import_event_id") != binding.import_event_id
                || receipt_digest != expected_digest
                || binding.receipt_sha256 != expected_digest
                || binding.domain != domain
                || binding.store_id != store_id
                || binding.bootstrap_source_revision != initial_revision
                || binding.bootstrap_membership != initial_membership
                || binding.through_seq != as_u64(row.get("import_source_head"))?
                || binding.chain_root_sha256 != parse_hex(row.get("chain_root_digest"))?
                || binding.transition_count != as_u64(row.get("transition_count"))?
                || binding.head_control_sha256 != parse_hex(row.get("head_control_digest"))?
                || binding.head_generation != as_u64(row.get("terminal_generation"))?
                || binding.head_token != row.get::<_, String>("terminal_token")
                || row.get::<_, Vec<u8>>("store_id") != store_id
                || row.get::<_, String>("bootstrap_source_revision") != bootstrap_revision
                || row.get::<_, String>("bootstrap_membership_digest") != bootstrap_membership
                || row.get::<_, i64>("bootstrap_membership_count") != bootstrap_count
                || supplied.is_some_and(|import| {
                    import.binding != binding || import.receipt_bytes != receipt_bytes
                })
            {
                return Err(DurableError::Corrupt(
                    "source publication import receipt binding differs",
                ));
            }
            let import_totals = tx.query_one(
                "SELECT count(*),COALESCE(sum(octet_length(package_bytes)),0)
                 FROM cmd2_source_publication_history
                 WHERE domain=$1 AND origin_kind='legacy_sto_import'",
                &[&domain],
            )?;
            let imported_count: i64 = import_totals.get(0);
            let imported_bytes: i64 = import_totals.get(1);
            if imported_count < 0
                || imported_bytes < 0
                || as_u64(imported_count)? != binding.transition_count
                || as_u64(imported_count)? > binding.bootstrap_membership.count
                || as_u64(imported_count)? > cmd::SELECTED_SOURCE_MAX_FILES as u64
                || as_u64(imported_bytes)? > (cmd::SELECTED_SOURCE_MAX_BYTES * 2) as u64
            {
                return Err(DurableError::Corrupt(
                    "source publication import history exceeds its receipt budget",
                ));
            }
            let rows = tx.query(
                "SELECT publication_generation,state,token,base_token,base_generation,
                        transaction_id,package_digest,package_bytes,source_commit_seq,
                        origin_kind,import_event_id,import_transition_index,
                        import_receipt_digest
                 FROM cmd2_source_publication_history
                 WHERE domain=$1 AND origin_kind='legacy_sto_import'
                 ORDER BY import_transition_index",
                &[&domain],
            )?;
            let package_bytes = rows
                .iter()
                .map(|row| row.get::<_, Vec<u8>>(7))
                .collect::<Vec<_>>();
            let package_bindings =
                crate::source_work_transaction::validate_legacy_selected_publication_import_rows(
                    &receipt_bytes,
                    &package_bytes,
                )
                .map_err(source_error)?;
            if package_bindings.len() != rows.len() {
                return Err(DurableError::Corrupt(
                    "source publication import history row count differs",
                ));
            }
            for (index, (stored, expected)) in rows.iter().zip(&package_bindings).enumerate() {
                let transition_index = u64::try_from(index + 1)
                    .map_err(|_| DurableError::Corrupt("source publication import index"))?;
                if stored.get::<_, String>(1) != "present"
                    || stored.get::<_, i64>(0) != as_i64(expected.generation)?
                    || stored.get::<_, Option<String>>(2).as_deref()
                        != Some(expected.token.as_str())
                    || stored.get::<_, Option<String>>(3) != expected.base_publication_token
                    || as_u64(stored.get::<_, i64>(4))? != expected.base_publication_generation
                    || stored.get::<_, String>(5) != expected.transaction_id
                    || parse_hex(stored.get::<_, String>(6))? != expected.package_sha256
                    || stored.get::<_, Vec<u8>>(7) != package_bytes[index]
                    || stored.get::<_, Option<i64>>(8).is_some()
                    || stored.get::<_, String>(9) != "legacy_sto_import"
                    || stored.get::<_, String>(10) != binding.import_event_id
                    || as_u64(stored.get::<_, i64>(11))? != transition_index
                    || transition_index != expected.transition_index
                    || parse_hex(stored.get::<_, String>(12))? != binding.receipt_sha256
                {
                    return Err(DurableError::Corrupt(
                        "source publication import history row differs from its package",
                    ));
                }
                if let Some(import) = supplied {
                    let source_package = import.packages.get(index).ok_or(
                        DurableError::Corrupt("source publication supplied import row absent"),
                    )?;
                    if source_package.canonical_bytes != package_bytes[index]
                        || source_package.sha256 != expected.package_sha256
                        || source_package.transition_index != expected.transition_index
                        || source_package.transaction_id != expected.transaction_id
                        || source_package.manifest_schema_version
                            != expected.manifest_schema_version
                        || source_package.manifest_sha256 != expected.manifest_sha256
                        || source_package.completion_sha256 != expected.completion_sha256
                        || source_package.control_sha256 != expected.control_sha256
                        || source_package.generation != expected.generation
                        || source_package.token != expected.token
                        || source_package.base_publication_token != expected.base_publication_token
                        || source_package.base_publication_generation
                            != expected.base_publication_generation
                    {
                        return Err(DurableError::Corrupt(
                            "source publication import history differs from selected source",
                        ));
                    }
                }
            }
        }
        (None, None) => {}
    }
    if history_count != generation / 2 + 1 {
        return Err(DurableError::Corrupt(
            "source publication transition history is incomplete",
        ));
    }
    let latest_matches: bool = tx
        .query_one(
            "SELECT to_jsonb(c)=to_jsonb(h)
         FROM cmd2_source_publication_current c
         JOIN cmd2_source_publication_history h
           ON h.domain=c.domain AND h.publication_generation=c.publication_generation
         WHERE c.domain=$1",
            &[&domain],
        )?
        .get(0);
    if !latest_matches {
        return Err(DurableError::Corrupt(
            "source publication current pointer differs from latest history",
        ));
    }
    let inconsistent_chain: bool = tx
        .query_one(
            "SELECT EXISTS(
            SELECT 1 FROM cmd2_source_publication_history h
            LEFT JOIN cmd2_source_publication_history p
              ON p.domain=h.domain AND p.publication_generation=h.base_generation
            LEFT JOIN cmd2_source_publication_import i
              ON i.domain=h.domain AND i.import_event_id=h.import_event_id
            WHERE h.domain=$1 AND
              (h.store_id<>$2 OR h.bootstrap_source_revision<>$3
               OR h.bootstrap_membership_digest<>$4
               OR h.bootstrap_membership_count<>$5
               OR (h.publication_generation=0 AND
                   (h.state<>'absent' OR h.source_commit_seq<>0
                    OR h.origin_kind<>'bootstrap_absent' OR h.import_event_id IS NOT NULL))
               OR (h.publication_generation>0 AND
                   (h.publication_generation<>h.base_generation+2 OR p.domain IS NULL
                    OR h.base_token IS DISTINCT FROM p.token
                    OR (h.origin_kind='legacy_sto_import' AND
                        (h.source_commit_seq IS NOT NULL OR i.domain IS NULL
                         OR h.import_receipt_digest<>i.receipt_digest
                         OR h.import_transition_index IS NULL
                         OR h.import_transition_index<> (
                           SELECT count(*) FROM cmd2_source_publication_history imported
                           WHERE imported.domain=h.domain
                             AND imported.origin_kind='legacy_sto_import'
                             AND imported.import_event_id=h.import_event_id
                             AND imported.publication_generation<=h.publication_generation)
                         OR h.import_transition_index>i.transition_count
                         OR h.publication_generation>i.terminal_generation))
                    OR (h.origin_kind='pg_commit' AND
                        (h.source_commit_seq IS NULL OR h.source_commit_seq<=COALESCE(
                           p.source_commit_seq,
                           (SELECT previous_import.import_source_head
                            FROM cmd2_source_publication_import previous_import
                            WHERE previous_import.domain=p.domain
                              AND previous_import.import_event_id=p.import_event_id),
                           0)
                         OR h.source_commit_seq>$6
                         OR NOT EXISTS(
                            SELECT 1 FROM cmd2_attempt a
                            WHERE a.domain=h.domain AND a.commit_seq=h.source_commit_seq
                              AND a.state='committed'
                              AND a.source_publication_package=h.package_bytes)))
                    OR h.origin_kind NOT IN ('legacy_sto_import','pg_commit'))))
         )",
            &[
                &domain,
                &&store_id[..],
                &bootstrap_revision,
                &bootstrap_membership,
                &bootstrap_count,
                &as_i64(source_head)?,
            ],
        )?
        .get(0);
    if inconsistent_chain {
        return Err(DurableError::Corrupt(
            "source publication transition lineage is incomplete",
        ));
    }
    if predicate.as_ref().is_some_and(|predicate| {
        predicate.get::<_, i64>(0) != generation
            || predicate.get::<_, String>(2) != definition.to_hex()
    }) {
        return Err(DurableError::Conflict(
            "source publication current predicate differs",
        ));
    }
    tx.execute(
        "INSERT INTO cmd2_predicate(domain,kind,owner,scope,token,definition_version,generation,complete)
         VALUES($1,'unique',$2,'source-publication','current',$3,$4,true)
         ON CONFLICT(domain,kind,owner,scope,token) DO NOTHING",
        &[&domain, &OWNER, &definition.to_hex(), &generation],
    )?;
    let predicate = tx.query_one(
        "SELECT generation,definition_version FROM cmd2_predicate
         WHERE domain=$1 AND kind='unique' AND owner=$2
           AND scope='source-publication' AND token='current' FOR UPDATE",
        &[&domain, &OWNER],
    )?;
    if predicate.get::<_, i64>(0) != generation
        || predicate.get::<_, String>(1) != definition.to_hex()
    {
        return Err(DurableError::Conflict(
            "source publication current predicate differs",
        ));
    }
    tx.execute(
        "UPDATE cmd2_predicate SET complete=true
         WHERE domain=$1 AND kind='unique' AND owner=$2
           AND scope='source-publication' AND token='current'
           AND generation=$3 AND definition_version=$4",
        &[&domain, &OWNER, &generation, &definition.to_hex()],
    )?;
    Ok(())
}

fn decode_indexes(raw: &[u8]) -> DurableResult<IndexRows> {
    let rows = serde_json::from_slice::<Vec<(String, String, String)>>(raw)
        .map_err(|_| DurableError::Corrupt("source indexes decode"))?;
    if rows
        .iter()
        .any(|(kind, token, path)| kind.is_empty() || token.is_empty() || path.is_empty())
    {
        return Err(DurableError::Corrupt("source indexes contain empty row"));
    }
    Ok(rows.into_iter().collect())
}

fn revision_indexes(writes: &[SourceRevisionWrite]) -> IndexRows {
    writes
        .iter()
        .filter(|write| write.expected_predecessor.is_none())
        .map(|write| {
            (
                "path".to_owned(),
                write.subject.clone(),
                write.subject.clone(),
            )
        })
        .collect()
}

fn expected_revision_indexes_for_family(
    family_tag: &str,
    writes: &[SourceRevisionWrite],
) -> DurableResult<IndexRows> {
    if family_tag.starts_with("tos-pg-global-claims-v1:") {
        claims::expected_revision_indexes(family_tag, writes)
    } else if let Some(family) = pg_growth_family(family_tag) {
        crate::source_native_cli::expected_growth_revision_indexes(family, writes)
    } else if family_tag == crate::source_creation_store::OBJECT_LINK_PG_FAMILY_TAG {
        crate::source_creation_store::expected_object_link_revision_indexes(writes)
    } else if family_tag == crate::source_creation_store::PG_ITEM_ADOPTION_FAMILY_TAG {
        let mut indexes = revision_indexes(writes);
        indexes.extend(crate::source_creation_store::pg_expected_revision_indexes(
            writes,
        )?);
        Ok(indexes)
    } else if family_tag == crate::source_public_text_entry::PG_FAMILY_TAG {
        public_text_insertion_paths(writes)?;
        Ok(revision_indexes(writes))
    } else if family_tag.starts_with("tos-pg-maintained-growth-v1:") {
        Err(DurableError::Refused(
            "PG Growth owner family is not admitted by the shared writer",
        ))
    } else {
        Ok(revision_indexes(writes))
    }
}

fn validate_revision_indexes(
    family_tag: &str,
    writes: &[SourceRevisionWrite],
    indexes: &IndexRows,
) -> DurableResult<()> {
    if pg_forms_historical_family(family_tag).is_some() {
        let expected_paths = revision_indexes(writes);
        let actual_paths = indexes
            .iter()
            .filter(|(kind, _, _)| kind == "path")
            .cloned()
            .collect::<IndexRows>();
        if actual_paths != expected_paths {
            return Err(DurableError::Corrupt(
                "PG Forms/Historical structural path index binding differs",
            ));
        }
        for (kind, token, path) in indexes.iter().filter(|(kind, _, _)| kind != "path") {
            let write =
                writes
                    .iter()
                    .find(|write| write.subject == *path)
                    .ok_or(DurableError::Corrupt(
                        "PG Forms/Historical semantic index path absent",
                    ))?;
            if kind != "form"
                || !path.ends_with(".human-forms.json")
                || path.starts_with("ToS/source-witnesses/.record-revisions/")
                || !revision_form_ids(&write.exact_bytes)?.contains(token)
            {
                return Err(DurableError::Corrupt(
                    "PG Forms/Historical semantic form index binding differs",
                ));
            }
        }
        return Ok(());
    }
    let expected = expected_revision_indexes_for_family(family_tag, writes)?;
    if *indexes != expected {
        return Err(DurableError::Corrupt(
            "source revision exact family index binding differs",
        ));
    }
    for (kind, token, path) in indexes {
        if kind == "path" {
            if token != path
                || !writes
                    .iter()
                    .any(|write| write.expected_predecessor.is_none() && write.subject == *path)
            {
                return Err(DurableError::Corrupt(
                    "source revision structural path index binding differs",
                ));
            }
        } else if kind == "sign-candidate" {
            if family_tag != "tos-pg-maintained-growth-v1:sign-promotion"
                || path.rsplit('/').next() != Some("sign.json")
                || !writes.iter().any(|write| write.subject == *path)
            {
                return Err(DurableError::Corrupt(
                    "source revision Sign candidate index binding differs",
                ));
            }
        } else if !(family_tag.ends_with(":claims.create")
            || pg_growth_family(family_tag).is_some()
            || family_tag == crate::source_creation_store::OBJECT_LINK_PG_FAMILY_TAG
            || family_tag == crate::source_creation_store::PG_ITEM_ADOPTION_FAMILY_TAG)
            || !["metadata", "claim", "event", "anchor", "form"].contains(&kind.as_str())
            || !writes.iter().any(|write| write.subject == *path)
        {
            return Err(DurableError::Corrupt(
                "source revision semantic index binding differs",
            ));
        }
    }
    Ok(())
}

/// A revision usually contributes only new semantic identities. Item adoption
/// also carries the already indexed Edition and its retained form identities.
/// Reusing those rows requires the exact same kind, path, definition and a
/// sealed predecessor write; another kind or owner path is still a conflict.
fn revision_retains_semantic_index(
    tx: &mut Transaction<'_>,
    domain: &str,
    attempt: &SourceRevisionAttempt,
    kind: &str,
    token: &str,
    path: &str,
) -> DurableResult<bool> {
    let kinds = if kind == "sign-candidate"
        && attempt.family_tag == "tos-pg-maintained-growth-v1:sign-promotion"
    {
        vec!["sign-candidate"]
    } else {
        vec!["metadata", "claim", "event", "anchor", "form"]
    };
    // One row establishes the owner; a second proves cross-kind ambiguity.
    let rows = tx.query(
        "SELECT kind,path,definition_digest FROM cmd2_source_index
         WHERE domain=$1 AND kind=ANY($2) AND token=$3 LIMIT 2",
        &[&domain, &kinds, &token],
    )?;
    if rows.is_empty() {
        return Ok(false);
    }
    if rows.len() != 1
        || attempt.family_tag != crate::source_creation_store::PG_ITEM_ADOPTION_FAMILY_TAG
        || rows[0].get::<_, String>("kind") != kind
        || rows[0].get::<_, String>("path") != path
        || rows[0].get::<_, String>("definition_digest") != attempt.definition.to_hex()
        || !attempt
            .writes
            .iter()
            .any(|write| write.subject == path && write.expected_predecessor.is_some())
    {
        return Err(DurableError::Conflict(
            "source revision semantic identity changed",
        ));
    }
    Ok(true)
}

fn revision_form_ids(raw: &[u8]) -> DurableResult<BTreeSet<String>> {
    let value = cmd::parse(raw).map_err(source_error)?;
    source_forms::apply_form_changes(
        Some(&value),
        cmd::field(&value, "subject").map_err(source_error)?,
        &[],
    )
    .map_err(source_error)?;
    let mut ids = BTreeSet::new();
    for section in ["forms", "prior_forms"] {
        for form in cmd::array(&value, section).map_err(source_error)? {
            let id = cmd::text(form, "form_id").map_err(source_error)?.to_owned();
            ids.insert(id);
        }
    }
    Ok(ids)
}

/// Existing maintained semantic rows are not recomputed from a whole corpus.
/// The bounded owner already validates the successor package; this narrow
/// check makes the durable path refuse an index identity transition it cannot
/// atomically maintain. Archive additions carry path rows only.
fn check_revision_semantic_indexes(
    tx: &mut Transaction<'_>,
    domain: &str,
    writes: &[SourceRevisionWrite],
    item_adoption_indexes: Option<&IndexRows>,
) -> DurableResult<()> {
    // `source_copy_kernel::validate_history` admits at most 32 current and
    // 256 retained forms. A single forms member therefore has at most 288
    // semantic rows; LIMIT+1 proves refusal without retaining an unbounded
    // per-path result if the durable index is corrupt or unexpectedly larger.
    const MAX_SEMANTIC_INDEX_ROWS_PER_SOURCE_PATH: usize = 32 + 256;
    let semantic_row_limit = i64::try_from(MAX_SEMANTIC_INDEX_ROWS_PER_SOURCE_PATH + 1)
        .map_err(|_| DurableError::Corrupt("semantic source index row limit"))?;
    for write in writes {
        let rows = tx.query(
            "SELECT kind,token,path FROM cmd2_source_index
             WHERE domain=$1 AND path COLLATE \"C\"=$2 COLLATE \"C\" AND kind<>'path'
             ORDER BY kind COLLATE \"C\",token COLLATE \"C\"
             LIMIT $3",
            &[&domain, &write.subject, &semantic_row_limit],
        )?;
        if rows.len() > MAX_SEMANTIC_INDEX_ROWS_PER_SOURCE_PATH {
            return Err(DurableError::Refused(
                "source semantic index rows exceed the maintained form history bound",
            ));
        }
        if rows.is_empty() {
            continue;
        }
        let mut has_metadata = false;
        let mut has_forms = false;
        for row in &rows {
            let kind: String = row.get("kind");
            let path: String = row.get("path");
            if path != write.subject {
                return Err(DurableError::Corrupt(
                    "source semantic index path binding differs",
                ));
            }
            match kind.as_str() {
                "metadata" => has_metadata = true,
                "form" => has_forms = true,
                "claim" | "event"
                    if item_adoption_indexes.is_some_and(|indexes| {
                        indexes.contains(&(kind.clone(), row.get("token"), write.subject.clone()))
                    }) => {}
                _ => {
                    return Err(DurableError::Refused(
                        "bounded source revision semantic index transition unsupported",
                    ));
                }
            }
        }
        // Parse each successor member once. The durable index may retain one
        // row per historical form version, so doing this inside the row loop
        // would turn a bounded 288-row check into repeated JSON validation.
        let metadata_ids = if has_metadata {
            let record = cmd::parse(&write.exact_bytes).map_err(source_error)?;
            let subject = source_forms::metadata_subject(&record).map_err(source_error)?;
            let id = cmd::text(&subject, "id").map_err(source_error)?;
            BTreeSet::from([id.to_owned()])
        } else {
            BTreeSet::new()
        };
        let form_ids = if has_forms {
            revision_form_ids(&write.exact_bytes)?
        } else {
            BTreeSet::new()
        };
        for row in &rows {
            let kind: String = row.get("kind");
            let token: String = row.get("token");
            let matches = match kind.as_str() {
                "metadata" => metadata_ids.contains(&token),
                "form" => form_ids.contains(&token),
                "claim" | "event"
                    if item_adoption_indexes.is_some_and(|indexes| {
                        indexes.contains(&(kind.clone(), token.clone(), write.subject.clone()))
                    }) =>
                {
                    true
                }
                _ => {
                    return Err(DurableError::Refused(
                        "bounded source revision semantic index transition unsupported",
                    ));
                }
            };
            if !matches {
                return Err(DurableError::Conflict(
                    "source revision semantic identity changed",
                ));
            }
        }
        let stored_metadata = rows
            .iter()
            .filter(|row| row.get::<_, String>("kind") == "metadata")
            .map(|row| row.get::<_, String>("token"))
            .collect::<BTreeSet<_>>();
        if !stored_metadata.is_empty() && stored_metadata != metadata_ids {
            return Err(DurableError::Conflict(
                "source revision metadata identity set changed",
            ));
        }
        let stored_forms = rows
            .iter()
            .filter(|row| row.get::<_, String>("kind") == "form")
            .map(|row| row.get::<_, String>("token"))
            .collect::<BTreeSet<_>>();
        if !stored_forms.is_empty() && stored_forms != form_ids {
            return Err(DurableError::Conflict(
                "source revision form identity set changed",
            ));
        }
        if let Some(expected) = item_adoption_indexes {
            for kind in ["claim", "event"] {
                let stored = rows
                    .iter()
                    .filter(|row| row.get::<_, String>("kind") == kind)
                    .map(|row| row.get::<_, String>("token"))
                    .collect::<BTreeSet<_>>();
                if stored.is_empty() {
                    continue;
                }
                let expected = expected
                    .iter()
                    .filter(|(expected_kind, _, path)| {
                        expected_kind == kind && path == &write.subject
                    })
                    .map(|(_, token, _)| token.clone())
                    .collect::<BTreeSet<_>>();
                if stored != expected {
                    return Err(DurableError::Conflict(
                        "Item adoption semantic identity set changed",
                    ));
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn check_revision_path_indexes(
    tx: &mut Transaction<'_>,
    domain: &str,
    writes: &[SourceRevisionWrite],
) -> DurableResult<IndexRows> {
    check_revision_path_indexes_inner(tx, domain, writes, None, &BTreeSet::new())
}

pub(crate) fn check_revision_path_indexes_for_family(
    tx: &mut Transaction<'_>,
    domain: &str,
    writes: &[SourceRevisionWrite],
    family_tag: &str,
) -> DurableResult<IndexRows> {
    let item_adoption_indexes =
        if family_tag == crate::source_creation_store::PG_ITEM_ADOPTION_FAMILY_TAG {
            Some(crate::source_creation_store::pg_expected_revision_indexes(
                writes,
            )?)
        } else {
            None
        };
    let allowed_insertions = if item_adoption_indexes.is_some() {
        crate::source_creation_store::pg_item_adoption_insertion_paths(writes)?
    } else if family_tag == crate::source_public_text_entry::PG_FAMILY_TAG {
        public_text_insertion_paths(writes)?
    } else {
        BTreeSet::new()
    };
    check_revision_path_indexes_inner(
        tx,
        domain,
        writes,
        item_adoption_indexes.as_ref(),
        &allowed_insertions,
    )
}

fn public_text_insertion_paths(writes: &[SourceRevisionWrite]) -> DurableResult<BTreeSet<String>> {
    use crate::source_public_text_entry as owner;
    if writes.len() != owner::FILES.len()
        || writes
            .iter()
            .any(|write| write.expected_predecessor.is_some() || write.proposed_revision != 1)
    {
        return Err(DurableError::Refused(
            "public Text creation requires one new complete package",
        ));
    }
    let packet = writes
        .iter()
        .find(|write| write.subject.ends_with("/source-text-unit.v1.json"))
        .ok_or(DurableError::Refused("public Text creation packet absent"))?;
    let home = owner::public_text_package_home(&packet.subject).map_err(source_error)?;
    let prefix = format!("{home}/");
    let mut files = BTreeMap::new();
    for write in writes {
        let name = write
            .subject
            .strip_prefix(&prefix)
            .filter(|name| owner::FILES.contains(name))
            .ok_or(DurableError::Refused(
                "public Text creation member outside package",
            ))?;
        if files
            .insert(name.to_owned(), write.exact_bytes.clone())
            .is_some()
        {
            return Err(DurableError::Refused("public Text creation member repeats"));
        }
    }
    // Re-derive the package receipt and native bindings from all exact bytes
    // at registration and recovery/commit, not from a caller path allowlist.
    owner::public_text_inventory_projection(&home, &packet.subject, &files)
        .map_err(source_error)?;
    Ok(writes.iter().map(|write| write.subject.clone()).collect())
}

fn check_revision_path_indexes_inner(
    tx: &mut Transaction<'_>,
    domain: &str,
    writes: &[SourceRevisionWrite],
    item_adoption_indexes: Option<&IndexRows>,
    allowed_insertions: &BTreeSet<String>,
) -> DurableResult<IndexRows> {
    let indexes = revision_indexes(writes);
    let history_insertions = writes
        .iter()
        .filter(|write| {
            write.expected_predecessor.is_none()
                && write.subject.ends_with("/source-revision-history.json")
        })
        .collect::<Vec<_>>();
    if history_insertions.len() > 1 {
        return Err(DurableError::Invalid(
            "source revision history insertion occurs more than once",
        ));
    }
    if let Some(history_write) = history_insertions.first().copied() {
        if !history_write.subject.starts_with("ToS/source-witnesses/") {
            return Err(DurableError::Refused(
                "bounded source revision history must remain under source witnesses",
            ));
        }
        let (parent, _) = history_write
            .subject
            .rsplit_once('/')
            .ok_or(DurableError::Invalid("source revision history parent"))?;
        let record_writes = writes
            .iter()
            .filter(|write| {
                write.expected_predecessor.is_some()
                    && write
                        .subject
                        .rsplit_once('/')
                        .is_some_and(|(write_parent, basename)| {
                            write_parent == parent
                                && basename != "source-revision-history.json"
                                && !basename.ends_with(".human-forms.json")
                        })
            })
            .collect::<Vec<_>>();
        if record_writes.len() != 1 {
            return Err(DurableError::Refused(
                "bounded source revision history insertion lacks one owner record successor",
            ));
        }
        let record_write = record_writes[0];
        let (_, record_basename) = record_write
            .subject
            .rsplit_once('/')
            .ok_or(DurableError::Invalid("source revision record parent"))?;
        let record = cmd::parse(&record_write.exact_bytes).map_err(source_error)?;
        let mut package = source_revisions::Package::new();
        package.insert(record_basename.to_owned(), record_write.exact_bytes.clone());
        package.insert(
            "source-revision-history.json".to_owned(),
            history_write.exact_bytes.clone(),
        );
        source_revisions::history(&package, &record).map_err(source_error)?;
    }
    for write in writes {
        let paths = tx.query(
            "SELECT path FROM cmd2_source_index
             WHERE domain=$1 AND kind='path' AND token=$2",
            &[&domain, &write.subject],
        )?;
        if write.expected_predecessor.is_none() {
            if !write
                .subject
                .starts_with("ToS/source-witnesses/.record-revisions/")
                && !write.subject.ends_with("/source-revision-history.json")
                && !allowed_insertions.contains(&write.subject)
            {
                return Err(DurableError::Refused(
                    "bounded source revision insertion is limited to owner history or retained archive members",
                ));
            }
            if !paths.is_empty() {
                return Err(DurableError::Conflict(
                    "source revision archive path index is already occupied",
                ));
            }
        } else if paths.len() != 1 || paths[0].get::<_, String>("path") != write.subject {
            return Err(DurableError::Conflict(
                "source revision predecessor path index is absent or ambiguous",
            ));
        }
    }
    check_revision_semantic_indexes(tx, domain, writes, item_adoption_indexes)?;
    Ok(indexes)
}

fn indexes_bytes(rows: &IndexRows) -> DurableResult<Vec<u8>> {
    serde_json::to_vec(&rows.iter().collect::<Vec<_>>())
        .map_err(|_| DurableError::Invalid("source indexes encode"))
}
fn reads_bytes(reads: &SourceReads) -> DurableResult<Vec<u8>> {
    if reads.managed_original.is_some() && reads.revision.is_some() {
        return Err(DurableError::Invalid(
            "managed original and revision read profiles cannot be combined",
        ));
    }
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
    let value = match (&reads.managed_original, &reads.revision) {
        (None, None) => serde_json::Value::Array(values),
        (Some(original), None) => {
            serde_json::json!({"profile":"tos.managed-agent-original-reads-v1","reads":values,"original":original})
        }
        (None, Some(revision)) => {
            if revision.family_tag.is_empty() || revision.family_tag.len() > 512 {
                return Err(DurableError::Invalid("source revision family tag length"));
            }
            if let Some(owner_configuration_raw) = revision.owner_configuration_raw.as_deref() {
                if owner_configuration_raw.len() > 65_536 {
                    return Err(DurableError::Refused(
                        "PG Forms/Historical owner configuration exceeds retained bound",
                    ));
                }
                let request_digest =
                    revision
                        .request_canonical_digest
                        .ok_or(DurableError::Invalid(
                            "retained owner configuration lacks request digest",
                        ))?;
                let raw_digest =
                    revision
                        .owner_configuration_raw_digest
                        .ok_or(DurableError::Invalid(
                            "retained owner configuration raw digest absent",
                        ))?;
                let canonical_digest =
                    revision
                        .owner_configuration_canonical_digest
                        .ok_or(DurableError::Invalid(
                            "retained owner configuration canonical digest absent",
                        ))?;
                let owner_configuration_text = std::str::from_utf8(owner_configuration_raw)
                    .map_err(|_| DurableError::Corrupt("retained owner configuration UTF-8"))?;
                let config = cmd::parse(owner_configuration_raw).map_err(source_error)?;
                if Digest256::of_bytes(owner_configuration_raw) != raw_digest
                    || cmd::record_digest(&config).map_err(source_error)? != canonical_digest
                {
                    return Err(DurableError::Conflict(
                        "retained owner configuration digest differs",
                    ));
                }
                serde_json::json!({
                    "profile":"tos.pg-family-tagged-source-revision-reads-v3",
                    "family":revision.family_tag,
                    "source_head_sequence":revision.source_head_sequence,
                    "predecessor_source_revision":revision.predecessor_source_revision.0.to_hex(),
                    "request_canonical_digest":request_digest.to_hex(),
                    "owner_configuration_raw":owner_configuration_text,
                    "owner_configuration_raw_digest":raw_digest.to_hex(),
                    "owner_configuration_canonical_digest":canonical_digest.to_hex(),
                    "reads":values
                })
            } else if revision.owner_configuration_raw_digest.is_some()
                || revision.owner_configuration_canonical_digest.is_some()
            {
                return Err(DurableError::Invalid(
                    "source revision owner configuration fields are incomplete",
                ));
            } else if let Some(request_digest) = revision.request_canonical_digest {
                serde_json::json!({
                    "profile":"tos.pg-family-tagged-source-revision-reads-v2",
                    "family":revision.family_tag,
                    "source_head_sequence":revision.source_head_sequence,
                    "predecessor_source_revision":revision.predecessor_source_revision.0.to_hex(),
                    "request_canonical_digest":request_digest.to_hex(),
                    "reads":values
                })
            } else {
                serde_json::json!({
                    "profile":"tos.pg-family-tagged-source-revision-reads-v1",
                    "family":revision.family_tag,
                    "source_head_sequence":revision.source_head_sequence,
                    "predecessor_source_revision":revision.predecessor_source_revision.0.to_hex(),
                    "reads":values
                })
            }
        }
        (Some(_), Some(_)) => {
            return Err(DurableError::Invalid(
                "managed original and source revision read profiles cannot be combined",
            ));
        }
    };
    let encoded =
        serde_json::to_vec(&value).map_err(|_| DurableError::Invalid("source reads encode"))?;
    if (reads.managed_original.is_some() || reads.revision.is_some()) && encoded.len() > 1_048_576 {
        return Err(DurableError::Refused(
            "source read envelope cannot fit existing cold metadata row bound",
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
        original["inventory"] = if inventory.commitment_version == 1 {
            serde_json::json!([inventory.root.to_hex(), inventory.dependencies])
        } else {
            serde_json::json!([
                inventory.root.to_hex(),
                inventory.dependencies,
                inventory.commitment_version
            ])
        };
    }
    Some(original)
}
fn original_digest(value: &serde_json::Value) -> Digest256 {
    Digest256::of_bytes(&serde_json::to_vec(value).expect("private original primitives encode"))
}

// Finite Agent dependency snapshot. SQL is only an ordering/cursor carrier;
// original projection byte values are parsed/emitted by the same FND visitor.
// This performs O(N) compact passes outside the publication/commit fence.
const RETAINED_PROJECTIONS: &str = "WITH selected_history AS (SELECT DISTINCT ON (subject) domain,subject,revision,commit_seq,content_digest,inventory_projection FROM cmd2_history WHERE domain=$1 AND commit_seq<=$2 ORDER BY subject,revision DESC) SELECT h.subject,h.content_digest,CASE WHEN p.effective_seq IS NULL THEN CASE WHEN octet_length(h.inventory_projection)<=1048576 THEN h.inventory_projection ELSE NULL END WHEN p.source_revision=h.revision AND p.content_digest=h.content_digest AND p.definition_digest=d.source_definition_digest THEN CASE WHEN octet_length(p.inventory_projection)<=1048576 THEN p.inventory_projection ELSE NULL END ELSE NULL END AS inventory_projection FROM selected_history h JOIN cmd2_domain d ON d.domain=h.domain LEFT JOIN LATERAL (SELECT source_revision,content_digest,definition_digest,inventory_projection,effective_seq FROM cmd2_source_inventory_projection WHERE domain=h.domain AND subject=h.subject AND effective_seq>=h.commit_seq AND effective_seq<=$2 ORDER BY effective_seq DESC LIMIT 1) p ON TRUE";

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
    addressed_inventory::AddressedMixedInventoryV1::checked_projection(
        &row.get::<_, String>("subject"),
        &raw,
        &row.get::<_, String>("content_digest"),
    )
}
fn retained_projection_root(
    tx: &mut Transaction<'_>,
    domain: &str,
    generation: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<(Digest256, BTreeMap<String, Digest256>, u64)> {
    retained_projection_root_with_work(
        tx,
        domain,
        generation,
        deadline,
        cancelled,
        &mut ManagedSourceWorkV1::default(),
    )
}

fn retained_projection_root_with_work(
    tx: &mut Transaction<'_>,
    domain: &str,
    generation: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
    source_work: &mut ManagedSourceWorkV1,
) -> DurableResult<(Digest256, BTreeMap<String, Digest256>, u64)> {
    active(deadline, cancelled)?;
    let admission = format!(
        "WITH members AS ({RETAINED_PROJECTIONS}) SELECT count(*),coalesce(sum(octet_length(inventory_projection)),0),count(*) FILTER(WHERE inventory_projection IS NULL) FROM members"
    );
    let admitted = tx.query_one(&admission, &[&domain, &as_i64(generation)?])?;
    ManagedSourceWorkV1::charge(
        &mut source_work.projection_admission_rows_witnessed,
        as_u64(admitted.get(0))?,
    )?;
    ManagedSourceWorkV1::charge(
        &mut source_work.projection_admission_bytes_witnessed,
        as_u64(admitted.get(1))?,
    )?;
    if as_u64(admitted.get(0))? > MAX_CUT
        || as_u64(admitted.get(1))? > 64 * 1024 * 1024
        || admitted.get::<_, i64>(2) != 0
    {
        return Err(DurableError::Refused(
            "Agent projection exceeds existing cold metadata envelope or is incomplete; FullOnly required",
        ));
    }
    let mut root = Digest256Hasher::new();
    part(&mut root, b"tos-managed-mixed-source-inventory-coverage-v1");
    let mut profiles = BTreeMap::<String, Digest256>::new();
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
        ManagedSourceWorkV1::charge(&mut source_work.projection_rows_returned, 1)?;
        let path: String = row.get("subject");
        ManagedSourceWorkV1::charge(
            &mut source_work.projection_subject_bytes_returned,
            path.len() as u64,
        )?;
        let raw = row
            .get::<_, Option<Vec<u8>>>("inventory_projection")
            .ok_or(DurableError::Corrupt("Agent projection payload absent"))?;
        ManagedSourceWorkV1::charge(
            &mut source_work.projection_payload_bytes_returned,
            raw.len() as u64,
        )?;
        active(deadline, cancelled)?;
        let value = projection_value(&row)?;
        if path <= cursor || !path.starts_with("ToS/") {
            return Err(DurableError::Corrupt(
                "Agent projection path coverage/order differs",
            ));
        }

        part(&mut root, path.as_bytes());
        part(&mut root, Digest256::of_bytes(&raw).as_bytes());
        for (key, digest) in
            addressed_inventory::collect_agent_profile_contributions(&path, &value)?
        {
            if profiles.get(&key).is_some_and(|prior| prior != &digest) {
                return Err(DurableError::Corrupt(
                    "mixed Agent source profile contributions disagree",
                ));
            }
            profiles.insert(key, digest);
        }
        // Every member contributes to the mixed root. Agent schema inputs are
        // collected through the same owner filter used by the addressed tree.
        count = count
            .checked_add(1)
            .ok_or(DurableError::Corrupt("Agent projection coverage overflow"))?;
        cursor = path;
    }
    Ok((root.finalize(), profiles, count))
}

fn visit_projection_section_with_work(
    tx: &mut Transaction<'_>,
    domain: &str,
    generation: u64,
    section: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
    source_work: &mut ManagedSourceWorkV1,
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
        ManagedSourceWorkV1::charge(&mut source_work.catalogue_section_rows_returned, 1)?;
        let raw = row
            .get::<_, Option<&[u8]>>("inventory_projection")
            .ok_or(DurableError::Corrupt("Agent catalogue projection absent"))?;
        ManagedSourceWorkV1::charge(
            &mut source_work.catalogue_projection_bytes_returned,
            raw.len() as u64,
        )?;
        active(deadline, cancelled)?;
        let key: String = row.get("entry_key");
        if key <= cursor {
            return Err(DurableError::Corrupt(
                "Agent dependency identity ordering/uniqueness differs",
            ));
        }
        let value = projection_value(&row)?;
        let entry = match section {
            "records" => {
                // Metadata index rows span every catalogue family. The full
                // mixed projection was checked above; bind its indexed ID to
                // exactly one typed entry rather than interpreting all rows
                // as Agents. The successor uses these entries to prove that
                // retained records in every family remain unchanged.
                let records = cmd::field(&value, "records")
                    .map_err(source_error)?
                    .as_object()
                    .ok_or(DurableError::Corrupt("mixed catalogue records map"))?;
                let mut selected = None;
                for (kind, entries) in records {
                    let kind = kind
                        .as_str()
                        .ok_or(DurableError::Corrupt("mixed catalogue family"))?;
                    for entry in entries
                        .as_array()
                        .ok_or(DurableError::Corrupt("mixed catalogue family rows"))?
                    {
                        if cmd::text(entry, "record_type").map_err(source_error)? != kind {
                            return Err(DurableError::Corrupt(
                                "mixed catalogue entry family differs",
                            ));
                        }
                        if cmd::text(entry, "record_id").map_err(source_error)? == key {
                            if selected.replace(entry).is_some() {
                                return Err(DurableError::Corrupt(
                                    "mixed catalogue indexed identity is ambiguous",
                                ));
                            }
                        }
                    }
                }
                selected.ok_or(DurableError::Corrupt("mixed catalogue entry absent"))?
            }
            "forms" => cmd::field(
                cmd::field(&value, "form").map_err(source_error)?,
                "raw_sha256",
            )
            .map_err(source_error)?,
            _ => cmd::field(cmd::field(&value, section).map_err(source_error)?, &key)
                .map_err(source_error)?,
        };
        ManagedSourceWorkV1::charge(&mut source_work.catalogue_entries_delivered, 1)?;
        visit(&key, entry)?;
        cursor = key;
    }
    Ok(())
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
    let dependencies = addressed_inventory::mixed_agent_dependency_commitment_v1(
        ctx, &profiles, deadline, cancelled,
    )?;
    Ok((
        ManagedAgentInventory {
            commitment_version: 4,
            root,
            dependencies,
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
    if let Some(stored) = row.get::<_, Option<String>>("source_definition_digest") {
        require_current_source_definition(parse_hex(stored)?)?;
    }
    require_current_source_definition(cohort.definition)?;
    if row.get::<_, Option<String>>("source_revision") != Some(cohort.initial_revision.0.to_hex())
        || row.get::<_, Option<String>>("source_membership_digest")
            != Some(cohort.initial_membership.digest.to_hex())
        || row.get::<_, Option<i64>>("source_membership_count")
            != Some(as_i64(cohort.initial_membership.count)?)
        || row.get::<_, Option<i64>>("source_epoch") != Some(as_i64(cohort.epoch)?)
        || row.get::<_, Option<String>>("source_definition_digest")
            != Some(cohort.definition.to_hex())
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

// Opt-in only; default durable_schema.sql remains unchanged. Every invocation
// refuses unknown columns, checks, keys or triggers. The companion is not
// considered admitted merely because CREATE TABLE IF NOT EXISTS succeeded.
fn managed_model_query_budget(
    tx: &mut Transaction<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<()> {
    active(deadline, cancelled)?;
    let remaining = deadline
        .saturating_duration_since(Instant::now())
        .as_millis();
    if remaining == 0 {
        return Err(DurableError::Refused("managed model query deadline"));
    }
    let statement = format!("{}ms", remaining.min(15_000));
    let lock = format!("{}ms", remaining.min(5_000));
    tx.query_one(
        "SELECT set_config('statement_timeout',$1,true),set_config('lock_timeout',$2,true)",
        &[&statement, &lock],
    )?;
    Ok(())
}

pub(crate) fn require_managed_model_schema_v2(
    tx: &mut Transaction<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<()> {
    for (name, history) in [
        ("cmd2_model_selection_v2", false),
        ("cmd2_model_manifest_history_v2", true),
    ] {
        managed_model_query_budget(tx, deadline, cancelled)?;
        let relation = tx.query_opt("SELECT c.oid::bigint FROM pg_catalog.pg_class c JOIN pg_catalog.pg_class d ON d.relname='cmd2_domain' AND d.relnamespace=c.relnamespace WHERE c.relname=$1 AND c.relkind='r' AND c.relpersistence='p' AND c.relnamespace=current_schema()::regnamespace", &[&name])?
            .ok_or(DurableError::Refused("opt-in model schema absent or unknown"))?;
        let oid: i64 = relation.get(0);
        managed_model_query_budget(tx, deadline, cancelled)?;
        let columns = tx.query("SELECT a.attname,pg_catalog.format_type(a.atttypid,a.atttypmod),a.attnotnull,a.attidentity::text,a.attgenerated::text FROM pg_catalog.pg_attribute a WHERE a.attrelid=$1::bigint::oid AND a.attnum>0 AND NOT a.attisdropped ORDER BY a.attnum", &[&oid])?;
        let expected = [
            ("domain", "text"),
            ("selected_generation_digest", "character(64)"),
            ("selected_audit_generation", "bigint"),
            ("schema_version", "smallint"),
            ("model_version", "bigint"),
            ("manifest_digest", "character(64)"),
        ];
        if columns.len() != expected.len()
            || columns.iter().zip(expected).any(|(row, (name, ty))| {
                row.get::<_, String>(0) != name
                    || row.get::<_, String>(1) != ty
                    || !row.get::<_, bool>(2)
                    || !row.get::<_, String>(3).is_empty()
                    || !row.get::<_, String>(4).is_empty()
            })
        {
            return Err(DurableError::Refused(
                "unknown managed model schema columns",
            ));
        }
        managed_model_query_budget(tx, deadline, cancelled)?;
        let constraints = tx.query("SELECT c.contype::text,c.conkey,c.confrelid::bigint,c.confkey,c.convalidated,c.condeferrable,c.condeferred,pg_catalog.pg_get_expr(c.conbin,c.conrelid),c.confupdtype::text,c.confdeltype::text,c.confmatchtype::text FROM pg_catalog.pg_constraint c WHERE c.conrelid=$1::bigint::oid", &[&oid])?;
        if constraints.len() != 7 {
            return Err(DurableError::Refused(
                "unknown managed model schema constraints",
            ));
        }
        managed_model_query_budget(tx, deadline, cancelled)?;
        let domain_oid: i64 = tx
            .query_one("SELECT 'cmd2_domain'::regclass::oid::bigint", &[])?
            .get(0);
        managed_model_query_budget(tx, deadline, cancelled)?;
        let domain_attnum: i16 = tx.query_one("SELECT attnum FROM pg_catalog.pg_attribute WHERE attrelid='cmd2_domain'::regclass AND attname='domain' AND NOT attisdropped", &[])?.get(0);
        let mut seen = BTreeSet::new();
        for constraint in constraints {
            if !constraint.get::<_, bool>(4)
                || constraint.get::<_, bool>(5)
                || constraint.get::<_, bool>(6)
            {
                return Err(DurableError::Refused(
                    "unknown managed model deferred constraint",
                ));
            }
            let kind: String = constraint.get(0);
            let keys: Vec<i16> = constraint.get(1);
            let valid = match kind.as_str() {
                "p" => {
                    keys == if history {
                        vec![1, 2, 3, 5]
                    } else {
                        vec![1, 2, 3]
                    }
                }
                "f" => {
                    keys == [1]
                        && constraint.get::<_, i64>(2) == domain_oid
                        && constraint.get::<_, Vec<i16>>(3) == [domain_attnum]
                        && constraint.get::<_, String>(8) == "a"
                        && constraint.get::<_, String>(9) == "a"
                        && constraint.get::<_, String>(10) == "s"
                }
                "c" => {
                    let expression: Option<String> = constraint.get(7);
                    match (keys.as_slice(), expression.as_deref()) {
                        (
                            [2],
                            Some("((selected_generation_digest)::text ~ '^[0-9a-f]{64}$'::text)"),
                        )
                        | ([2], Some("(selected_generation_digest ~ '^[0-9a-f]{64}$'::text)"))
                        | ([3], Some("(selected_audit_generation >= 0)"))
                        | ([4], Some("(schema_version = 2)"))
                        | ([5], Some("(model_version > 0)"))
                        | ([6], Some("((manifest_digest)::text ~ '^[0-9a-f]{64}$'::text)"))
                        | ([6], Some("(manifest_digest ~ '^[0-9a-f]{64}$'::text)")) => true,
                        _ => false,
                    }
                }
                _ => false,
            };
            if !valid || !seen.insert((kind, keys)) {
                return Err(DurableError::Refused(
                    "unknown managed model constraint expression/key",
                ));
            }
        }
        managed_model_query_budget(tx, deadline, cancelled)?;
        let extras: i64 = tx.query_one("SELECT (SELECT count(*) FROM pg_catalog.pg_trigger WHERE tgrelid=$1::bigint::oid AND NOT tgisinternal)+(SELECT count(*) FROM pg_catalog.pg_attrdef WHERE adrelid=$1::bigint::oid)+(SELECT count(*) FROM pg_catalog.pg_rewrite WHERE ev_class=$1::bigint::oid)", &[&oid])?.get(0);
        if extras != 0 {
            return Err(DurableError::Refused(
                "unknown managed model trigger/default/rule",
            ));
        }
    }
    Ok(())
}

fn read_managed_model_pointer_v2(
    tx: &mut Transaction<'_>,
    source: &tos_compiler::ManagedOverlaySourceBindingV2,
    for_update: bool,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<Option<ManagedManifestSelectionV2>> {
    let domain = source.domain.as_str();
    let generation = source.selected_generation_digest.as_str();
    let audit = as_i64(source.selected_audit_generation)?;
    let sql = if for_update {
        "SELECT model_version,manifest_digest FROM cmd2_model_selection_v2 WHERE domain=$1 AND selected_generation_digest=$2 AND selected_audit_generation=$3 AND schema_version=2 FOR UPDATE"
    } else {
        "SELECT model_version,manifest_digest FROM cmd2_model_selection_v2 WHERE domain=$1 AND selected_generation_digest=$2 AND selected_audit_generation=$3 AND schema_version=2 FOR SHARE"
    };
    managed_model_query_budget(tx, deadline, cancelled)?;
    let Some(row) = tx.query_opt(sql, &[&domain, &generation, &audit])? else {
        return Ok(None);
    };
    let version = as_u64(row.get(0))?;
    let digest: String = row.get(1);
    let manifest_digest = Digest256::from_hex(&digest)
        .map_err(|_| DurableError::Corrupt("managed selected manifest digest"))?;
    managed_model_query_budget(tx, deadline, cancelled)?;
    let history = tx.query_opt("SELECT manifest_digest FROM cmd2_model_manifest_history_v2 WHERE domain=$1 AND selected_generation_digest=$2 AND selected_audit_generation=$3 AND schema_version=2 AND model_version=$4 FOR SHARE", &[&domain, &generation, &audit, &as_i64(version)?])?;
    if version == 0 || history.map(|row| row.get::<_, String>(0)) != Some(digest) {
        return Err(DurableError::Corrupt(
            "managed selected pointer/history mismatch",
        ));
    }
    Ok(Some(ManagedManifestSelectionV2 {
        model_version: version,
        manifest_digest,
    }))
}

fn held_generation_metadata(
    tx: &mut Transaction<'_>,
    store: &SegmentStore,
    generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
) -> DurableResult<()> {
    let selection = generation.selected().facts();
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
    let profile: String = row.get("profile_id");
    // All maintained source-file writers share selected placement and metadata
    // law. The unselected generic shadow profile is not a source-file writer.
    if !known_profile(profile.as_bytes()) || profile.as_bytes() == PROFILE_ID {
        return Err(DurableError::Corrupt(
            "selected metadata source profile unsupported",
        ));
    }
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
    {
        return Err(DurableError::Corrupt("selected metadata placement differs"));
    }
    expose_metadata(row, path).map(|(metadata, _)| metadata)
}

fn held_retained_source_metadata(
    tx: &mut Transaction<'_>,
    cohort: &ManagedSourceCohort,
) -> DurableResult<()> {
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
    Ok(())
}

fn charge_member_batch_metadata(work: &mut u64, bytes: i32) -> DurableResult<()> {
    let bytes =
        u64::try_from(bytes).map_err(|_| DurableError::Corrupt("batch metadata byte count"))?;
    if bytes > 1_048_576 {
        return Err(DurableError::Refused("batch metadata row bound"));
    }
    *work = work
        .checked_add(bytes)
        .filter(|n| *n <= cmd::SELECTED_SOURCE_MAX_BYTES as u64)
        .ok_or(DurableError::Refused(
            "batch cumulative metadata work bound",
        ))?;
    Ok(())
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
        parent
            .selected()
            .facts()
            .audited_root
            .require_store(store)?;
        current
            .selected()
            .facts()
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

    pub(crate) fn managed_model_addressed_projection_with_work(
        &mut self,
        store: &SegmentStore,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        path: &RelativePath,
        deadline: Instant,
        cancelled: &AtomicBool,
        source_work: &mut ManagedSourceWorkV1,
    ) -> DurableResult<Option<tos_foundation::JsonValue>> {
        active(deadline, cancelled)?;
        let selected = match generation.selected() {
            SelectedSourceGeneration::Addressed(selected) => selected,
            _ => {
                return Err(DurableError::Refused(
                    "addressed model requires V2 source selection",
                ));
            }
        };
        selected.cut.require_store(store)?;
        let domain = generation.cohort().domain();
        let digest = selected.cut.inventory.projection_digest_with_work(
            store,
            path,
            selected.limits,
            deadline,
            cancelled,
            &mut source_work.tree,
        )?;
        let current = selected.lookup_current_with_work(
            domain,
            path,
            deadline,
            cancelled,
            &mut source_work.tree,
        )?;
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")?;
        let remaining_ms = deadline
            .saturating_duration_since(Instant::now())
            .as_millis();
        if remaining_ms == 0 {
            return Err(DurableError::Refused("selected model projection deadline"));
        }
        let statement_ms = remaining_ms.min(15_000).to_string();
        let lock_ms = remaining_ms.min(5_000).to_string();
        tx.query_one(
            "SELECT set_config('statement_timeout',$1,true),set_config('lock_timeout',$2,true)",
            &[&statement_ms, &lock_ms],
        )?;
        held_generation_metadata(&mut tx, store, generation)?;
        let size = tx.query_opt(
            "SELECT octet_length(inventory_projection) FROM cmd2_current WHERE domain=$1 AND subject=$2",
            &[&domain, &path.as_str()],
        )?;
        let result = match (digest, current, size) {
            (None, None, None) => None,
            (Some(digest), Some(current), Some(size)) => {
                if size
                    .get::<_, Option<i32>>(0)
                    .is_none_or(|n| n <= 0 || n > 1_048_576)
                {
                    return Err(DurableError::Refused(
                        "selected model projection exceeds row bound",
                    ));
                }
                let row = tx.query_one(
                    "SELECT * FROM cmd2_current WHERE domain=$1 AND subject=$2",
                    &[&domain, &path.as_str()],
                )?;
                ManagedSourceWorkV1::charge(&mut source_work.point_projection_rows_returned, 1)?;
                let metadata = selected_source_metadata(&row, &current, domain, path)?;
                let raw = row
                    .get::<_, Option<Vec<u8>>>("inventory_projection")
                    .ok_or(DurableError::Corrupt("selected model projection absent"))?;
                ManagedSourceWorkV1::charge(
                    &mut source_work.point_projection_bytes_returned,
                    raw.len() as u64,
                )?;
                if Digest256::of_bytes(&raw) != digest {
                    return Err(DurableError::Corrupt(
                        "selected model projection tree differs",
                    ));
                }
                Some(
                    addressed_inventory::AddressedMixedInventoryV1::checked_projection(
                        path.as_str(),
                        &raw,
                        &metadata.sha256.to_hex(),
                    )?,
                )
            }
            _ => {
                return Err(DurableError::Conflict(
                    "selected model projection presence differs",
                ));
            }
        };
        active(deadline, cancelled)?;
        tx.commit()?;
        Ok(result)
    }

    pub(crate) fn managed_model_generation_description(
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        projection: Digest256,
    ) -> DurableResult<tos_compiler::managed_source::ManagedSourceGenerationV1> {
        let installation = generation.selected().facts();
        let cut = installation.descriptor_cut;
        if cut.commitment_version != 1 {
            return Err(DurableError::Refused(
                "addressed source selection requires versioned managed model consumer",
            ));
        }
        let cohort = generation.cohort();
        Ok(tos_compiler::managed_source::ManagedSourceGenerationV1 {
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
        })
    }

    pub(crate) fn managed_model_generation_description_v2(
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        projection: Digest256,
    ) -> DurableResult<tos_compiler::managed_source::ManagedSourceGenerationV2> {
        let selected = match generation.selected() {
            SelectedSourceGeneration::Addressed(selected) => selected,
            _ => {
                return Err(DurableError::Refused(
                    "V2 model requires addressed source selection",
                ));
            }
        };
        selected.cut.require_store(&selected.store)?;
        let cut = &selected.cut;
        let cohort = generation.cohort();
        Ok(tos_compiler::managed_source::ManagedSourceGenerationV2 {
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
            addressed_metadata_tree_sha256: cut.metadata.commitment.to_hex(),
            addressed_metadata_members: cut.metadata.entries,
            state_profile_sha256: cut.state_profile_digest.to_hex(),
            addressed_inventory_root_sha256: cut.inventory.root().to_hex(),
            log_sha256: cut.log_digest.to_hex(),
            addressed_current_tree_sha256: cut.current.commitment.to_hex(),
            current_members: cut.current.entries,
            addressed_history_tree_sha256: cut.history.commitment.to_hex(),
            history_members: cut.history.entries,
            inventory_projection_sha256: projection.to_hex(),
        })
    }

    /// Addressed selection identity only: no global inventory transcript is
    /// manufactured and this descriptive binding alone grants no source read.
    pub(crate) fn managed_overlay_binding_v2(
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
    ) -> DurableResult<tos_compiler::ManagedOverlaySourceBindingV2> {
        let selected = match generation.selected() {
            SelectedSourceGeneration::Addressed(selected) => selected,
            _ => {
                return Err(DurableError::Refused(
                    "overlay requires addressed source selection",
                ));
            }
        };
        selected.cut.require_store(&selected.store)?;
        let cut = &selected.cut;
        let cohort = generation.cohort();
        Ok(tos_compiler::ManagedOverlaySourceBindingV2 {
            schema: "tos_managed_model_source_binding_v2".into(),
            domain: cohort.domain().to_owned(),
            store_id: cut.store_id,
            domain_sha256: cut.domain_digest.to_hex(),
            selected_generation_digest: generation.digest().to_hex(),
            database_oid: cut.database_oid,
            state_profile_sha256: cut.state_profile_digest.to_hex(),
            log_sha256: cut.log_digest.to_hex(),
            bootstrap_source_revision: cohort.initial_revision().0.to_hex(),
            bootstrap_membership_sha256: cohort.initial_membership().digest.to_hex(),
            bootstrap_members: cohort.initial_membership().count,
            selected_audit_generation: generation.audit_generation(),
            through_commit_seq: generation.commit_seq(),
            epoch: cohort.epoch(),
            definition_sha256: cohort.definition_digest().to_hex(),
            schema_profile_sha256: cut.schema_profile_digest.to_hex(),
            addressed_current_tree_sha256: cut.current.commitment.to_hex(),
            addressed_history_tree_sha256: cut.history.commitment.to_hex(),
            addressed_metadata_tree_sha256: cut.metadata.commitment.to_hex(),
            addressed_inventory_root_sha256: cut.inventory.root().to_hex(),
            current_source_members: cut.current.entries,
            historical_source_members: cut.history.entries,
            metadata_members: cut.metadata.entries,
        })
    }

    /// Durable STO candidate first, short same-owner PG CAS second. Inserting
    /// history and changing the pointer are one transaction. A source advance
    /// naturally makes the row unavailable through its exact generation/audit
    /// key, without mutating source state or disabling source-audit triggers.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn publish_managed_manifest_v2(
        &mut self,
        store: &SegmentStore,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        filesystem: &CreationFilesystem,
        package: CreationPackage<'_>,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        expected: Option<ManagedManifestSelectionV2>,
        candidate: Digest256,
        restore: Option<&crate::backup_recovery::VerifiedManagedModelRestoreV2>,
        limits: tos_compiler::ManagedManifestLimitsV2,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<(
        ManagedManifestSelectionV2,
        tos_segment_store::AuthenticatedTreeWorkV1,
    )> {
        self.publish_managed_manifest_v2_with_work(
            store,
            generation,
            filesystem,
            package,
            contract,
            rule_version,
            rights_version,
            job_id,
            job_fence,
            expected,
            candidate,
            restore,
            limits,
            deadline,
            cancelled,
            &mut audit_delta::AuditDeltaWork::default(),
        )
    }

    pub(crate) fn publish_managed_manifest_v2_with_work(
        &mut self,
        store: &SegmentStore,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        filesystem: &CreationFilesystem,
        package: CreationPackage<'_>,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        expected: Option<ManagedManifestSelectionV2>,
        candidate: Digest256,
        restore: Option<&crate::backup_recovery::VerifiedManagedModelRestoreV2>,
        limits: tos_compiler::ManagedManifestLimitsV2,
        deadline: Instant,
        cancelled: &AtomicBool,
        audit_work: &mut audit_delta::AuditDeltaWork,
    ) -> DurableResult<(
        ManagedManifestSelectionV2,
        tos_segment_store::AuthenticatedTreeWorkV1,
    )> {
        active(deadline, cancelled)?;
        let source = Self::managed_overlay_binding_v2(generation)?;
        let installation = generation.selected().facts();
        let (manifest, mut work) = tos_compiler::ManagedManifestV2::read_selected(
            store,
            installation.audited_root,
            candidate,
            limits,
            deadline,
            cancelled,
        )
        .map_err(|_| DurableError::Refused("managed immutable manifest validation failed"))?;
        if manifest.source_binding() != &source {
            return Err(DurableError::Conflict(
                "prepared manifest selected source differs",
            ));
        }
        let projection = Digest256::from_hex(&source.addressed_inventory_root_sha256)
            .map_err(|_| DurableError::Invalid("overlay inventory digest"))?;
        self.with_managed_generation_fences(
            store, generation, projection, filesystem, Some(package), contract,
            rule_version, rights_version, job_id, job_fence, deadline, cancelled,
            |_owner, tx| {
                require_managed_model_schema_v2(tx, deadline, cancelled)?;
                if manifest.recovery_rebind().is_some() != restore.is_some() {
                    return Err(DurableError::Refused("managed recovery requires actual verified restore witness"));
                }
                if let Some(parent_digest) = manifest.parent_manifest_digest() {
                    let parent_digest = Digest256::from_hex(parent_digest)
                        .map_err(|_| DurableError::Invalid("overlay parent manifest digest"))?;
                    let (parent, parent_work) = tos_compiler::ManagedManifestV2::read_selected(
                        store, installation.audited_root, parent_digest, limits, deadline, cancelled,
                    ).map_err(|_| DurableError::Refused("overlay retained parent manifest unavailable"))?;
                    manifest.require_successor_of(&parent, parent_digest)
                        .map_err(|_| DurableError::Conflict("overlay parent/interval binding differs"))?;
                    work.read_nodes = work.read_nodes.checked_add(parent_work.read_nodes)
                        .ok_or(DurableError::Refused("model read work counter overflow"))?;
                    work.read_bytes = work.read_bytes.checked_add(parent_work.read_bytes)
                        .ok_or(DurableError::Refused("model read byte counter overflow"))?;
                    let previous = read_managed_model_pointer_v2(
                        tx, parent.source_binding(), true, deadline, cancelled,
                    )?.ok_or(DurableError::Conflict("overlay parent selection unavailable"))?;
                    if previous.manifest_digest != parent_digest {
                        return Err(DurableError::Conflict("overlay previous selected model changed"));
                    }
                    if let Some(delta) = manifest.delta() {
                        if parent.source_binding().through_commit_seq != delta.parent_through_commit_seq {
                            return Err(DurableError::Conflict("overlay committed parent sequence changed"));
                        }
                    }
                    if let Some(recovery) = manifest.recovery_rebind() {
                        let witness = restore.ok_or(DurableError::Refused("verified restore absent"))?;
                        if witness.domain != source.domain
                            || witness.old_database_oid != parent.source_binding().database_oid
                            || witness.new_database_oid != source.database_oid
                            || !witness.manifests.contains(&parent_digest.to_hex())
                            || recovery.backup_receipt_sha256 != witness.backup_receipt_sha256
                            || recovery.restored_metadata_sha256 != witness.metadata_sha256
                            || recovery.store_inventory_sha256 != witness.store_inventory_sha256
                            || recovery.restored_cut_digest != witness.cut_digest
                            || source.log_sha256 != witness.cut_digest {
                            return Err(DurableError::Conflict("recovery witness/current source/retained model differs"));
                        }
                        // Cold selection legitimately advances the source audit
                        // fence through selector/bookkeeping updates. Admit only
                        // its complete no-op semantic interval; no gaps, key moves,
                        // deletion or changed committed value may reuse old roots.
                        let delta_limits = super::audit_delta::AuditDeltaLimits {
                            max_rows: 4096, max_bytes: 4 * 1024 * 1024,
                            max_key_bytes: MAX_MEMBERSHIP_KEY_BYTES,
                        };
                        let interval = super::audit_delta::load_interval_controlled_with_work(
                            tx, &source.domain, recovery.old_audit_generation, delta_limits,
                            Some(super::audit_delta::AuditDeltaControl { deadline, cancelled }),
                            audit_work,
                        ).map_err(managed_model_recovery_audit_error)?;
                        if interval.through_generation() != recovery.new_audit_generation
                            || interval.deltas().iter().any(|delta|
                                delta.old_key.is_none() || delta.old_key != delta.new_key
                                    || delta.old_commitment.is_none()
                                    || delta.old_commitment != delta.new_commitment) {
                            return Err(DurableError::Conflict("recovery cold admission changed semantic source rows"));
                        }
                        interval.verify_final_rows_controlled_with_work(
                            tx, &source.domain, delta_limits,
                            Some(super::audit_delta::AuditDeltaControl { deadline, cancelled }),
                            audit_work,
                        ).map_err(|_| DurableError::Conflict("recovery audited final rows differ"))?;
                    }
                }
                let current = read_managed_model_pointer_v2(tx, &source, true, deadline, cancelled)?;
                if current != expected {
                    return Err(DurableError::Conflict("managed manifest CAS prior differs"));
                }
                let version = match current {
                    None => 1,
                    Some(old) => old.model_version.checked_add(1)
                        .ok_or(DurableError::Invalid("managed model version overflow"))?,
                };
                let domain = source.domain.as_str();
                let selected_digest = source.selected_generation_digest.as_str();
                let audit = as_i64(source.selected_audit_generation)?;
                let version_i64 = as_i64(version)?;
                let digest = candidate.to_hex();
                // No ON CONFLICT UPDATE: a historical version is immutable.
                managed_model_query_budget(tx, deadline, cancelled)?;
                tx.execute("INSERT INTO cmd2_model_manifest_history_v2(domain,selected_generation_digest,selected_audit_generation,schema_version,model_version,manifest_digest) VALUES($1,$2,$3,2,$4,$5)",
                    &[&domain, &selected_digest, &audit, &version_i64, &digest])?;
                managed_model_query_budget(tx, deadline, cancelled)?;
                let affected = if let Some(old) = current {
                    tx.execute("UPDATE cmd2_model_selection_v2 SET model_version=$4,manifest_digest=$5 WHERE domain=$1 AND selected_generation_digest=$2 AND selected_audit_generation=$3 AND schema_version=2 AND model_version=$6 AND manifest_digest=$7",
                        &[&domain, &selected_digest, &audit, &version_i64, &digest, &as_i64(old.model_version)?, &old.manifest_digest.to_hex()])?
                } else {
                    tx.execute("INSERT INTO cmd2_model_selection_v2(domain,selected_generation_digest,selected_audit_generation,schema_version,model_version,manifest_digest) VALUES($1,$2,$3,2,$4,$5) ON CONFLICT DO NOTHING",
                        &[&domain, &selected_digest, &audit, &version_i64, &digest])?
                };
                if affected != 1 {
                    return Err(DurableError::Conflict("managed manifest concurrent CAS"));
                }
                let selected = ManagedManifestSelectionV2 { model_version: version, manifest_digest: candidate };
                if read_managed_model_pointer_v2(tx, &source, false, deadline, cancelled)? != Some(selected) {
                    return Err(DurableError::Corrupt("managed manifest pointer/history differs"));
                }
                active(deadline, cancelled)?;
                Ok((selected, work))
            },
        )
    }

    /// Current reader keeps source/owner/policy fences across the actual
    /// manifest and addressed lookup. A retained historical row alone cannot
    /// become current; the opaque manifest is issued only inside this scope.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn with_current_managed_manifest_v2<T>(
        &mut self,
        store: &SegmentStore,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        filesystem: &CreationFilesystem,
        package: CreationPackage<'_>,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        limits: tos_compiler::ManagedManifestLimitsV2,
        expected: ManagedManifestSelectionV2,
        deadline: Instant,
        cancelled: &AtomicBool,
        seek: impl FnOnce(&tos_compiler::ManagedManifestV2) -> DurableResult<T>,
    ) -> DurableResult<(T, tos_segment_store::AuthenticatedTreeWorkV1)> {
        let source = Self::managed_overlay_binding_v2(generation)?;
        let projection = Digest256::from_hex(&source.addressed_inventory_root_sha256)
            .map_err(|_| DurableError::Invalid("overlay inventory digest"))?;
        self.with_managed_generation_fences(
            store,
            generation,
            projection,
            filesystem,
            Some(package),
            contract,
            rule_version,
            rights_version,
            job_id,
            job_fence,
            deadline,
            cancelled,
            |_owner, tx| {
                require_managed_model_schema_v2(tx, deadline, cancelled)?;
                let selected = read_managed_model_pointer_v2(
                    tx, &source, false, deadline, cancelled,
                )?
                .ok_or(DurableError::Refused(
                    "current managed manifest unavailable; cold rebuild required",
                ))?;
                if selected != expected {
                    return Err(DurableError::Conflict(
                        "current managed manifest/version changed",
                    ));
                }
                let (manifest, work) = tos_compiler::ManagedManifestV2::read_selected(
                    store,
                    generation.selected().facts().audited_root,
                    selected.manifest_digest,
                    limits,
                    deadline,
                    cancelled,
                )
                .map_err(|_| {
                    DurableError::Refused("managed immutable manifest validation failed")
                })?;
                if manifest.source_binding() != &source {
                    return Err(DurableError::Conflict(
                        "selected manifest source binding changed",
                    ));
                }
                let result = seek(&manifest)?;
                Ok((result, work))
            },
        )
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
        if matches!(
            generation.selected(),
            SelectedSourceGeneration::Addressed(_)
        ) {
            return self.visit_managed_model_catalogue(generation, deadline, cancelled, |_| Ok(()));
        }
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
        visit: impl FnMut(&tos_foundation::JsonValue) -> DurableResult<()>,
    ) -> DurableResult<Digest256> {
        self.visit_managed_model_catalogue_with_work(
            generation,
            deadline,
            cancelled,
            &mut ManagedSourceWorkV1::default(),
            visit,
        )
    }
    pub(crate) fn visit_managed_model_catalogue_with_work(
        &mut self,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        deadline: Instant,
        cancelled: &AtomicBool,
        source_work: &mut ManagedSourceWorkV1,
        mut visit: impl FnMut(&tos_foundation::JsonValue) -> DurableResult<()>,
    ) -> DurableResult<Digest256> {
        active(deadline, cancelled)?;
        let domain = generation.cohort().domain();
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY; SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let expected_projection = managed_model_projection_identity(&mut tx, generation)?;
        let (root, _, count) = retained_projection_root_with_work(
            &mut tx,
            domain,
            generation.commit_seq(),
            deadline,
            cancelled,
            source_work,
        )?;
        let projection_binding_matches = match generation.selected() {
            SelectedSourceGeneration::Addressed(selected) => {
                expected_projection == selected.cut.inventory.root()
            }
            _ => root == expected_projection,
        };
        if count != generation.member_count() || !projection_binding_matches {
            return Err(DurableError::Refused(
                "managed model catalogue coverage unproved; FullOnly required",
            ));
        }
        visit_projection_section_with_work(
            &mut tx,
            domain,
            generation.commit_seq(),
            "records",
            deadline,
            cancelled,
            source_work,
            |_, entry| visit(entry),
        )?;
        tx.commit()?;
        active(deadline, cancelled)?;
        Ok(root)
    }

    /// Hold the exact source-navigation owner boundary while distinguishing an
    /// immutable V2 bootstrap anchor from the live PG source generation. This
    /// reuses the same owner mutex, audit/domain/job lock order, schema profile,
    /// cohort check and RR lifetime as generic reads, but deliberately does
    /// not compare current PG head/audit/projection digests to the old anchor.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn with_current_source_navigation_lease<T>(
        &mut self,
        store: &SegmentStore,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        proof: &tos_compiler::managed_source::ManagedSourceProofV2,
        filesystem: &CreationFilesystem,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
        operate: impl for<'lease, 'tx> FnOnce(
            &crate::source_managed_selection::SourceNavigationCurrentLeaseV1<'lease>,
            &mut Transaction<'tx>,
        ) -> DurableResult<T>,
    ) -> DurableResult<T> {
        active(deadline, cancelled)?;
        proof
            .validate()
            .map_err(|_| DurableError::Refused("source navigation anchor proof invalid"))?;
        let anchor_projection = Digest256::from_hex(proof.inventory_projection())
            .map_err(|_| DurableError::Invalid("source navigation anchor projection"))?;
        if !crate::source_managed_selection::ManagedSelectedProof::matches_selected(
            proof,
            generation,
            anchor_projection,
        )? {
            return Err(DurableError::Conflict(
                "source navigation immutable anchor differs",
            ));
        }
        let installation = generation.selected().facts();
        installation.audited_root.require_store(store)?;
        if store.store_id() != installation.descriptor_cut.store_id
            || store.domain_digest() != installation.descriptor_cut.domain_digest
            || installation.domain != generation.cohort().domain()
            || installation.descriptor_cut.database_oid != proof.generation.database_oid
        {
            return Err(DurableError::Conflict(
                "source navigation anchor store/database differs",
            ));
        }
        let owner = filesystem
            .hold_source_navigation_owner(deadline, cancelled)
            .map_err(source_error)?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let domain = generation.cohort().domain();
        let audit = tx.query_one(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        let audit_generation = as_u64(audit.get(0))?;
        if audit.get::<_, String>(1) != "normal" {
            return Err(DurableError::Refused("physical maintenance is active"));
        }
        let row = tx.query_one(
            "SELECT * FROM cmd2_domain WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        // This checks the immutable bootstrap revision/membership, epoch,
        // definition, and maintained source completeness against today's row.
        cohort_matches(&row, generation.cohort(), true)?;
        let head_sequence = as_u64(row.get("head_seq"))?;
        let source_generation = row
            .get::<_, Option<i64>>("source_generation")
            .map(as_u64)
            .transpose()?
            .ok_or(DurableError::Conflict(
                "source navigation live generation unavailable",
            ))?;
        let schema = row
            .get::<_, Option<String>>("schema_profile_digest")
            .ok_or(DurableError::Conflict(
                "source navigation live schema profile unavailable",
            ))?;
        let current_database_oid = database_oid(&mut tx)?;
        if head_sequence != source_generation
            || head_sequence < generation.commit_seq()
            || source_generation < generation.commit_seq()
            || audit_generation < generation.audit_generation()
            || Digest256::from_hex(&schema)
                .map_err(|_| DurableError::Corrupt("source navigation schema profile invalid"))?
                != schema_profile_digest()
            || current_database_oid != proof.generation.database_oid
            || as_u64(row.get("rule_version"))? != rule_version
            || row.get::<_, String>("contract_digest") != contract.to_hex()
        {
            return Err(DurableError::Conflict(
                "source navigation live source/contract changed",
            ));
        }
        let rights_allowed = row.get::<_, bool>("rights_allowed");
        let current_rights_version = as_u64(row.get("rights_version"))?;
        if !rights_allowed || current_rights_version != rights_version {
            return Err(DurableError::Refused(
                "source navigation current rights changed",
            ));
        }
        let job = tx.query_opt(
            "SELECT fence_epoch FROM cmd2_job WHERE domain=$1 AND job_id=$2 FOR SHARE",
            &[&domain, &job_id],
        )?;
        if job.map(|row| row.get::<_, i64>(0)) != Some(as_i64(job_fence)?) {
            return Err(DurableError::Refused(
                "source navigation current job changed",
            ));
        }
        let source_membership_root = parse_hex(
            row.get::<_, Option<String>>("source_membership_digest")
                .ok_or(DurableError::Corrupt(
                    "source navigation bootstrap membership root absent",
                ))?,
        )?;
        let source_membership_count =
            as_u64(row.get::<_, Option<i64>>("source_membership_count").ok_or(
                DurableError::Corrupt("source navigation bootstrap membership count absent"),
            )?)?;
        let parse_optional_digest = |column: &str| -> DurableResult<Option<Digest256>> {
            row.get::<_, Option<String>>(column)
                .map(parse_hex)
                .transpose()
        };
        let selected_generation_digest = parse_optional_digest("selected_generation_digest")?;
        let source_projection_digest = parse_optional_digest("source_projection_digest")?;
        owner
            .verify_current(deadline, cancelled)
            .map_err(source_error)?;
        active(deadline, cancelled)?;
        let lease =
            crate::source_managed_selection::SourceNavigationCurrentLeaseV1::under_held_fences(
                proof,
                generation,
                &owner,
                &installation.audited_root,
                store,
                deadline,
                cancelled,
                head_sequence,
                source_generation,
                audit_generation,
                current_database_oid,
                rights_allowed,
                current_rights_version,
                as_u64(row.get("rule_version"))?,
                parse_hex(row.get::<_, String>("contract_digest"))?,
                parse_hex(schema)?,
                source_membership_root,
                source_membership_count,
                selected_generation_digest,
                source_projection_digest,
            );
        let result = operate(&lease, &mut tx)?;
        active(deadline, cancelled)?;
        lease.check_current().map_err(source_error)?;
        installation.audited_root.require_store(store)?;
        tx.commit()?;
        Ok(result)
    }

    pub(crate) fn with_current_managed_model_and_tx<
        T,
        P: crate::source_managed_selection::ManagedSelectedProof,
    >(
        &mut self,
        store: &SegmentStore,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        proof: &P,
        filesystem: &CreationFilesystem,
        package: CreationPackage<'_>,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
        seek: impl for<'lease, 'tx> FnOnce(
            &crate::source_managed_selection::ManagedCurrentModelLease<'lease, P>,
            &mut Transaction<'tx>,
        ) -> T,
    ) -> DurableResult<T> {
        self.with_current_managed_model_and_tx_owner(
            store,
            generation,
            proof,
            filesystem,
            Some(package),
            contract,
            rule_version,
            rights_version,
            job_id,
            job_fence,
            deadline,
            cancelled,
            seek,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn with_current_managed_model_and_tx_owner<
        T,
        P: crate::source_managed_selection::ManagedSelectedProof,
    >(
        &mut self,
        store: &SegmentStore,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        proof: &P,
        filesystem: &CreationFilesystem,
        package: Option<CreationPackage<'_>>,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
        seek: impl for<'lease, 'tx> FnOnce(
            &crate::source_managed_selection::ManagedCurrentModelLease<'lease, P>,
            &mut Transaction<'tx>,
        ) -> T,
    ) -> DurableResult<T> {
        active(deadline, cancelled)?;
        let projection = Digest256::from_hex(proof.inventory_projection())
            .map_err(|_| DurableError::Invalid("managed model projection digest"))?;
        if !proof.matches_selected(generation, projection)? {
            return Err(DurableError::Conflict(
                "managed model retained generation differs",
            ));
        }
        let publication_projection = match generation.selected() {
            SelectedSourceGeneration::Addressed(selected) => selected.cut.inventory.root(),
            _ => projection,
        };
        self.with_managed_generation_fences(
            store,
            generation,
            publication_projection,
            filesystem,
            package,
            contract,
            rule_version,
            rights_version,
            job_id,
            job_fence,
            deadline,
            cancelled,
            |owner, tx| {
                let installation = generation.selected().facts();
                let lease =
                    crate::source_managed_selection::ManagedCurrentModelLease::under_held_fences(
                        proof,
                        owner,
                        installation.audited_root,
                        store,
                        deadline,
                        cancelled,
                    );
                lease.require_managed_basis(proof).map_err(source_error)?;
                let result = seek(&lease, tx);
                lease.require_managed_basis(proof).map_err(source_error)?;
                Ok(result)
            },
        )
    }

    /// One owner/source/rights/job lock boundary for both retained V1 model
    /// reads and opt-in model-pointer CAS. Model bookkeeping never changes
    /// cmd2_domain or its source-audit trigger.
    fn with_managed_generation_fences<T>(
        &mut self,
        store: &SegmentStore,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        publication_projection: Digest256,
        filesystem: &CreationFilesystem,
        package: Option<CreationPackage<'_>>,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
        operate: impl FnOnce(&CreationOwnerFence, &mut Transaction<'_>) -> DurableResult<T>,
    ) -> DurableResult<T> {
        active(deadline, cancelled)?;
        let installation = generation.selected().facts();
        installation.audited_root.require_store(store)?;
        if store.store_id() != installation.descriptor_cut.store_id
            || store.domain_digest() != installation.descriptor_cut.domain_digest
            || installation.domain != generation.cohort().domain()
        {
            return Err(DurableError::Conflict("managed model source store differs"));
        }
        let owner = match package {
            Some(package) => filesystem.hold_creation_owner(package, deadline, cancelled),
            None => filesystem.hold_source_navigation_owner(deadline, cancelled),
        }
        .map_err(source_error)?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()?;
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
            || row.get::<_, Option<String>>("source_projection_digest")
                != Some(publication_projection.to_hex())
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
        let result = operate(&owner, &mut tx)?;
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
        let (inventory, count) = match generation.selected() {
            SelectedSourceGeneration::Addressed(selected) => (
                selected.cut.inventory.select_agent(
                    &selected.store,
                    ctx,
                    selected.limits,
                    deadline,
                    cancelled,
                )?,
                selected.cut.current.entries,
            ),
            _ => retained_agent_inventory(
                &mut tx,
                domain,
                generation.commit_seq(),
                ctx,
                deadline,
                cancelled,
            )?,
        };
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
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let result = read_generation_source_metadata_in_tx(
            &mut tx, store, generation, path, deadline, cancelled,
        )?;
        tx.commit()?;
        Ok(result)
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

    /// Addressed batch over one held current generation. Only one attempt's
    /// sealed receipts is resident; no receipt cache survives this operation.
    pub(crate) fn read_generation_source_members(
        &mut self,
        store: &SegmentStore,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        paths: &[RelativePath],
        max_member_bytes: u64,
        max_total_bytes: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Vec<ManagedCurrentMember>> {
        active(deadline, cancelled)?;
        if paths.is_empty()
            || paths.len() > MAX_MEMBERS
            || max_member_bytes == 0
            || max_member_bytes > 8_388_608
            || max_total_bytes == 0
            || max_total_bytes > cmd::SELECTED_SOURCE_MAX_BYTES as u64
            || paths
                .iter()
                .map(RelativePath::as_str)
                .collect::<BTreeSet<_>>()
                .len()
                != paths.len()
        {
            return Err(DurableError::Refused(
                "current member batch exceeds operational bounds",
            ));
        }
        if store.store_id() != generation.cohort().store_id
            || store.custody_domain() != generation.cohort().domain().as_bytes()
        {
            return Err(DurableError::Conflict(
                "current batch custody domain differs",
            ));
        }
        let _custody = store.hold_audit_root()?;
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        held_generation_metadata(&mut tx, store, generation)?;
        let domain = generation.cohort().domain();
        let mut work = 0u64;
        let mut output_bytes = 0u64;
        let mut groups: BTreeMap<(Vec<u8>, u64), Vec<(usize, u64, postgres::Row)>> =
            BTreeMap::new();
        // Bound each SQL carrier and cumulative metadata before fetching it.
        for (index, path) in paths.iter().enumerate() {
            active(deadline, cancelled)?;
            let size: i32 = tx.query_opt("SELECT octet_length(row_to_json(c)::text) FROM cmd2_current c WHERE domain=$1 AND subject=$2", &[&domain, &path.as_str()])?
                .ok_or(DurableError::Conflict("current batch member absent"))?.get(0);
            charge_member_batch_metadata(&mut work, size)?;
            let current = tx.query_one(
                "SELECT * FROM cmd2_current WHERE domain=$1 AND subject=$2",
                &[&domain, &path.as_str()],
            )?;
            let revision = as_u64(current.get("revision"))?;
            let size: i32 = tx.query_opt("SELECT octet_length(row_to_json(h)::text) FROM cmd2_history h WHERE domain=$1 AND subject=$2 AND revision=$3", &[&domain, &path.as_str(), &as_i64(revision)?])?
                .ok_or(DurableError::Corrupt("batch historical locator absent"))?.get(0);
            charge_member_batch_metadata(&mut work, size)?;
            let history = tx.query_one(
                "SELECT * FROM cmd2_history WHERE domain=$1 AND subject=$2 AND revision=$3",
                &[&domain, &path.as_str(), &as_i64(revision)?],
            )?;
            let prepare: Vec<u8> = history.get("prepare_id");
            let attempt = tx.query_opt("SELECT state,attempt_fence,commit_seq FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2 FOR SHARE", &[&domain, &prepare])?
                .ok_or(DurableError::Corrupt("batch historical attempt absent"))?;
            if attempt.get::<_, String>(0) != "committed"
                || attempt.get::<_, Option<i64>>(2) != Some(history.get("commit_seq"))
            {
                return Err(DurableError::Corrupt(
                    "batch historical attempt not committed",
                ));
            }
            let fence = as_u64(attempt.get(1))?;
            groups
                .entry((prepare, fence))
                .or_default()
                .push((index, revision, history));
        }
        let mut output: Vec<Option<ManagedCurrentMember>> =
            (0..paths.len()).map(|_| None).collect();
        for ((prepare, fence), addresses) in groups {
            active(deadline, cancelled)?;
            let receipts = match store.recover_attempt_fenced(&prepare, fence, 0)? {
                Some(AttemptRecovery::Sealed { receipts }) => receipts,
                _ => {
                    return Err(DurableError::Corrupt(
                        "batch historical fenced intent not sealed",
                    ));
                }
            };
            // Recheck the current generation after actual cold receipt recovery.
            // Domain/policy/fence locks and global STO custody remain held.
            held_generation_metadata(&mut tx, store, generation)?;
            for (index, revision, history) in addresses {
                active(deadline, cancelled)?;
                let path = &paths[index];
                let pin: Vec<u8> = history.get("pin_id");
                if pin.len() != 16
                    || receipts
                        .iter()
                        .any(|r| r.pin_id().as_slice() != pin.as_slice())
                {
                    return Err(DurableError::Corrupt("batch historical fenced pin differs"));
                }
                let expected: String = history.get("sto_receipt_id");
                let receipt = receipts
                    .iter()
                    .find(|r| r.receipt_id().to_hex() == expected)
                    .ok_or(DurableError::Corrupt(
                        "batch historical frame receipt absent",
                    ))?;
                check_history_locator(&history, receipt, domain, path.as_str(), revision)?;
                let size: i32 = tx.query_opt("SELECT octet_length(row_to_json(c)::text) FROM cmd2_current c WHERE domain=$1 AND subject=$2", &[&domain, &path.as_str()])?
                    .ok_or(DurableError::Conflict("current batch member disappeared"))?.get(0);
                charge_member_batch_metadata(&mut work, size)?;
                let current = tx.query_one(
                    "SELECT * FROM cmd2_current WHERE domain=$1 AND subject=$2",
                    &[&domain, &path.as_str()],
                )?;
                check_history_locator(&current, receipt, domain, path.as_str(), revision)?;
                let selected = generation
                    .selected()
                    .lookup_current(domain, path, deadline, cancelled)?
                    .ok_or(DurableError::Conflict(
                        "selected batch current frame absent",
                    ))?;
                let metadata = selected_source_metadata(&current, &selected, domain, path)?;
                if selected.placement != receipt.placement() {
                    return Err(DurableError::Conflict(
                        "selected recovered batch frame differs",
                    ));
                }
                if metadata.size_bytes > max_member_bytes {
                    return Err(DurableError::Refused("current batch member byte bound"));
                }
                output_bytes = output_bytes
                    .checked_add(metadata.size_bytes)
                    .filter(|n| *n <= max_total_bytes)
                    .ok_or(DurableError::Refused("current batch aggregate byte bound"))?;
                let (_, dependency_claims) = expose_metadata(&current, path)?;
                let mut raw = Vec::new();
                store.read_selected(receipt, max_member_bytes, &mut raw)?;
                active(deadline, cancelled)?;
                output[index] = Some(ManagedCurrentMember {
                    path: path.clone(),
                    custody_revision: revision,
                    current_generation: generation.commit_seq(),
                    commit_seq: as_u64(current.get("commit_seq"))?,
                    raw,
                    metadata,
                    dependency_claims,
                    placement: receipt.placement(),
                });
            }
            // receipts are dropped before the next exact attempt is recovered.
        }
        active(deadline, cancelled)?;
        held_generation_metadata(&mut tx, store, generation)?;
        tx.commit()?;
        output
            .into_iter()
            .map(|member| member.ok_or(DurableError::Corrupt("batch result incomplete")))
            .collect()
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
        self.execute_managed_agent_creation_from_captures_with_work(
            store,
            generation,
            schema_cut,
            prepare_id,
            filesystem,
            context,
            software,
            components,
            worker,
            contract,
            rule_version,
            rights_version,
            job_id,
            job_fence,
            deadline,
            cancelled,
            &mut ManagedSourceWorkV1::default(),
        )
    }

    /// Same real operation with caller-retained audit/tree logical accounting.
    pub fn execute_managed_agent_creation_from_captures_with_work(
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
        source_work: &mut ManagedSourceWorkV1,
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
            source_work,
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
        source_work: &mut ManagedSourceWorkV1,
    ) -> DurableResult<crate::source_current_cut::ManagedCurrentSourceGeneration> {
        active(deadline, cancelled)?;
        if attempt.continuation.as_ref().is_some_and(|continuation| {
            matches!(continuation.parent, SelectedSourceGeneration::Addressed(_))
        }) {
            return self.continue_addressed_creation(
                store,
                cohort,
                attempt,
                package,
                filesystem,
                deadline,
                cancelled,
                source_work,
            );
        }
        ManagedSourceWorkV1::charge(&mut source_work.legacy_continuations_attempted, 1)?;
        let continuation = attempt.continuation.as_ref().ok_or(DurableError::Refused(
            "warm source chain absent; explicit cold reopen required",
        ))?;
        let committed = continuation.committed.borrow();
        let committed = committed.as_ref().ok_or(DurableError::Refused(
            "warm source commit not observed; explicit cold reopen required",
        ))?;
        let parent = continuation
            .parent
            .legacy_view()
            .ok_or(DurableError::Refused(
                "addressed parent requires V2 continuation",
            ))?;
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
        // The opaque parent retains the hasher only after exhaustive cold
        // verification or our immediately preceding verified transition. The
        // audit/head checks above reject every unrelated metadata mutation;
        // only this addressed new row extends the unchanged V1 transcript.
        let mut log_hash = continuation
            .parent
            .log_frontier()
            .ok_or(DurableError::Refused(
                "warm successor requires a live verified log frontier",
            ))?
            .clone();
        if log_hash.clone().finalize() != parent.descriptor_cut.log_digest {
            return Err(DurableError::Corrupt("warm verified log frontier differs"));
        }
        let log = tx.query_one(
            "SELECT commit_seq,event_kind,command_id,delta_digest,members_root FROM cmd2_log WHERE domain=$1 AND commit_seq=$2",
            &[&domain, &as_i64(head)?],
        )?;
        let kind: String = log.get(1);
        let command: String = log.get(2);
        let delta: String = log.get(3);
        let root: String = log.get(4);
        if as_u64(log.get(0))? != head
            || kind != "command"
            || command != receipt.command_id
            || delta != receipt.delta_digest.to_hex()
            || root != receipt.member_root.to_hex()
        {
            return Err(DurableError::Corrupt("warm final log differs"));
        }
        for value in [
            &as_i64(head)?.to_be_bytes()[..],
            kind.as_bytes(),
            command.as_bytes(),
            delta.as_bytes(),
            root.as_bytes(),
        ] {
            part(&mut log_hash, value);
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
                log_digest: log_hash.clone().finalize(),
                historical_members: history_rows.len() as u64,
                current_members: current_rows.len() as u64,
                history_membership_root: logical_membership_root(HISTORY_KEY_TAG, &history_rows),
                current_membership_root: logical_membership_root(CURRENT_KEY_TAG, &current_rows),
            },
            log_frontier: VerifiedLogFrontier(log_hash),
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
        job_id: &str,
        job_fence: u64,
        job_mode: BootstrapJobMode,
        owner_check: &dyn Fn() -> DurableResult<()>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<ReopenedSourceCohort> {
        active(deadline, cancelled)?;
        owner_check()?;
        if job_id.is_empty()
            || job_id.len() > 256
            || job_id.contains('\0')
            || job_fence == 0
            || job_fence > i64::MAX as u64
            || job_mode == BootstrapJobMode::Initialize && job_fence != 1
        {
            return Err(DurableError::Refused(
                "source bootstrap initial job selection invalid",
            ));
        }
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
        if job_mode == BootstrapJobMode::Initialize {
            owner_check()?;
            self.create_domain(domain, contract)?;
            owner_check()?;
        }
        owner_check()?;
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'")?;
        // Keep the shared fence lock so all audited writers serialize with
        // bootstrap. Its numeric generation is not an emptiness signal: an
        // exact job lease may have been provisioned before this operation.
        // Source emptiness and domain compatibility are checked directly below.
        lock_audit_fence(&mut tx, domain)?;
        let row = tx
            .query_opt(
                "SELECT * FROM cmd2_domain WHERE domain=$1 FOR UPDATE",
                &[&domain],
            )?
            .ok_or(DurableError::Refused(
                "source bootstrap selected domain is absent",
            ))?;
        let has_source_rows: bool = tx
            .query_one(
                "SELECT EXISTS(
                 SELECT 1 FROM cmd2_attempt WHERE domain=$1
                 UNION ALL SELECT 1 FROM cmd2_member WHERE domain=$1
                 UNION ALL SELECT 1 FROM cmd2_current WHERE domain=$1
                 UNION ALL SELECT 1 FROM cmd2_history WHERE domain=$1
                 UNION ALL SELECT 1 FROM cmd2_receipt WHERE domain=$1
                 UNION ALL SELECT 1 FROM cmd2_log WHERE domain=$1
                 UNION ALL SELECT 1 FROM cmd2_source_index WHERE domain=$1
                 UNION ALL SELECT 1 FROM cmd2_predicate WHERE domain=$1
                 UNION ALL SELECT 1 FROM cmd2_source_inventory_projection WHERE domain=$1
                 UNION ALL SELECT 1 FROM cmd2_source_text_identity WHERE domain=$1
                 UNION ALL SELECT 1 FROM cmd2_source_claim_dependency WHERE domain=$1
                 UNION ALL SELECT 1 FROM cmd2_source_claim_navigation_reference WHERE domain=$1
                 UNION ALL SELECT 1 FROM cmd2_source_item_file_assignment WHERE domain=$1
                 UNION ALL SELECT 1 FROM cmd2_outbox WHERE domain=$1
             )",
                &[&domain],
            )?
            .get(0);
        if row.get::<_, i64>("head_seq") != 0
            || row.get::<_, Option<String>>("source_revision").is_some()
            || row
                .get::<_, Option<String>>("source_membership_digest")
                .is_some()
            || row
                .get::<_, Option<i64>>("source_membership_count")
                .is_some()
            || row.get::<_, Option<i64>>("source_epoch").is_some()
            || row.get::<_, Option<i64>>("source_generation").is_some()
            || row.get::<_, bool>("source_complete")
            || row
                .get::<_, Option<String>>("source_definition_digest")
                .is_some()
            || row.get::<_, i64>("published_seq") != 0
            || row
                .get::<_, Option<String>>("complete_cut_digest")
                .is_some()
            || row
                .get::<_, Option<i64>>("complete_cut_generation")
                .is_some()
            || row
                .get::<_, Option<String>>("selected_generation_digest")
                .is_some()
            || row
                .get::<_, Option<String>>("source_projection_digest")
                .is_some()
            || row
                .get::<_, Option<i64>>("source_claim_navigation_reference_baseline_seq")
                .is_some()
            || row.get::<_, i64>("rights_version") != 0
            || !row.get::<_, bool>("rights_allowed")
            || row.get::<_, i64>("rule_version") != 0
            || row.get::<_, String>("contract_digest") != contract.to_hex()
            || row.get::<_, Option<String>>("schema_profile_digest")
                != Some(schema_profile_digest().to_hex())
            || has_source_rows
        {
            return Err(DurableError::Refused(
                "bootstrap requires an unused matching durable domain",
            ));
        }
        let jobs = tx.query(
            "SELECT job_id,fence_epoch FROM cmd2_job WHERE domain=$1 ORDER BY job_id FOR UPDATE",
            &[&domain],
        )?;
        let selected_job = jobs.iter().find(|row| row.get::<_, String>(0) == job_id);
        match job_mode {
            BootstrapJobMode::Initialize => {
                if jobs.len() > 1
                    || jobs.iter().any(|row| {
                        row.get::<_, String>(0) != job_id
                            || row.get::<_, i64>(1) != job_fence as i64
                    })
                {
                    return Err(DurableError::Refused(
                        "source bootstrap initial job row differs",
                    ));
                }
                if selected_job.is_none() {
                    tx.execute(
                        "INSERT INTO cmd2_job(domain,job_id,fence_epoch) VALUES($1,$2,$3)",
                        &[&domain, &job_id, &(job_fence as i64)],
                    )?;
                }
            }
            BootstrapJobMode::RequireExisting => {
                if selected_job.map(|row| row.get::<_, i64>(1)) != Some(job_fence as i64) {
                    return Err(DurableError::Refused(
                        "source bootstrap selected job lease differs",
                    ));
                }
            }
        }
        tx.execute("UPDATE cmd2_domain SET source_revision=$2,source_membership_digest=$3,source_membership_count=$4,source_epoch=1,source_generation=0,source_complete=false,source_definition_digest=$5 WHERE domain=$1",
            &[&domain,&selected_revision.0.to_hex(),&selected_membership.digest.to_hex(),&as_i64(selected_membership.count)?,&def.to_hex()])?;
        owner_check()?;
        tx.commit()?;
        let source_metadata = original_metadata(cut);
        let originals = ctx
            .files
            .iter()
            .filter(|f| f.path.as_str().starts_with("ToS/"))
            .collect::<Vec<_>>();
        for (batch, files) in originals.chunks(MAX_MEMBERS).enumerate() {
            active(deadline, cancelled)?;
            owner_check()?;
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
            owner_check()?;
            let members = seal_members(store, &prepare, fence, ORIGINAL, &identities)?;
            owner_check()?;
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
            owner_check()?;
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
                    job_id,
                    job_fence,
                    full_base_seq: head,
                },
                CommitMode::Bootstrap { owner_check },
            )?;
        }
        owner_check()?;
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout = '5s'; SET LOCAL statement_timeout = '15s'")?;
        lock_audit_fence(&mut tx, domain)?;
        for (kind, token, path) in &indexes {
            tx.execute("INSERT INTO cmd2_source_index(domain,kind,token,path,definition_digest) VALUES($1,$2,$3,$4,$5)", &[&domain,kind,token,path,&def.to_hex()])?;
        }
        let text_identity_members = ctx
            .files
            .iter()
            .filter(|file| {
                file.path.as_str().starts_with("ToS/")
                    && crate::source_text_identity::is_candidate(
                        file.path
                            .as_str()
                            .rsplit('/')
                            .next()
                            .unwrap_or(file.path.as_str()),
                    )
            })
            .map(|file| (file.path.as_str().to_owned(), file.raw.clone()))
            .collect::<BTreeMap<_, _>>();
        crate::source_text_identity_index::rebuild_current(
            &mut tx,
            domain,
            &text_identity_members,
            def,
        )?;
        let annotation_identity_members = ctx
            .files
            .iter()
            .filter(|file| crate::source_annotation_identity_index::is_packet(file.path.as_str()))
            .map(|file| (file.path.as_str().to_owned(), file.raw.clone()))
            .collect::<BTreeMap<_, _>>();
        crate::source_annotation_identity_index::rebuild_current(
            &mut tx,
            domain,
            &annotation_identity_members,
            def,
        )?;
        let shared_schema_count = ctx
            .files
            .iter()
            .filter(|file| {
                crate::source_claim_dependency_index::is_shared_schema_source_path(
                    file.path.as_str(),
                )
            })
            .take(crate::source_claim_dependency_index::MAX_SHARED_SCHEMA_PATHS + 1)
            .count();
        if ctx.files.iter().any(|file| {
            crate::source_claim_dependency_index::is_shared_schema_source_path(file.path.as_str())
                && file.path.as_str().len() > crate::source_claim_dependency_index::MAX_PATH_BYTES
        }) {
            return Err(DurableError::Refused(
                "Claim navigation shared schema path byte bound exceeded",
            ));
        }
        if shared_schema_count > crate::source_claim_dependency_index::MAX_SHARED_SCHEMA_PATHS {
            return Err(DurableError::Refused(
                "Claim navigation shared schema path bound exceeded",
            ));
        }
        // Claim navigation consumes shared schemas by exact path only. Keep an
        // empty sentinel in this subset so it does not retain duplicate schema
        // bodies or imply that their contents were indexed.
        let claim_dependency_members = ctx
            .files
            .iter()
            .filter(|file| {
                crate::source_claim_dependency_index::is_claim_navigation_source_member(
                    file.path.as_str(),
                )
            })
            .map(|file| {
                let bytes = if crate::source_claim_dependency_index::is_shared_schema_source_path(
                    file.path.as_str(),
                ) {
                    Vec::new()
                } else {
                    file.raw.clone()
                };
                (file.path.as_str().to_owned(), bytes)
            })
            .collect::<BTreeMap<_, _>>();
        let navigation_baseline_seq = as_u64(
            tx.query_one(
                "SELECT head_seq FROM cmd2_domain WHERE domain=$1 FOR UPDATE",
                &[&domain],
            )?
            .get(0),
        )?;
        crate::source_claim_dependency_index::rebuild_current(
            &mut tx,
            domain,
            &claim_dependency_members,
            def,
            navigation_baseline_seq,
        )?;
        let item_file_members = ctx
            .files
            .iter()
            .filter(|file| {
                crate::source_item_file_index::is_candidate_manifest_path(file.path.as_str())
                    || crate::source_item_file_index::is_candidate_item_record_path(
                        file.path.as_str(),
                    )
                    || crate::source_item_file_index::is_candidate_item_rights_path(
                        file.path.as_str(),
                    )
            })
            .map(|file| (file.path.as_str().to_owned(), file.raw.clone()))
            .collect::<BTreeMap<_, _>>();
        crate::source_item_file_index::rebuild_current(&mut tx, domain, &item_file_members, def)?;
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
        owner_check()?;
        tx.commit()?;
        self.cold_reopen_source_cohort_with_owner_check(
            store,
            domain,
            cut,
            selected_revision,
            selected_membership,
            context,
            software,
            components,
            worker,
            owner_check,
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

    /// Register one semantically prepared maintained record revision against
    /// the exact PG read epoch. This route carries only the addressed readset
    /// and proposed STO members; it never reopens the complete source cohort.
    pub(crate) fn register_source_revision(
        &mut self,
        store: &SegmentStore,
        prepared: &impl PreparedPgSourceRevision,
        readset: &revision::SourceRevisionReadSet,
        prepare_id: &[u8],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<SourceRevisionAttempt> {
        active(deadline, cancelled)?;
        if prepare_id.is_empty() || prepare_id.len() > u16::MAX as usize {
            return Err(DurableError::Invalid("source revision prepare ID length"));
        }
        let selection = &readset.selection;
        if store.custody_domain() != selection.cohort.domain.as_bytes()
            || store.store_id() != selection.cohort.store_id
        {
            return Err(DurableError::Conflict(
                "source revision carrier domain differs",
            ));
        }
        let context = prepared.context();
        let proposal = prepared.proposal();
        prepared.validate_readset(readset)?;
        let (publication_package, authored_changes) =
            prepared.publication_package_and_source_changes()?;
        let mut authored_proposal = proposal.clone();
        authored_proposal.changes = authored_changes;
        let (mut reads, writes, metadata, _) =
            revision_package(&authored_proposal, readset, prepared.family_tag())?;
        if let Some(package) = publication_package.as_ref() {
            let observed = reads.reads.iter().find_map(|read| match read {
                PredicateRead::Generation {
                    predicate,
                    observed_generation,
                } if predicate.owner == OWNER
                    && predicate.kind.as_str() == "unique"
                    && predicate.scope == "source-publication"
                    && predicate.token == "current" =>
                {
                    Some(*observed_generation)
                }
                _ => None,
            });
            if observed != Some(package.base_publication_generation) {
                return Err(DurableError::Conflict(
                    "source publication package lacks its exact base generation read",
                ));
            }
        }
        let projections = revision_owner_projections(
            prepared.family_tag(),
            &writes,
            prepared.inventory_owner_contributions(&writes)?,
        )?;
        let request = cmd::parse(&context.request_raw).map_err(source_error)?;
        let request_canonical_digest = cmd::record_digest(&request).map_err(source_error)?;
        if request_canonical_digest != proposal.request_canonical_sha256 {
            return Err(DurableError::Conflict(
                "source revision canonical request digest differs from owner proposal",
            ));
        }
        if !prepared.legacy_family_encoding() {
            let retain_owner_configuration =
                pg_forms_historical_family(prepared.family_tag()).is_some();
            if retain_owner_configuration && context.configuration_raw.len() > 65_536 {
                return Err(DurableError::Refused(
                    "PG Forms/Historical owner configuration exceeds retained bound",
                ));
            }
            reads.revision = Some(SourceRevisionReadBinding {
                family_tag: prepared.family_tag().to_owned(),
                source_head_sequence: selection.head_seq,
                predecessor_source_revision: selection.cohort.initial_revision,
                request_canonical_digest: Some(request_canonical_digest),
                owner_configuration_raw: retain_owner_configuration
                    .then(|| context.configuration_raw.clone()),
                owner_configuration_raw_digest: retain_owner_configuration
                    .then_some(proposal.configuration_raw_sha256),
                owner_configuration_canonical_digest: retain_owner_configuration
                    .then_some(proposal.configuration_canonical_sha256),
            });
        }
        let work_revision = prepared.work_revision_evidence(&writes)?;
        let command_id = cmd::text(&request, "command_id").map_err(source_error)?;
        let raw_request_digest = Digest256::of_bytes(&context.request_raw);
        let configuration_raw_digest = proposal.configuration_raw_sha256;
        let configuration_canonical_digest = proposal.configuration_canonical_sha256;
        let domain = selection.cohort.domain.as_str();
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let _initial_audit_generation = lock_audit_fence(&mut tx, domain)?;
        let row = tx.query_one("SELECT * FROM cmd2_domain WHERE domain=$1", &[&domain])?;
        cohort_matches(&row, &selection.cohort, true)?;
        if as_u64(row.get("rights_version"))? != selection.rights_version
            || as_u64(row.get("rule_version"))? != selection.rule_version
            || row.get::<_, String>("contract_digest") != selection.contract.to_hex()
        {
            return Err(DurableError::Conflict(
                "source revision selection changed before registration",
            ));
        }
        let job = tx.query_one(
            "SELECT fence_epoch FROM cmd2_job WHERE domain=$1 AND job_id=$2",
            &[&domain, &selection.job_id],
        )?;
        if as_u64(job.get(0))? != selection.job_fence {
            return Err(DurableError::Conflict(
                "source revision job lease changed before registration",
            ));
        }
        for write in &writes {
            if tx
                .query_opt(
                    "SELECT 1 FROM cmd2_current WHERE domain=$1 AND subject=$2",
                    &[&domain, &write.subject],
                )?
                .is_none()
            {
                if write.expected_predecessor.is_some() {
                    return Err(DurableError::Conflict(
                        "source revision predecessor disappeared",
                    ));
                }
                if !reads.reads.iter().any(|read| {
                    matches!(read, PredicateRead::Absent { namespace, key } if namespace == "source" && key == &write.subject)
                }) {
                    reads.reads.push(PredicateRead::Absent {
                        namespace: "source".to_owned(),
                        key: write.subject.clone(),
                    });
                }
            } else if write.expected_predecessor.is_none() {
                return Err(DurableError::Conflict(
                    "source revision insertion target is occupied",
                ));
            }
        }
        let new_writes = writes
            .iter()
            .filter(|write| write.expected_predecessor.is_none())
            .cloned()
            .collect::<Vec<_>>();
        creation::add_derived_path_predicates(
            &mut tx,
            domain,
            &new_writes,
            selection.cohort.definition,
        )?;
        let indexes = prepared.check_indexes(&mut tx, domain, &writes)?;
        validate_revision_indexes(prepared.family_tag(), &writes, &indexes)?;
        if pg_forms_historical_family(prepared.family_tag()).is_some()
            && indexes
                != crate::source_native_cli::pg_forms_historical::pg_rederive_revision_indexes(
                    context, &writes,
                )?
        {
            return Err(DurableError::Conflict(
                "PG Forms/Historical owner index derivation differs from exact package",
            ));
        }
        let predicate_keys = reads
            .reads
            .iter()
            .filter_map(|read| match read {
                PredicateRead::Generation { predicate, .. }
                    if predicate.owner == OWNER
                        && !(predicate.scope == "source-cohort"
                            && predicate.token == "generation") =>
                {
                    Some((
                        predicate.kind.as_str().to_owned(),
                        predicate.scope.clone(),
                        predicate.token.clone(),
                    ))
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        // The readset is extended with absent archive/member targets while the
        // same registration transaction owns the audit fence.
        let delta = revision_delta(
            &authored_proposal,
            selection,
            &reads,
            &writes,
            &metadata,
            &projections,
            publication_package.as_ref(),
            work_revision.as_ref(),
        )?;
        let reads_encoded = reads_bytes(&reads)?;
        let indexes_encoded = indexes_bytes(&indexes)?;
        let projections_encoded = projections_bytes(&projections)?;
        let existing = tx.query_opt(
            "SELECT * FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2 FOR UPDATE",
            &[&domain, &prepare_id],
        )?;
        if let Some(existing) = existing {
            if existing.get::<_, String>("command_id") != command_id
                || existing.get::<_, String>("raw_request_digest") != raw_request_digest.to_hex()
                || existing.get::<_, String>("delta_digest") != delta.to_hex()
                || existing.get::<_, Option<String>>("source_configuration_raw_digest")
                    != Some(configuration_raw_digest.to_hex())
                || existing.get::<_, Option<String>>("source_configuration_canonical_digest")
                    != Some(configuration_canonical_digest.to_hex())
                || existing.get::<_, Option<Vec<u8>>>("source_reads") != Some(reads_encoded.clone())
                || existing.get::<_, Option<Vec<u8>>>("source_indexes")
                    != Some(indexes_encoded.clone())
                || existing.get::<_, Option<Vec<u8>>>("source_projections")
                    != Some(projections_encoded.clone())
                || existing.get::<_, Option<Vec<u8>>>("source_publication_package")
                    != publication_package
                        .as_ref()
                        .map(|package| package.canonical_bytes.clone())
                || existing.get::<_, String>("state") == "aborted"
            {
                return Err(DurableError::Conflict(
                    "source revision registration identity differs",
                ));
            }
            let fence = as_u64(existing.get("attempt_fence"))?;
            let audit_generation = lock_audit_fence(&mut tx, domain)?;
            tx.commit()?;
            return Ok(SourceRevisionAttempt {
                domain: domain.to_owned(),
                store_id: selection.cohort.store_id,
                prepare_id: prepare_id.to_vec(),
                fence,
                command_id: command_id.to_owned(),
                raw_request_digest,
                configuration_raw_digest,
                configuration_canonical_digest,
                epoch: selection.epoch,
                source_generation: selection.source_generation,
                audit_generation,
                rights_version: selection.rights_version,
                rule_version: selection.rule_version,
                job_id: selection.job_id.clone(),
                job_fence: selection.job_fence,
                contract: selection.contract,
                definition: selection.cohort.definition,
                delta,
                family_tag: prepared.family_tag().to_owned(),
                source_head_sequence: Some(selection.head_seq),
                predecessor_source_revision: selection.cohort.initial_revision,
                reads,
                metadata,
                indexes,
                predicate_keys,
                writes,
                projections,
                publication_package,
                work_revision,
                recovered_work_recorded_at: None,
            });
        }
        for (kind, scope, token) in &predicate_keys {
            tx.execute(
                "INSERT INTO cmd2_predicate(domain,kind,owner,scope,token,definition_version,generation,complete)
                 VALUES($1,$2,$3,$4,$5,$6,0,true) ON CONFLICT DO NOTHING",
                &[
                    &domain,
                    kind,
                    &OWNER,
                    scope,
                    token,
                    &selection.cohort.definition.to_hex(),
                ],
            )?;
            let predicate = tx.query_one(
                "SELECT generation,complete,definition_version FROM cmd2_predicate
                 WHERE domain=$1 AND kind=$2 AND owner=$3 AND scope=$4 AND token=$5",
                &[&domain, kind, &OWNER, scope, token],
            )?;
            if !predicate.get::<_, bool>(1)
                || predicate.get::<_, String>(2) != selection.cohort.definition.to_hex()
            {
                return Err(DurableError::Refused(
                    "source revision predicate lacks complete definition",
                ));
            }
            let expected = reads.reads.iter().find_map(|read| match read {
                PredicateRead::Generation {
                    predicate: observed,
                    observed_generation,
                } if observed.kind.as_str() == kind
                    && observed.scope == *scope
                    && observed.token == *token =>
                {
                    Some(*observed_generation)
                }
                _ => None,
            });
            if expected != Some(as_u64(predicate.get(0))?) {
                return Err(DurableError::Conflict(
                    "source revision predicate generation changed",
                ));
            }
        }
        tx.execute(
            "INSERT INTO cmd2_attempt(domain,prepare_id,command_id,raw_request_digest,delta_digest,
             state,attempt_fence,source_reads,source_indexes,source_epoch,source_projections,source_publication_package,
             source_configuration_raw_digest,source_configuration_canonical_digest)
             VALUES($1,$2,$3,$4,$5,'registered',1,$6,$7,$8,$9,$10,$11,$12)",
            &[
                &domain,
                &prepare_id,
                &command_id,
                &raw_request_digest.to_hex(),
                &delta.to_hex(),
                &reads_encoded,
                &indexes_encoded,
                &as_i64(selection.epoch)?,
                &projections_encoded,
                &publication_package.as_ref().map(|package| package.canonical_bytes.clone()),
                &configuration_raw_digest.to_hex(),
                &configuration_canonical_digest.to_hex(),
            ],
        )?;
        // The audited predicate/attempt inserts above advance the fence.  The
        // attach phase must bind to this post-registration generation, not to
        // the selection generation captured before those writes.
        let audit_generation = lock_audit_fence(&mut tx, domain)?;
        tx.commit()?;
        Ok(SourceRevisionAttempt {
            domain: domain.to_owned(),
            store_id: selection.cohort.store_id,
            prepare_id: prepare_id.to_vec(),
            fence: 1,
            command_id: command_id.to_owned(),
            raw_request_digest,
            configuration_raw_digest,
            configuration_canonical_digest,
            epoch: selection.epoch,
            source_generation: selection.source_generation,
            audit_generation,
            rights_version: selection.rights_version,
            rule_version: selection.rule_version,
            job_id: selection.job_id.clone(),
            job_fence: selection.job_fence,
            contract: selection.contract,
            definition: selection.cohort.definition,
            delta,
            family_tag: prepared.family_tag().to_owned(),
            source_head_sequence: Some(selection.head_seq),
            predecessor_source_revision: selection.cohort.initial_revision,
            reads,
            metadata,
            indexes,
            predicate_keys,
            writes,
            projections,
            publication_package,
            work_revision,
            recovered_work_recorded_at: None,
        })
    }

    /// Reconstruct a ready/committed revision attempt from its exact PG/STO
    /// carriers after process loss. A merely registered attempt is refused
    /// because its proposed bytes were never durably sealed.
    pub(crate) fn recover_source_revision_attempt(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        prepare_id: &[u8],
        expected_command_id: &str,
        expected_raw_request_digest: Digest256,
        expected_configuration_raw_digest: Digest256,
        expected_configuration_canonical_digest: Digest256,
        expected_family_tag: &str,
        legacy_family_encoding: bool,
        job_id: &str,
        job_fence: u64,
        work_recovery: Option<&WorkRevisionRecoveryBinding>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<SourceRevisionAttempt> {
        active(deadline, cancelled)?;
        if store.custody_domain() != domain.as_bytes() {
            return Err(DurableError::Conflict(
                "source revision recovery custody domain differs",
            ));
        }
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let fence_row = tx.query_one(
            "SELECT maintenance_state,generation FROM cmd2_audit_fence WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        // Writers acquire the audit fence before the domain row.  Recovery
        // follows the same order so a replay read cannot deadlock a commit
        // while still refusing maintenance and stale rights before STO reads.
        let domain_row = tx.query_one(
            "SELECT * FROM cmd2_domain WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?;
        if fence_row.get::<_, String>(0) != "normal"
            || !domain_row.get::<_, bool>("rights_allowed")
            || !domain_row.get::<_, bool>("source_complete")
            || domain_row.get::<_, Option<String>>("schema_profile_digest")
                != Some(schema_profile_digest().to_hex())
        {
            return Err(DurableError::Refused(
                "source revision recovery current source is not readable",
            ));
        }
        let current_job = tx
            .query_opt(
                "SELECT fence_epoch FROM cmd2_job WHERE domain=$1 AND job_id=$2",
                &[&domain, &job_id],
            )?
            .ok_or(DurableError::Refused(
                "source revision recovery job lease absent",
            ))?;
        if as_u64(current_job.get(0))? != job_fence {
            return Err(DurableError::Conflict(
                "source revision recovery job lease changed",
            ));
        }
        let attempt = tx
            .query_opt(
                "SELECT * FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2 FOR SHARE",
                &[&domain, &prepare_id],
            )?
            .ok_or(DurableError::Refused("source revision attempt absent"))?;
        let publication_package_raw =
            attempt.get::<_, Option<Vec<u8>>>("source_publication_package");
        let state: String = attempt.get("state");
        if state == "registered" {
            return Err(DurableError::Indeterminate(
                "source revision was registered but not durably sealed",
            ));
        }
        if state != "ready" && state != "committed" {
            return Err(DurableError::Refused(
                "source revision attempt is not replayable",
            ));
        }
        let persisted_raw_request_digest =
            parse_hex(attempt.get::<_, String>("raw_request_digest"))?;
        let persisted_configuration_raw_digest = parse_hex(
            attempt
                .get::<_, Option<String>>("source_configuration_raw_digest")
                .ok_or(DurableError::Corrupt(
                    "source revision configuration binding absent",
                ))?,
        )?;
        let persisted_configuration_canonical_digest = parse_hex(
            attempt
                .get::<_, Option<String>>("source_configuration_canonical_digest")
                .ok_or(DurableError::Corrupt(
                    "source revision canonical configuration binding absent",
                ))?,
        )?;
        if attempt.get::<_, String>("command_id") != expected_command_id
            || persisted_raw_request_digest != expected_raw_request_digest
            || persisted_configuration_raw_digest != expected_configuration_raw_digest
            || persisted_configuration_canonical_digest != expected_configuration_canonical_digest
        {
            return Err(DurableError::Conflict(
                "source revision recovery request/config binding differs",
            ));
        }
        let reads = decode_reads(
            &attempt
                .get::<_, Option<Vec<u8>>>("source_reads")
                .ok_or(DurableError::Corrupt("source revision reads absent"))?,
        )?;
        let revision_binding = reads.revision.as_ref();
        if revision_binding.is_some_and(|binding| binding.family_tag != expected_family_tag)
            || (revision_binding.is_none() && !legacy_family_encoding)
        {
            return Err(DurableError::Conflict(
                "source revision recovery family binding differs",
            ));
        }
        let family_tag = revision_binding
            .map(|binding| binding.family_tag.clone())
            .unwrap_or_else(|| expected_family_tag.to_owned());
        let sign_promotion_family = pg_growth_family(&family_tag).is_some_and(|family| {
            family == crate::source_native_cli::PgGrowthFamily::SignPromotion
        });
        if sign_promotion_family
            && revision_binding.is_none_or(|binding| binding.request_canonical_digest.is_none())
        {
            return Err(DurableError::Corrupt(
                "Sign source revision owner binding is incomplete",
            ));
        }
        if pg_forms_historical_family(&family_tag).is_some() {
            let binding = revision_binding.ok_or(DurableError::Corrupt(
                "Forms/Historical source revision owner binding is absent",
            ))?;
            if binding.owner_configuration_raw_digest != Some(persisted_configuration_raw_digest)
                || binding.owner_configuration_canonical_digest
                    != Some(persisted_configuration_canonical_digest)
                || binding.owner_configuration_raw.is_none()
            {
                return Err(DurableError::Corrupt(
                    "Forms/Historical source revision owner configuration binding differs",
                ));
            }
        } else if revision_binding.is_some_and(|binding| {
            binding.owner_configuration_raw.is_some()
                || binding.owner_configuration_raw_digest.is_some()
                || binding.owner_configuration_canonical_digest.is_some()
        }) {
            return Err(DurableError::Corrupt(
                "source revision carries an unrelated owner configuration",
            ));
        }
        let source_head_sequence = revision_binding.map(|binding| binding.source_head_sequence);
        let predecessor_source_revision = if let Some(binding) = revision_binding {
            binding.predecessor_source_revision
        } else {
            SourceRevision(parse_hex(
                domain_row
                    .get::<_, Option<String>>("source_revision")
                    .ok_or(DurableError::Corrupt(
                        "source revision recovery predecessor absent",
                    ))?,
            )?)
        };
        let has_work_revision = reads.reads.iter().any(|read| {
            matches!(read, PredicateRead::Generation { predicate, .. }
                if predicate.scope == crate::source_pg_work_revision::PREDICATE_SCOPE)
        });
        let work_revision = if has_work_revision {
            if state != "committed" && !(state == "ready" && work_recovery.is_some()) {
                return Err(DurableError::Indeterminate(
                    "PG Work revision retry must reselect its exact current request",
                ));
            }
            crate::source_pg_work_revision::require_schema_v1(&mut tx, deadline, cancelled)?;
            if state == "committed" {
                Some(crate::source_pg_work_revision::load_committed(
                    &mut tx,
                    domain,
                    prepare_id,
                    store.store_id(),
                )?)
            } else {
                None
            }
        } else {
            None
        };
        let definition_digest = parse_hex(
            domain_row
                .get::<_, Option<String>>("source_definition_digest")
                .ok_or(DurableError::Corrupt("source revision definition absent"))?,
        )?;
        require_current_source_definition(definition_digest)?;
        let members = tx.query(
            "SELECT * FROM cmd2_member WHERE domain=$1 AND prepare_id=$2
             ORDER BY member_slot LIMIT $3",
            &[&domain, &prepare_id, &as_i64((MAX_MEMBERS + 1) as u64)?],
        )?;
        if members.is_empty() || members.len() > MAX_MEMBERS {
            return Err(DurableError::Corrupt(
                "source revision recovery member count",
            ));
        }
        let persisted = store
            .recover_attempt_fenced(prepare_id, as_u64(attempt.get("attempt_fence"))?, 0)?
            .ok_or(DurableError::Indeterminate(
                "source revision recovery STO intent absent",
            ))?;
        let receipts = match persisted {
            AttemptRecovery::Sealed { receipts } => receipts,
            _ => {
                return Err(DurableError::Indeterminate(
                    "source revision recovery STO intent uncertain",
                ));
            }
        };
        if receipts.len() != members.len() {
            return Err(DurableError::Corrupt(
                "source revision recovery receipt count",
            ));
        }
        let _guard = store.verify_and_hold_fenced(
            prepare_id,
            as_u64(attempt.get("attempt_fence"))?,
            0,
            &receipts,
            verification_budget(receipts.len()),
        )?;
        let mut writes = Vec::with_capacity(members.len());
        let mut metadata = SourceMetadata::new();
        for row in &members {
            if row.get::<_, String>("profile_id").as_bytes() != REVISION {
                return Err(DurableError::Corrupt(
                    "source revision recovery profile differs",
                ));
            }
            let slot = row.get::<_, i32>("member_slot") as u32;
            let receipt = receipts
                .iter()
                .find(|receipt| receipt.binding().member_slot == slot)
                .ok_or(DurableError::Corrupt(
                    "source revision recovery receipt slot absent",
                ))?;
            let mut exact_bytes = Vec::new();
            store.read_selected(receipt, 8_388_608, &mut exact_bytes)?;
            if Digest256::of_bytes(&exact_bytes)
                != parse_hex(row.get::<_, String>("content_digest"))?
            {
                return Err(DurableError::Corrupt(
                    "source revision recovery STO bytes differ",
                ));
            }
            let expected_predecessor = row
                .get::<_, Option<i64>>("expected_revision")
                .map(|revision| {
                    Ok::<_, DurableError>((
                        as_u64(revision)?,
                        parse_hex(row.get::<_, Option<String>>("expected_digest").ok_or(
                            DurableError::Corrupt("source revision predecessor digest absent"),
                        )?)?,
                    ))
                })
                .transpose()?;
            let proposed_revision = as_u64(row.get("proposed_revision"))?;
            let subject: String = row.get("subject");
            metadata.insert(subject.clone(), row_source_metadata(row)?);
            writes.push(SourceRevisionWrite {
                subject,
                expected_predecessor,
                proposed_revision,
                exact_bytes,
            });
        }
        let projections_raw = attempt
            .get::<_, Option<Vec<u8>>>("source_projections")
            .ok_or(DurableError::Corrupt("source revision projections absent"))?;
        let projections = serde_json::from_slice::<ProjectionRows>(&projections_raw)
            .map_err(|_| DurableError::Corrupt("source revision projections encoding"))?;
        if projections_bytes(&projections)? != projections_raw {
            return Err(DurableError::Corrupt(
                "source revision projections are noncanonical",
            ));
        }
        validate_revision_owner_projections(&family_tag, &writes, &projections)?;
        if let Some(evidence) = work_revision.as_ref() {
            let observed_generation = reads
                .reads
                .iter()
                .find_map(|read| match read {
                    PredicateRead::Generation {
                        predicate,
                        observed_generation,
                    } if predicate.scope == crate::source_pg_work_revision::PREDICATE_SCOPE => {
                        Some(*observed_generation)
                    }
                    _ => None,
                })
                .ok_or(DurableError::Corrupt("PG Work predicate read absent"))?;
            crate::source_pg_work_revision::validate_attempt_evidence(
                evidence,
                domain,
                store.store_id(),
                expected_command_id,
                persisted_raw_request_digest,
                persisted_configuration_raw_digest,
                persisted_configuration_canonical_digest,
                observed_generation,
                &writes,
            )?;
        }
        let recovered_work_recorded_at = if state == "ready" && has_work_revision {
            Some(work_revision_recorded_at(
                &writes,
                work_recovery.ok_or(DurableError::Corrupt(
                    "ready PG Work revision recovery binding absent",
                ))?,
            )?)
        } else {
            None
        };
        let indexes = decode_indexes(
            &attempt
                .get::<_, Option<Vec<u8>>>("source_indexes")
                .ok_or(DurableError::Corrupt("source revision indexes absent"))?,
        )?;
        let expected_indexes = expected_revision_indexes_for_family(&family_tag, &writes)?;
        // Pre-existing committed attempts may carry the legacy empty binding;
        // replay never reapplies their source mutation. A ready attempt must
        // carry the complete derived path set before a new commit is allowed.
        let legacy_empty_indexes = state == "committed"
            && indexes.is_empty()
            && !family_tag.starts_with("tos-pg-global-claims-v1:")
            && pg_forms_historical_family(&family_tag).is_none()
            && pg_growth_family(&family_tag).is_none()
            && family_tag != crate::source_creation_store::OBJECT_LINK_PG_FAMILY_TAG
            && family_tag != crate::source_creation_store::PG_ITEM_ADOPTION_FAMILY_TAG;
        if (!legacy_empty_indexes
            && pg_forms_historical_family(&family_tag).is_none()
            && indexes != expected_indexes)
            || validate_revision_indexes(&family_tag, &writes, &indexes).is_err()
        {
            return Err(DurableError::Corrupt(
                "source revision structural index binding differs",
            ));
        }
        let retained_owner_rows = if pg_forms_historical_family(&family_tag).is_some()
            || sign_promotion_family
        {
            let exact_sources = reads
                .reads
                .iter()
                .filter_map(|read| match read {
                    PredicateRead::Exact {
                        namespace,
                        key,
                        expected_version: Some(version),
                        expected_digest: Some(digest),
                    } if namespace == "source" && key.starts_with("ToS/") => {
                        Some((key.clone(), (*version, *digest)))
                    }
                    _ => None,
                })
                .collect::<BTreeMap<_, _>>();
            let locator_paths = reads
                .locators
                .keys()
                .filter(|path| path.starts_with("ToS/"))
                .cloned()
                .collect::<Vec<_>>();
            if locator_paths.is_empty()
                || locator_paths.len() != exact_sources.len()
                || locator_paths.iter().any(|path| {
                    exact_sources.get(path).is_none_or(|(version, digest)| {
                        reads.locators.get(path) != Some(digest) || *version == 0
                    })
                })
                || locator_paths.len() > cmd::SELECTED_SOURCE_MAX_FILES
            {
                return Err(DurableError::Corrupt(
                    "retained source owner exact read closure is incomplete",
                ));
            }
            let versions = locator_paths
                .iter()
                .map(|path| {
                    let version = exact_sources.get(path).map(|(version, _)| *version).ok_or(
                        DurableError::Corrupt("retained source owner exact read version absent"),
                    )?;
                    as_i64(version)
                })
                .collect::<DurableResult<Vec<_>>>()?;
            let rows = tx.query(
                "SELECT h.* FROM unnest($2::text[],$3::bigint[]) AS wanted(subject,revision)
                 JOIN cmd2_history h
                   ON h.domain=$1 AND h.subject=wanted.subject AND h.revision=wanted.revision
                 ORDER BY h.subject COLLATE \"C\"",
                &[&domain, &locator_paths, &versions],
            )?;
            if rows.len() != locator_paths.len()
                || rows.iter().any(|row| {
                    let path: String = row.get("subject");
                    let expected = exact_sources.get(&path);
                    expected.is_none_or(|(version, digest)| {
                        as_u64(row.get("revision")).ok() != Some(*version)
                            || parse_hex(row.get::<_, String>("content_digest")).ok()
                                != Some(*digest)
                            || reads.locators.get(&path) != Some(digest)
                    })
                })
            {
                return Err(DurableError::Conflict(
                    "retained source owner exact read locators differ",
                ));
            }
            Some(rows)
        } else {
            None
        };
        let predicate_keys = reads
            .reads
            .iter()
            .filter_map(|read| match read {
                PredicateRead::Generation { predicate, .. }
                    if predicate.owner == OWNER
                        && !(predicate.scope == "source-cohort"
                            && predicate.token == "generation") =>
                {
                    Some((
                        predicate.kind.as_str().to_owned(),
                        predicate.scope.clone(),
                        predicate.token.clone(),
                    ))
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let source_generation = reads
            .reads
            .iter()
            .find_map(|read| match read {
                PredicateRead::Generation {
                    predicate,
                    observed_generation,
                } if predicate.scope == "source-cohort" && predicate.token == "generation" => {
                    Some(*observed_generation)
                }
                _ => None,
            })
            .ok_or(DurableError::Corrupt(
                "source revision generation read absent",
            ))?;
        let audit_generation = as_u64(
            tx.query_one(
                "SELECT generation FROM cmd2_audit_fence WHERE domain=$1",
                &[&domain],
            )?
            .get(0),
        )?;
        let contract = parse_hex(domain_row.get::<_, String>("contract_digest"))?;
        let rights_version = as_u64(domain_row.get("rights_version"))?;
        let rule_version = as_u64(domain_row.get("rule_version"))?;
        let epoch = as_u64(
            attempt
                .get::<_, Option<i64>>("source_epoch")
                .ok_or(DurableError::Corrupt("source revision epoch absent"))?,
        )?;
        let delta = parse_hex(attempt.get::<_, String>("delta_digest"))?;
        let fence = as_u64(attempt.get("attempt_fence"))?;
        let cohort_initial_revision = SourceRevision(parse_hex(
            domain_row
                .get::<_, Option<String>>("source_revision")
                .ok_or(DurableError::Corrupt(
                    "source revision recovery bootstrap absent",
                ))?,
        )?);
        let cohort_initial_membership = SourceMembershipV1 {
            count: as_u64(
                domain_row
                    .get::<_, Option<i64>>("source_membership_count")
                    .ok_or(DurableError::Corrupt(
                        "source revision recovery bootstrap count absent",
                    ))?,
            )?,
            digest: parse_hex(
                domain_row
                    .get::<_, Option<String>>("source_membership_digest")
                    .ok_or(DurableError::Corrupt(
                        "source revision recovery bootstrap digest absent",
                    ))?,
            )?,
        };
        let cohort = ManagedSourceCohort {
            domain: domain.to_owned(),
            store_id: store.store_id(),
            initial_revision: cohort_initial_revision,
            initial_membership: cohort_initial_membership,
            epoch,
            definition: definition_digest,
            generation: source_generation,
        };
        drop(_guard);
        tx.commit()?;
        let files = if pg_forms_historical_family(&family_tag).is_some() || sign_promotion_family {
            let rows = retained_owner_rows.as_ref().ok_or(DurableError::Corrupt(
                "source revision retained owner rows absent",
            ))?;
            let expected_rows = rows.iter().collect::<Vec<_>>();
            let retained_bytes = self.read_retained_source_rows(
                store,
                &cohort,
                &expected_rows,
                deadline,
                cancelled,
            )?;
            if retained_bytes.len() != rows.len() {
                return Err(DurableError::Corrupt(
                    "Forms/Historical retained read closure count differs",
                ));
            }
            let mut files = Vec::with_capacity(rows.len());
            for (row, raw) in rows.iter().zip(retained_bytes) {
                if raw.len() > cmd::SELECTED_SOURCE_MAX_MEMBER_BYTES {
                    return Err(DurableError::Refused(
                        "Forms/Historical retained member exceeds selected source bound",
                    ));
                }
                let path: String = row.get("subject");
                if Digest256::of_bytes(&raw) != parse_hex(row.get::<_, String>("content_digest"))? {
                    return Err(DurableError::Corrupt(
                        "Forms/Historical retained read bytes differ",
                    ));
                }
                files.push(cmd::SourceFile {
                    path: RelativePath::parse(&path)
                        .map_err(|_| DurableError::Corrupt("Forms/Historical retained path"))?,
                    raw,
                });
            }
            files
        } else {
            Vec::new()
        };
        if pg_forms_historical_family(&family_tag).is_some() {
            let binding = reads.revision.as_ref().ok_or(DurableError::Corrupt(
                "Forms/Historical source revision binding absent",
            ))?;
            let owner_configuration_raw = binding
                .owner_configuration_raw
                .as_ref()
                .ok_or(DurableError::Corrupt(
                    "Forms/Historical retained owner configuration absent",
                ))?
                .clone();
            let owner_configuration_raw_digest =
                binding
                    .owner_configuration_raw_digest
                    .ok_or(DurableError::Corrupt(
                        "Forms/Historical owner configuration digest absent",
                    ))?;
            let owner_configuration_canonical_digest = binding
                .owner_configuration_canonical_digest
                .ok_or(DurableError::Corrupt(
                    "Forms/Historical owner canonical configuration digest absent",
                ))?;
            let request_canonical_digest =
                binding
                    .request_canonical_digest
                    .ok_or(DurableError::Corrupt(
                        "Forms/Historical request digest absent",
                    ))?;
            let config = cmd::parse(&owner_configuration_raw).map_err(source_error)?;
            let owner_schema = cmd::text(&config, "schema_version").map_err(source_error)?;
            let (expected_schema, operation) = pg_forms_historical_family(&family_tag)
                .ok_or(DurableError::Corrupt("Forms/Historical family tag absent"))?;
            if owner_schema != expected_schema
                || Digest256::of_bytes(&owner_configuration_raw) != owner_configuration_raw_digest
                || cmd::record_digest(&config).map_err(source_error)?
                    != owner_configuration_canonical_digest
            {
                return Err(DurableError::Corrupt(
                    "Forms/Historical retained owner configuration differs",
                ));
            }
            let changes = writes
                .iter()
                .map(|write| {
                    Ok(cmd::SourceChange {
                        path: RelativePath::parse(&write.subject).map_err(|_| {
                            DurableError::Corrupt("Forms/Historical successor path")
                        })?,
                        before: write.expected_predecessor.map(|(_, digest)| digest),
                        after: Some(write.exact_bytes.clone()),
                    })
                })
                .collect::<DurableResult<Vec<_>>>()?;
            let proposal = cmd::PreparedCommand {
                handler_id: pg_forms_historical_handler(owner_schema, operation).to_owned(),
                operation: operation.to_owned(),
                base_revision: binding.predecessor_source_revision,
                request_canonical_sha256: request_canonical_digest,
                configuration_raw_sha256: owner_configuration_raw_digest,
                configuration_canonical_sha256: owner_configuration_canonical_digest,
                response: cmd::object(Vec::new()),
                changes,
                reads: Vec::new(),
                replayed: false,
            };
            let context = CommandContext {
                base_revision: binding.predecessor_source_revision,
                configuration_raw: owner_configuration_raw,
                request_raw: Vec::new(),
                recorded_at: String::new(),
                effective_uid: 0,
                files,
            };
            let contributions =
                crate::source_native_cli::pg_forms_historical::pg_owner_contributions_for_attempt(
                    &context,
                    &proposal,
                    owner_schema,
                    operation,
                    &expected_command_id,
                    request_canonical_digest,
                    persisted_configuration_raw_digest,
                    persisted_configuration_canonical_digest,
                    &family_tag,
                )?;
            let expected_projections =
                revision_owner_projections(&family_tag, &writes, contributions)?;
            if expected_projections != projections {
                return Err(DurableError::Corrupt(
                    "Forms/Historical retained owner projection differs",
                ));
            }
            let expected_indexes =
                crate::source_native_cli::pg_forms_historical::pg_rederive_revision_indexes(
                    &context, &writes,
                )?;
            if expected_indexes != indexes {
                return Err(DurableError::Corrupt(
                    "Forms/Historical retained semantic index rows differ",
                ));
            }
        } else if sign_promotion_family {
            let binding = reads.revision.as_ref().ok_or(DurableError::Corrupt(
                "Sign source revision binding is absent",
            ))?;
            let request_canonical_digest =
                binding
                    .request_canonical_digest
                    .ok_or(DurableError::Corrupt(
                        "Sign source revision canonical request digest is absent",
                    ))?;
            let family = pg_growth_family(&family_tag)
                .filter(|family| *family == crate::source_native_cli::PgGrowthFamily::SignPromotion)
                .ok_or(DurableError::Corrupt(
                    "Sign source revision family tag is absent",
                ))?;
            let changes = writes
                .iter()
                .map(|write| {
                    Ok(cmd::SourceChange {
                        path: RelativePath::parse(&write.subject)
                            .map_err(|_| DurableError::Corrupt("Sign successor path"))?,
                        before: write.expected_predecessor.map(|(_, digest)| digest),
                        after: Some(write.exact_bytes.clone()),
                    })
                })
                .collect::<DurableResult<Vec<_>>>()?;
            let proposal = cmd::PreparedCommand {
                handler_id: family.handler().to_owned(),
                operation: family.operation().to_owned(),
                base_revision: binding.predecessor_source_revision,
                request_canonical_sha256: request_canonical_digest,
                configuration_raw_sha256: persisted_configuration_raw_digest,
                configuration_canonical_sha256: persisted_configuration_canonical_digest,
                response: cmd::object(Vec::new()),
                changes,
                reads: Vec::new(),
                replayed: false,
            };
            let context = CommandContext {
                base_revision: binding.predecessor_source_revision,
                configuration_raw: Vec::new(),
                request_raw: Vec::new(),
                recorded_at: String::new(),
                effective_uid: 0,
                files,
            };
            let contributions =
                crate::source_sign::pg::expected_inventory_contributions_from_writes(
                    &context, &proposal, &writes,
                )
                .map_err(source_error)?;
            let expected_projections =
                revision_owner_projections(&family_tag, &writes, contributions)?;
            if expected_projections != projections {
                return Err(DurableError::Corrupt(
                    "Sign retained owner projection differs",
                ));
            }
        }
        let publication_package = self.recover_source_publication_package(
            store,
            &cohort,
            publication_package_raw.as_deref(),
            &writes,
            deadline,
            cancelled,
        )?;
        Ok(SourceRevisionAttempt {
            domain: domain.to_owned(),
            store_id: store.store_id(),
            prepare_id: prepare_id.to_vec(),
            fence,
            command_id: expected_command_id.to_owned(),
            raw_request_digest: persisted_raw_request_digest,
            configuration_raw_digest: persisted_configuration_raw_digest,
            configuration_canonical_digest: persisted_configuration_canonical_digest,
            epoch,
            source_generation,
            audit_generation,
            rights_version,
            rule_version,
            job_id: job_id.to_owned(),
            job_fence,
            contract,
            definition: definition_digest,
            delta,
            family_tag,
            source_head_sequence,
            predecessor_source_revision,
            reads,
            metadata,
            indexes,
            predicate_keys,
            writes,
            projections,
            publication_package,
            work_revision,
            recovered_work_recorded_at,
        })
    }

    fn recover_source_publication_package(
        &mut self,
        store: &SegmentStore,
        cohort: &ManagedSourceCohort,
        raw_package: Option<&[u8]>,
        writes: &[SourceRevisionWrite],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Option<crate::source_work_transaction::SelectedPublicationPackage>> {
        let Some(raw_package) = raw_package else {
            return Ok(None);
        };
        let envelope = cmd::parse(raw_package).map_err(source_error)?;
        let refs = cmd::array(&envelope, "source_refs").map_err(source_error)?;
        if refs.len() > MAX_MEMBERS || refs.len() != writes.len() {
            return Err(DurableError::Corrupt(
                "source publication recovery write count differs",
            ));
        }
        let mut writes_by_path = BTreeMap::new();
        for write in writes {
            if writes_by_path
                .insert(write.subject.as_str(), write)
                .is_some()
            {
                return Err(DurableError::Corrupt(
                    "source publication recovery writes contain a duplicate",
                ));
            }
        }
        let mut seen = BTreeSet::new();
        let mut predecessor_keys = BTreeSet::new();
        for source_ref in refs {
            let path = cmd::text(source_ref, "path").map_err(source_error)?;
            if !seen.insert(path.to_owned()) {
                return Err(DurableError::Corrupt(
                    "source publication recovery refs contain a duplicate",
                ));
            }
            let write = writes_by_path.get(path).ok_or(DurableError::Corrupt(
                "source publication recovery reference write absent",
            ))?;
            let before = cmd::field(source_ref, "before").map_err(source_error)?;
            let after = cmd::field(source_ref, "after").map_err(source_error)?;
            let before_present = before != &tos_foundation::JsonValue::Null;
            let after_present = after != &tos_foundation::JsonValue::Null;
            if before_present != write.expected_predecessor.is_some() || !after_present {
                return Err(DurableError::Corrupt(
                    "source publication recovery reference shape differs",
                ));
            }
            if let Some((revision, _)) = write.expected_predecessor {
                predecessor_keys.insert((path.to_owned(), revision));
            }
        }
        if !writes_by_path.is_empty() && seen.len() != writes_by_path.len() {
            return Err(DurableError::Corrupt(
                "source publication recovery omitted an attempt write",
            ));
        }

        let history_paths: Vec<String> = predecessor_keys
            .iter()
            .map(|(path, _)| path.clone())
            .collect();
        let history_revisions: Vec<i64> = predecessor_keys
            .iter()
            .map(|(_, revision)| as_i64(*revision))
            .collect::<DurableResult<_>>()?;
        let history_rows = if predecessor_keys.is_empty() {
            Vec::new()
        } else {
            self.client.query(
                "SELECT h.* FROM cmd2_history h
                 JOIN unnest($2::text[],$3::bigint[]) AS wanted(subject,revision)
                   ON h.subject=wanted.subject AND h.revision=wanted.revision
                 WHERE h.domain=$1 ORDER BY h.subject,h.revision",
                &[&cohort.domain, &history_paths, &history_revisions],
            )?
        };
        if history_rows.len() != predecessor_keys.len() {
            return Err(DurableError::Conflict(
                "source publication predecessor history is incomplete",
            ));
        }
        let expected_rows = history_rows.iter().collect::<Vec<_>>();
        let before_bodies = if expected_rows.is_empty() {
            Vec::new()
        } else {
            self.read_retained_source_rows(store, cohort, &expected_rows, deadline, cancelled)?
        };
        let mut before_by_key = BTreeMap::new();
        for (row, body) in history_rows.iter().zip(before_bodies) {
            let path: String = row.get("subject");
            let revision = as_u64(row.get("revision"))?;
            if Digest256::of_bytes(&body) != parse_hex(row.get::<_, String>("content_digest"))? {
                return Err(DurableError::Corrupt(
                    "source publication predecessor body differs",
                ));
            }
            before_by_key.insert((path, revision), body);
        }
        let mut retained = Vec::with_capacity(refs.len());
        for source_ref in refs {
            let path = cmd::text(source_ref, "path").map_err(source_error)?;
            let write = writes_by_path.remove(path).ok_or(DurableError::Corrupt(
                "source publication recovery reference write absent",
            ))?;
            let before =
                write
                    .expected_predecessor
                    .map(|(revision, _)| {
                        before_by_key.remove(&(path.to_owned(), revision)).ok_or(
                            DurableError::Corrupt("source publication predecessor body absent"),
                        )
                    })
                    .transpose()?;
            retained.push(
                crate::source_work_transaction::SelectedPublicationRetainedWrite {
                    path: path.to_owned(),
                    before,
                    after: Some(write.exact_bytes.clone()),
                },
            );
        }
        if !writes_by_path.is_empty() || !before_by_key.is_empty() {
            return Err(DurableError::Corrupt(
                "source publication recovery left unbound bytes",
            ));
        }
        let (package, _) = crate::source_work_transaction::recover_selected_publication_package(
            raw_package,
            &retained,
        )
        .map_err(DurableError::Source)?;
        Ok(Some(package))
    }

    /// Seal/attach the exact successor STO members, then run the ordinary
    /// durable PG transaction. The owner callback is re-read inside both the
    /// owner check and replay outcome fence.
    pub(crate) fn commit_source_revision(
        &mut self,
        store: &SegmentStore,
        attempt: &SourceRevisionAttempt,
        contract: Digest256,
        owner_check: &dyn Fn() -> DurableResult<()>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<(DurableCommitReceipt, DurableTiming)> {
        active(deadline, cancelled)?;
        require_current_source_definition(attempt.definition)?;
        if attempt.definition != definition()
            || attempt.contract != contract
            || store.custody_domain() != attempt.domain.as_bytes()
        {
            return Err(DurableError::Conflict(
                "registered source revision package changed",
            ));
        }
        let identities = attempt
            .writes
            .iter()
            .enumerate()
            .map(|(slot, write)| ShadowWriteIdentity {
                member_slot: slot as u32,
                subject: &write.subject,
                expected_predecessor: write.expected_predecessor,
                proposed_revision: write.proposed_revision,
                exact_bytes: &write.exact_bytes,
            })
            .collect::<Vec<_>>();
        if identities.is_empty() || identities.len() > MAX_MEMBERS {
            return Err(DurableError::Invalid("source revision member count"));
        }
        let state = self.resolve_attempt(&attempt.domain, &attempt.prepare_id)?;
        let receipts: Vec<ByteDurabilityReceipt> = match state {
            AttemptResolution::Registered => {
                let members =
                    match store.recover_attempt_fenced(&attempt.prepare_id, attempt.fence, 0)? {
                        None => seal_members(
                            store,
                            &attempt.prepare_id,
                            attempt.fence,
                            REVISION,
                            &identities,
                        )?,
                        Some(AttemptRecovery::Sealed { receipts }) => {
                            members_from_receipts(&identities, receipts)?
                        }
                        _ => {
                            return Err(DurableError::Indeterminate(
                                "source revision seal retains uncertain custody",
                            ));
                        }
                    };
                self.attach_ready_profile_bound(
                    store,
                    &attempt.domain,
                    &attempt.prepare_id,
                    attempt.fence,
                    &members,
                    REVISION,
                    Some(attempt.delta),
                    Some(&attempt.metadata),
                    None,
                )?;
                members.into_iter().map(|member| member.receipt).collect()
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
                                REVISION,
                            )?;
                        }
                        members.into_iter().map(|member| member.receipt).collect()
                    }
                    _ => {
                        return Err(DurableError::Corrupt(
                            "source revision exact intent/bytes absent",
                        ));
                    }
                }
            }
            AttemptResolution::Aborted => {
                return Err(DurableError::Refused("source revision attempt is aborted"));
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
                expected_rule_version: attempt.rule_version,
                expected_rights_version: attempt.rights_version,
                job_id: &attempt.job_id,
                job_fence: attempt.job_fence,
                full_base_seq: 0,
            },
            CommitMode::Revision {
                attempt,
                owner_check,
                deadline,
                cancelled,
            },
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
        // A changed live delegation cannot be repaired by reconstructing the
        // retained original package. This is refusal-only and takes no owner
        // mutex; the final current-owner/commit fence remains mandatory.
        filesystem
            .current_context(&input.context, deadline, cancelled)
            .map_err(source_error)?;
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
        let mut observed_rows = Vec::with_capacity(input.observations.len());
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
            observed_rows.push(row);
        }
        let observed_raw =
            self.read_retained_source_rows(store, cohort, &observed_rows, deadline, cancelled)?;
        for (observed, raw) in input.observations.values().zip(observed_raw) {
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
            let actual = match expected.commitment_version {
                3 => Self::replay_addressed_inventory(
                    store,
                    &input.basis,
                    &input.context,
                    deadline,
                    cancelled,
                )?,
                4 => {
                    retained_agent_inventory(
                        &mut projection_tx,
                        &cohort.domain,
                        input.basis.generation,
                        &input.context,
                        deadline,
                        cancelled,
                    )?
                    .0
                }
                1 | 2 => {
                    return Err(DurableError::Refused(
                        "legacy Agent-only inventory requires fresh mixed-source bootstrap",
                    ));
                }
                _ => {
                    return Err(DurableError::Refused(
                        "unsupported original inventory commitment",
                    ));
                }
            };
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
        let mut output_names = Vec::with_capacity(outputs.len());
        for row in &outputs {
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
            output_names.push(name);
        }
        let output_rows = outputs.iter().collect::<Vec<_>>();
        let output_raw =
            self.read_retained_source_rows(store, cohort, &output_rows, deadline, cancelled)?;
        for (name, raw) in output_names.into_iter().zip(output_raw) {
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

    /// Retained reconstruction shares exactly one attempt's cold receipts at
    /// a time. Every requested historical locator is rechecked after recovery;
    /// the current domain/rights fence spans the whole addressed operation.
    fn read_retained_source_rows(
        &mut self,
        store: &SegmentStore,
        cohort: &ManagedSourceCohort,
        expected: &[&postgres::Row],
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<Vec<Vec<u8>>> {
        active(deadline, cancelled)?;
        if expected.len() > cmd::SELECTED_SOURCE_MAX_FILES
            || store.store_id() != cohort.store_id
            || store.custody_domain() != cohort.domain.as_bytes()
        {
            return Err(DurableError::Conflict(
                "retained source batch custody or count differs",
            ));
        }
        let mut total = 0u64;
        let mut unique = BTreeSet::new();
        for row in expected {
            let path: String = row.get("subject");
            let revision = as_u64(row.get("revision"))?;
            let bytes = as_u64(row.get("content_length"))?;
            if bytes > 8_388_608 || !unique.insert((path, revision)) {
                return Err(DurableError::Refused(
                    "retained source member exceeds existing cap",
                ));
            }
            total = total
                .checked_add(bytes)
                .filter(|n| *n <= cmd::SELECTED_SOURCE_MAX_BYTES as u64)
                .ok_or(DurableError::Refused("retained source aggregate byte cap"))?;
        }
        let _custody = store.hold_audit_root()?;
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        held_retained_source_metadata(&mut tx, cohort)?;
        let mut metadata_work = 0u64;
        let mut groups: BTreeMap<(Vec<u8>, u64), Vec<usize>> = BTreeMap::new();
        for (index, wanted) in expected.iter().enumerate() {
            active(deadline, cancelled)?;
            let path: String = wanted.get("subject");
            let revision = as_u64(wanted.get("revision"))?;
            let bytes: i32 = tx.query_one("SELECT octet_length(row_to_json(h)::text) FROM cmd2_history h WHERE domain=$1 AND subject=$2 AND revision=$3", &[&cohort.domain, &path, &as_i64(revision)?])?.get(0);
            charge_member_batch_metadata(&mut metadata_work, bytes)?;
            let row = tx.query_one(
                "SELECT * FROM cmd2_history WHERE domain=$1 AND subject=$2 AND revision=$3",
                &[&cohort.domain, &path, &as_i64(revision)?],
            )?;
            if metadata_locator_digest(&row) != metadata_locator_digest(wanted) {
                return Err(DurableError::Conflict("retained source locator changed"));
            }
            let prepare: Vec<u8> = row.get("prepare_id");
            let attempt = tx.query_opt("SELECT state,attempt_fence,commit_seq FROM cmd2_attempt WHERE domain=$1 AND prepare_id=$2 FOR SHARE", &[&cohort.domain, &prepare])?
                .ok_or(DurableError::Corrupt("historical attempt absent"))?;
            if attempt.get::<_, String>(0) != "committed"
                || attempt.get::<_, Option<i64>>(2) != Some(row.get("commit_seq"))
            {
                return Err(DurableError::Corrupt("historical attempt not committed"));
            }
            groups
                .entry((prepare, as_u64(attempt.get(1))?))
                .or_default()
                .push(index);
        }
        let mut result: Vec<Option<Vec<u8>>> = (0..expected.len()).map(|_| None).collect();
        let mut returned_bytes = 0usize;
        for ((prepare, fence), indexes) in groups {
            active(deadline, cancelled)?;
            let receipts = match store.recover_attempt_fenced(&prepare, fence, 0)? {
                Some(AttemptRecovery::Sealed { receipts }) => receipts,
                _ => return Err(DurableError::Corrupt("historical fenced intent not sealed")),
            };
            held_retained_source_metadata(&mut tx, cohort)?;
            for index in indexes {
                active(deadline, cancelled)?;
                let wanted = expected[index];
                let path: String = wanted.get("subject");
                let revision = as_u64(wanted.get("revision"))?;
                let bytes: i32 = tx.query_one("SELECT octet_length(row_to_json(h)::text) FROM cmd2_history h WHERE domain=$1 AND subject=$2 AND revision=$3", &[&cohort.domain, &path, &as_i64(revision)?])?.get(0);
                charge_member_batch_metadata(&mut metadata_work, bytes)?;
                let row = tx.query_one(
                    "SELECT * FROM cmd2_history WHERE domain=$1 AND subject=$2 AND revision=$3",
                    &[&cohort.domain, &path, &as_i64(revision)?],
                )?;
                if metadata_locator_digest(&row) != metadata_locator_digest(wanted) {
                    return Err(DurableError::Conflict("retained source locator changed"));
                }
                let pin: Vec<u8> = row.get("pin_id");
                if pin.len() != 16
                    || receipts
                        .iter()
                        .any(|r| r.pin_id().as_slice() != pin.as_slice())
                {
                    return Err(DurableError::Corrupt("historical fenced pin differs"));
                }
                let id: String = row.get("sto_receipt_id");
                let receipt = receipts
                    .iter()
                    .find(|r| r.receipt_id().to_hex() == id)
                    .ok_or(DurableError::Corrupt("historical frame receipt absent"))?;
                check_history_locator(&row, receipt, &cohort.domain, &path, revision)?;
                let mut raw = Vec::new();
                store.read_selected(receipt, 8_388_608, &mut raw)?;
                active(deadline, cancelled)?;
                returned_bytes = returned_bytes
                    .checked_add(raw.len())
                    .filter(|n| *n <= cmd::SELECTED_SOURCE_MAX_BYTES)
                    .ok_or(DurableError::Refused("retained source aggregate byte cap"))?;
                result[index] = Some(raw);
            }
            // Only this attempt's receipts live here, never an all-attempt cache.
        }
        active(deadline, cancelled)?;
        held_retained_source_metadata(&mut tx, cohort)?;
        tx.commit()?;
        result
            .into_iter()
            .map(|v| v.ok_or(DurableError::Corrupt("retained batch result incomplete")))
            .collect()
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
                .facts()
                .audited_root
                .require_store(store)?;
        }
        let indexes = creation_indexes(package, worker, deadline, cancelled)?;
        let projections = creation_projections(package, worker, deadline, cancelled)?;
        let writes = package
            .changes()
            .iter()
            .map(|change| {
                if change.before.is_some() {
                    return Err(DurableError::Refused(
                        "source creation does not admit predecessor members",
                    ));
                }
                Ok(SourceRevisionWrite {
                    subject: change.path.as_str().to_owned(),
                    expected_predecessor: None,
                    proposed_revision: 1,
                    exact_bytes: change.after.clone().ok_or(DurableError::Refused(
                        "source creation does not admit member deletion",
                    ))?,
                })
            })
            .collect::<DurableResult<Vec<_>>>()?;
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
                writes: writes.clone(),
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
            revision: None,
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
            writes,
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
        require_current_source_definition(attempt.definition)?;
        if attempt.delta != registered_source_delta(package, &attempt.reads, &attempt.projections)?
            || attempt.definition != definition()
            || store.custody_domain() != attempt.domain.as_bytes()
        {
            return Err(DurableError::Conflict("registered source package changed"));
        }
        if let Some(continuation) = &attempt.continuation {
            continuation
                .parent
                .facts()
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
            None,
            false,
            deadline,
            cancelled,
        )
    }

    /// Bootstrap-only cold reopen retaining the protected owner recheck through
    /// the final completeness and selected-generation publication edges.
    pub(crate) fn cold_reopen_source_cohort_with_owner_check(
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
        owner_check: &dyn Fn() -> DurableResult<()>,
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
            Some(owner_check),
            false,
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
            None,
            false,
            deadline,
            cancelled,
        )
    }

    /// Explicit source-verified re-selection for an addressed restore or
    /// cold open. This authenticates a retained V2 descriptor or, for the
    /// activation-before-publication checkpoint, recomputes and verifies the
    /// complete OID-bound V1 descriptor against the fresh database cut. It
    /// then performs the complete source/index/custody audit and publishes a
    /// fresh selection bound to this database identity before returning.
    pub fn cold_reopen_source_cohort_addressed_reselect(
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
        let streamed = streamed
            .map(|(workspace, profile)| {
                profile
                    .validate(workspace)
                    .map(|profile| (workspace, profile))
            })
            .transpose()?;
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
            streamed,
            None,
            true,
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
        owner_check: Option<&dyn Fn() -> DurableResult<()>>,
        addressed_reselect: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<ReopenedSourceCohort> {
        active(deadline, cancelled)?;
        if let Some(owner_check) = owner_check {
            owner_check()?;
        }
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
        if let Some(owner_check) = owner_check {
            owner_check()?;
        }
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
        require_current_source_definition(parse_hex(
            domain_row
                .get::<_, Option<String>>("source_definition_digest")
                .ok_or(DurableError::Corrupt("source definition absent"))?,
        )?)?;
        // The opt-in audit profile is the persistent per-domain V2 protocol
        // marker. The table is deliberately absent from legacy V1 installs.
        let addressed_profile_table = metadata_tx
            .query_one(
                "SELECT to_regclass('cmd2_audit_delta_v1_domain') IS NOT NULL",
                &[],
            )?
            .get::<_, bool>(0);
        let addressed_profile = if addressed_profile_table {
            metadata_tx
                .query_opt(
                    "SELECT profile_digest FROM cmd2_audit_delta_v1_domain WHERE domain=$1",
                    &[&domain],
                )?
                .map(|row| parse_hex(row.get::<_, String>(0).trim_end().into()))
                .transpose()?
        } else {
            None
        };
        if let Some(profile) = addressed_profile {
            if profile != audit_delta::audit_delta_schema_digest()
                && !(addressed_reselect && audit_delta::profile_is_supported(profile))
            {
                return Err(DurableError::Corrupt(
                    "active addressed audit profile differs",
                ));
            }
        }
        let addressed_protocol_active = addressed_profile.is_some();
        if addressed_protocol_active && !addressed_reselect {
            return Err(DurableError::Refused(
                "active addressed protocol requires explicit cold reselection",
            ));
        }
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
        let public_text_packages = current_public_text_packages(
            &mut metadata_tx,
            domain,
            current_count,
            streamed
                .map(|(_, profile)| profile.max_sql_statement_ms)
                .unwrap_or(60_000),
            deadline,
            cancelled,
        )?;
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
                    || !valid_owned_predicate_shape(OWNER, &kind, &scope, &token)
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
        if let Some(owner_check) = owner_check {
            owner_check()?;
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
            recovery::check_current_source_lineage(
                &mut rights_tx,
                domain,
                row,
                &raw,
                expected.get(&path).map(Vec::as_slice),
                source_metadata.get(&path),
            )?;
            rights_tx.commit()?;
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
        let mut projections = optional_agent_projections(&ctx, worker, deadline, cancelled)?;
        if !public_text_packages.is_empty() {
            let Some(projections) = projections.as_mut() else {
                return Err(DurableError::Corrupt(
                    "Public Text current projection inventory unavailable",
                ));
            };
            overlay_current_public_text_contributions(&files, &public_text_packages, projections)?;
        }
        // Native bounded commands invalidate this derived root atomically with
        // their source writes. This explicit, exhaustive cold route may rebuild
        // those projections after verifying all current source and owner rows.
        // A still-published projection remains exact evidence, not a repair hint.
        let rebuild_projections = domain_row
            .get::<_, Option<String>>("source_projection_digest")
            .is_none();
        if rebuild_projections
            && addressed_protocol_active
            && domain_row
                .get::<_, Option<String>>("selected_generation_digest")
                .is_some()
        {
            // Activation can survive a crash before the V2 inventory root is
            // published. A selected, fully verified V1 checkpoint is valid in
            // that state, including after restore to a different database OID.
            // Positively authenticate its exact cut and both membership streams;
            // an invalidated V2 descriptor cannot pass this V1-only reader.
            self.cold_open_selected_generation_v1_addressed_transition(
                store, domain, streamed, deadline, cancelled,
            )?;
        }
        if !rebuild_projections && addressed_protocol_active && projections.is_none() {
            return Err(DurableError::Corrupt(
                "addressed source projection inventory unavailable",
            ));
        }
        if let Some(projections) = &projections {
            let addressed_selection_matches = if !rebuild_projections && addressed_protocol_active {
                addressed_successor::verify_published_addressed_inventory(
                    store,
                    &domain_row,
                    domain,
                    before.through_commit_seq,
                    before.database_oid,
                    &ctx,
                    projections,
                    addressed_reselect,
                    addressed_profile
                        .ok_or(DurableError::Corrupt("addressed audit profile absent"))?,
                    streamed.map_or_else(generation_limits, |(_, profile)| profile.generation),
                    deadline,
                    cancelled,
                )?
            } else {
                false
            };
            let selected_generation_digest =
                domain_row.get::<_, Option<String>>("selected_generation_digest");
            if !rebuild_projections
                && addressed_protocol_active
                && !addressed_selection_matches
                && selected_generation_digest.is_some()
            {
                // Activation can precede V2 publication. Positively open the
                // selected V1 descriptor against the fresh audit, recomputing
                // its OID-bound state digest with the source database identity.
                // This old descriptor is evidence only; the cold route below
                // reselects and publishes a new target-database generation.
                self.cold_open_selected_generation_v1_addressed_transition(
                    store, domain, streamed, deadline, cancelled,
                )?;
            }
            if rows.len() != projections.len()
                || rows
                    .iter()
                    .any(|row| !projections.contains_key(&row.get::<_, String>("subject")))
                || indexed_projection_count != projections.len() as u64
            {
                return Err(DurableError::Corrupt(
                    "rebuilt Agent projection membership differs",
                ));
            }
            if !rebuild_projections
                && ((!addressed_selection_matches
                    && domain_row.get::<_, Option<String>>("source_projection_digest")
                        != Some(projection_root(projections).to_hex()))
                    || &indexed_projections != projections
                    || rows.iter().any(|row| {
                        let path: String = row.get("subject");
                        row.get::<_, Option<Vec<u8>>>("inventory_projection")
                            .as_ref()
                            != projections.get(&path)
                            || row
                                .get::<_, Option<Vec<u8>>>("retained_inventory_projection")
                                .as_ref()
                                != projections.get(&path)
                    }))
            {
                return Err(DurableError::Corrupt(
                    "published Agent projection differs from verified source",
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
        if let Some(owner_check) = owner_check {
            owner_check()?;
        }
        finish_worker(worker, deadline, cancelled)?;
        if let Some(owner_check) = owner_check {
            owner_check()?;
        }
        let current_membership = membership_of_files(&files);
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()?;
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
        crate::source_text_identity_index::verify_for_reassessment(
            &mut tx,
            domain,
            &files,
            cohort.definition,
        )?;
        let annotation_identity_members = files
            .iter()
            .filter(|(path, _)| crate::source_annotation_identity_index::is_packet(path))
            .map(|(path, raw)| (path.clone(), raw.clone()))
            .collect::<BTreeMap<_, _>>();
        crate::source_annotation_identity_index::verify_current(
            &mut tx,
            domain,
            &annotation_identity_members,
            cohort.definition,
        )?;
        let shared_schema_count = files
            .iter()
            .filter(|(path, _)| {
                crate::source_claim_dependency_index::is_shared_schema_source_path(path)
            })
            .take(crate::source_claim_dependency_index::MAX_SHARED_SCHEMA_PATHS + 1)
            .count();
        if files.iter().any(|(path, _)| {
            crate::source_claim_dependency_index::is_shared_schema_source_path(path)
                && path.len() > crate::source_claim_dependency_index::MAX_PATH_BYTES
        }) {
            return Err(DurableError::Refused(
                "Claim navigation shared schema path byte bound exceeded",
            ));
        }
        if shared_schema_count > crate::source_claim_dependency_index::MAX_SHARED_SCHEMA_PATHS {
            return Err(DurableError::Refused(
                "Claim navigation shared schema path bound exceeded",
            ));
        }
        // Shared schemas contribute endpoint paths only; the empty sentinel
        // avoids another retained copy of their source bytes.
        let claim_dependency_members = files
            .iter()
            .filter(|(path, _)| {
                crate::source_claim_dependency_index::is_claim_navigation_source_member(path)
            })
            .map(|(path, raw)| {
                let bytes =
                    if crate::source_claim_dependency_index::is_shared_schema_source_path(path) {
                        Vec::new()
                    } else {
                        raw.clone()
                    };
                (path.clone(), bytes)
            })
            .collect::<BTreeMap<_, _>>();
        crate::source_claim_dependency_index::verify_for_reassessment(
            &mut tx,
            domain,
            &claim_dependency_members,
            cohort.definition,
        )?;
        crate::source_item_file_index::verify_for_reassessment(
            &mut tx,
            domain,
            &files,
            cohort.definition,
        )?;
        if let Some(profile) = addressed_profile {
            audit_delta::reselect_cold_profile(&mut tx, domain, before.audit_generation, profile)
                .map_err(addressed_successor::delta_error)?;
        }
        if rebuild_projections {
            if let Some(projections) = &projections {
                for current in &rows {
                    active(deadline, cancelled)?;
                    let path: String = current.get("subject");
                    let projection = projections.get(&path).ok_or(DurableError::Corrupt(
                        "rebuilt Agent projection member missing",
                    ))?;
                    recovery::refresh_current_projection(
                        &mut tx,
                        domain,
                        current,
                        projection,
                        indexed_projections.get(&path) == Some(projection),
                        before.through_commit_seq,
                        cohort.definition,
                    )?;
                }
            }
        }
        // Existing empty/absent predicates must be verified and re-enabled too.
        tx.execute("UPDATE cmd2_predicate SET complete=true WHERE domain=$1 AND owner=$2 AND definition_version=$3", &[&domain,&OWNER,&cohort.definition.to_hex()])?;
        let projection_digest = projections
            .as_ref()
            .map(|rows| projection_root(rows).to_hex());
        let publication_import = legacy_selected_publication_import_from_members(
            &mut tx,
            &files,
            domain,
            store.store_id(),
            initial_revision,
            initial_membership,
            before.through_commit_seq,
        )?;
        ensure_source_publication_bootstrap_state(
            &mut tx,
            domain,
            store.store_id(),
            initial_revision,
            initial_membership,
            before.through_commit_seq,
            cohort.definition,
            publication_import.as_ref(),
        )?;
        tx.execute("UPDATE cmd2_domain SET source_complete=true,source_generation=head_seq,source_projection_digest=$2,selected_generation_digest=NULL,complete_cut_digest=NULL,complete_cut_generation=NULL WHERE domain=$1", &[&domain,&projection_digest])?;
        if let Some(owner_check) = owner_check {
            owner_check()?;
        }
        tx.commit()?;
        if let Some(owner_check) = owner_check {
            owner_check()?;
        }
        let verified = match streamed {
            Some((workspace, profile)) => self
                .cold_verify_cut_streamed(store, domain, workspace, profile, deadline, cancelled)?,
            None => self.cold_verify_cut_with_budget(store, domain, Some((deadline, cancelled)))?,
        };
        if let Some(owner_check) = owner_check {
            owner_check()?;
        }
        let generation = self.build_complete_generation(store, &verified, deadline, cancelled)?;
        if let Some(owner_check) = owner_check {
            owner_check()?;
        }
        self.select_complete_generation_with_owner_check(&generation, owner_check)?;
        if let Some(owner_check) = owner_check {
            owner_check()?;
        }
        let selected = match streamed {
            Some((workspace, profile)) => self.cold_open_selected_generation_streamed(
                store, domain, workspace, profile, deadline, cancelled,
            )?,
            None => self.cold_open_selected_generation(store, domain, deadline, cancelled)?,
        };
        if let Some(owner_check) = owner_check {
            owner_check()?;
        }
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

fn validate_selected_record_revision_admission(
    context: &cmd::CommandContext,
    proposal: &cmd::PreparedCommand,
    family_tag: &str,
) -> DurableResult<()> {
    let configuration = cmd::parse(&context.configuration_raw).map_err(source_error)?;
    let family = source_revisions::RevisionFamily::parse(
        cmd::text(&configuration, "schema_version").map_err(source_error)?,
    )
    .map_err(source_error)?;
    if !matches!(
        family,
        source_revisions::RevisionFamily::CorpusSelectedV2
            | source_revisions::RevisionFamily::CorpusSelectedV3
            | source_revisions::RevisionFamily::NativeSelected
    ) || (family != source_revisions::RevisionFamily::NativeSelected
        && cmd::text(&configuration, "record_type").map_err(source_error)? == "work")
    {
        return Ok(());
    }
    if family.pg_family_tag(&configuration).map_err(source_error)? != family_tag {
        return Err(DurableError::Conflict(
            "selected revision owner family differs at shared admission",
        ));
    }
    source_revisions::validate_pg_selected_revision_proposal(context, proposal)
        .map_err(source_error)
}

// Small PG pages are separate from the finite complete maps returned to the
// creation owner. An explicit generation profile owns the metadata envelope;
// legacy callers without a profile retain the finite default ceilings.
const COLD_SOURCE_PAGE_ROWS: i64 = 8;
fn admit_cold_source_metadata(
    tx: &mut Transaction<'_>,
    domain: &str,
    profile: Option<StreamedGenerationProfile>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<[u64; 3]> {
    let row_limit = profile.map_or(100_000, |p| p.max_metadata_rows) as u64;
    let mut counts = [0; 3];
    let mut rows = 0u64;
    let mut bytes = 0u64;
    for (i, table) in ["cmd2_current", "cmd2_source_index", "cmd2_predicate"]
        .into_iter()
        .enumerate()
    {
        active(deadline, cancelled)?;
        // One excess row proves refusal. Admissible inputs still contribute
        // every row and the same composite JSON byte sizes; oversized domains
        // need not be serialized in full just to reject their row count.
        let scan_rows = as_i64(row_limit - rows + 1)?;
        let query = format!(
            "SELECT count(*),coalesce(max(octet_length(row_to_json(t)::text)),0),
                    coalesce(sum(octet_length(row_to_json(t)::text)),0)::bigint
             FROM (SELECT * FROM {table} WHERE domain=$1 LIMIT $2) t"
        );
        let admitted = tx.query_one(&query, &[&domain, &scan_rows])?;
        counts[i] = as_u64(admitted.get(0))?;
        rows = rows
            .checked_add(counts[i])
            .ok_or(DurableError::Refused("cold source metadata row overflow"))?;
        bytes = bytes
            .checked_add(as_u64(admitted.get(2))?)
            .ok_or(DurableError::Refused("cold source metadata byte overflow"))?;
        if rows > row_limit
            || bytes > profile.map_or(64 * 1024 * 1024, |p| p.max_metadata_bytes) as u64
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
    let mut after = (Vec::<u8>::new(), -1i32);
    loop {
        active(deadline, cancelled)?;
        let page = tx.query(
            "SELECT c.*,a.attempt_fence AS source_attempt_fence,
                    CASE WHEN p.effective_seq IS NULL
                         THEN CASE WHEN octet_length(h.inventory_projection)<=1048576
                                   THEN h.inventory_projection ELSE NULL END
                         WHEN p.source_revision=h.revision
                              AND p.content_digest=h.content_digest
                              AND p.definition_digest=d.source_definition_digest
                         THEN CASE WHEN octet_length(p.inventory_projection)<=1048576
                                   THEN p.inventory_projection ELSE NULL END
                         ELSE NULL END AS retained_inventory_projection
             FROM cmd2_current c JOIN cmd2_attempt a USING(domain,prepare_id)
             JOIN cmd2_domain d ON d.domain=c.domain
             LEFT JOIN cmd2_history h
               ON h.domain=c.domain AND h.subject=c.subject AND h.revision=c.revision
             LEFT JOIN LATERAL (
               SELECT source_revision,content_digest,definition_digest,inventory_projection,effective_seq
               FROM cmd2_source_inventory_projection
               WHERE domain=c.domain AND subject=c.subject AND effective_seq>=h.commit_seq
                 AND effective_seq<=d.head_seq
               ORDER BY effective_seq DESC LIMIT 1
             ) p ON TRUE
             WHERE c.domain=$1 AND
               (c.prepare_id,c.member_slot) > ($2,$3)
             ORDER BY c.prepare_id,c.member_slot LIMIT $4",
            &[&domain, &after.0, &after.1, &COLD_SOURCE_PAGE_ROWS],
        )?;
        if page.is_empty() {
            break;
        }
        for row in page {
            active(deadline, cancelled)?;
            let key = (
                row.get::<_, Vec<u8>>("prepare_id"),
                row.get::<_, i32>("member_slot"),
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

fn managed_model_recovery_audit_error(error: super::audit_delta::AuditDeltaError) -> DurableError {
    use super::audit_delta::{AuditDeltaError, ColdRequiredReason};
    DurableError::Refused(match error {
        AuditDeltaError::Database(_) => "recovery audit database failure",
        AuditDeltaError::ColdRequired(reason) => match reason {
            ColdRequiredReason::InvalidBounds => "recovery audit bounds invalid",
            ColdRequiredReason::InvalidLimits => "recovery audit limits invalid",
            ColdRequiredReason::IntervalTooLarge => "recovery audit interval exceeds bound",
            ColdRequiredReason::GenerationGap => "recovery audit generation gap",
            ColdRequiredReason::MaintenanceActive => "recovery audit maintenance active",
            ColdRequiredReason::RepeatableSnapshotRequired => {
                "recovery audit requires repeatable snapshot"
            }
            ColdRequiredReason::UnknownTable => "recovery audit table unknown",
            ColdRequiredReason::UnknownOperation => "recovery audit operation unknown",
            ColdRequiredReason::MalformedStableKey => "recovery audit stable key malformed",
            ColdRequiredReason::MalformedCommitment => "recovery audit commitment malformed",
            ColdRequiredReason::ChainDiscontinuity => "recovery audit chain discontinuity",
            ColdRequiredReason::FinalCommitmentMismatch => {
                "recovery audit final commitment differs"
            }
            ColdRequiredReason::CancelledOrExpired => "recovery audit cancelled or expired",
            ColdRequiredReason::DomainNotActivated => "recovery audit domain not activated",
            ColdRequiredReason::ProfileMismatch => "recovery audit profile differs",
            ColdRequiredReason::WorkCounterOverflow => "recovery audit work overflow",
        },
    })
}

fn cold_source_key_rows(
    tx: &mut Transaction<'_>,
    domain: &str,
    predicate: bool,
    count: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
    consume: impl FnMut(&postgres::Row) -> DurableResult<()>,
) -> DurableResult<()> {
    cold_source_key_rows_controlled(
        tx, None, domain, predicate, count, deadline, cancelled, consume,
    )
}

fn cold_source_key_rows_controlled(
    tx: &mut Transaction<'_>,
    max_statement_ms: Option<u64>,
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
        if let Some(milliseconds) = max_statement_ms {
            source_cohort_streamed::set_streamed_pg_limits(tx, milliseconds, deadline, cancelled)?;
        }
        let page = match (predicate, after.as_ref()) {
            (false, None) => tx.query(
                "SELECT * FROM cmd2_source_index WHERE domain=$1
                 ORDER BY kind COLLATE \"C\",token COLLATE \"C\" LIMIT $2",
                &[&domain, &COLD_SOURCE_PAGE_ROWS],
            )?,
            (false, Some(key)) => tx.query(
                "SELECT * FROM cmd2_source_index WHERE domain=$1 AND
                   (kind COLLATE \"C\",token COLLATE \"C\") >
                   ($2 COLLATE \"C\",$3 COLLATE \"C\")
                 ORDER BY kind COLLATE \"C\",token COLLATE \"C\" LIMIT $4",
                &[&domain, &key.0, &key.1, &COLD_SOURCE_PAGE_ROWS],
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
        CommitMode::Bootstrap { owner_check } => {
            owner_check()?;
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
            require_current_source_definition(attempt.definition)?;
            let domain_definition = domain
                .get::<_, Option<String>>("source_definition_digest")
                .ok_or(DurableError::Corrupt("source definition absent"))?;
            require_current_source_definition(parse_hex(domain_definition)?)?;
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
        CommitMode::Revision {
            attempt,
            owner_check,
            deadline,
            cancelled,
        } => {
            owner_check()?;
            let domain = tx.query_one(
                "SELECT * FROM cmd2_domain WHERE domain=$1",
                &[&request.domain],
            )?;
            let expected_reads = reads_bytes(&attempt.reads)?;
            let expected_indexes = indexes_bytes(&attempt.indexes)?;
            let expected_projections = projections_bytes(&attempt.projections)?;
            require_current_source_definition(attempt.definition)?;
            let domain_definition = domain
                .get::<_, Option<String>>("source_definition_digest")
                .ok_or(DurableError::Corrupt("source definition absent"))?;
            require_current_source_definition(parse_hex(domain_definition)?)?;
            if attempt.domain != request.domain
                || attempt.prepare_id != request.prepare_id
                || attempt.fence != request.attempt_fence
                || registered.get::<_, String>("command_id") != attempt.command_id
                || registered.get::<_, String>("raw_request_digest")
                    != attempt.raw_request_digest.to_hex()
                || attempt.definition != definition()
                || attempt.contract != request.expected_contract_digest
                || attempt.rule_version != request.expected_rule_version
                || attempt.rights_version != request.expected_rights_version
                || attempt.job_id != request.job_id
                || attempt.job_fence != request.job_fence
                || domain.get::<_, Option<String>>("source_definition_digest")
                    != Some(attempt.definition.to_hex())
                || !domain.get::<_, bool>("source_complete")
                || as_u64(domain.get("rule_version"))? != request.expected_rule_version
                || domain.get::<_, String>("contract_digest")
                    != request.expected_contract_digest.to_hex()
                || domain.get::<_, Option<String>>("schema_profile_digest")
                    != Some(schema_profile_digest().to_hex())
                || registered.get::<_, Option<i64>>("source_epoch") != Some(as_i64(attempt.epoch)?)
                || registered.get::<_, String>("delta_digest") != attempt.delta.to_hex()
                || registered.get::<_, Option<String>>("source_configuration_raw_digest")
                    != Some(attempt.configuration_raw_digest.to_hex())
                || registered.get::<_, Option<String>>("source_configuration_canonical_digest")
                    != Some(attempt.configuration_canonical_digest.to_hex())
                || registered.get::<_, Option<Vec<u8>>>("source_reads") != Some(expected_reads)
                || registered.get::<_, Option<Vec<u8>>>("source_indexes") != Some(expected_indexes)
                || registered.get::<_, Option<Vec<u8>>>("source_projections")
                    != Some(expected_projections)
                || members
                    .iter()
                    .any(|m| m.get::<_, String>("profile_id").as_bytes() != REVISION)
            {
                return Err(DurableError::Conflict(
                    "source revision attempt/cohort exact registered binding changed",
                ));
            }
            validate_revision_indexes(&attempt.family_tag, &attempt.writes, &attempt.indexes)?;
            validate_revision_owner_projections(
                &attempt.family_tag,
                &attempt.writes,
                &attempt.projections,
            )?;
            let requires_work_revision = attempt.reads.reads.iter().any(|read| {
                matches!(read, PredicateRead::Generation { predicate, .. }
                    if predicate.scope == crate::source_pg_work_revision::PREDICATE_SCOPE)
            });
            match (requires_work_revision, attempt.work_revision.as_ref()) {
                (true, Some(evidence)) => {
                    let observed_generation = attempt
                        .reads
                        .reads
                        .iter()
                        .find_map(|read| match read {
                            PredicateRead::Generation {
                                predicate,
                                observed_generation,
                            } if predicate.scope
                                == crate::source_pg_work_revision::PREDICATE_SCOPE =>
                            {
                                Some(*observed_generation)
                            }
                            _ => None,
                        })
                        .ok_or(DurableError::Corrupt("PG Work predicate read absent"))?;
                    crate::source_pg_work_revision::validate_attempt_evidence(
                        evidence,
                        &request.domain,
                        evidence.observation.store_id,
                        &attempt.command_id,
                        attempt.raw_request_digest,
                        attempt.configuration_raw_digest,
                        attempt.configuration_canonical_digest,
                        observed_generation,
                        &attempt.writes,
                    )?;
                }
                (false, None) => {}
                _ => {
                    return Err(DurableError::Conflict(
                        "PG Work revision evidence/attempt binding differs",
                    ));
                }
            }
            let job = tx.query_opt(
                "SELECT fence_epoch FROM cmd2_job WHERE domain=$1 AND job_id=$2 FOR UPDATE",
                &[&request.domain, &request.job_id],
            )?;
            if job.map(|row| row.get::<_, i64>(0)) != Some(as_i64(request.job_fence)?) {
                return Err(DurableError::Refused(
                    "source revision current job lease changed",
                ));
            }
            if replayed {
                return Ok(());
            }
            validate_revision_indexes(&attempt.family_tag, &attempt.writes, &attempt.indexes)?;
            for (kind, token, path) in &attempt.indexes {
                if kind == "path" {
                    if !path.starts_with("ToS/")
                        || tx
                            .query_opt(
                                "SELECT 1 FROM cmd2_source_index
                                 WHERE domain=$1 AND kind='path' AND token=$2
                                 LIMIT 1",
                                &[&request.domain, token],
                            )?
                            .is_some()
                    {
                        return Err(DurableError::Conflict(
                            "source revision structural path index changed",
                        ));
                    }
                } else {
                    revision_retains_semantic_index(
                        tx,
                        &request.domain,
                        attempt,
                        kind,
                        token,
                        path,
                    )?;
                }
            }
            let dependencies = attempt
                .reads
                .reads
                .iter()
                .filter_map(|read| match read {
                    PredicateRead::Exact { key, .. } => Some(key.clone()),
                    _ => None,
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            for member in members {
                let carrier = row_source_metadata(member)?;
                let expected = attempt
                    .metadata
                    .get(member.get::<_, String>("subject").as_str())
                    .ok_or(DurableError::Corrupt(
                        "source revision carrier metadata absent",
                    ))?;
                if carrier != *expected
                    || carrier.mode != 0o644
                    || carrier.dependencies != Some(dependencies.clone())
                {
                    return Err(DurableError::Conflict(
                        "source revision carrier metadata differs",
                    ));
                }
            }
            if !replayed {
                if domain.get::<_, Option<i64>>("source_epoch") != Some(as_i64(attempt.epoch)?) {
                    return Err(DurableError::Conflict(
                        "source revision cohort epoch changed",
                    ));
                }
                let mut observed = BTreeMap::new();
                for read in &attempt.reads.reads {
                    if let PredicateRead::Generation {
                        predicate,
                        observed_generation,
                    } = read
                    {
                        if predicate.owner != OWNER
                            || predicate.definition_version != attempt.definition.to_hex()
                        {
                            return Err(DurableError::Conflict(
                                "source revision predicate owner differs",
                            ));
                        }
                        if predicate.scope == "source-cohort" && predicate.token == "generation" {
                            if *observed_generation != attempt.source_generation {
                                return Err(DurableError::Corrupt(
                                    "source revision generation readset differs",
                                ));
                            }
                            continue;
                        }
                        observed.insert(
                            (
                                predicate.kind.as_str().to_owned(),
                                predicate.scope.clone(),
                                predicate.token.clone(),
                            ),
                            *observed_generation,
                        );
                    }
                }
                let mut keys = attempt.predicate_keys.clone();
                keys.extend(observed.keys().cloned());
                for (kind, scope, token) in keys {
                    tx.execute(
                        "INSERT INTO cmd2_predicate(domain,kind,owner,scope,token,definition_version,generation,complete)
                         VALUES($1,$2,$3,$4,$5,$6,0,true) ON CONFLICT DO NOTHING",
                        &[
                            &request.domain,
                            &kind,
                            &OWNER,
                            &scope,
                            &token,
                            &attempt.definition.to_hex(),
                        ],
                    )?;
                    let row = tx.query_one(
                        "SELECT generation,complete,definition_version FROM cmd2_predicate
                         WHERE domain=$1 AND kind=$2 AND owner=$3 AND scope=$4 AND token=$5 FOR UPDATE",
                        &[&request.domain, &kind, &OWNER, &scope, &token],
                    )?;
                    if !row.get::<_, bool>(1)
                        || row.get::<_, String>(2) != attempt.definition.to_hex()
                    {
                        return Err(DurableError::Refused(
                            "source revision predicate definition/completeness invalidated",
                        ));
                    }
                    if observed.get(&(kind, scope, token)).is_some_and(|expected| {
                        as_u64(row.get(0)).map_or(true, |actual| actual != *expected)
                    }) {
                        return Err(DurableError::Conflict(
                            "affected source revision predicate changed",
                        ));
                    }
                }
                let exact_paths = attempt.reads.locators.keys().collect::<Vec<_>>();
                let exact_current = tx
                    .query(
                        "SELECT * FROM cmd2_current WHERE domain=$1 AND subject=ANY($2)",
                        &[&request.domain, &exact_paths],
                    )?
                    .into_iter()
                    .map(|row| (row.get::<_, String>("subject"), row))
                    .collect::<BTreeMap<_, _>>();
                for read in &attempt.reads.reads {
                    match read {
                        PredicateRead::Exact {
                            key,
                            expected_version,
                            expected_digest,
                            ..
                        } => {
                            let row = exact_current.get(key).ok_or(DurableError::Conflict(
                                "exact source revision read disappeared",
                            ))?;
                            if Some(as_u64(row.get("revision"))?) != *expected_version
                                || Some(parse_hex(row.get("content_digest"))?) != *expected_digest
                                || attempt.reads.locators.get(key)
                                    != Some(&metadata_locator_digest(row))
                            {
                                return Err(DurableError::Conflict(
                                    "exact source revision body/version/locator changed",
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
                                return Err(DurableError::Conflict(
                                    "source revision absence changed",
                                ));
                            }
                        }
                        PredicateRead::Generation { .. } => {}
                    }
                }
            }
            active(*deadline, cancelled)?;
            Ok(())
        }
        CommitMode::PgCreation {
            attempt,
            owner_check,
            ..
        } => creation::check_commit_owner(
            tx,
            request,
            registered,
            members,
            attempt,
            *owner_check,
            replayed,
        ),
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

fn apply_source_publication_change(
    tx: &mut postgres::Transaction<'_>,
    request: &CommitShadowAttempt<'_>,
    attempt: &SourceRevisionAttempt,
    seq: u64,
) -> DurableResult<()> {
    let Some(package) = attempt.publication_package.as_ref() else {
        return Ok(());
    };
    if package.canonical_bytes.is_empty()
        || package.canonical_bytes.len() > 1_048_576
        || Digest256::of_bytes(&package.canonical_bytes) != package.sha256
        || package.generation != package.base_publication_generation.saturating_add(2)
        || !package.token.starts_with("sha256:")
    {
        return Err(DurableError::Corrupt(
            "source publication package identity is invalid",
        ));
    }
    let mut writes = BTreeMap::new();
    for write in &attempt.writes {
        if writes.insert(write.subject.as_str(), write).is_some() {
            return Err(DurableError::Corrupt(
                "source publication attempt contains duplicate writes",
            ));
        }
    }
    if writes.len() != package.source_refs.len() {
        return Err(DurableError::Corrupt(
            "source publication package/write closure differs",
        ));
    }
    for source_ref in &package.source_refs {
        let write = writes
            .remove(source_ref.path.as_str())
            .ok_or(DurableError::Corrupt(
                "source publication reference write absent",
            ))?;
        let after_digest = Digest256::of_bytes(&write.exact_bytes);
        if source_ref.after.as_ref().is_none_or(|after| {
            after.sha256 != after_digest || after.byte_size != write.exact_bytes.len() as u64
        }) || source_ref.before.as_ref().map(|before| before.sha256)
            != write.expected_predecessor.map(|(_, digest)| digest)
        {
            return Err(DurableError::Conflict(
                "source publication package no longer binds the exact write set",
            ));
        }
    }
    if !writes.is_empty() {
        return Err(DurableError::Corrupt(
            "source publication package leaves unbound writes",
        ));
    }
    let key = (
        "unique".to_owned(),
        "source-publication".to_owned(),
        "current".to_owned(),
    );
    if !attempt.predicate_keys.contains(&key) {
        return Err(DurableError::Corrupt(
            "source publication current fence absent from readset",
        ));
    }
    let observed_generation = attempt.reads.reads.iter().find_map(|read| match read {
        PredicateRead::Generation {
            predicate,
            observed_generation,
        } if predicate.kind.as_str() == "unique"
            && predicate.owner == OWNER
            && predicate.scope == "source-publication"
            && predicate.token == "current" =>
        {
            Some(*observed_generation)
        }
        _ => None,
    });
    if observed_generation != Some(package.base_publication_generation) {
        return Err(DurableError::Conflict(
            "source publication base generation differs from its read fence",
        ));
    }
    let current = tx
        .query_opt(
            "SELECT state,publication_generation,token,store_id,
                bootstrap_source_revision,bootstrap_membership_digest,
                bootstrap_membership_count
         FROM cmd2_source_publication_current WHERE domain=$1 FOR UPDATE",
            &[&request.domain],
        )?
        .ok_or(DurableError::Corrupt(
            "source publication current pointer is absent",
        ))?;
    let current_generation: i64 = current.get("publication_generation");
    let current_token: Option<String> = current.get("token");
    let expected_base_token = package.base_publication_token.as_deref();
    if current_generation != as_i64(package.base_publication_generation)?
        || current_token.as_deref() != expected_base_token
        || current.get::<_, Vec<u8>>("store_id") != attempt.store_id
        || (package.base_publication_generation == 0
            && current.get::<_, String>("state") != "absent")
        || (package.base_publication_generation > 0
            && current.get::<_, String>("state") != "present")
    {
        return Err(DurableError::Conflict(
            "source publication current pointer changed before commit",
        ));
    }
    let generation = as_i64(package.generation)?;
    let commit_seq = as_i64(seq)?;
    let package_digest = package.sha256.to_hex();
    let token = package.token.clone();
    let transaction_id = package.transaction_id.clone();
    let base_token = package.base_publication_token.clone();
    let base_generation = as_i64(package.base_publication_generation)?;
    let store_id = attempt.store_id;
    let bootstrap_revision: String = current.get("bootstrap_source_revision");
    let bootstrap_membership: String = current.get("bootstrap_membership_digest");
    let bootstrap_count: i64 = current.get("bootstrap_membership_count");
    let inserted = tx.execute(
        "INSERT INTO cmd2_source_publication_history(
            domain,publication_generation,state,token,base_token,base_generation,
            transaction_id,package_digest,package_bytes,source_commit_seq,origin_kind,
            import_event_id,import_transition_index,import_receipt_digest,store_id,
            bootstrap_source_revision,bootstrap_membership_digest,bootstrap_membership_count)
         VALUES($1,$2,'present',$3,$4,$5,$6,$7,$8,$9,'pg_commit',NULL,NULL,NULL,$10,$11,$12,$13)",
        &[
            &request.domain,
            &generation,
            &token,
            &base_token,
            &base_generation,
            &transaction_id,
            &package_digest,
            &package.canonical_bytes,
            &commit_seq,
            &&store_id[..],
            &bootstrap_revision,
            &bootstrap_membership,
            &bootstrap_count,
        ],
    )?;
    if inserted != 1 {
        return Err(DurableError::Conflict(
            "source publication history insertion changed",
        ));
    }
    let changed = tx.execute(
        "UPDATE cmd2_source_publication_current SET state='present',origin_kind='pg_commit',
            publication_generation=$2,token=$3,base_token=$4,base_generation=$5,
            transaction_id=$6,package_digest=$7,package_bytes=$8,source_commit_seq=$9,
            import_event_id=NULL,import_transition_index=NULL,import_receipt_digest=NULL
         WHERE domain=$1 AND publication_generation=$10
           AND token IS NOT DISTINCT FROM $11 AND store_id=$12",
        &[
            &request.domain,
            &generation,
            &token,
            &base_token,
            &base_generation,
            &transaction_id,
            &package_digest,
            &package.canonical_bytes,
            &commit_seq,
            &current_generation,
            &current_token,
            &&store_id[..],
        ],
    )?;
    if changed != 1 {
        return Err(DurableError::Conflict(
            "source publication current pointer compare-and-swap failed",
        ));
    }
    let predicate_changed = tx.execute(
        "UPDATE cmd2_predicate SET generation=$5
         WHERE domain=$1 AND kind='unique' AND owner=$2
           AND scope='source-publication' AND token='current'
           AND generation=$3 AND complete AND definition_version=$4
           AND $5>$3",
        &[
            &request.domain,
            &OWNER,
            &as_i64(package.base_publication_generation)?,
            &attempt.definition.to_hex(),
            &generation,
        ],
    )?;
    if predicate_changed != 1 {
        return Err(DurableError::Conflict(
            "source publication current predicate compare-and-swap failed",
        ));
    }
    Ok(())
}

pub(super) fn apply_source_change(
    tx: &mut Transaction<'_>,
    request: &CommitShadowAttempt<'_>,
    mode: &CommitMode<'_>,
    seq: u64,
) -> DurableResult<()> {
    match mode {
        CommitMode::Shadow => Ok(()),
        CommitMode::Bootstrap { .. } => {
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
            crate::source_text_identity_index::replace_current_members(
                tx,
                &request.domain,
                attempt
                    .writes
                    .iter()
                    .map(|write| (write.subject.as_str(), Some(write.exact_bytes.as_slice()))),
                attempt.definition,
            )?;
            crate::source_annotation_identity_index::replace_current_members(
                tx,
                &request.domain,
                attempt
                    .writes
                    .iter()
                    .map(|write| (write.subject.as_str(), Some(write.exact_bytes.as_slice()))),
                attempt.definition,
            )?;
            crate::source_claim_dependency_index::replace_current_members(
                tx,
                &request.domain,
                attempt
                    .writes
                    .iter()
                    .map(|write| (write.subject.as_str(), Some(write.exact_bytes.as_slice()))),
                attempt.definition,
                seq,
            )?;
            crate::source_item_file_index::replace_current_members(
                tx,
                &request.domain,
                attempt
                    .writes
                    .iter()
                    .map(|write| (write.subject.as_str(), Some(write.exact_bytes.as_slice()))),
                attempt.definition,
            )?;
            for (kind, scope, token) in predicates(&attempt.indexes) {
                let changed=tx.execute("UPDATE cmd2_predicate SET generation=generation+1 WHERE domain=$1 AND kind=$2 AND owner=$3 AND scope=$4 AND token=$5 AND complete AND definition_version=$6 AND generation<9223372036854775807", &[&request.domain,&kind,&OWNER,&scope,&token,&attempt.definition.to_hex()])?;
                if changed != 1 {
                    return Err(DurableError::Refused(
                        "source invalidation exhausted or lost complete definition",
                    ));
                }
            }
            // Source advance deliberately invalidates publication below. Prove
            // the addressed parent binding while the same commit transaction
            // still holds its sequencer/audit locks and the parent is selected.
            if let Some(continuation) = &attempt.continuation {
                if let SelectedSourceGeneration::Addressed(parent) = &continuation.parent {
                    let selector = tx.query_one(
                        "SELECT head_seq,published_seq,complete_cut_digest,complete_cut_generation,selected_generation_digest,source_projection_digest FROM cmd2_domain WHERE domain=$1",
                        &[&request.domain],
                    )?;
                    if seq
                        != parent
                            .cut
                            .through_seq
                            .checked_add(1)
                            .ok_or(DurableError::Corrupt("addressed parent sequence overflow"))?
                        || as_u64(selector.get("head_seq"))? != parent.cut.through_seq
                        || as_u64(selector.get("published_seq"))? != parent.cut.through_seq
                        || selector.get::<_, Option<String>>("complete_cut_digest")
                            != Some(parent.state_digest.to_hex())
                        || selector.get::<_, Option<i64>>("complete_cut_generation")
                            != Some(as_i64(parent.selected_audit_generation)?)
                        || selector.get::<_, Option<String>>("selected_generation_digest")
                            != Some(parent.digest.to_hex())
                        || selector.get::<_, Option<String>>("source_projection_digest")
                            != Some(parent.cut.inventory.root().to_hex())
                    {
                        return Err(DurableError::Conflict(
                            "addressed parent selection binding differs before source advance",
                        ));
                    }
                }
            }
            tx.execute("UPDATE cmd2_domain SET source_generation=$2,source_projection_digest=NULL,selected_generation_digest=NULL,complete_cut_digest=NULL,complete_cut_generation=NULL WHERE domain=$1 AND source_complete", &[&request.domain,&as_i64(seq)?])?;
            Ok(())
        }
        CommitMode::Revision {
            attempt,
            deadline,
            cancelled,
            ..
        } => {
            validate_revision_indexes(&attempt.family_tag, &attempt.writes, &attempt.indexes)?;
            validate_revision_owner_projections(
                &attempt.family_tag,
                &attempt.writes,
                &attempt.projections,
            )?;
            for (kind, token, path) in &attempt.indexes {
                if kind == "path" {
                    if !path.starts_with("ToS/") {
                        return Err(DurableError::Corrupt(
                            "source revision structural index binding differs",
                        ));
                    }
                } else {
                    if revision_retains_semantic_index(
                        tx,
                        &request.domain,
                        attempt,
                        kind,
                        token,
                        path,
                    )? {
                        // The exact existing identity remains owned by the same
                        // source path; only its source version changes.
                        continue;
                    }
                }
                let inserted = tx.execute(
                    "INSERT INTO cmd2_source_index(
                         domain,kind,token,path,definition_digest)
                     VALUES($1,$2,$3,$4,$5)",
                    &[
                        &request.domain,
                        kind,
                        token,
                        path,
                        &attempt.definition.to_hex(),
                    ],
                )?;
                if inserted != 1 {
                    return Err(DurableError::Conflict(
                        "source revision structural index insertion changed",
                    ));
                }
            }
            for write in &attempt.writes {
                let projection = attempt.projections.get(&write.subject).map(Vec::as_slice);
                let current = tx.execute(
                    "UPDATE cmd2_current SET inventory_projection=$3
                     WHERE domain=$1 AND subject=$2 AND prepare_id=$4",
                    &[
                        &request.domain,
                        &write.subject,
                        &projection,
                        &request.prepare_id,
                    ],
                )?;
                let history = tx.execute(
                    "UPDATE cmd2_history SET inventory_projection=$3
                     WHERE domain=$1 AND subject=$2 AND prepare_id=$4 AND commit_seq=$5",
                    &[
                        &request.domain,
                        &write.subject,
                        &projection,
                        &request.prepare_id,
                        &as_i64(seq)?,
                    ],
                )?;
                let indexed = tx.execute(
                    "UPDATE cmd2_source_index SET inventory_projection=$3
                     WHERE domain=$1 AND kind='path' AND path=$2
                       AND definition_digest=$4",
                    &[
                        &request.domain,
                        &write.subject,
                        &projection,
                        &attempt.definition.to_hex(),
                    ],
                )?;
                if current != 1 || history != 1 || indexed != 1 {
                    return Err(DurableError::Corrupt(
                        "source revision projection membership differs",
                    ));
                }
            }
            crate::source_text_identity_index::replace_current_members(
                tx,
                &request.domain,
                attempt
                    .writes
                    .iter()
                    .map(|write| (write.subject.as_str(), Some(write.exact_bytes.as_slice()))),
                attempt.definition,
            )?;
            crate::source_annotation_identity_index::replace_current_members(
                tx,
                &request.domain,
                attempt
                    .writes
                    .iter()
                    .map(|write| (write.subject.as_str(), Some(write.exact_bytes.as_slice()))),
                attempt.definition,
            )?;
            crate::source_claim_dependency_index::replace_current_members(
                tx,
                &request.domain,
                attempt
                    .writes
                    .iter()
                    .map(|write| (write.subject.as_str(), Some(write.exact_bytes.as_slice()))),
                attempt.definition,
                seq,
            )?;
            crate::source_item_file_index::replace_current_members(
                tx,
                &request.domain,
                attempt
                    .writes
                    .iter()
                    .map(|write| (write.subject.as_str(), Some(write.exact_bytes.as_slice()))),
                attempt.definition,
            )?;
            let mutation_predicates = revision_mutation_predicates(&attempt.indexes);
            for (kind, scope, token) in &mutation_predicates {
                let changed = tx.execute(
                    "UPDATE cmd2_predicate SET generation=generation+1
                     WHERE domain=$1 AND kind=$2 AND owner=$3 AND scope=$4 AND token=$5
                       AND complete AND definition_version=$6
                       AND generation<9223372036854775807",
                    &[
                        &request.domain,
                        kind,
                        &OWNER,
                        scope,
                        token,
                        &attempt.definition.to_hex(),
                    ],
                )?;
                if changed != 1 {
                    return Err(DurableError::Refused(
                        "source revision invalidation exhausted or lost complete definition",
                    ));
                }
            }
            if let Some(evidence) = &attempt.work_revision {
                crate::source_pg_work_revision::apply_source_change(
                    tx,
                    &request.domain,
                    evidence.observation.store_id,
                    &request.prepare_id,
                    seq,
                    evidence.observation.generation,
                    attempt.definition,
                    evidence,
                    crate::source_pg_work_revision::compound_members_digest(&attempt.writes),
                    *deadline,
                    cancelled,
                )?;
            }
            apply_source_publication_change(tx, request, attempt, seq)?;
            // A bounded revision advances source generation while retaining no
            // stale selected/current projection identity. The next addressed
            // reader must consume the new outbox generation and re-establish
            // its own selection proof.
            tx.execute(
                "UPDATE cmd2_domain SET source_generation=$2,
                    source_projection_digest=NULL,selected_generation_digest=NULL,
                    complete_cut_digest=NULL,complete_cut_generation=NULL
                 WHERE domain=$1 AND source_complete",
                &[&request.domain, &as_i64(seq)?],
            )?;
            Ok(())
        }
        CommitMode::PgCreation { attempt, .. } => {
            creation::apply_source_change(tx, request, attempt, seq)?;
            crate::source_text_identity_index::replace_current_members(
                tx,
                &request.domain,
                attempt
                    .writes
                    .iter()
                    .map(|write| (write.subject.as_str(), Some(write.exact_bytes.as_slice()))),
                attempt.definition,
            )?;
            crate::source_annotation_identity_index::replace_current_members(
                tx,
                &request.domain,
                attempt
                    .writes
                    .iter()
                    .map(|write| (write.subject.as_str(), Some(write.exact_bytes.as_slice()))),
                attempt.definition,
            )?;
            crate::source_claim_dependency_index::replace_current_members(
                tx,
                &request.domain,
                attempt
                    .writes
                    .iter()
                    .map(|write| (write.subject.as_str(), Some(write.exact_bytes.as_slice()))),
                attempt.definition,
                seq,
            )?;
            crate::source_item_file_index::replace_current_members(
                tx,
                &request.domain,
                attempt
                    .writes
                    .iter()
                    .map(|write| (write.subject.as_str(), Some(write.exact_bytes.as_slice()))),
                attempt.definition,
            )?;
            Ok(())
        }
    }
}

pub(super) fn verify_owner_before_outcome(mode: &CommitMode<'_>) -> DurableResult<()> {
    match mode {
        CommitMode::Creation {
            owner,
            deadline,
            cancelled,
            ..
        } => {
            owner
                .verify_current(*deadline, cancelled)
                .map_err(source_error)?;
        }
        CommitMode::Revision { owner_check, .. } => owner_check()?,
        CommitMode::PgCreation { owner_check, .. } => {
            creation::verify_owner_before_outcome(*owner_check)?
        }
        CommitMode::Shadow => {}
        CommitMode::Bootstrap { owner_check } => owner_check()?,
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
            let fields = value
                .as_array()
                .filter(|fields| fields.len() == 2 || fields.len() == 3)
                .ok_or(DurableError::Corrupt("original Agent inventory tuple"))?;
            let commitment_version = if fields.len() == 2 {
                1
            } else {
                u16::try_from(
                    fields[2]
                        .as_u64()
                        .ok_or(DurableError::Corrupt("original inventory version"))?,
                )
                .map_err(|_| DurableError::Corrupt("original inventory version"))?
            };
            if ![3, 4].contains(&commitment_version) {
                return Err(DurableError::Refused(
                    "legacy Agent-only inventory requires fresh mixed-source bootstrap",
                ));
            }
            let dependencies = retained_text(&fields[1])?.to_owned();
            if !dependencies.starts_with("sha256:")
                || Digest256::from_prefixed(&dependencies).is_err()
            {
                return Err(DurableError::Corrupt("original Agent dependency digest"));
            }
            Some(ManagedAgentInventory {
                commitment_version,
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
        revision: None,
    };
    let tuples = if value.is_array() {
        &value
    } else {
        match value.get("profile").and_then(serde_json::Value::as_str) {
            Some("tos.managed-agent-original-reads-v1") => {
                reads.managed_original = Some(
                    value
                        .get("original")
                        .filter(|v| v.is_object())
                        .ok_or(DurableError::Corrupt(
                            "persisted managed original companion",
                        ))?
                        .clone(),
                );
            }
            Some("tos.pg-family-tagged-source-revision-reads-v1") => {
                let object = value
                    .as_object()
                    .filter(|object| {
                        object.len() == 5
                            && [
                                "profile",
                                "family",
                                "source_head_sequence",
                                "predecessor_source_revision",
                                "reads",
                            ]
                            .iter()
                            .all(|key| object.contains_key(*key))
                    })
                    .ok_or(DurableError::Corrupt(
                        "persisted family-tagged source revision fields",
                    ))?;
                let family_tag = object
                    .get("family")
                    .and_then(serde_json::Value::as_str)
                    .filter(|tag| !tag.is_empty() && tag.len() <= 512)
                    .ok_or(DurableError::Corrupt(
                        "persisted source revision family tag",
                    ))?
                    .to_owned();
                let source_head_sequence = object
                    .get("source_head_sequence")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or(DurableError::Corrupt(
                        "persisted source revision head sequence",
                    ))?;
                let predecessor_source_revision = SourceRevision(parse_hex(
                    object
                        .get("predecessor_source_revision")
                        .and_then(serde_json::Value::as_str)
                        .ok_or(DurableError::Corrupt(
                            "persisted source revision predecessor",
                        ))?
                        .to_owned(),
                )?);
                reads.revision = Some(SourceRevisionReadBinding {
                    family_tag,
                    source_head_sequence,
                    predecessor_source_revision,
                    request_canonical_digest: None,
                    owner_configuration_raw: None,
                    owner_configuration_raw_digest: None,
                    owner_configuration_canonical_digest: None,
                });
            }
            Some("tos.pg-family-tagged-source-revision-reads-v2") => {
                let object = value
                    .as_object()
                    .filter(|object| {
                        object.len() == 6
                            && [
                                "profile",
                                "family",
                                "source_head_sequence",
                                "predecessor_source_revision",
                                "request_canonical_digest",
                                "reads",
                            ]
                            .iter()
                            .all(|key| object.contains_key(*key))
                    })
                    .ok_or(DurableError::Corrupt(
                        "persisted family-tagged source revision fields",
                    ))?;
                let family_tag = object
                    .get("family")
                    .and_then(serde_json::Value::as_str)
                    .filter(|tag| !tag.is_empty() && tag.len() <= 512)
                    .ok_or(DurableError::Corrupt(
                        "persisted source revision family tag",
                    ))?
                    .to_owned();
                let source_head_sequence = object
                    .get("source_head_sequence")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or(DurableError::Corrupt(
                        "persisted source revision head sequence",
                    ))?;
                let predecessor_source_revision = SourceRevision(parse_hex(
                    object
                        .get("predecessor_source_revision")
                        .and_then(serde_json::Value::as_str)
                        .ok_or(DurableError::Corrupt(
                            "persisted source revision predecessor",
                        ))?
                        .to_owned(),
                )?);
                let request_canonical_digest = Some(parse_hex(
                    object
                        .get("request_canonical_digest")
                        .and_then(serde_json::Value::as_str)
                        .ok_or(DurableError::Corrupt(
                            "persisted source revision canonical request digest",
                        ))?
                        .to_owned(),
                )?);
                reads.revision = Some(SourceRevisionReadBinding {
                    family_tag,
                    source_head_sequence,
                    predecessor_source_revision,
                    request_canonical_digest,
                    owner_configuration_raw: None,
                    owner_configuration_raw_digest: None,
                    owner_configuration_canonical_digest: None,
                });
            }
            Some("tos.pg-family-tagged-source-revision-reads-v3") => {
                let object = value
                    .as_object()
                    .filter(|object| {
                        object.len() == 9
                            && [
                                "profile",
                                "family",
                                "source_head_sequence",
                                "predecessor_source_revision",
                                "request_canonical_digest",
                                "owner_configuration_raw",
                                "owner_configuration_raw_digest",
                                "owner_configuration_canonical_digest",
                                "reads",
                            ]
                            .iter()
                            .all(|key| object.contains_key(*key))
                    })
                    .ok_or(DurableError::Corrupt(
                        "persisted Forms/Historical revision fields",
                    ))?;
                let family_tag = object
                    .get("family")
                    .and_then(serde_json::Value::as_str)
                    .filter(|tag| !tag.is_empty() && tag.len() <= 512)
                    .ok_or(DurableError::Corrupt(
                        "persisted source revision family tag",
                    ))?
                    .to_owned();
                if pg_forms_historical_family(&family_tag).is_none() {
                    return Err(DurableError::Corrupt(
                        "persisted retained owner configuration family differs",
                    ));
                }
                let source_head_sequence = object
                    .get("source_head_sequence")
                    .and_then(serde_json::Value::as_u64)
                    .ok_or(DurableError::Corrupt(
                        "persisted source revision head sequence",
                    ))?;
                let predecessor_source_revision = SourceRevision(parse_hex(
                    object
                        .get("predecessor_source_revision")
                        .and_then(serde_json::Value::as_str)
                        .ok_or(DurableError::Corrupt(
                            "persisted source revision predecessor",
                        ))?
                        .to_owned(),
                )?);
                let request_canonical_digest = Some(parse_hex(
                    object
                        .get("request_canonical_digest")
                        .and_then(serde_json::Value::as_str)
                        .ok_or(DurableError::Corrupt(
                            "persisted source revision canonical request digest",
                        ))?
                        .to_owned(),
                )?);
                let owner_configuration_raw = object
                    .get("owner_configuration_raw")
                    .and_then(serde_json::Value::as_str)
                    .ok_or(DurableError::Corrupt(
                        "persisted Forms/Historical owner configuration",
                    ))?
                    .as_bytes()
                    .to_vec();
                if owner_configuration_raw.len() > 65_536 {
                    return Err(DurableError::Corrupt(
                        "persisted Forms/Historical owner configuration exceeds bound",
                    ));
                }
                let owner_configuration_raw_digest = parse_hex(
                    object
                        .get("owner_configuration_raw_digest")
                        .and_then(serde_json::Value::as_str)
                        .ok_or(DurableError::Corrupt(
                            "persisted Forms/Historical owner configuration digest",
                        ))?
                        .to_owned(),
                )?;
                let owner_configuration_canonical_digest = parse_hex(
                    object
                        .get("owner_configuration_canonical_digest")
                        .and_then(serde_json::Value::as_str)
                        .ok_or(DurableError::Corrupt(
                            "persisted Forms/Historical owner canonical configuration digest",
                        ))?
                        .to_owned(),
                )?;
                let config = cmd::parse(&owner_configuration_raw).map_err(source_error)?;
                if Digest256::of_bytes(&owner_configuration_raw) != owner_configuration_raw_digest
                    || cmd::record_digest(&config).map_err(source_error)?
                        != owner_configuration_canonical_digest
                {
                    return Err(DurableError::Corrupt(
                        "persisted Forms/Historical owner configuration digest differs",
                    ));
                }
                reads.revision = Some(SourceRevisionReadBinding {
                    family_tag,
                    source_head_sequence,
                    predecessor_source_revision,
                    request_canonical_digest,
                    owner_configuration_raw: Some(owner_configuration_raw),
                    owner_configuration_raw_digest: Some(owner_configuration_raw_digest),
                    owner_configuration_canonical_digest: Some(
                        owner_configuration_canonical_digest,
                    ),
                });
            }
            _ => return Err(DurableError::Corrupt("persisted source reads profile")),
        }
        value
            .get("reads")
            .ok_or(DurableError::Corrupt("persisted source read tuples"))?
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

#[cfg(test)]
mod public_text_rederivation_tests {
    use super::*;

    fn json_bytes(value: &serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(value).unwrap()
    }

    fn package_fixture() -> (String, BTreeMap<String, Vec<u8>>) {
        let home = "ToS/source-witnesses/works/tos.test".to_owned();
        let mut named = crate::source_public_text_entry::FILES
            .iter()
            .map(|name| ((*name).to_owned(), b"{}".to_vec()))
            .collect::<BTreeMap<_, _>>();
        named.insert(
            "source-text-layer.v1.json".into(),
            json_bytes(&serde_json::json!({
                "schema_version":"tos_source_text_layer_v1",
                "layer_id":"tos.textlayer.test"
            })),
        );
        named.insert(
            "source-text-unit.v1.json".into(),
            json_bytes(&serde_json::json!({
                "schema_version":"tos_source_text_unit_packet_v1",
                "packet_id":"tos.textunit.test"
            })),
        );
        named.insert(
            "native-bindings.json".into(),
            json_bytes(&serde_json::json!({
                "schema_version":"tos_native_unit_bindings_v1",
                "bindings":[{"id":"tos.binding.test"}]
            })),
        );
        named.insert(
            "source-create-receipt.json".into(),
            json_bytes(&serde_json::json!({
                "schema_version":"tos_local_source_create_receipt_v1",
                "source_path":format!("{home}/source-text-unit.v1.json"),
                "grants_admission":false
            })),
        );
        let files = named
            .into_iter()
            .map(|(name, raw)| (format!("{home}/{name}"), raw))
            .collect();
        (home, files)
    }

    fn generic_projection(path: &str, raw: &[u8]) -> Vec<u8> {
        let digest = Digest256::of_bytes(raw).to_hex();
        let projection = cmd::object(vec![
            (
                "schema_version",
                cmd::string(tos_compiler::managed_source::MANAGED_MIXED_INVENTORY_MEMBER_SCHEMA_V2),
            ),
            ("path", cmd::string(path)),
            ("raw_sha256", cmd::string(&digest)),
            (
                "records",
                cmd::object(
                    source_claims::MIXED_INVENTORY_RECORD_KINDS
                        .iter()
                        .map(|kind| (*kind, tos_foundation::JsonValue::Array(Vec::new())))
                        .collect(),
                ),
            ),
            ("source_profiles", cmd::object(Vec::new())),
            ("events", tos_foundation::JsonValue::Array(Vec::new())),
            ("anchors", tos_foundation::JsonValue::Array(Vec::new())),
            ("form", tos_foundation::JsonValue::Null),
            ("claim_owner", tos_foundation::JsonValue::Null),
            ("public_text_owner", tos_foundation::JsonValue::Null),
        ]);
        cmd::canonical(&projection).unwrap()
    }

    #[test]
    fn typed_current_public_text_rederives_without_suffix_inference() {
        let (home, mut files) = package_fixture();
        let generic_path =
            "ToS/source-witnesses/relations/formation-research/source-create-receipt.json";
        files.insert(generic_path.to_owned(), b"{}".to_vec());
        let package_files = crate::source_public_text_entry::FILES
            .iter()
            .map(|name| ((*name).to_owned(), files[&format!("{home}/{name}")].clone()))
            .collect::<BTreeMap<_, _>>();
        let committed_owners = crate::source_public_text_entry::FILES
            .iter()
            .map(|name| {
                let path = format!("{home}/{name}");
                let owner = crate::source_public_text_entry::public_text_inventory_projection(
                    &home,
                    &path,
                    &package_files,
                )
                .unwrap();
                let owner = cmd::parse(&cmd::canonical(&owner).unwrap()).unwrap();
                (path, owner)
            })
            .collect();
        let packages = [CurrentPublicTextPackageV1 {
            package_home: home.clone(),
            committed_owners,
        }];
        let mut projections = files
            .iter()
            .map(|(path, raw)| (path.clone(), generic_projection(path, raw)))
            .collect::<ProjectionRows>();

        let streamed_contributions =
            regenerate_current_public_text_contributions(&files, &packages).unwrap();
        assert_eq!(
            streamed_contributions.len(),
            crate::source_public_text_entry::FILES.len()
        );
        for (path, owner) in &streamed_contributions {
            assert!(
                public_text_owner_contributions_match(&packages[0].committed_owners[path], owner,)
                    .unwrap()
            );
        }

        let existing_owner_path = format!("{home}/content.txt");
        let mut existing_projection =
            addressed_inventory::AddressedMixedInventoryV1::checked_projection(
                &existing_owner_path,
                &projections[&existing_owner_path],
                &Digest256::of_bytes(&files[&existing_owner_path]).to_hex(),
            )
            .unwrap();
        let tos_foundation::JsonValue::Object(fields) = &mut existing_projection else {
            panic!("checked inventory projection must be an object");
        };
        let (_, owner) = fields
            .iter_mut()
            .find(|(key, _)| key.as_str() == Some("public_text_owner"))
            .expect("checked inventory projection has a public text owner field");
        *owner = packages[0].committed_owners[&existing_owner_path].clone();
        projections.insert(
            existing_owner_path,
            cmd::canonical(&existing_projection).unwrap(),
        );

        overlay_current_public_text_contributions(&files, &packages, &mut projections).unwrap();
        let packet_path = format!("{home}/source-text-unit.v1.json");
        let packet = addressed_inventory::AddressedMixedInventoryV1::checked_projection(
            &packet_path,
            &projections[&packet_path],
            &Digest256::of_bytes(&files[&packet_path]).to_hex(),
        )
        .unwrap();
        assert_ne!(
            cmd::field(&packet, "public_text_owner").unwrap(),
            &tos_foundation::JsonValue::Null
        );
        let generic = addressed_inventory::AddressedMixedInventoryV1::checked_projection(
            generic_path,
            &projections[generic_path],
            &Digest256::of_bytes(&files[generic_path]).to_hex(),
        )
        .unwrap();
        assert_eq!(
            cmd::field(&generic, "public_text_owner").unwrap(),
            &tos_foundation::JsonValue::Null
        );

        let mut stale_files = files.clone();
        stale_files.insert(format!("{home}/content.txt"), b"changed".to_vec());
        let mut stale_projections = projections.clone();
        assert!(
            overlay_current_public_text_contributions(
                &stale_files,
                &packages,
                &mut stale_projections,
            )
            .is_err()
        );

        let mut incomplete_files = files;
        incomplete_files.remove(&format!("{home}/source-create-receipt.json"));
        assert!(
            overlay_current_public_text_contributions(
                &incomplete_files,
                &packages,
                &mut projections,
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod owned_predicate_shape_tests {
    use super::*;

    #[test]
    fn recognizes_current_owner_scopes_and_rejects_malformed_or_unknown_rows() {
        let digest = Digest256::of_bytes(b"predicate-shape");
        let prefixed = digest.to_prefixed();
        let bare = digest.to_hex();
        let uppercase = format!("sha256:{}", "A".repeat(64));
        let valid = [
            ("range", "source-home", "ToS"),
            ("range", "source-home", "ToS/source-witnesses/example"),
            ("range", "source-inventory", "all"),
            ("range", "source-dependencies-v2", "2"),
            ("range", "claim-dependency-v1", prefixed.as_str()),
            ("range", "claim-navigation-reference-v2", prefixed.as_str()),
            ("range", "item-edition-v1", prefixed.as_str()),
            ("range", "item-file-v1", prefixed.as_str()),
            ("range", "item-rights-v1", prefixed.as_str()),
            ("range", "pg-work-revision-v1", bare.as_str()),
            ("unique", "metadata", "tos.agent.example"),
            ("unique", "path", "ToS/source-witnesses/example"),
            ("unique", "text-identity", "tos.textunit.example"),
            ("unique", "semantic-annotation-entity", "tos.entity.example"),
            ("unique", "sign-candidate", "tos.claim.example"),
            ("unique", "source-publication", "current"),
        ];
        for (kind, scope, token) in valid {
            assert!(
                valid_owned_predicate_shape(OWNER, kind, scope, token),
                "{scope}"
            );
        }

        let invalid = [
            ("range", "claim-dependency-v1", bare.as_str()),
            ("range", "claim-navigation-reference-v2", bare.as_str()),
            ("range", "item-edition-v1", "sha256:not-a-digest"),
            ("range", "item-file-v1", "sha256:not-a-digest"),
            ("range", "item-rights-v1", "sha256:not-a-digest"),
            ("range", "item-rights-v1", uppercase.as_str()),
            ("range", "pg-work-revision-v1", prefixed.as_str()),
            ("range", "source-dependencies-v2", "3"),
            ("range", "unrecognized", "token"),
            ("unique", "semantic-annotation-entity", ""),
            ("unique", "text-identity", ""),
            ("unique", "source-publication", "stale"),
            ("unique", "unrecognized", "token"),
        ];
        for (kind, scope, token) in invalid {
            assert!(!valid_owned_predicate_shape(OWNER, kind, scope, token));
        }
        assert!(!valid_owned_predicate_shape(
            "other-owner",
            "range",
            "claim-navigation-reference-v2",
            &prefixed,
        ));
        assert!(!valid_owned_predicate_shape(
            OWNER,
            "other-kind",
            "claim-navigation-reference-v2",
            &prefixed,
        ));
    }
}

#[cfg(test)]
mod item_claim_owner_current_history_tests {
    use super::*;
    use std::path::Path;

    const EDITION_HOME: &str = "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/editions/repository-82e7e281";
    const ITEM_HOME: &str = "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/editions/repository-82e7e281/items/acquired-note-utf8-20260910";

    fn retain_file(root: &Path, relative: &str, files: &mut BTreeMap<String, Vec<u8>>) {
        let raw = std::fs::read(root.join(relative))
            .unwrap_or_else(|error| panic!("retained fixture file {relative}: {error}"));
        if let Some(existing) = files.insert(relative.to_owned(), raw.clone()) {
            assert_eq!(existing, raw, "duplicate fixture path has identical bytes");
        }
    }

    fn retain_package(root: &Path, home: &str, files: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(root.join(home)).expect("selected package directory") {
            let entry = entry.expect("selected package entry");
            if !entry
                .file_type()
                .expect("selected package file type")
                .is_file()
            {
                continue;
            }
            let name = entry
                .file_name()
                .into_string()
                .expect("selected package filename is UTF-8");
            retain_file(root, &format!("{home}/{name}"), files);
        }
    }

    fn retain_history_archives(root: &Path, home: &str, files: &mut BTreeMap<String, Vec<u8>>) {
        let history_path = format!("{home}/{}", source_revisions::HISTORY);
        let history_raw = std::fs::read(root.join(&history_path)).expect("selected history");
        let history: serde_json::Value =
            serde_json::from_slice(&history_raw).expect("selected history JSON");
        let receipts = history
            .get("receipts")
            .and_then(serde_json::Value::as_array)
            .expect("history receipts");
        for receipt in receipts {
            let archive = receipt
                .get("archive_path")
                .and_then(serde_json::Value::as_str)
                .expect("history archive path");
            let manifest_path = format!("{archive}/manifest.json");
            retain_file(root, &manifest_path, files);
            let manifest_raw = std::fs::read(root.join(&manifest_path)).expect("archive manifest");
            let manifest: serde_json::Value =
                serde_json::from_slice(&manifest_raw).expect("archive manifest JSON");
            let members = manifest
                .get("files")
                .and_then(serde_json::Value::as_object)
                .expect("archive file map");
            for binding in members.values() {
                let blob = binding
                    .get("blob")
                    .and_then(serde_json::Value::as_str)
                    .expect("archive blob name");
                retain_file(root, &format!("{archive}/{blob}"), files);
            }
        }
    }

    fn assert_member(rows: &tos_foundation::JsonValue, path: &str, sha256: &str, bytes: u64) {
        let row = rows
            .as_array()
            .expect("member rows")
            .iter()
            .find(|row| {
                row.object_get("ref")
                    .and_then(tos_foundation::JsonValue::as_str)
                    == Some(path)
            })
            .unwrap_or_else(|| panic!("member {path} is retained"));
        assert_eq!(cmd::text(row, "sha256").unwrap(), sha256);
        assert_eq!(cmd::integer(row, "bytes").unwrap(), bytes);
    }

    #[test]
    fn current_item_owner_reconstructs_adoption_and_later_record_revisions() {
        let fixture = crate::source_creation_store::source_native_test_fixtures::retained_history();
        let repository = fixture.path().join("source");
        let repository = repository.canonicalize().expect("repository root");
        let mut retained = BTreeMap::new();
        retain_package(&repository, EDITION_HOME, &mut retained);
        retain_package(&repository, ITEM_HOME, &mut retained);
        retain_history_archives(&repository, EDITION_HOME, &mut retained);
        retain_history_archives(&repository, ITEM_HOME, &mut retained);

        let ctx = CommandContext {
            base_revision: SourceRevision(Digest256::of_bytes(b"fixture source revision")),
            configuration_raw: b"{}\n".to_vec(),
            request_raw: b"{}\n".to_vec(),
            recorded_at: "2026-10-13T00:00:00Z".to_owned(),
            effective_uid: 0,
            files: retained
                .iter()
                .map(|(path, raw)| SourceFile {
                    path: RelativePath::parse(path).expect("fixture member path"),
                    raw: raw.clone(),
                })
                .collect(),
        };
        let item_claim_path = format!("{ITEM_HOME}/{}", source_claims::CLAIM_STREAM);
        let item_claim_form_path = format!(
            "{ITEM_HOME}/source-claims.67a935708838478f096052106d632fb13c186708024d21794349744041d3f9ad.human-forms.json"
        );
        assert_eq!(
            item_claim_form_owner_stream(&ctx, &item_claim_form_path).as_deref(),
            Some(item_claim_path.as_str()),
            "the selected current Item form dispatches through its exact Claim stream"
        );
        assert_eq!(
            item_claim_form_owner_stream(
                &ctx,
                "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/source-claims.d285a13c464ec786f2e1bbbe622e10d57bb1a722ef20c6bc78b92d44c62cdc11.human-forms.json"
            ),
            None,
            "Work/Expression growth forms remain with their existing owner route"
        );
        let historical_stream = "ToS/source-witnesses/history/example/historical-claims.jsonl";
        let historical_form = format!(
            "ToS/source-witnesses/history/example/historical-claims.{}.human-forms.json",
            "a".repeat(64)
        );
        assert_eq!(
            source_claims::pg_inventory::claim_form_stream_path(&historical_form).as_deref(),
            Some(historical_stream),
            "historical Claim forms resolve to their archive stream before current Item routing"
        );
        let deadline = Instant::now() + std::time::Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let owner = item_claim_owner_contribution(&ctx, &item_claim_path, deadline, &cancelled)
            .expect("current Item owner contribution");

        let adoption = cmd::field(&owner, "adoption_v1").expect("strict historical V1 owner");
        assert_eq!(
            cmd::text(adoption, "schema_version").unwrap(),
            "tos_managed_item_adoption_claim_owner_inventory_v1"
        );
        assert_eq!(
            cmd::array(adoption, "direct_members").unwrap().len(),
            15,
            "the original V1 adoption package remains unchanged"
        );
        let adoption_lineage = cmd::field(adoption, "lineage").unwrap();
        assert_member(
            cmd::field(adoption_lineage, "predecessor_members").unwrap(),
            &format!("{EDITION_HOME}/edition.human-forms.json"),
            "sha256:9ffcd7d4faa984f379fbb336c688b9aa96c5efb271d4fdff775154c78efb816e",
            2118,
        );
        assert_member(
            cmd::field(adoption_lineage, "successor_members").unwrap(),
            &format!("{EDITION_HOME}/edition.human-forms.json"),
            "sha256:b9ab36492d91c45ad228010fca51a24361a08dbee9903aaff29af7209f377005",
            4146,
        );
        let direct_members = cmd::field(adoption, "direct_members").unwrap();
        assert_member(
            direct_members,
            &format!("{ITEM_HOME}/item.human-forms.json"),
            "sha256:336c8f9994175e57528ec4dd8adbbb4d21c73f7418d49b29b55dc4d8c5b9b757",
            2100,
        );
        assert_member(
            direct_members,
            &item_claim_form_path,
            "sha256:6169f0e3f9b7234f830ee46ea8a57935f085cbea76c057b193cec4a30e0a0d6a",
            1486,
        );
        let adoption_claim = cmd::array(adoption, "claims").unwrap().first().unwrap();
        let adoption_form = cmd::field(adoption_claim, "form").unwrap();
        assert_eq!(
            cmd::text(adoption_form, "ref").unwrap(),
            item_claim_form_path
        );
        assert_eq!(
            cmd::text(adoption_form, "sha256").unwrap(),
            "sha256:6169f0e3f9b7234f830ee46ea8a57935f085cbea76c057b193cec4a30e0a0d6a"
        );
        let claim_form = cmd::parse(retained.get(&item_claim_form_path).unwrap()).unwrap();
        assert_eq!(
            cmd::array(adoption_form, "form_ids").unwrap(),
            crate::source_forms::projection_form_ids(&claim_form)
                .unwrap()
                .as_slice()
        );

        let current = cmd::field(&owner, "current").unwrap();
        let edition = cmd::field(current, "edition").unwrap();
        let item = cmd::field(current, "item").unwrap();
        let edition_chain = cmd::array(edition, "history_chain").unwrap();
        let item_chain = cmd::array(item, "history_chain").unwrap();
        let adoption_archive = cmd::text(adoption_lineage, "parent_archive_ref").unwrap();
        let adoption_receipt_sha = cmd::text(adoption_lineage, "parent_transition_sha256").unwrap();
        let adoption_rows = edition_chain
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                row.object_get("operation")
                    .and_then(tos_foundation::JsonValue::as_str)
                    == Some("item.adopt")
                    && row
                        .object_get("archive_path")
                        .and_then(tos_foundation::JsonValue::as_str)
                        == Some(adoption_archive)
                    && row
                        .object_get("receipt_sha256")
                        .and_then(tos_foundation::JsonValue::as_str)
                        == Some(adoption_receipt_sha)
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(adoption_rows.len(), 1, "exact adoption is retained");
        let later_edition_revisions = edition_chain[adoption_rows[0] + 1..]
            .iter()
            .filter(|row| {
                row.object_get("operation")
                    .and_then(tos_foundation::JsonValue::as_str)
                    == Some("record.revise")
            })
            .count();
        assert!(
            later_edition_revisions >= 1,
            "the Edition has a record revision after adoption"
        );
        assert!(
            item_chain.iter().any(|row| {
                row.object_get("operation")
                    .and_then(tos_foundation::JsonValue::as_str)
                    == Some("record.revise")
            }),
            "the Item has a later record revision"
        );
        let current_edition_record = cmd::parse(
            retained
                .get(&format!("{EDITION_HOME}/edition.json"))
                .expect("current Edition record bytes"),
        )
        .unwrap();
        let current_item_record = cmd::parse(
            retained
                .get(&format!("{ITEM_HOME}/item.json"))
                .expect("current Item record bytes"),
        )
        .unwrap();
        assert!(
            cmd::same(
                cmd::field(edition, "source").unwrap(),
                &cmd::reference(&current_edition_record, "record_id", "record_version").unwrap()
            )
            .unwrap()
        );
        assert!(
            cmd::same(
                cmd::field(item, "source").unwrap(),
                &cmd::reference(&current_item_record, "record_id", "record_version").unwrap()
            )
            .unwrap()
        );
        let direct_count = |home: &str| {
            let prefix = format!("{home}/");
            retained
                .keys()
                .filter(|path| {
                    path.strip_prefix(&prefix)
                        .is_some_and(|tail| !tail.is_empty() && !tail.contains('/'))
                })
                .count()
        };
        assert_eq!(
            cmd::array(edition, "direct_members").unwrap().len(),
            direct_count(EDITION_HOME)
        );
        assert_eq!(
            cmd::array(item, "direct_members").unwrap().len(),
            direct_count(ITEM_HOME)
        );
        assert_member(
            cmd::field(item, "direct_members").unwrap(),
            &item_claim_form_path,
            "sha256:6169f0e3f9b7234f830ee46ea8a57935f085cbea76c057b193cec4a30e0a0d6a",
            1486,
        );

        let mut changed_current = ctx.clone();
        let form = changed_current
            .files
            .iter_mut()
            .find(|member| member.path.as_str() == item_claim_form_path)
            .expect("retained Item Claim form");
        form.raw.push(b' ');
        let changed_form_sha256 = Digest256::of_bytes(&form.raw).to_prefixed();
        let changed_form_bytes = form.raw.len() as u64;
        assert_ne!(
            changed_form_sha256,
            "sha256:6169f0e3f9b7234f830ee46ea8a57935f085cbea76c057b193cec4a30e0a0d6a"
        );
        assert_eq!(changed_form_bytes, 1487);
        let changed_owner =
            item_claim_owner_contribution(&changed_current, &item_claim_path, deadline, &cancelled)
                .expect("current Item owner accepts a distinct current form byte representation");
        let changed_current_item =
            cmd::field(cmd::field(&changed_owner, "current").unwrap(), "item").unwrap();
        assert_member(
            cmd::field(changed_current_item, "direct_members").unwrap(),
            &item_claim_form_path,
            &changed_form_sha256,
            changed_form_bytes,
        );
        let changed_adoption = cmd::field(&changed_owner, "adoption_v1").unwrap();
        assert!(
            cmd::same(adoption, changed_adoption).unwrap(),
            "a current form edit does not rewrite the immutable adoption owner"
        );

        let item_history = cmd::parse(
            retained
                .get(&format!("{ITEM_HOME}/{}", source_revisions::HISTORY))
                .expect("retained Item source revision history"),
        )
        .unwrap();
        let first_item_transition = cmd::array(&item_history, "receipts")
            .unwrap()
            .first()
            .expect("first Item record revision");
        let first_item_archive = cmd::text(first_item_transition, "archive_path").unwrap();
        let archive_manifest = cmd::parse(
            retained
                .get(&format!("{first_item_archive}/manifest.json"))
                .expect("first Item record revision archive manifest"),
        )
        .unwrap();
        let archived_item_binding =
            cmd::field(cmd::field(&archive_manifest, "files").unwrap(), "item.json").unwrap();
        let archived_item_blob = cmd::text(archived_item_binding, "blob").unwrap();
        let archived_item_blob_path = format!("{first_item_archive}/{archived_item_blob}");
        let mut tampered_history = ctx.clone();
        let archived_item = tampered_history
            .files
            .iter_mut()
            .find(|member| member.path.as_str() == archived_item_blob_path)
            .expect("first Item record revision item.json blob");
        archived_item.raw.push(b' ');
        assert!(
            item_claim_owner_contribution(
                &tampered_history,
                &item_claim_path,
                deadline,
                &cancelled
            )
            .is_err(),
            "the Item owner rejects a preserved historical blob whose bytes no longer match its manifest"
        );

        let stream_raw = retained
            .get(&item_claim_path)
            .expect("current Item Claim stream");
        let records = cmd::object(
            source_claims::MIXED_INVENTORY_RECORD_KINDS
                .iter()
                .map(|kind| (*kind, tos_foundation::JsonValue::Array(Vec::new())))
                .collect(),
        );
        let projection = mixed_projection_envelope(
            &item_claim_path,
            stream_raw,
            records,
            cmd::object(Vec::new()),
            tos_foundation::JsonValue::Array(Vec::new()),
            tos_foundation::JsonValue::Array(Vec::new()),
            tos_foundation::JsonValue::Null,
            owner,
            tos_foundation::JsonValue::Null,
        )
        .expect("current mixed inventory owner projection");
        let checked = addressed_inventory::AddressedMixedInventoryV1::checked_projection(
            &item_claim_path,
            &projection,
            &Digest256::of_bytes(stream_raw).to_hex(),
        )
        .expect("current Item owner remains accepted by the strict projection reader");
        assert_eq!(
            cmd::text(
                cmd::field(&checked, "claim_owner").unwrap(),
                "schema_version"
            )
            .unwrap(),
            "tos_managed_item_adoption_claim_owner_inventory_v2"
        );
    }
}

#[cfg(test)]
mod item_claim_stream_route_tests {
    use super::is_item_claim_stream;

    #[test]
    fn recognizes_edition_nested_item_claims_and_rejects_unowned_shapes() {
        assert!(is_item_claim_stream(
            "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/editions/repository-82e7e281/items/acquired-note-utf8-20260910/source-claims.jsonl"
        ));
        assert!(is_item_claim_stream(
            "ToS/source-witnesses/items/example/source-claims.jsonl"
        ));
        assert!(!is_item_claim_stream(
            "ToS/source-witnesses/items/example/extra/source-claims.jsonl"
        ));
        assert!(!is_item_claim_stream(
            "ToS/source-witnesses/works/example/items/example/source-claims.jsonl"
        ));
        assert!(!is_item_claim_stream(
            "ToS/source-witnesses/works/example/editions/items/example/source-claims.jsonl"
        ));
        assert!(!is_item_claim_stream(
            "ToS/source-witnesses/works/example/editions/example/items/example/revisions/source-claims.jsonl"
        ));
        assert!(!is_item_claim_stream(
            "ToS/contracts/example/editions/example/items/example/source-claims.jsonl"
        ));
    }
}

/// Reuse the exact selected-placement law inside a caller-owned consistent cut.
pub(crate) fn read_generation_source_metadata_in_tx(
    tx: &mut Transaction<'_>,
    store: &SegmentStore,
    generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
    path: &RelativePath,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<Option<MemberMetadata>> {
    active(deadline, cancelled)?;
    let selection = generation.selected();
    selection.facts().audited_root.require_store(store)?;
    let domain = generation.cohort().domain();
    let selected = selection.lookup_current(domain, path, deadline, cancelled)?;
    held_generation_metadata(tx, store, generation)?;
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
    Ok(result)
}

#[cfg(test)]
mod publication_import_reassessment_tests {
    use super::*;

    #[test]
    #[ignore = "requires explicitly selected disposable PostgreSQL"]
    fn postgres_contract_publication_reassessment_keeps_import_head() {
        let url = std::env::var("TOS_METADATA_TEST_PG_URL").expect("disposable PG URL required");
        let mut client = postgres::Client::connect(&url, postgres::NoTls).unwrap();
        let mut tx = client.transaction().unwrap();
        tx.batch_execute("CREATE TEMP TABLE cmd2_source_publication_import (domain text PRIMARY KEY, import_source_head bigint) ON COMMIT DROP").unwrap();
        assert_eq!(
            publication_import_origin_head(&mut tx, "selected", 152).unwrap(),
            152
        );
        tx.execute(
            "INSERT INTO cmd2_source_publication_import VALUES ('selected',152)",
            &[],
        )
        .unwrap();
        assert_eq!(
            publication_import_origin_head(&mut tx, "selected", 156).unwrap(),
            152
        );
        assert_eq!(
            publication_import_origin_head(&mut tx, "other", 156).unwrap(),
            156
        );
        assert!(publication_import_origin_head(&mut tx, "selected", 151).is_err());
        tx.execute(
            "UPDATE cmd2_source_publication_import SET import_source_head=-1",
            &[],
        )
        .unwrap();
        assert!(publication_import_origin_head(&mut tx, "selected", 156).is_err());
    }
}
