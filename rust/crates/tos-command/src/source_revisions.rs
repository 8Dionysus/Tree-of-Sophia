//! Maintained record revision preparation (handlers 22–27).
//!
//! Exact selected bytes produce proposed record/form/history and retained
//! predecessor writes. Native reservation can read exact anchored manifest
//! members; this module performs no ambient discovery or publication. Schema
//! probes and current account observations do not grant
//! admission; `PreparedCommand::commit` remains a production-admission refusal.

use crate::source_command::{self as cmd, *};
use crate::source_forms;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tos_foundation::{
    CanonicalProfile, Digest256, JsonLimits, JsonString, JsonValue, RelativePath,
    canonical_count_v1, python_strip_unicode16_v1,
};
use tos_source_store::{
    CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1, SourceMembershipV1,
};
use tos_validation::PredicateRead;
use tos_validation::item_rules::ItemLimits;
use tos_validation::native_compound::{
    CandidateNativeRecordHistoryReadObservation, NativeRecordHistoryReadObservation,
    NativeTransportState,
};
use tos_validation::record_biblio_cut::{SourceCutInput, SourceCutInputWithIdentity};
use tos_validation::source_cut::CandidateCutWorkerSchemaExecutor;
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

const HISTORY: &str = "source-revision-history.json";
const PROTOCOL: &str = "tos_selected_source_metadata_v1";
const REGISTRY: &str = "ToS/doctrine/semantic-interchange/entity-types.v1.json";
const CORPUS_SCHEMA: &str = "ToS/contracts/corpus-record.schema.json";
const BASE_FIELDS: &[&str] = &[
    "preferred_label",
    "variant_labels",
    "notes",
    "field_languages",
    "source_refs",
    "extensions",
    "semantic_content",
];
const CORPUS_FIELDS: &[&str] = &["preferred_label", "notes", "field_languages", "source_refs"];
const DEPENDENCIES: &[&str] = &[
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_commands.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_command_contracts.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_revisions.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/human_forms.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/knowledge_assessment.py",
    "scripts/source_record_profiles.py",
    "scripts/native_text_binding.py",
    "scripts/source_owner_context.py",
    "scripts/source_witness_human_forms.py",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
];
const SELECTED_DEPENDENCIES: &[&str] = &[
    "scripts/source_metadata_snapshot.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_metadata_transactions.py",
    "mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_selected_revisions.py",
];

/// Native v1/v2 grants stay separate; v3 alone admits descriptive Edition,
/// Collection and Item corrections. Native witnesses never become Corpus.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevisionFamily {
    Historical,
    PublicProfile,
    PublicProfileScope,
    CorpusFlat,
    CorpusSelectedV2,
    CorpusSelectedV3,
    NativeSelected,
}
impl RevisionFamily {
    pub fn handler_id(self) -> &'static str {
        match self {
            Self::Historical => "historical-source-revision",
            Self::PublicProfile => "public-profile-revision",
            Self::PublicProfileScope => "public-profile-scope-revision",
            Self::CorpusFlat => "native-corpus-flat-revision",
            Self::CorpusSelectedV2 | Self::CorpusSelectedV3 => "native-corpus-selected-revision",
            Self::NativeSelected => "native-witness-link-selected-revision",
        }
    }
    pub(crate) fn selected(self) -> bool {
        matches!(
            self,
            Self::CorpusSelectedV2 | Self::CorpusSelectedV3 | Self::NativeSelected
        )
    }
    fn profile(self) -> bool {
        matches!(self, Self::PublicProfile | Self::PublicProfileScope)
    }
    pub(crate) fn parse(schema: &str) -> SourceCommandResult<Self> {
        Ok(match schema {
            "tos_local_source_revision_owner_v1" => Self::Historical,
            "tos_local_profile_revision_owner_v1" => Self::PublicProfile,
            "tos_local_profile_revision_owner_v2" => Self::PublicProfileScope,
            "tos_local_corpus_revision_owner_v1" => Self::CorpusFlat,
            "tos_local_corpus_revision_owner_v2" => Self::CorpusSelectedV2,
            "tos_local_corpus_revision_owner_v3" => Self::CorpusSelectedV3,
            "tos_local_native_metadata_revision_owner_v1" => Self::NativeSelected,
            _ => {
                return Err(SourceCommandError::Unsupported(
                    "record revision owner schema",
                ));
            }
        })
    }
}

/// Evidence selected by the publication owner, never supplied by request prose.
/// Every retained transaction must contain its exact original revision request
/// and before/after bytes. Recovery reconstructs these bytes independently.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedRevisionTransaction {
    pub transaction_id: String,
    pub status: RevisionTransactionStatus,
    pub authorization_raw: Vec<u8>,
    pub before: Vec<SourceFile>,
    pub after: Vec<SourceFile>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevisionTransactionStatus {
    Pending,
    Committed,
    RolledBack,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RevisionPublication {
    /// Exact observed cooperating-reader token (None is the legacy baseline).
    pub token: Option<String>,
    pub transactions: Vec<RetainedRevisionTransaction>,
}

pub(crate) type Package = BTreeMap<String, Vec<u8>>;
struct Inspection {
    files: Package,
    record: JsonValue,
    subject: JsonValue,
    history: JsonValue,
    profile: JsonValue,
    schemas: Vec<String>,
    native_identity_snapshot: Option<String>,
    native_text_snapshot: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransactionReconstructionMode {
    CurrentOwner,
    ArtifactHistoryEvidence,
}

/// Successful-only CMD evidence that the retained Artifact corrections replay
/// from the exact publication carriers under the maintained evidence context.
/// The private fields prevent callers from promoting a status-only assertion.
#[derive(Debug)]
pub(crate) struct ArtifactCorrectionReplayObservation<I = tos_foundation::SourceRevision> {
    input_identity: I,
    current_membership: SourceMembershipV1,
    source_root: String,
    source_path: String,
    record_id: String,
    origin_record_sha256: String,
    origin_record_byte_size: usize,
    history_sha256: Option<String>,
    transactions: Vec<ArtifactCorrectionReplayTransactionObservation>,
    publication_state_bytes: usize,
    returned_state_bytes: usize,
}

#[derive(Debug)]
pub(crate) struct ArtifactCorrectionReplayTransactionObservation {
    transaction_id: String,
    manifest_sha256: String,
    receipt_sha256: String,
}

impl ArtifactCorrectionReplayObservation {
    pub(crate) fn source_revision(&self) -> tos_foundation::SourceRevision {
        self.input_identity
    }

    pub(crate) fn current_membership(&self) -> SourceMembershipV1 {
        self.current_membership
    }

    pub(crate) fn source_root(&self) -> &str {
        &self.source_root
    }

    pub(crate) fn source_path(&self) -> &str {
        &self.source_path
    }

    pub(crate) fn record_id(&self) -> &str {
        &self.record_id
    }

    pub(crate) fn origin_record_sha256(&self) -> &str {
        &self.origin_record_sha256
    }

    pub(crate) fn origin_record_byte_size(&self) -> usize {
        self.origin_record_byte_size
    }

    pub(crate) fn history_sha256(&self) -> Option<&str> {
        self.history_sha256.as_deref()
    }

    pub(crate) fn transactions(&self) -> &[ArtifactCorrectionReplayTransactionObservation] {
        &self.transactions
    }

    pub(crate) fn publication_state_bytes(&self) -> usize {
        self.publication_state_bytes
    }

    pub(crate) fn returned_state_bytes(&self) -> usize {
        self.returned_state_bytes
    }
}

impl ArtifactCorrectionReplayTransactionObservation {
    pub(crate) fn transaction_id(&self) -> &str {
        &self.transaction_id
    }

    pub(crate) fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }

    pub(crate) fn receipt_sha256(&self) -> &str {
        &self.receipt_sha256
    }
}

impl tos_validation::source_foundation_discovery::ArtifactCorrectionReplayEvidence
    for ArtifactCorrectionReplayObservation
{
    fn source_revision(&self) -> tos_foundation::SourceRevision {
        self.input_identity
    }

    fn current_membership(&self) -> SourceMembershipV1 {
        self.current_membership
    }

    fn source_path(&self) -> &str {
        &self.source_path
    }

    fn record_id(&self) -> &str {
        &self.record_id
    }

    fn origin_record_sha256(&self) -> &str {
        &self.origin_record_sha256
    }

    fn origin_record_byte_size(&self) -> usize {
        self.origin_record_byte_size
    }

    fn history_sha256(&self) -> Option<&str> {
        self.history_sha256.as_deref()
    }

    fn transaction_count(&self) -> usize {
        self.transactions.len()
    }

    fn transaction_at(
        &self,
        index: usize,
    ) -> Option<
        tos_validation::source_foundation_discovery::ArtifactCorrectionReplayTransactionRef<'_>,
    > {
        self.transactions.get(index).map(|transaction| {
            tos_validation::source_foundation_discovery::ArtifactCorrectionReplayTransactionRef {
                transaction_id: &transaction.transaction_id,
                manifest_sha256: &transaction.manifest_sha256,
                receipt_sha256: &transaction.receipt_sha256,
            }
        })
    }

    fn publication_state_bytes(&self) -> usize {
        self.publication_state_bytes
    }

    fn returned_state_bytes(&self) -> usize {
        self.returned_state_bytes
    }
}

/// Candidate-fenced correction evidence keeps the opaque current-input identity
/// and never synthesizes a `SourceRevision`.
pub(crate) type CandidateArtifactCorrectionReplayObservation<I> =
    ArtifactCorrectionReplayObservation<I>;

impl<I: Copy + Eq>
    tos_validation::source_foundation_discovery::CandidateArtifactCorrectionReplayEvidence<I>
    for ArtifactCorrectionReplayObservation<I>
{
    fn input_identity(&self) -> &I {
        &self.input_identity
    }

    fn current_membership(&self) -> SourceMembershipV1 {
        self.current_membership
    }

    fn source_path(&self) -> &str {
        &self.source_path
    }

    fn record_id(&self) -> &str {
        &self.record_id
    }

    fn origin_record_sha256(&self) -> &str {
        &self.origin_record_sha256
    }

    fn origin_record_byte_size(&self) -> usize {
        self.origin_record_byte_size
    }

    fn history_sha256(&self) -> Option<&str> {
        self.history_sha256.as_deref()
    }

    fn transaction_count(&self) -> usize {
        self.transactions.len()
    }

    fn transaction_at(
        &self,
        index: usize,
    ) -> Option<
        tos_validation::source_foundation_discovery::ArtifactCorrectionReplayTransactionRef<'_>,
    > {
        self.transactions.get(index).map(|transaction| {
            tos_validation::source_foundation_discovery::ArtifactCorrectionReplayTransactionRef {
                transaction_id: &transaction.transaction_id,
                manifest_sha256: &transaction.manifest_sha256,
                receipt_sha256: &transaction.receipt_sha256,
            }
        })
    }

    fn publication_state_bytes(&self) -> usize {
        self.publication_state_bytes
    }

    fn returned_state_bytes(&self) -> usize {
        self.returned_state_bytes
    }
}

fn path(value: &str) -> SourceCommandResult<RelativePath> {
    RelativePath::parse(value).map_err(|_| SourceCommandError::Invalid("unsafe source path"))
}
/// Exact selected metadata bytes for a read. No writer request or authority is carried.
pub struct RecordVersionReadInput<'a> {
    pub files: &'a [SourceFile],
    pub source_revision: tos_foundation::SourceRevision,
    pub effective_uid: u64,
    pub schema_source_path: &'a str,
}

/// Small, explicitly selected current metadata view used by candidate-fenced
/// Artifact correction replay. It owns only the paths named by the maintained
/// history/publication proof, while preserving the borrowed input's opaque
/// identity; it is never assigned a `SourceRevision`.
struct CandidateRecordVersionReadInput<'a, I: Copy + Eq> {
    input: &'a dyn SourceCutInputWithIdentity<I>,
    input_identity: I,
    files: Vec<SourceFile>,
    schema_source_path: String,
    effective_uid: u64,
}

/// Transport over one live selected source-root read epoch. The implementor
/// holds publication/currentness and enforces filesystem custody for every path.
pub trait ReadonlyRecordFiles {
    fn read(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<u8>>;
    /// Immediate children only; true denotes a directory. Absence observed here
    /// belongs to the same held epoch as subsequent reads.
    fn list_directory(
        &mut self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Vec<(String, bool)>>;
}
const READONLY_RECORD_MAX_BYTES: usize = 64 * 1024 * 1024;
const READONLY_RECORD_MEMBER_BYTES: usize = 8 * 1024 * 1024;

fn selected_metadata_path(location: &str) -> SourceCommandResult<()> {
    path(location)?;
    if !location.starts_with("ToS/source-witnesses/")
        || !location.ends_with(".json")
        || location.ends_with(".human-forms.json")
        || location.ends_with(HISTORY)
        || location.split('/').any(|part| {
            part.starts_with('.')
                || matches!(
                    part,
                    "owner-local" | "catalog" | "payload" | "local-content" | "private"
                )
        })
    {
        return Err(SourceCommandError::Denied("selected metadata owner path"));
    }
    Ok(())
}

struct ReadonlyFileCollector<'a, T> {
    transport: &'a mut T,
    files: BTreeMap<String, Vec<u8>>,
    bytes: usize,
    deadline: Instant,
    cancelled: &'a AtomicBool,
}
impl<T: ReadonlyRecordFiles> ReadonlyFileCollector<'_, T> {
    fn read(&mut self, name: &str) -> SourceCommandResult<()> {
        path(name)?;
        if self.files.contains_key(name) {
            return Ok(());
        }
        if self.files.len() >= cmd::SELECTED_SOURCE_MAX_FILES
            || Instant::now() >= self.deadline
            || self.cancelled.load(std::sync::atomic::Ordering::Relaxed)
        {
            return Err(SourceCommandError::Unsupported(
                "readonly record collection budget or cancellation",
            ));
        }
        let allowance =
            READONLY_RECORD_MEMBER_BYTES.min(READONLY_RECORD_MAX_BYTES.saturating_sub(self.bytes));
        let raw = self
            .transport
            .read(name, allowance, self.deadline, self.cancelled)?;
        if raw.len() > allowance {
            return Err(SourceCommandError::Unsupported(
                "readonly record collection byte budget",
            ));
        }
        self.bytes = self
            .bytes
            .checked_add(raw.len())
            .filter(|sum| *sum <= READONLY_RECORD_MAX_BYTES)
            .ok_or(SourceCommandError::Unsupported(
                "readonly record collection byte budget",
            ))?;
        self.files.insert(name.to_owned(), raw);
        Ok(())
    }
    fn parsed(&self, name: &str) -> SourceCommandResult<JsonValue> {
        cmd::parse(self.files.get(name).ok_or(SourceCommandError::Invalid(
            "collector selected file absent",
        ))?)
    }
}

fn collect_schema_refs(
    value: &JsonValue,
    from: &str,
    out: &mut BTreeSet<String>,
    depth: usize,
) -> SourceCommandResult<()> {
    if depth > 128 {
        return Err(SourceCommandError::Unsupported("schema dependency depth"));
    }
    if let Some(fields) = value.as_object() {
        for (key, value) in fields {
            if matches!(key.as_str(), Some("$ref" | "$dynamicRef")) {
                let reference = value
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("schema reference text"))?;
                let base = reference.split('#').next().unwrap_or("");
                if base.is_empty() {
                    continue;
                }
                let selected = if let Some(name) = base
                    .strip_prefix("https://tree-of-sophia.local/")
                    .or_else(|| base.strip_prefix("https://treeofsophia.local/"))
                {
                    name.to_owned()
                } else if base.contains(':') || base.starts_with('/') {
                    return Err(SourceCommandError::Unsupported(
                        "schema dependency outside selected contracts",
                    ));
                } else {
                    format!("{}/{}", split(from)?.0, base)
                };
                path(&selected)?;
                if !selected.starts_with("ToS/contracts/") || !selected.ends_with(".schema.json") {
                    return Err(SourceCommandError::Unsupported(
                        "schema dependency outside selected contracts",
                    ));
                }
                out.insert(selected);
            }
            collect_schema_refs(value, from, out, depth + 1)?;
        }
    } else if let JsonValue::Array(values) = value {
        for value in values {
            collect_schema_refs(value, from, out, depth + 1)?;
        }
    }
    Ok(())
}

/// Collect only inputs for one catalog-selected subject record. Selection is
/// navigation, not acceptance: the resolver subsequently verifies schemas,
/// typed history, manifest bindings and exact successor reconstruction.
pub fn collect_readonly_record_files(
    transport: &mut impl ReadonlyRecordFiles,
    owner_path: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<SourceFile>> {
    selected_metadata_path(owner_path)?;
    let (parent, base) = split(owner_path)?;
    let mut reader = ReadonlyFileCollector {
        transport,
        files: BTreeMap::new(),
        bytes: 0,
        deadline,
        cancelled,
    };
    reader.read(owner_path)?;
    reader.read(REGISTRY)?;
    let record = reader.parsed(owner_path)?;
    let registry = reader.parsed(REGISTRY)?;
    let schema_version = cmd::text(&record, "schema_version")?;
    let mut schemas =
        BTreeSet::from(["ToS/contracts/semantic-entity-type-registry.schema.json".to_owned()]);
    match schema_version {
        "tos_corpus_record_v1" => {
            schemas.insert(CORPUS_SCHEMA.to_owned());
        }
        "tos_historical_record_v1" => {
            schemas.insert(CORPUS_SCHEMA.to_owned());
            schemas.insert("ToS/contracts/historical-record.schema.json".to_owned());
        }
        "tos_artifact_source_witness_v1" => {
            schemas.insert("ToS/contracts/artifact-source-witness.schema.json".to_owned());
        }
        "tos_artifact_source_witness_v2" => {
            schemas.insert("ToS/contracts/artifact-source-witness-v2.schema.json".to_owned());
        }
        "tos_scholarly_composite_witness_v1" => {
            schemas.insert("ToS/contracts/scholarly-composite-witness.schema.json".to_owned());
        }
        "tos_source_link_v1" => {
            schemas.insert("ToS/contracts/source-link.schema.json".to_owned());
        }
        _ => {
            let profiles = cmd::array(&registry, "types")?
                .iter()
                .filter_map(|entry| entry.object_get("source_record_profile"))
                .filter(|profile| {
                    cmd::text(profile, "record_type").ok() == cmd::text(&record, "record_type").ok()
                })
                .collect::<Vec<_>>();
            if profiles.len() != 1 {
                return Err(SourceCommandError::Unsupported(
                    "exact metadata schema has no unique declared profile",
                ));
            }
            let routes = cmd::array(profiles[0], "schemas")?
                .iter()
                .filter(|route| cmd::text(route, "schema_version").ok() == Some(schema_version))
                .collect::<Vec<_>>();
            if routes.len() != 1 {
                return Err(SourceCommandError::Unsupported(
                    "metadata source schema not supported",
                ));
            }
            schemas.insert(CORPUS_SCHEMA.to_owned());
            schemas.insert("ToS/contracts/source-metadata-record.schema.json".to_owned());
            schemas.insert(cmd::text(routes[0], "schema_ref")?.to_owned());
            schemas.extend(texts(routes[0], "schema_dependencies", 128)?);
        }
    }
    let mut checked = BTreeSet::new();
    while let Some(name) = schemas
        .iter()
        .find(|name| !checked.contains(*name))
        .cloned()
    {
        if schemas.len() > 128
            || !name.starts_with("ToS/contracts/")
            || !name.ends_with(".schema.json")
        {
            return Err(SourceCommandError::Unsupported(
                "selected schema dependency budget or route",
            ));
        }
        reader.read(&name)?;
        collect_schema_refs(&reader.parsed(&name)?, &name, &mut schemas, 0)?;
        checked.insert(name);
    }
    let children = reader
        .transport
        .list_directory(parent, deadline, cancelled)?;
    if children.len() > cmd::SELECTED_SOURCE_MAX_FILES {
        return Err(SourceCommandError::Unsupported(
            "record home directory budget",
        ));
    }
    if children.iter().any(|(name, _)| name == HISTORY) {
        let history_path = format!("{parent}/{HISTORY}");
        reader.read(&history_path)?;
        let history = reader.parsed(&history_path)?;
        let receipts = cmd::array(&history, "receipts")?;
        if receipts.len() > 128 {
            return Err(SourceCommandError::Unsupported(
                "correction receipt count budget",
            ));
        }
        let subject = source_forms::metadata_subject(&record)?;
        let route = cmd::object(vec![("record_id", cmd::field(&subject, "id")?.clone())]);
        for receipt in receipts {
            let revision = cmd::text(receipt, "previous_revision")?;
            digest_text(cmd::field(receipt, "previous_revision")?)?;
            let directory = archive_path(&route, revision)?;
            if cmd::text(receipt, "archive_path")? != directory {
                return Err(SourceCommandError::Conflict(
                    "archive locator differs from exact subject",
                ));
            }
            let manifest_path = format!("{directory}/manifest.json");
            reader.read(&manifest_path)?;
            let manifest = reader.parsed(&manifest_path)?;
            let binding = cmd::field(cmd::field(&manifest, "files")?, base)?;
            let digest = digest_text(cmd::field(binding, "sha256")?)?;
            let blob = cmd::text(binding, "blob")?;
            if blob != format!("{}.blob", &digest[7..]) {
                return Err(SourceCommandError::Invalid("archive blob locator"));
            }
            reader.read(&format!("{directory}/{blob}"))?;
        }
    }
    reader
        .files
        .into_iter()
        .map(|(name, raw)| {
            Ok(SourceFile {
                path: path(&name)?,
                raw,
            })
        })
        .collect()
}

/// Collect a bounded schema closure from the same held read epoch. This is
/// used by streamed source consumers whose fixed profile must be checked once
/// without materializing the whole source cut.
pub(crate) fn collect_readonly_schema_files(
    transport: &mut impl ReadonlyRecordFiles,
    roots: &[&str],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<SourceFile>> {
    if roots.is_empty() || roots.len() > 128 {
        return Err(SourceCommandError::Unsupported(
            "readonly schema root count budget",
        ));
    }
    let mut reader = ReadonlyFileCollector {
        transport,
        files: BTreeMap::new(),
        bytes: 0,
        deadline,
        cancelled,
    };
    let mut schemas = BTreeSet::new();
    for root in roots {
        if !root.starts_with("ToS/contracts/") || !root.ends_with(".schema.json") {
            return Err(SourceCommandError::Denied(
                "readonly schema root outside source contracts",
            ));
        }
        path(root)?;
        schemas.insert((*root).to_owned());
    }
    let mut checked = BTreeSet::new();
    while let Some(name) = schemas
        .iter()
        .find(|name| !checked.contains(*name))
        .cloned()
    {
        if schemas.len() > 128 {
            return Err(SourceCommandError::Unsupported(
                "selected schema dependency budget",
            ));
        }
        reader.read(&name)?;
        collect_schema_refs(&reader.parsed(&name)?, &name, &mut schemas, 0)?;
        checked.insert(name);
    }
    reader
        .files
        .into_iter()
        .map(|(name, raw)| {
            Ok(SourceFile {
                path: path(&name)?,
                raw,
            })
        })
        .collect()
}

/// Validate one already selected source instance against exact schema bytes
/// from a bounded read input. The caller still owns complete-cut coverage and
/// source-root currentness.
pub(crate) fn validate_readonly_schema(
    input: &RecordVersionReadInput<'_>,
    source_path: &str,
    refs: &[String],
    root: &str,
    instance: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    input.check()?;
    path(source_path)?;
    schema_at(
        worker,
        deadline,
        cancelled,
        input,
        refs,
        root,
        instance,
        Some(source_path),
        false,
    )
}

pub(crate) trait RecordRead {
    type Identity: Copy + Eq;

    fn files(&self) -> &[SourceFile];
    fn input_identity(&self) -> Self::Identity;
    fn effective_uid(&self) -> u64 {
        0
    }
    fn schema_source_path(&self) -> SourceCommandResult<String>;
    fn writer(&self) -> Option<&CommandContext> {
        None
    }
    fn check(&self) -> SourceCommandResult<()>;
    fn file(&self, path: &RelativePath) -> SourceCommandResult<Option<&[u8]>> {
        let mut selected = self.files().iter().filter(|file| file.path == *path);
        let raw = selected.next().map(|file| file.raw.as_slice());
        if selected.next().is_some() {
            return Err(SourceCommandError::Invalid(
                "duplicate selected source path",
            ));
        }
        Ok(raw)
    }
}
impl RecordRead for CommandContext {
    type Identity = tos_foundation::SourceRevision;

    fn files(&self) -> &[SourceFile] {
        &self.files
    }
    fn input_identity(&self) -> Self::Identity {
        self.base_revision
    }
    fn effective_uid(&self) -> u64 {
        self.effective_uid
    }
    fn schema_source_path(&self) -> SourceCommandResult<String> {
        Ok(cmd::text(&cmd::parse(&self.configuration_raw)?, "source_path")?.to_owned())
    }
    fn writer(&self) -> Option<&CommandContext> {
        Some(self)
    }
    fn check(&self) -> SourceCommandResult<()> {
        CommandContext::check(self)
    }
}
impl RecordRead for RecordVersionReadInput<'_> {
    type Identity = tos_foundation::SourceRevision;

    fn files(&self) -> &[SourceFile] {
        self.files
    }
    fn input_identity(&self) -> Self::Identity {
        self.source_revision
    }
    fn effective_uid(&self) -> u64 {
        self.effective_uid
    }
    fn schema_source_path(&self) -> SourceCommandResult<String> {
        path(self.schema_source_path)?;
        Ok(self.schema_source_path.to_owned())
    }
    fn check(&self) -> SourceCommandResult<()> {
        let total = self
            .files
            .iter()
            .try_fold(0usize, |sum, file| sum.checked_add(file.raw.len()));
        if self.files.len() > cmd::SELECTED_SOURCE_MAX_FILES
            || total.is_none_or(|total| total > READONLY_RECORD_MAX_BYTES)
            || self
                .files
                .iter()
                .map(|file| &file.path)
                .collect::<BTreeSet<_>>()
                .len()
                != self.files.len()
        {
            return Err(SourceCommandError::Invalid(
                "selected source byte budget or duplicate path",
            ));
        }
        self.schema_source_path()?;
        Ok(())
    }
}

impl<I: Copy + Eq> RecordRead for CandidateRecordVersionReadInput<'_, I> {
    type Identity = I;

    fn files(&self) -> &[SourceFile] {
        &self.files
    }

    fn input_identity(&self) -> Self::Identity {
        self.input_identity
    }

    fn effective_uid(&self) -> u64 {
        self.effective_uid
    }

    fn schema_source_path(&self) -> SourceCommandResult<String> {
        path(&self.schema_source_path)?;
        Ok(self.schema_source_path.clone())
    }

    fn check(&self) -> SourceCommandResult<()> {
        if self.input.input_identity() != &self.input_identity {
            return Err(SourceCommandError::Conflict(
                "candidate selected metadata input identity changed",
            ));
        }
        let total = self
            .files
            .iter()
            .try_fold(0usize, |sum, file| sum.checked_add(file.raw.len()));
        if self.files.len() > cmd::SELECTED_SOURCE_MAX_FILES
            || total.is_none_or(|total| total > READONLY_RECORD_MAX_BYTES)
            || self
                .files
                .iter()
                .map(|file| &file.path)
                .collect::<BTreeSet<_>>()
                .len()
                != self.files.len()
        {
            return Err(SourceCommandError::Invalid(
                "candidate selected source byte budget or duplicate path",
            ));
        }
        self.schema_source_path()?;
        Ok(())
    }
}

/// One schema worker bound to the same typed read identity as the selected
/// metadata context. Candidate workers prove their resource closure through
/// that identity and prepared schema-set binding; immutable-cut workers still
/// compare each selected schema's exact source bytes.
trait RecordSchemaWorker: CutSchemaExecutor {
    type Identity: Copy + Eq;

    fn input_identity(&self) -> Self::Identity;
    fn contract_digest(&self, contract: &str) -> Option<Digest256>;
    fn resource_closure_bound_to_identity(&self) -> bool {
        false
    }

    fn public_profile(
        &mut self,
        cut: Option<&CorpusCutReader>,
        deadline: Instant,
        cancelled: &AtomicBool,
        ctx: &impl RecordRead<Identity = Self::Identity>,
        config: &JsonValue,
        record: &JsonValue,
    ) -> SourceCommandResult<(JsonValue, Vec<String>, Option<String>, Option<String>)>
    where
        Self: Sized,
    {
        let _ = (cut, deadline, cancelled, ctx, config, record);
        Err(SourceCommandError::Unsupported(
            "public profile requires the immutable-cut schema worker",
        ))
    }
}

impl RecordSchemaWorker for CutWorkerSchemaExecutor {
    type Identity = tos_foundation::SourceRevision;

    fn input_identity(&self) -> Self::Identity {
        self.source_revision()
    }

    fn contract_digest(&self, contract: &str) -> Option<Digest256> {
        CutWorkerSchemaExecutor::contract_digest(self, contract)
    }

    fn public_profile(
        &mut self,
        cut: Option<&CorpusCutReader>,
        deadline: Instant,
        cancelled: &AtomicBool,
        ctx: &impl RecordRead<Identity = Self::Identity>,
        config: &JsonValue,
        record: &JsonValue,
    ) -> SourceCommandResult<(JsonValue, Vec<String>, Option<String>, Option<String>)> {
        public_profile(cut, self, deadline, cancelled, ctx, config, record)
    }
}

impl<I: Copy + Eq> RecordSchemaWorker for CandidateCutWorkerSchemaExecutor<I> {
    type Identity = I;

    fn input_identity(&self) -> Self::Identity {
        *CandidateCutWorkerSchemaExecutor::input_identity(self)
    }

    fn contract_digest(&self, contract: &str) -> Option<Digest256> {
        CandidateCutWorkerSchemaExecutor::contract_digest(self, contract)
    }

    fn resource_closure_bound_to_identity(&self) -> bool {
        true
    }
}

fn required<'a>(ctx: &'a impl RecordRead, name: &str) -> SourceCommandResult<&'a [u8]> {
    ctx.file(&path(name)?)?
        .ok_or(SourceCommandError::Unsupported(
            "required exact source contract or retained bytes absent",
        ))
}
fn strip(value: &str) -> SourceCommandResult<&str> {
    python_strip_unicode16_v1(value, 1_048_576)
        .map_err(|_| SourceCommandError::Invalid("bounded Python string strip"))
}
fn split(name: &str) -> SourceCommandResult<(&str, &str)> {
    name.rsplit_once('/').ok_or(SourceCommandError::Invalid(
        "source path has no owner parent",
    ))
}
pub(crate) fn names(source_path: &str) -> SourceCommandResult<[String; 3]> {
    let (_, base) = split(source_path)?;
    let stem = base
        .strip_suffix(".json")
        .ok_or(SourceCommandError::Invalid("source record basename"))?;
    Ok([
        base.into(),
        format!("{stem}.human-forms.json"),
        HISTORY.into(),
    ])
}
fn texts(v: &JsonValue, key: &str, max: usize) -> SourceCommandResult<Vec<String>> {
    let values = cmd::array(v, key)?;
    if values.len() > max {
        return Err(SourceCommandError::Invalid("scope array budget"));
    }
    let result = values
        .iter()
        .map(|v| {
            v.as_str()
                .map(String::from)
                .ok_or(SourceCommandError::Invalid("scope text"))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    if result.iter().collect::<BTreeSet<_>>().len() != result.len() {
        return Err(SourceCommandError::Invalid("duplicate scope member"));
    }
    Ok(result)
}
fn has(v: &JsonValue, key: &str, name: &str) -> SourceCommandResult<bool> {
    Ok(texts(v, key, 128)?.iter().any(|s| s == name))
}
pub(crate) fn valid_id(id: &str, prefix: &str, form: bool) -> bool {
    let Some(tail) = id.strip_prefix(prefix) else {
        return false;
    };
    !tail.is_empty()
        && tail.as_bytes()[0].is_ascii_lowercase_or_digit()
        && tail.bytes().all(|b| {
            b.is_ascii_lowercase()
                || b.is_ascii_digit()
                || b == b'.'
                || b == b'-'
                || form && b == b'_'
        })
        && (form || !tail.split(['.', '-']).any(str::is_empty))
}
trait LowerOrDigit {
    fn is_ascii_lowercase_or_digit(self) -> bool;
}
impl LowerOrDigit for u8 {
    fn is_ascii_lowercase_or_digit(self) -> bool {
        self.is_ascii_lowercase() || self.is_ascii_digit()
    }
}
fn digest_text(value: &JsonValue) -> SourceCommandResult<&str> {
    let text = value
        .as_str()
        .ok_or(SourceCommandError::Invalid("digest text"))?;
    let suffix = text
        .strip_prefix("sha256:")
        .ok_or(SourceCommandError::Invalid("digest prefix"))?;
    if suffix.len() != 64
        || !suffix
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(SourceCommandError::Invalid("digest grammar"));
    }
    Ok(text)
}
pub(crate) fn configuration(
    ctx: &CommandContext,
) -> SourceCommandResult<(JsonValue, RevisionFamily)> {
    ctx.check()?;
    let config = cmd::parse(&ctx.configuration_raw)?;
    let family = RevisionFamily::parse(cmd::text(&config, "schema_version")?)?;
    let mut keys = vec![
        "schema_version",
        "uid",
        "principal_id",
        "source_root",
        "source_path",
        "authority_ref",
        "allowed_form_ids",
        "allowed_operations",
        "expires_at",
        "record_id",
        "allowed_fields",
    ];
    if family.profile() {
        keys.push("profile_type_id");
    }
    if matches!(
        family,
        RevisionFamily::CorpusFlat
            | RevisionFamily::CorpusSelectedV2
            | RevisionFamily::CorpusSelectedV3
            | RevisionFamily::NativeSelected
    ) {
        keys.push("record_type");
    }
    if family == RevisionFamily::NativeSelected {
        keys.push("record_schema_version");
    }
    cmd::exact_keys(&config, &keys)?;
    if cmd::integer(&config, "uid")? != ctx.effective_uid
        || ["principal_id", "authority_ref"].iter().any(|k| {
            cmd::text(&config, k)
                .and_then(strip)
                .map_or(true, str::is_empty)
        })
    {
        return Err(SourceCommandError::Denied(
            "current Unix account or owner identity",
        ));
    }
    cmd::validate_expiry(cmd::text(&config, "expires_at")?, &ctx.recorded_at)?;
    if !cmd::text(&config, "source_root")?.starts_with('/') {
        return Err(SourceCommandError::Denied("source root must be absolute"));
    }
    let source_path = cmd::text(&config, "source_path")?;
    path(source_path)?;
    let parts: Vec<_> = source_path.split('/').collect();
    if parts.len() < 5
        || parts[..2] != ["ToS", "source-witnesses"]
        || parts
            .iter()
            .any(|p| matches!(*p, "payload" | "local-content" | "catalog" | "owner-local"))
        || !source_path.ends_with(".json")
        || source_path.ends_with(".human-forms.json")
    {
        return Err(SourceCommandError::Denied(
            "exact public metadata source path",
        ));
    }
    if family == RevisionFamily::NativeSelected && parts.iter().any(|p| p.starts_with('.')) {
        return Err(SourceCommandError::Denied(
            "native source path hidden component",
        ));
    }
    let operations = texts(&config, "allowed_operations", 32)?;
    if operations
        .iter()
        .any(|op| op != "record.revise" && !(family.selected() && op == "record.recover"))
    {
        return Err(SourceCommandError::Denied(
            "record revision operation grant",
        ));
    }
    let forms = texts(&config, "allowed_form_ids", 32)?;
    if forms.iter().any(|id| !valid_id(id, "tos.form.", true))
        || family == RevisionFamily::NativeSelected && forms.is_empty()
    {
        return Err(SourceCommandError::Denied("record revision form scope"));
    }
    let allowed = allowed_fields(family, &config)?;
    if texts(&config, "allowed_fields", 128)?
        .iter()
        .any(|field| !allowed.contains(&field.as_str()))
    {
        return Err(SourceCommandError::Denied("record revision field grant"));
    }
    if family == RevisionFamily::Historical {
        let id = cmd::text(&config, "record_id")?;
        let kind = ["historical-event", "historical-process", "historical-state"]
            .into_iter()
            .find(|kind| valid_id(id, &format!("tos.{kind}."), false))
            .ok_or(SourceCommandError::Denied("historical source identity"))?;
        if parts.last() != Some(&format!("{kind}.json").as_str()) {
            return Err(SourceCommandError::Denied("historical typed basename"));
        }
    }
    Ok((config, family))
}

pub(crate) fn history(files: &Package, record: &JsonValue) -> SourceCommandResult<JsonValue> {
    let subject = source_forms::metadata_subject(record)?;
    let value = match files.get(HISTORY) {
        Some(raw) => cmd::parse(raw)?,
        None => cmd::object(vec![
            (
                "schema_version",
                cmd::string("tos_source_revision_history_v1"),
            ),
            ("record_id", cmd::field(&subject, "id")?.clone()),
            ("receipts", JsonValue::Array(vec![])),
        ]),
    };
    cmd::exact_keys(&value, &["schema_version", "record_id", "receipts"])?;
    let v2 = cmd::text(&value, "schema_version")? == "tos_source_revision_history_v2";
    if !v2 && cmd::text(&value, "schema_version")? != "tos_source_revision_history_v1"
        || cmd::field(&value, "record_id")? != cmd::field(&subject, "id")?
    {
        return Err(SourceCommandError::Invalid(
            "record revision history identity or schema",
        ));
    }
    let receipts = cmd::array(&value, "receipts")?;
    if receipts.len() > 128 || files.contains_key(HISTORY) && receipts.is_empty() {
        return Err(SourceCommandError::Invalid(
            "retained revision history capacity or empty ledger",
        ));
    }
    let mut commands = BTreeSet::new();
    let mut previous: Option<&JsonValue> = None;
    for receipt in receipts {
        let selected = receipt.object_get("publication").is_some();
        let mut keys = vec![
            "command_id",
            "request_digest",
            "principal_id",
            "authority_ref",
            "owner_configuration",
            "recorded_at",
            "reason",
            "previous_source",
            "source",
            "previous_revision",
            "archive_path",
            "dependencies",
            "changed_fields",
            "forms",
            "grants_admission",
            "request",
        ];
        if selected {
            keys.push("publication");
        }
        cmd::exact_keys(receipt, &keys)?;
        if selected && !v2 {
            return Err(SourceCommandError::Invalid(
                "selected revision requires v2 history",
            ));
        }
        let request = cmd::field(receipt, "request")?;
        if cmd::text(request, "operation")? != "record.revise" {
            return Err(SourceCommandError::Unsupported(
                "compound parent history requires its typed owner receipt verifier",
            ));
        }
        exact_ref(cmd::field(receipt, "previous_source")?)?;
        exact_ref(cmd::field(receipt, "source")?)?;
        let fields = cmd::field(request, "fields")?
            .as_object()
            .ok_or(SourceCommandError::Invalid("retained fields"))?;
        let mut changed = fields
            .iter()
            .map(|(k, _)| {
                k.as_str()
                    .map(String::from)
                    .ok_or(SourceCommandError::Invalid("retained field name"))
            })
            .collect::<SourceCommandResult<Vec<_>>>()?;
        changed.sort();
        let changed = JsonValue::Array(changed.iter().map(|s| cmd::string(s)).collect());
        if !commands.insert(cmd::text(receipt, "command_id")?)
            || cmd::text(receipt, "request_digest")? != cmd::record_digest(request)?.to_prefixed()
            || cmd::field(receipt, "command_id")? != cmd::field(request, "command_id")?
            || !cmd::same(
                cmd::field(receipt, "previous_source")?,
                cmd::field(request, "expected_source")?,
            )?
            || cmd::field(receipt, "previous_revision")?
                != cmd::field(request, "expected_revision")?
            || cmd::field(receipt, "owner_configuration")?
                != cmd::field(request, "expected_configuration")?
            || cmd::field(receipt, "dependencies")? != cmd::field(request, "expected_dependencies")?
            || cmd::field(receipt, "reason")? != cmd::field(request, "reason")?
            || cmd::field(receipt, "changed_fields")? != &changed
            || cmd::field(receipt, "grants_admission")? != &JsonValue::Bool(false)
            || cmd::field(cmd::field(receipt, "source")?, "id")? != cmd::field(&subject, "id")?
            || cmd::field(cmd::field(receipt, "previous_source")?, "id")?
                != cmd::field(&subject, "id")?
            || cmd::integer(cmd::field(receipt, "previous_source")?, "version")?.checked_add(1)
                != Some(cmd::integer(cmd::field(receipt, "source")?, "version")?)
            || previous
                .map(|p| cmd::same(p, cmd::field(receipt, "previous_source")?))
                .transpose()?
                .is_some_and(|same| !same)
        {
            return Err(SourceCommandError::Conflict(
                "broken retained source revision lineage",
            ));
        }
        // Timestamp parser is shared with the protected owner configuration;
        // a receipt need only be valid, not unexpired.
        tos_validation::retirement_rules::observed_instant_order(
            cmd::text(receipt, "recorded_at")?,
            cmd::text(receipt, "recorded_at")?,
        )
        .map_err(|_| SourceCommandError::Invalid("retained receipt instant"))?;
        if selected {
            let publication = cmd::field(receipt, "publication")?;
            cmd::exact_keys(
                publication,
                &["protocol", "transaction_id", "selected_files"],
            )?;
            if cmd::text(publication, "protocol")? != PROTOCOL {
                return Err(SourceCommandError::Invalid("selected publication protocol"));
            }
            digest_text(cmd::field(publication, "transaction_id")?)?;
            let names = texts(publication, "selected_files", 3)?;
            if names.len() != 3
                || !names.iter().any(|n| n == HISTORY)
                || names.iter().any(|n| n.contains('/') || n.is_empty())
            {
                return Err(SourceCommandError::Invalid("selected publication files"));
            }
        }
        previous = Some(cmd::field(receipt, "source")?);
    }
    if let Some(previous) = previous
        && !cmd::same(previous, &subject)?
    {
        return Err(SourceCommandError::Conflict(
            "current record is not retained revision head",
        ));
    }
    Ok(value)
}

pub(crate) fn read_archive(
    ctx: &impl RecordRead,
    config: &JsonValue,
    receipt: &JsonValue,
) -> SourceCommandResult<(Package, JsonValue)> {
    read_archive_bounded(ctx, config, receipt, None, None)
}

fn read_archive_bounded(
    ctx: &impl RecordRead,
    config: &JsonValue,
    receipt: &JsonValue,
    remaining_read_bytes: Option<u64>,
    remaining_state_bytes: Option<usize>,
) -> SourceCommandResult<(Package, JsonValue)> {
    let location = archive_path(config, cmd::text(receipt, "previous_revision")?)?;
    if cmd::text(receipt, "archive_path")? != location {
        return Err(SourceCommandError::Conflict(
            "archive locator is not derived from exact subject and package",
        ));
    }
    let manifest_raw = required(ctx, &format!("{location}/manifest.json"))?;
    if remaining_read_bytes.is_some_and(|cap| manifest_raw.len() as u64 > cap)
        || remaining_state_bytes.is_some_and(|cap| manifest_raw.len() > cap)
    {
        return Err(SourceCommandError::Unsupported(
            "retained metadata archive manifest budget",
        ));
    }
    let manifest = cmd::parse(manifest_raw)?;
    let manifest_state = canonical_count_v1(
        &manifest,
        CanonicalProfile::SourceCommandInputV1,
        JsonLimits {
            max_bytes: 8_388_608,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| SourceCommandError::Unsupported("retained metadata manifest state budget"))?;
    let selected = cmd::text(&manifest, "schema_version")? == "tos_source_package_archive_v2";
    let mut keys = vec![
        "schema_version",
        "source_path",
        "source",
        "revision",
        "files",
    ];
    if selected {
        keys.push("publication_protocol");
    }
    cmd::exact_keys(&manifest, &keys)?;
    if !selected && cmd::text(&manifest, "schema_version")? != "tos_source_package_archive_v1"
        || selected && cmd::text(&manifest, "publication_protocol")? != PROTOCOL
        || cmd::field(&manifest, "source_path")? != cmd::field(config, "source_path")?
        || !cmd::same(
            cmd::field(&manifest, "source")?,
            cmd::field(receipt, "previous_source")?,
        )?
        || cmd::field(&manifest, "revision")? != cmd::field(receipt, "previous_revision")?
    {
        return Err(SourceCommandError::Conflict(
            "archive manifest binding differs",
        ));
    }
    let refs = cmd::field(&manifest, "files")?
        .as_object()
        .ok_or(SourceCommandError::Invalid("archive files map"))?;
    if refs.is_empty() || refs.len() > 64 {
        return Err(SourceCommandError::Invalid("archive package file budget"));
    }
    let readonly = ctx.writer().is_none();
    let (_, selected_base) = split(cmd::text(config, "source_path")?)?;
    let mut manifest_bindings = Vec::new();
    let mut declared_bytes = 0u64;
    let mut files = Package::new();
    let mut locations = Vec::new();
    let mut blobs = BTreeSet::new();
    let mut copied_bytes = manifest_raw.len() as u64;
    let mut state_bytes = std::mem::size_of::<Package>()
        .checked_add(std::mem::size_of::<JsonValue>())
        .and_then(|sum| sum.checked_add(manifest_state))
        .ok_or(SourceCommandError::Unsupported(
            "retained metadata archive state overflow",
        ))?;
    if remaining_read_bytes.is_some_and(|cap| copied_bytes > cap)
        || remaining_state_bytes.is_some_and(|cap| state_bytes > cap)
    {
        return Err(SourceCommandError::Unsupported(
            "retained metadata archive budget",
        ));
    }
    for (name, binding) in refs {
        let name = name
            .as_str()
            .ok_or(SourceCommandError::Invalid("archive filename"))?;
        if name.contains('/') || name.is_empty() || matches!(name, "." | "..") {
            return Err(SourceCommandError::Invalid("archive basename"));
        }
        cmd::exact_keys(binding, &["blob", "sha256", "bytes"])?;
        let digest = digest_text(cmd::field(binding, "sha256")?)?;
        let blob = cmd::text(binding, "blob")?;
        if blob != format!("{}.blob", &digest[7..]) {
            return Err(SourceCommandError::Invalid("archive blob locator"));
        }
        let declared = cmd::integer(binding, "bytes")?;
        declared_bytes = declared_bytes
            .checked_add(declared)
            .filter(|sum| declared <= 2_097_152 && *sum <= 8_388_608)
            .ok_or(SourceCommandError::Unsupported(
                "archive declared package byte budget",
            ))?;
        manifest_bindings.push((
            JsonString::from_utf8(name),
            cmd::object(vec![
                ("sha256", cmd::string(digest)),
                ("bytes", cmd::number(declared)),
            ]),
        ));
        if readonly
            && selected
            && !names(cmd::text(config, "source_path")?)?
                .iter()
                .any(|allowed| allowed == name)
        {
            return Err(SourceCommandError::Conflict(
                "selected archive exceeds exact metadata scope",
            ));
        }
        if readonly && (name.contains('\\') || name.contains('\0')) {
            return Err(SourceCommandError::Invalid("archive basename"));
        }
        if readonly && name != selected_base {
            continue;
        }
        let blob_path = format!("{location}/{blob}");
        let raw = required(ctx, &blob_path)?;
        copied_bytes = copied_bytes
            .checked_add(raw.len() as u64)
            .filter(|sum| remaining_read_bytes.is_none_or(|cap| *sum <= cap))
            .ok_or(SourceCommandError::Unsupported(
                "retained metadata archive read budget",
            ))?;
        state_bytes = state_bytes
            .checked_add(raw.len())
            .and_then(|sum| sum.checked_add(name.len() + blob_path.len()))
            .and_then(|sum| sum.checked_add(std::mem::size_of::<(String, Vec<u8>)>()))
            .and_then(|sum| sum.checked_add(std::mem::size_of::<(JsonString, JsonValue)>()))
            .filter(|sum| remaining_state_bytes.is_none_or(|cap| *sum <= cap))
            .ok_or(SourceCommandError::Unsupported(
                "retained metadata archive state budget",
            ))?;
        if raw.len() > 2_097_152
            || Digest256::of_bytes(raw).to_prefixed() != digest
            || raw.len() as u64 != cmd::integer(binding, "bytes")?
        {
            return Err(SourceCommandError::Conflict("archive byte binding differs"));
        }
        blobs.insert(blob.to_string());
        files.insert(name.into(), raw.to_vec());
        locations.push((
            JsonString::from_utf8(name),
            cmd::object(vec![
                ("archive_path", cmd::string(&blob_path)),
                ("sha256", cmd::string(digest)),
                ("bytes", cmd::number(raw.len() as u64)),
            ]),
        ));
    }
    let prefix = format!("{location}/");
    for file in ctx.files().iter().filter(|_| !readonly) {
        if let Some(name) = file.path.as_str().strip_prefix(&prefix) {
            if name != "manifest.json" && !blobs.contains(name) {
                return Err(SourceCommandError::Conflict(
                    "archive contains unbound selected file",
                ));
            }
        }
    }
    let source_path = cmd::text(config, "source_path")?;
    let (_, base) = split(source_path)?;
    if selected
        && (!files.contains_key(base)
            || files
                .keys()
                .any(|n| !names(source_path).is_ok_and(|names| names.contains(n))))
    {
        return Err(SourceCommandError::Conflict(
            "selected archive exceeds exact metadata scope",
        ));
    }
    if files.values().map(Vec::len).sum::<usize>() > 8_388_608
        || (if readonly {
            cmd::record_digest(&JsonValue::Object(manifest_bindings))?.to_prefixed()
        } else {
            revision(&files)?
        }) != cmd::text(receipt, "previous_revision")?
    {
        return Err(SourceCommandError::Conflict(
            "archive package digest or budget",
        ));
    }
    let old = cmd::parse(
        files
            .get(base)
            .ok_or(SourceCommandError::Conflict("archive record missing"))?,
    )?;
    if !cmd::same(
        &source_forms::metadata_subject(&old)?,
        cmd::field(receipt, "previous_source")?,
    )? {
        return Err(SourceCommandError::Conflict(
            "archive exact source mismatch",
        ));
    }
    if let Some(request) = receipt.object_get("request") {
        let successor = revised(&old, request)?;
        if !cmd::same(
            &source_forms::metadata_subject(&successor)?,
            cmd::field(receipt, "source")?,
        )? {
            return Err(SourceCommandError::Conflict(
                "retained request does not reconstruct successor",
            ));
        }
    }
    Ok((files, JsonValue::Object(locations)))
}
fn verify_history(
    ctx: &impl RecordRead,
    config: &JsonValue,
    files: &Package,
    record: &JsonValue,
) -> SourceCommandResult<JsonValue> {
    Ok(verify_history_accounted(ctx, config, files, record, None, None, None)?.0)
}

fn verify_history_accounted(
    ctx: &impl RecordRead,
    config: &JsonValue,
    files: &Package,
    record: &JsonValue,
    max_read_bytes: Option<u64>,
    max_state_bytes: Option<usize>,
    operation: Option<(Instant, &AtomicBool)>,
) -> SourceCommandResult<(JsonValue, u64)> {
    let value = if ctx.writer().is_none()
        && cmd::text(record, "schema_version")? == "tos_corpus_record_v1"
    {
        let (deadline, cancelled) = operation.ok_or(SourceCommandError::Invalid(
            "readonly history operation missing",
        ))?;
        let raw = cmd::canonical(record)?;
        let observed = tos_validation::native_compound::inspect_record_history(
            files, &raw, deadline, cancelled,
        )
        .map_err(|reason| SourceCommandError::SchemaExecution {
            path: ctx.schema_source_path().unwrap_or_default(),
            root: "selected metadata retained history".into(),
            reason,
        })?;
        cmd::parse(
            &serde_json::to_vec(&observed)
                .map_err(|_| SourceCommandError::Invalid("history conversion"))?,
        )?
    } else {
        history(files, record)?
    };
    let history_state = canonical_count_v1(
        &value,
        CanonicalProfile::SourceCommandInputV1,
        JsonLimits {
            max_bytes: 8_388_608,
            ..JsonLimits::default()
        },
    )
    .map_err(|_| SourceCommandError::Unsupported("retained metadata history state budget"))?;
    let archive_state = max_state_bytes
        .map(|limit| {
            limit
                .checked_sub(history_state)
                .ok_or(SourceCommandError::Unsupported(
                    "retained metadata history state budget",
                ))
        })
        .transpose()?;
    let (_, base) = split(cmd::text(config, "source_path")?)?;
    let receipts = cmd::array(&value, "receipts")?;
    let mut bytes_read = 0u64;
    for (index, receipt) in receipts.iter().enumerate() {
        if ctx.writer().is_none() {
            let request = cmd::field(receipt, "request")?;
            let operation = cmd::text(request, "operation")?;
            let parent_kind = match operation {
                "work.expression.create" => Some(("work", "expression_claim_refs")),
                "expression.responsibility.attach" => {
                    Some(("expression", "responsibility_claim_refs"))
                }
                "expression.edition.create" => Some(("expression", "embodiment_claim_refs")),
                "item.adopt" => Some(("edition", "exemplar_claim_refs")),
                "collection.work.attach" => Some(("collection", "membership_claim_refs")),
                "record.revise" => None,
                _ => {
                    return Err(SourceCommandError::Unsupported(
                        "retained metadata operation",
                    ));
                }
            };
            if operation == "record.revise" {
                let selected = receipt.object_get("publication").is_some();
                let mut keys = vec![
                    "schema_version",
                    "operation",
                    "fields",
                    "forms",
                    "reason",
                    "command_id",
                    "expected_configuration",
                    "expected_source",
                    "expected_revision",
                    "expected_dependencies",
                ];
                if selected {
                    keys.push("expected_publication");
                }
                cmd::exact_keys(request, &keys)?;
                if cmd::text(request, "schema_version")? != "tos_local_source_command_v1" {
                    return Err(SourceCommandError::Invalid(
                        "retained metadata request schema",
                    ));
                }
                let family = match cmd::text(record, "schema_version")? {
                    "tos_corpus_record_v1" => RevisionFamily::CorpusSelectedV3,
                    "tos_historical_record_v1" => RevisionFamily::Historical,
                    "tos_artifact_source_witness_v1"
                    | "tos_artifact_source_witness_v2"
                    | "tos_scholarly_composite_witness_v1"
                    | "tos_source_link_v1" => RevisionFamily::NativeSelected,
                    _ => RevisionFamily::PublicProfileScope,
                };
                let allowed = allowed_fields(family, config)?;
                let fields = cmd::field(request, "fields")?
                    .as_object()
                    .ok_or(SourceCommandError::Invalid("retained fields"))?;
                if fields.is_empty()
                    || fields
                        .iter()
                        .any(|(key, _)| key.as_str().is_none_or(|name| !allowed.contains(&name)))
                {
                    return Err(SourceCommandError::Invalid(
                        "retained metadata correction fields",
                    ));
                }
            }
            if let Some((kind, field)) = parent_kind {
                if cmd::text(record, "record_type")? != kind
                    || cmd::field(request, "fields")?
                        .as_object()
                        .is_none_or(|fields| {
                            fields.len() != 1 || fields[0].0.as_str() != Some(field)
                        })
                {
                    return Err(SourceCommandError::Conflict(
                        "compound history selected parent differs",
                    ));
                }
            }
            if receipt.object_get("publication").is_some() {
                let mut expected = names(cmd::text(config, "source_path")?)?;
                expected.sort();
                if texts(cmd::field(receipt, "publication")?, "selected_files", 3)? != expected {
                    return Err(SourceCommandError::Conflict(
                        "history selected another metadata unit",
                    ));
                }
            }
        }
        let remaining_read = max_read_bytes.map(|limit| limit.saturating_sub(bytes_read));
        let (archived, _) =
            read_archive_bounded(ctx, config, receipt, remaining_read, archive_state)?;
        let manifest = format!("{}/manifest.json", cmd::text(receipt, "archive_path")?);
        bytes_read = bytes_read
            .checked_add(required(ctx, &manifest)?.len() as u64)
            .and_then(|sum| {
                archived
                    .values()
                    .try_fold(sum, |sum, raw| sum.checked_add(raw.len() as u64))
            })
            .ok_or(SourceCommandError::Unsupported(
                "record history read byte overflow",
            ))?;
        if max_read_bytes.is_some_and(|limit| bytes_read > limit) {
            return Err(SourceCommandError::Unsupported(
                "retained metadata read budget",
            ));
        }
        let previous = cmd::parse(
            archived
                .get(base)
                .ok_or(SourceCommandError::Conflict("archived source missing"))?,
        )?;
        if ctx.writer().is_none() {
            continue;
        }
        let retained = history(&archived, &previous)?;
        let prefix = cmd::array(&retained, "receipts")?;
        let mut same_prefix = prefix.len() == index;
        if same_prefix {
            for (left, right) in prefix.iter().zip(&receipts[..index]) {
                if !cmd::same(left, right)? {
                    same_prefix = false;
                    break;
                }
            }
        }
        if !same_prefix {
            return Err(SourceCommandError::Conflict(
                "retained predecessor history prefix differs",
            ));
        }
    }
    Ok((value, bytes_read))
}
fn inspect(
    cut: Option<&CorpusCutReader>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &CommandContext,
    config: &JsonValue,
    family: RevisionFamily,
) -> SourceCommandResult<Inspection> {
    let source_path = cmd::text(config, "source_path")?;
    let (_, base) = split(source_path)?;
    let files = package(ctx, source_path, family.selected(), None)?;
    let record = cmd::parse(
        files
            .get(base)
            .ok_or(SourceCommandError::Invalid("record missing"))?,
    )?;
    let (profile, schemas, native_identity_snapshot, native_text_snapshot) = profile(
        cut, worker, deadline, cancelled, ctx, config, family, &record,
    )?;
    let subject = source_forms::metadata_subject(&record)?;
    let history = verify_history(ctx, config, &files, &record)?;
    Ok(Inspection {
        files,
        record,
        subject,
        history,
        profile,
        schemas,
        native_identity_snapshot,
        native_text_snapshot,
    })
}
fn dependencies(
    ctx: &impl RecordRead,
    config: &JsonValue,
    family: RevisionFamily,
    inspection: &Inspection,
) -> SourceCommandResult<String> {
    let mut names: BTreeSet<String> = DEPENDENCIES.iter().map(|s| s.to_string()).collect();
    if !family.profile() {
        names.extend(inspection.schemas.iter().cloned());
    }
    if family.selected() {
        names.extend(SELECTED_DEPENDENCIES.iter().map(|s| s.to_string()));
    }
    if family == RevisionFamily::NativeSelected {
        names.extend(["mechanics/growth-cycle/parts/branch-growth-cycle/scripts/source_native_metadata_commands.py".into(),"scripts/build_source_witness_catalog.py".into()]);
    }
    let mut entries = names
        .iter()
        .map(|name| {
            Ok((
                JsonString::from_utf8(name),
                cmd::string(&Digest256::of_bytes(required(ctx, name)?).to_prefixed()),
            ))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    if family.profile() {
        let contracts = inspection
            .schemas
            .iter()
            .map(|name| {
                Ok((
                    JsonString::from_utf8(name),
                    cmd::string(&Digest256::of_bytes(required(ctx, name)?).to_prefixed()),
                ))
            })
            .collect::<SourceCommandResult<Vec<_>>>()?;
        entries.push((
            JsonString::from_utf8("source_contracts"),
            JsonValue::Object(contracts),
        ));
    }
    if let Some(snapshot) = &inspection.native_identity_snapshot {
        entries.push((
            JsonString::from_utf8("native_semantic_identity_snapshot"),
            cmd::string(snapshot),
        ));
    }
    if let Some(snapshot) = &inspection.native_text_snapshot {
        entries.push((
            JsonString::from_utf8("native_text_binding_snapshot"),
            cmd::string(snapshot),
        ));
        entries.push((
            JsonString::from_utf8("native_binding_implementation"),
            JsonValue::Object(
                [
                    "scripts/native_text_binding.py",
                    "scripts/source_owner_context.py",
                ]
                .iter()
                .map(|name| {
                    Ok((
                        JsonString::from_utf8(name),
                        cmd::string(&Digest256::of_bytes(required(ctx, name)?).to_prefixed()),
                    ))
                })
                .collect::<SourceCommandResult<Vec<_>>>()?,
            ),
        ));
    }
    let _ = config;
    Ok(cmd::record_digest(&JsonValue::Object(entries))?.to_prefixed())
}
fn revised(record: &JsonValue, request: &JsonValue) -> SourceCommandResult<JsonValue> {
    let mut revised = record.clone();
    for (key, value) in cmd::field(request, "fields")?
        .as_object()
        .ok_or(SourceCommandError::Invalid("revision fields"))?
    {
        cmd::set(
            &mut revised,
            key.as_str()
                .ok_or(SourceCommandError::Invalid("revision field name"))?,
            value.clone(),
        )?;
    }
    let version = cmd::integer(record, "record_version")?
        .checked_add(1)
        .ok_or(SourceCommandError::Invalid("record version exhausted"))?;
    cmd::set(&mut revised, "record_version", cmd::number(version))?;
    Ok(revised)
}
fn proposal<W, C>(
    cut: Option<&CorpusCutReader>,
    worker: &mut W,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &C,
    config: &JsonValue,
    family: RevisionFamily,
    inspection: &Inspection,
    request: &JsonValue,
    scope_operation: &str,
) -> SourceCommandResult<(
    JsonValue,
    JsonValue,
    Package,
    Vec<JsonValue>,
    Vec<JsonValue>,
)>
where
    W: RecordSchemaWorker,
    C: RecordRead<Identity = W::Identity>,
{
    scope(config, request, scope_operation)?;
    let revised = revised(&inspection.record, request)?;
    let (_, schemas, _, _) = profile(
        cut, worker, deadline, cancelled, ctx, config, family, &revised,
    )?;
    if schemas != inspection.schemas {
        return Err(SourceCommandError::Denied(
            "correction changed source schema selection",
        ));
    }
    if family == RevisionFamily::NativeSelected && cmd::text(config, "record_type")? == "artifact" {
        for field in ["basis", "provider_independent"] {
            if cmd::field(cmd::field(&inspection.record, "path_identity")?, field)?
                != cmd::field(cmd::field(&revised, "path_identity")?, field)?
            {
                return Err(SourceCommandError::Denied(
                    "physical identity path basis changed",
                ));
            }
        }
    }
    let source_path = cmd::text(config, "source_path")?;
    let [basename, formname, _] = names(source_path)?;
    let current = inspection
        .files
        .get(&formname)
        .map(|b| cmd::parse(b))
        .transpose()?;
    if let Some(current) = &current {
        validate_forms(worker, deadline, cancelled, ctx, current)?;
        let selected = cmd::array(request, "forms")?
            .iter()
            .map(|v| cmd::text(v, "form_id"))
            .collect::<SourceCommandResult<BTreeSet<_>>>()?;
        for form in cmd::array(current, "forms")? {
            if !selected.contains(cmd::text(form, "form_id")?) {
                return Err(SourceCommandError::Invalid(
                    "revision must rebind every current form",
                ));
            }
        }
    }
    let subject = source_forms::metadata_subject(&revised)?;
    let changes = cmd::array(request, "forms")?
        .iter()
        .map(|selection| {
            source_forms::prepare_form_change(
                &revised,
                current.as_ref(),
                cmd::text(config, "principal_id")?,
                cmd::text(selection, "form_id")?,
                cmd::text(selection, "field_id")?,
            )
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let forms = source_forms::apply_form_changes(current.as_ref(), &subject, &changes)?;
    validate_forms(worker, deadline, cancelled, ctx, &forms)?;
    let views = source_forms::materialize_source_forms(&revised, &forms)?;
    if views
        .iter()
        .any(|v| cmd::text(v, "state").ok() != Some("ready"))
        || !views
            .iter()
            .any(|v| cmd::text(v, "role").ok() == Some("name"))
    {
        return Err(SourceCommandError::Invalid(
            "revision forms must be ready copies including a name",
        ));
    }
    let refs = changes
        .iter()
        .map(|change| cmd::reference(cmd::field(change, "form")?, "form_id", "form_version"))
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let mut output = inspection.files.clone();
    let raw = cmd::published(&revised)?;
    if raw.len() > 1_048_576 {
        return Err(SourceCommandError::Invalid(
            "revised metadata reader budget",
        ));
    }
    output.insert(basename, raw);
    output.insert(formname, cmd::published(&forms)?);
    Ok((revised, subject, output, views, refs))
}
fn validate_forms<W, C>(
    worker: &mut W,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &C,
    forms: &JsonValue,
) -> SourceCommandResult<()>
where
    W: RecordSchemaWorker,
    C: RecordRead<Identity = W::Identity>,
{
    let refs = [
        "ToS/contracts/knowledge-assessment.schema.json",
        "ToS/contracts/human-form.schema.json",
        "ToS/contracts/human-form-set.schema.json",
    ]
    .map(String::from);
    let schema_source_path = ctx.schema_source_path()?;
    schema_at(
        worker,
        deadline,
        cancelled,
        ctx,
        &refs,
        "ToS/contracts/human-form-set.schema.json",
        forms,
        Some(&schema_source_path),
        false,
    )?;
    tos_validation::source_forms::inspect_lineage_raw(&cmd::published(forms)?)
        .map_err(|_| SourceCommandError::Conflict("human form retained lineage"))?;
    Ok(())
}
fn result(
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &CommandContext,
    config: &JsonValue,
    family: RevisionFamily,
    inspection: &Inspection,
    publication: Option<&RevisionPublication>,
    receipt: Option<&JsonValue>,
    replayed: bool,
) -> SourceCommandResult<JsonValue> {
    let [_, formname, _] = names(cmd::text(config, "source_path")?)?;
    let views = match inspection.files.get(&formname) {
        Some(raw) => {
            let forms = cmd::parse(raw)?;
            validate_forms(worker, deadline, cancelled, ctx, &forms)?;
            source_forms::materialize_source_forms(&inspection.record, &forms)?
        }
        None => vec![],
    };
    let mut operations = vec!["describe", "prepare-revise", "record.revise"];
    let mut supported = vec!["record.revise"];
    if family.selected() {
        operations.push("record.recover");
        supported.push("record.recover");
    }
    operations.push("inspect-version");
    let mut value = cmd::object(vec![
        (
            "schema_version",
            cmd::string(if family.selected() {
                "tos_local_source_revision_result_v2"
            } else {
                "tos_local_source_revision_result_v1"
            }),
        ),
        ("authentication", cmd::string("local-unix-account")),
        (
            "owner_configuration",
            cmd::string(&cmd::record_digest(config)?.to_prefixed()),
        ),
        ("source_path", cmd::field(config, "source_path")?.clone()),
        ("source", inspection.subject.clone()),
        ("revision", cmd::string(&revision(&inspection.files)?)),
        (
            "command_operations",
            JsonValue::Array(operations.into_iter().map(cmd::string).collect()),
        ),
        (
            "supported_operations",
            JsonValue::Array(supported.into_iter().map(cmd::string).collect()),
        ),
        (
            "allowed_operations",
            cmd::field(config, "allowed_operations")?.clone(),
        ),
        (
            "allowed_fields",
            cmd::field(config, "allowed_fields")?.clone(),
        ),
        (
            "allowed_form_ids",
            cmd::field(config, "allowed_form_ids")?.clone(),
        ),
        ("receipt", receipt.cloned().unwrap_or(JsonValue::Null)),
        ("replayed", JsonValue::Bool(replayed)),
        ("grants_admission", JsonValue::Bool(false)),
        ("materializations", JsonValue::Array(views)),
    ]);
    if family.selected() {
        let publication = publication.ok_or(SourceCommandError::Unsupported(
            "selected cooperating-reader publication evidence required",
        ))?;
        cmd::set(
            &mut value,
            "publication_snapshot",
            publication
                .token
                .as_deref()
                .map(cmd::string)
                .unwrap_or(JsonValue::Null),
        )?;
        cmd::set(&mut value, "publication_protocol", cmd::string(PROTOCOL))?;
        let mut selected = names(cmd::text(config, "source_path")?)?.to_vec();
        selected.sort();
        cmd::set(
            &mut value,
            "selected_files",
            JsonValue::Array(selected.iter().map(|v| cmd::string(v)).collect()),
        )?;
        cmd::set(&mut value, "recovery", JsonValue::Null)?;
    }
    if family.profile() {
        cmd::set(
            &mut value,
            "profile_type_id",
            cmd::field(config, "profile_type_id")?.clone(),
        )?;
        cmd::set(
            &mut value,
            "source_record_profile",
            inspection.profile.clone(),
        )?;
    } else if family != RevisionFamily::Historical {
        cmd::set(
            &mut value,
            "record_type",
            cmd::field(&inspection.profile, "record_type")?.clone(),
        )?;
        cmd::set(&mut value, "source_profile", inspection.profile.clone())?;
    }
    Ok(value)
}

pub(crate) fn transaction_id(request: &JsonValue) -> SourceCommandResult<String> {
    let binding = cmd::object(vec![
        ("command_id", cmd::field(request, "command_id")?.clone()),
        (
            "expected_configuration",
            cmd::field(request, "expected_configuration")?.clone(),
        ),
    ]);
    let mut raw = cmd::canonical(&binding)?;
    raw.extend(cmd::canonical(request)?);
    Ok(Digest256::of_bytes(&raw).to_prefixed())
}
fn receipt(
    config: &JsonValue,
    family: RevisionFamily,
    inspection: &Inspection,
    request: &JsonValue,
    subject: &JsonValue,
    refs: &[JsonValue],
    instant: &str,
) -> SourceCommandResult<JsonValue> {
    let fields = cmd::field(request, "fields")?
        .as_object()
        .ok_or(SourceCommandError::Invalid("revision fields"))?;
    let mut changed = fields
        .iter()
        .map(|(k, _)| {
            k.as_str()
                .map(String::from)
                .ok_or(SourceCommandError::Invalid("revision field name"))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    changed.sort();
    let mut value = cmd::object(vec![
        ("command_id", cmd::field(request, "command_id")?.clone()),
        (
            "request_digest",
            cmd::string(&cmd::record_digest(request)?.to_prefixed()),
        ),
        ("principal_id", cmd::field(config, "principal_id")?.clone()),
        (
            "authority_ref",
            cmd::field(config, "authority_ref")?.clone(),
        ),
        (
            "owner_configuration",
            cmd::field(request, "expected_configuration")?.clone(),
        ),
        ("recorded_at", cmd::string(instant)),
        ("reason", cmd::field(request, "reason")?.clone()),
        ("previous_source", inspection.subject.clone()),
        ("source", subject.clone()),
        (
            "previous_revision",
            cmd::field(request, "expected_revision")?.clone(),
        ),
        (
            "archive_path",
            cmd::string(&archive_path(
                config,
                cmd::text(request, "expected_revision")?,
            )?),
        ),
        (
            "dependencies",
            cmd::field(request, "expected_dependencies")?.clone(),
        ),
        (
            "changed_fields",
            JsonValue::Array(changed.iter().map(|s| cmd::string(s)).collect()),
        ),
        ("forms", JsonValue::Array(refs.to_vec())),
        ("grants_admission", JsonValue::Bool(false)),
        ("request", request.clone()),
    ]);
    if family.selected() {
        let mut files = names(cmd::text(config, "source_path")?)?.to_vec();
        files.sort();
        cmd::set(
            &mut value,
            "publication",
            cmd::object(vec![
                ("protocol", cmd::string(PROTOCOL)),
                ("transaction_id", cmd::string(&transaction_id(request)?)),
                (
                    "selected_files",
                    JsonValue::Array(files.iter().map(|s| cmd::string(s)).collect()),
                ),
            ]),
        )?;
    }
    Ok(value)
}
fn successor<W, C>(
    cut: Option<&CorpusCutReader>,
    worker: &mut W,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &C,
    config: &JsonValue,
    family: RevisionFamily,
    inspection: &Inspection,
    request: &JsonValue,
    instant: &str,
    scope_operation: &str,
) -> SourceCommandResult<(Inspection, JsonValue)>
where
    W: RecordSchemaWorker,
    C: RecordRead<Identity = W::Identity>,
{
    if cmd::array(&inspection.history, "receipts")?.len() >= 128 {
        return Err(SourceCommandError::Invalid(
            "source revision history capacity reached",
        ));
    }
    let (record, subject, mut files, _, refs) = proposal(
        cut,
        worker,
        deadline,
        cancelled,
        ctx,
        config,
        family,
        inspection,
        request,
        scope_operation,
    )?;
    let receipt = receipt(
        config, family, inspection, request, &subject, &refs, instant,
    )?;
    let mut history = inspection.history.clone();
    let mut receipts = cmd::array(&history, "receipts")?.to_vec();
    receipts.push(receipt.clone());
    if family.selected() {
        cmd::set(
            &mut history,
            "schema_version",
            cmd::string("tos_source_revision_history_v2"),
        )?;
    }
    cmd::set(&mut history, "receipts", JsonValue::Array(receipts))?;
    files.insert(HISTORY.into(), cmd::published(&history)?);
    if files.len() > 64
        || files.values().any(|b| b.len() > 2_097_152)
        || files.values().map(Vec::len).sum::<usize>() > 8_388_608
    {
        return Err(SourceCommandError::Invalid("revised package budget"));
    }
    Ok((
        Inspection {
            files,
            record,
            subject,
            history,
            profile: inspection.profile.clone(),
            schemas: inspection.schemas.clone(),
            native_identity_snapshot: inspection.native_identity_snapshot.clone(),
            native_text_snapshot: inspection.native_text_snapshot.clone(),
        },
        receipt,
    ))
}
fn source_changes(
    ctx: &CommandContext,
    config: &JsonValue,
    output: &Package,
) -> SourceCommandResult<Vec<SourceChange>> {
    let source_path = cmd::text(config, "source_path")?;
    let (parent, _) = split(source_path)?;
    let selected = names(source_path)?;
    let mut changes = Vec::new();
    for (name, raw) in output {
        let path = path(&format!("{parent}/{name}"))?;
        let before = ctx.file(&path)?;
        // Flat packages retain unrelated siblings in their predecessor archive.
        // Byte-identical siblings are preservation, not publication writes.
        if !selected.contains(name) && before == Some(raw.as_slice()) {
            continue;
        }
        changes.push(SourceChange {
            before: before.map(Digest256::of_bytes),
            path,
            after: Some(raw.clone()),
        });
    }
    Ok(changes)
}
fn retain_archive(
    ctx: &CommandContext,
    config: &JsonValue,
    family: RevisionFamily,
    inspection: &Inspection,
) -> SourceCommandResult<Vec<SourceChange>> {
    let revision = revision(&inspection.files)?;
    let location = archive_path(config, &revision)?;
    let source_path = cmd::text(config, "source_path")?;
    let mut manifest = cmd::object(vec![
        (
            "schema_version",
            cmd::string(if family.selected() {
                "tos_source_package_archive_v2"
            } else {
                "tos_source_package_archive_v1"
            }),
        ),
        ("source_path", cmd::string(source_path)),
        ("source", inspection.subject.clone()),
        ("revision", cmd::string(&revision)),
        ("files", file_refs(&inspection.files, true)),
    ]);
    if family.selected() {
        cmd::set(&mut manifest, "publication_protocol", cmd::string(PROTOCOL))?;
    }
    let mut writes = BTreeMap::new();
    for raw in inspection.files.values() {
        writes.insert(
            format!("{}.blob", Digest256::of_bytes(raw).to_hex()),
            raw.clone(),
        );
    }
    writes.insert("manifest.json".into(), cmd::published(&manifest)?);
    let existing_manifest = ctx
        .file(&path(&format!("{location}/manifest.json"))?)?
        .is_some();
    if existing_manifest {
        let stub = cmd::object(vec![
            ("previous_revision", cmd::string(&revision)),
            ("previous_source", inspection.subject.clone()),
            ("archive_path", cmd::string(&location)),
        ]);
        if read_archive(ctx, config, &stub)?.0 != inspection.files {
            return Err(SourceCommandError::Conflict(
                "existing archive differs from predecessor bytes",
            ));
        }
        return Ok(vec![]);
    }
    let mut changes = Vec::new();
    for (name, raw) in writes {
        let path = path(&format!("{location}/{name}"))?;
        if ctx.file(&path)?.is_some() {
            return Err(SourceCommandError::Conflict(
                "partial archive cannot be overwritten",
            ));
        }
        changes.push(SourceChange {
            path,
            before: None,
            after: Some(raw),
        });
    }
    Ok(changes)
}
fn retained_package(
    files: &[SourceFile],
    config: &JsonValue,
    require_all: bool,
) -> SourceCommandResult<Package> {
    let source_path = cmd::text(config, "source_path")?;
    let (parent, base) = split(source_path)?;
    let names = names(source_path)?;
    let mut result = Package::new();
    for file in files {
        let prefix = format!("{parent}/");
        let name = file
            .path
            .as_str()
            .strip_prefix(&prefix)
            .ok_or(SourceCommandError::Denied(
                "retained transaction outside selected home",
            ))?;
        if !names.iter().any(|s| s == name)
            || result.insert(name.into(), file.raw.clone()).is_some()
        {
            return Err(SourceCommandError::Denied(
                "retained transaction outside exact selected files",
            ));
        }
    }
    if !result.contains_key(base) || require_all && result.len() != 3 {
        return Err(SourceCommandError::Invalid(
            "retained selected before/after cardinality",
        ));
    }
    if result.values().any(|r| r.len() > 2_097_152)
        || result.values().map(Vec::len).sum::<usize>() > 8_388_608
    {
        return Err(SourceCommandError::Invalid(
            "retained transaction byte budget",
        ));
    }
    Ok(result)
}

fn artifact_history_evidence_config(
    source_root: &str,
    source_path: &str,
    record_id: &str,
    correction: &JsonValue,
) -> SourceCommandResult<JsonValue> {
    if !source_root.starts_with('/') || source_root.len() > 4096 || source_root.contains('\0') {
        return Err(SourceCommandError::Denied(
            "Artifact evidence source root must be bounded and absolute",
        ));
    }
    let request = cmd::field(correction, "request")?;
    let allowed_form_ids = cmd::array(request, "forms")?
        .iter()
        .map(|selection| Ok(cmd::string(cmd::text(selection, "form_id")?)))
        .collect::<SourceCommandResult<Vec<_>>>()?;
    Ok(cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_local_native_metadata_revision_owner_v1"),
        ),
        ("source_root", cmd::string(source_root)),
        ("source_path", cmd::string(source_path)),
        ("record_id", cmd::string(record_id)),
        ("record_type", cmd::string("artifact")),
        (
            "record_schema_version",
            cmd::string("tos_artifact_source_witness_v2"),
        ),
        (
            "principal_id",
            cmd::field(correction, "principal_id")?.clone(),
        ),
        (
            "authority_ref",
            cmd::field(correction, "authority_ref")?.clone(),
        ),
        (
            "allowed_operations",
            JsonValue::Array(vec![cmd::string("record.revise")]),
        ),
        (
            "allowed_fields",
            JsonValue::Array(
                [
                    "bibliography",
                    "find_context",
                    "path_identity",
                    "physical_description",
                ]
                .into_iter()
                .map(cmd::string)
                .collect(),
            ),
        ),
        ("allowed_form_ids", JsonValue::Array(allowed_form_ids)),
    ]))
}

fn revision_publication_state_bytes(
    publication: &RevisionPublication,
) -> SourceCommandResult<usize> {
    let mut state = std::mem::size_of::<RevisionPublication>()
        .checked_add(
            publication
                .token
                .as_ref()
                .map_or(0, |token| token.capacity()),
        )
        .and_then(|state| {
            state.checked_add(
                publication
                    .transactions
                    .capacity()
                    .checked_mul(std::mem::size_of::<RetainedRevisionTransaction>())?,
            )
        })
        .ok_or(SourceCommandError::Unsupported(
            "Artifact history publication state overflow",
        ))?;
    for transaction in &publication.transactions {
        state = state
            .checked_add(transaction.transaction_id.capacity())
            .and_then(|sum| sum.checked_add(transaction.authorization_raw.capacity()))
            .and_then(|sum| {
                sum.checked_add(
                    transaction
                        .before
                        .capacity()
                        .checked_mul(std::mem::size_of::<SourceFile>())?,
                )
            })
            .and_then(|sum| {
                sum.checked_add(
                    transaction
                        .after
                        .capacity()
                        .checked_mul(std::mem::size_of::<SourceFile>())?,
                )
            })
            .ok_or(SourceCommandError::Unsupported(
                "Artifact history publication state overflow",
            ))?;
        for file in transaction.before.iter().chain(&transaction.after) {
            state = state
                .checked_add(file.path.as_str().len())
                .and_then(|sum| sum.checked_add(file.raw.capacity()))
                .ok_or(SourceCommandError::Unsupported(
                    "Artifact history publication state overflow",
                ))?;
        }
    }
    Ok(state)
}

fn reconstruct_transaction<W, C>(
    cut: Option<&CorpusCutReader>,
    worker: &mut W,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &C,
    config: &JsonValue,
    family: RevisionFamily,
    transaction: &RetainedRevisionTransaction,
    mode: TransactionReconstructionMode,
    scope_operation: &str,
) -> SourceCommandResult<(JsonValue, Inspection, Inspection, JsonValue)>
where
    W: RecordSchemaWorker,
    C: RecordRead<Identity = W::Identity>,
{
    if mode == TransactionReconstructionMode::ArtifactHistoryEvidence
        && (family != RevisionFamily::NativeSelected
            || cmd::text(config, "record_type")? != "artifact")
    {
        return Err(SourceCommandError::Denied(
            "historical evidence replay is restricted to native Artifacts",
        ));
    }
    let authorization = cmd::parse(&transaction.authorization_raw)?;
    cmd::exact_keys(
        &authorization,
        &[
            "schema_version",
            "principal_id",
            "authority_ref",
            "source_path",
            "record_id",
            "record_type",
            "request",
        ],
    )?;
    if cmd::text(&authorization, "schema_version")?
        != "tos_selected_metadata_revision_authorization_v1"
    {
        return Err(SourceCommandError::Denied("pending transaction adapter"));
    }
    for field in [
        "principal_id",
        "authority_ref",
        "source_path",
        "record_id",
        "record_type",
    ] {
        if cmd::field(&authorization, field)? != cmd::field(config, field)? {
            return Err(SourceCommandError::Denied(
                "retained transaction current owner scope differs",
            ));
        }
    }
    let original = cmd::field(&authorization, "request")?;
    if request(original, family)? != "record.revise"
        || transaction.transaction_id != transaction_id(original)?
    {
        return Err(SourceCommandError::Conflict(
            "retained transaction request identity",
        ));
    }
    scope(config, original, scope_operation)?;
    if mode == TransactionReconstructionMode::CurrentOwner
        && cmd::text(original, "expected_configuration")?
            != cmd::record_digest(config)?.to_prefixed()
    {
        return Err(SourceCommandError::Conflict(
            "retained transaction configuration changed",
        ));
    }
    let files = retained_package(&transaction.before, config, false)?;
    let (_, base) = split(cmd::text(config, "source_path")?)?;
    let record = cmd::parse(
        files
            .get(base)
            .ok_or(SourceCommandError::Invalid("retained record missing"))?,
    )?;
    let (profile, schemas, native_identity_snapshot, native_text_snapshot) = profile(
        cut, worker, deadline, cancelled, ctx, config, family, &record,
    )?;
    let subject = source_forms::metadata_subject(&record)?;
    let retained = match mode {
        TransactionReconstructionMode::CurrentOwner => {
            verify_history(ctx, config, &files, &record)?
        }
        // Artifact evidence replay first binds this exact package and history
        // to the cut-native observation, whose typed reader verifies every
        // retained archive and predecessor history prefix.
        TransactionReconstructionMode::ArtifactHistoryEvidence => history(&files, &record)?,
    };
    let before = Inspection {
        files,
        record,
        subject,
        history: retained,
        profile,
        schemas,
        native_identity_snapshot,
        native_text_snapshot,
    };
    let output = retained_package(&transaction.after, config, true)?;
    let retained_history = history(
        &output,
        &cmd::parse(
            output
                .get(base)
                .ok_or(SourceCommandError::Invalid("retained successor missing"))?,
        )?,
    )?;
    let receipt =
        cmd::array(&retained_history, "receipts")?
            .last()
            .ok_or(SourceCommandError::Invalid(
                "retained successor receipt missing",
            ))?;
    if cmd::array(&retained_history, "receipts")?.len()
        != cmd::array(&before.history, "receipts")?.len() + 1
    {
        return Err(SourceCommandError::Conflict(
            "retained successor must append exactly one receipt",
        ));
    }
    if !cmd::same(cmd::field(original, "expected_source")?, &before.subject)?
        || cmd::text(original, "expected_revision")? != revision(&before.files)?
        || (mode == TransactionReconstructionMode::CurrentOwner
            && cmd::text(original, "expected_dependencies")?
                != dependencies(ctx, config, family, &before)?)
    {
        return Err(SourceCommandError::Conflict(
            "retained predecessor or dependencies differ",
        ));
    }
    let (after, reconstructed) = successor(
        cut,
        worker,
        deadline,
        cancelled,
        ctx,
        config,
        family,
        &before,
        original,
        cmd::text(receipt, "recorded_at")?,
        scope_operation,
    )?;
    if output != after.files || !cmd::same(receipt, &reconstructed)? {
        return Err(SourceCommandError::Conflict(
            "retained bytes do not reconstruct exact delegated successor",
        ));
    }
    // In Artifact evidence mode the caller's same-cut native history read has
    // already checked the exact predecessor archive package revision/source,
    // every archive blob, and each archived receipt prefix. The publication
    // replay below independently binds this request's predecessor revision to
    // that archive identity, so another archive read here would duplicate the
    // same custody proof against a different adapter surface.
    if mode == TransactionReconstructionMode::CurrentOwner
        && read_archive(ctx, config, &reconstructed)?.0 != before.files
    {
        return Err(SourceCommandError::Conflict(
            "retained transaction predecessor archive differs",
        ));
    }
    Ok((original.clone(), before, after, reconstructed))
}

/// Replay every committed selected Artifact correction using the evidence-only
/// context from the maintained Artifact creation verifier. The active owner
/// configuration and present dependency freshness remain exclusive to the
/// ordinary writer/recovery path above.
pub(crate) fn replay_artifact_corrections_from_cut(
    input: &RecordVersionReadInput<'_>,
    source_root: &str,
    cut: &CorpusCutReader,
    native_history: &NativeRecordHistoryReadObservation,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ArtifactCorrectionReplayObservation> {
    input.check()?;
    if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(SourceCommandError::Unsupported(
            "Artifact correction replay deadline or cancellation",
        ));
    }
    if cut.current().revision() != input.source_revision
        || worker.source_revision() != input.source_revision
        || native_history.source_revision() != input.source_revision
    {
        return Err(SourceCommandError::Conflict(
            "Artifact correction replay source revisions differ",
        ));
    }
    let current_membership = cut
        .stream(input.source_revision)
        .map_err(|_| {
            SourceCommandError::Unsupported(
                "Artifact correction replay source-cut membership unavailable",
            )
        })?
        .expectation();
    if native_history.current_membership() != current_membership {
        return Err(SourceCommandError::Conflict(
            "Artifact correction replay membership differs from exact source cut",
        ));
    }

    replay_artifact_corrections_with_context(
        input,
        source_root,
        Some(cut),
        current_membership,
        native_history,
        worker,
        deadline,
        cancelled,
    )
}

fn replay_artifact_corrections_with_context<I, W, C>(
    ctx: &C,
    source_root: &str,
    cut: Option<&CorpusCutReader>,
    current_membership: SourceMembershipV1,
    native_history: &NativeRecordHistoryReadObservation<I>,
    worker: &mut W,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ArtifactCorrectionReplayObservation<I>>
where
    I: Copy + Eq,
    W: RecordSchemaWorker<Identity = I>,
    C: RecordRead<Identity = I>,
{
    ctx.check()?;
    if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(SourceCommandError::Unsupported(
            "Artifact correction replay deadline or cancellation",
        ));
    }
    if worker.input_identity() != ctx.input_identity()
        || *native_history.input_identity() != ctx.input_identity()
    {
        return Err(SourceCommandError::Conflict(
            "Artifact correction replay typed input identities differ",
        ));
    }
    if native_history.current_membership() != current_membership {
        return Err(SourceCommandError::Conflict(
            "Artifact correction replay membership differs from completed input records",
        ));
    }

    let source_path = native_history.record_path();
    path(source_path)?;
    if source_path.len() > 1024
        || source_path.split('/').count() < 5
        || !source_path.starts_with("ToS/source-witnesses/artifacts/")
        || source_path.rsplit('/').next() != Some("artifact-witness.json")
        || source_path.split('/').any(|part| {
            part.starts_with('.')
                || matches!(
                    part,
                    "payload" | "private" | "local-content" | "catalog" | "owner-local"
                )
        })
        || native_history.identity_field() != "artifact_id"
    {
        return Err(SourceCommandError::Denied(
            "Artifact correction replay requires its exact public record path",
        ));
    }
    if ctx.schema_source_path()? != source_path {
        return Err(SourceCommandError::Conflict(
            "Artifact correction replay schema path differs from readonly source input",
        ));
    }
    let expected_names = names(source_path)?;
    let selected = native_history.selected_package();
    if selected.len() > expected_names.len()
        || selected.keys().any(|name| !expected_names.contains(name))
    {
        return Err(SourceCommandError::Conflict(
            "Artifact correction replay selected package exceeds its exact files",
        ));
    }
    for name in &expected_names {
        let full_path = path(&format!("{}/{}", split(source_path)?.0, name))?;
        if selected.get(name).map(Vec::as_slice) != ctx.file(&full_path)? {
            return Err(SourceCommandError::Conflict(
                "Artifact correction replay package differs from captured command files",
            ));
        }
    }
    let (_, base) = split(source_path)?;
    let current_record_raw = selected.get(base).ok_or(SourceCommandError::Invalid(
        "Artifact current record missing",
    ))?;
    let current_record = cmd::parse(current_record_raw)?;
    if cmd::text(&current_record, "schema_version")? != "tos_artifact_source_witness_v2"
        || cmd::text(&current_record, "artifact_id")? != native_history.identity()
    {
        return Err(SourceCommandError::Conflict(
            "Artifact correction replay identity or fixed schema differs",
        ));
    }
    let current_history = history(selected, &current_record)?;
    let corrections = cmd::array(&current_history, "receipts")?;
    if corrections.len() != native_history.history_receipt_count()
        || native_history.transactions().len() != corrections.len()
    {
        return Err(SourceCommandError::Conflict(
            "Artifact correction replay history count differs from native observation",
        ));
    }
    let current_history_sha256 = selected
        .get(HISTORY)
        .map(|raw| Digest256::of_bytes(raw).to_prefixed());
    if current_history_sha256.as_deref() != native_history.history_sha256() {
        return Err(SourceCommandError::Conflict(
            "Artifact correction replay history digest differs from exact source bytes",
        ));
    }
    if !corrections.is_empty() && selected.len() != expected_names.len() {
        return Err(SourceCommandError::Conflict(
            "Artifact correction replay requires all three selected successor files",
        ));
    }
    if corrections.is_empty()
        && (native_history.origin_record_sha256()
            != Digest256::of_bytes(current_record_raw).to_hex()
            || native_history.origin_record_byte_size() != current_record_raw.len())
    {
        return Err(SourceCommandError::Conflict(
            "Artifact origin bytes differ from exact current record",
        ));
    }

    let mut selected_ids = Vec::with_capacity(corrections.len());
    let mut seen_ids = BTreeSet::new();
    for (index, correction) in corrections.iter().enumerate() {
        let publication_ref = cmd::field(correction, "publication")?;
        let transaction_id = cmd::text(publication_ref, "transaction_id")?;
        digest_text(&cmd::string(transaction_id))?;
        let native_transaction =
            native_history
                .transactions()
                .get(index)
                .ok_or(SourceCommandError::Conflict(
                    "Artifact correction native transaction order is incomplete",
                ))?;
        if native_transaction.transaction_id() != transaction_id
            || native_transaction.transport() != NativeTransportState::Committed
            || !seen_ids.insert(transaction_id)
        {
            return Err(SourceCommandError::Conflict(
                "Artifact correction transaction is absent, repeated, or uncommitted",
            ));
        }
        selected_ids.push(transaction_id);
    }
    let mut expected_manifest_sha256 = BTreeMap::new();
    for transaction in native_history.transactions() {
        if expected_manifest_sha256
            .insert(transaction.transaction_id(), transaction.manifest_sha256())
            .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "Artifact native history repeats a selected transaction",
            ));
        }
    }
    let publication = read_record_revision_publication_inner(
        ctx,
        ctx.effective_uid(),
        &selected_ids,
        Some(&expected_manifest_sha256),
        Some((deadline, cancelled)),
        false,
    )?;
    let publication_state_bytes = revision_publication_state_bytes(&publication)?;
    let mut publication_by_id = BTreeMap::new();
    for transaction in &publication.transactions {
        if publication_by_id
            .insert(transaction.transaction_id.as_str(), transaction)
            .is_some()
        {
            return Err(SourceCommandError::Invalid(
                "duplicate Artifact correction publication transaction",
            ));
        }
    }

    let mut evidence_transactions = Vec::with_capacity(corrections.len());
    let mut previous_after: Option<Package> = None;
    let mut retained_origin_checked = false;
    for (index, correction) in corrections.iter().enumerate() {
        if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(SourceCommandError::Unsupported(
                "Artifact correction replay deadline or cancellation",
            ));
        }
        let transaction_id = selected_ids[index];
        let retained =
            publication_by_id
                .get(transaction_id)
                .copied()
                .ok_or(SourceCommandError::Conflict(
                    "Artifact correction retained publication is absent",
                ))?;
        if retained.status != RevisionTransactionStatus::Committed {
            return Err(SourceCommandError::Conflict(
                "Artifact correction lacks committed publication evidence",
            ));
        }
        let native_transaction = &native_history.transactions()[index];
        let config = artifact_history_evidence_config(
            source_root,
            source_path,
            native_history.identity(),
            correction,
        )?;
        let (original, before, after, reconstructed) = reconstruct_transaction(
            cut,
            worker,
            deadline,
            cancelled,
            ctx,
            &config,
            RevisionFamily::NativeSelected,
            retained,
            TransactionReconstructionMode::ArtifactHistoryEvidence,
            "record.revise",
        )?;
        if !cmd::same(&original, cmd::field(correction, "request")?)?
            || !cmd::same(&reconstructed, correction)?
        {
            return Err(SourceCommandError::Conflict(
                "Artifact retained publication differs from exact correction receipt",
            ));
        }
        if cmd::array(&before.history, "receipts")?.len() != index
            || cmd::array(&after.history, "receipts")?.len() != index + 1
        {
            return Err(SourceCommandError::Conflict(
                "Artifact correction transaction does not append its exact history prefix",
            ));
        }
        for (prefix_index, receipt) in cmd::array(&before.history, "receipts")?.iter().enumerate() {
            if !cmd::same(receipt, &corrections[prefix_index])? {
                return Err(SourceCommandError::Conflict(
                    "Artifact correction predecessor history prefix differs",
                ));
            }
        }
        for (prefix_index, receipt) in cmd::array(&after.history, "receipts")?.iter().enumerate() {
            if !cmd::same(receipt, &corrections[prefix_index])? {
                return Err(SourceCommandError::Conflict(
                    "Artifact correction successor history prefix differs",
                ));
            }
        }
        if let Some(previous) = &previous_after
            && previous != &before.files
        {
            return Err(SourceCommandError::Conflict(
                "Artifact correction retained packages are not contiguous",
            ));
        }
        if !retained_origin_checked {
            let origin = before.files.get(base).ok_or(SourceCommandError::Invalid(
                "Artifact origin record missing",
            ))?;
            if Digest256::of_bytes(origin).to_hex() != native_history.origin_record_sha256()
                || origin.len() != native_history.origin_record_byte_size()
            {
                return Err(SourceCommandError::Conflict(
                    "Artifact first correction predecessor differs from retained origin",
                ));
            }
            retained_origin_checked = true;
        }
        let receipt_sha256 = cmd::record_digest(correction)?.to_prefixed();
        evidence_transactions.push(ArtifactCorrectionReplayTransactionObservation {
            transaction_id: transaction_id.to_owned(),
            manifest_sha256: native_transaction.manifest_sha256().to_owned(),
            receipt_sha256,
        });
        previous_after = Some(after.files);
    }
    if let Some(previous) = previous_after
        && &previous != selected
    {
        return Err(SourceCommandError::Conflict(
            "Artifact correction replay head differs from exact current package",
        ));
    }

    let mut observation = ArtifactCorrectionReplayObservation {
        input_identity: ctx.input_identity(),
        current_membership,
        source_root: source_root.to_owned(),
        source_path: source_path.to_owned(),
        record_id: native_history.identity().to_owned(),
        origin_record_sha256: native_history.origin_record_sha256().to_owned(),
        origin_record_byte_size: native_history.origin_record_byte_size(),
        history_sha256: current_history_sha256,
        transactions: evidence_transactions,
        publication_state_bytes,
        returned_state_bytes: 0,
    };
    observation.returned_state_bytes = artifact_correction_observation_state_bytes(&observation)?;
    Ok(observation)
}

const CANDIDATE_REPLAY_FILE_NODE_UPPER: usize = 512;

fn candidate_source_refusal(refusal: tos_validation::item_rules::ItemRefusal) -> SourceCommandError {
    use tos_validation::item_rules::ItemRefusal;
    match refusal {
        ItemRefusal::Budget | ItemRefusal::BudgetCheck { .. } => {
            SourceCommandError::Invalid("candidate Artifact replay source budget")
        }
        ItemRefusal::Deadline => {
            SourceCommandError::Unsupported("candidate Artifact replay source deadline")
        }
        ItemRefusal::Source(_) => {
            SourceCommandError::Conflict("candidate Artifact replay source changed")
        }
        ItemRefusal::Unsupported(_) => {
            SourceCommandError::Unsupported("candidate Artifact replay source unavailable")
        }
    }
}

fn candidate_current_member_file<I: Copy + Eq>(
    input: &dyn SourceCutInputWithIdentity<I>,
    input_identity: I,
    files: &mut BTreeMap<String, SourceFile>,
    file_path: &str,
    required_file: bool,
    expected_digest: Option<&str>,
    max_member_bytes: usize,
    max_total_bytes: u64,
    total_bytes: &mut u64,
    max_state_bytes: usize,
    state_bytes: &mut usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    if let Some(existing) = files.get(file_path) {
        if expected_digest
            .is_some_and(|digest| Digest256::of_bytes(&existing.raw).to_prefixed() != digest)
        {
            return Err(SourceCommandError::Conflict(
                "candidate replay member differs from native history read",
            ));
        }
        return Ok(());
    }
    if file_path.len() > 1024 || Instant::now() >= deadline || cancelled.load(Ordering::Relaxed) {
        return Err(SourceCommandError::Unsupported(
            "candidate Artifact replay source path or deadline",
        ));
    }
    let relative = path(file_path)?;
    let source = input.source_input();
    let presence = source
        .path_presence(file_path, deadline, cancelled)
        .map_err(candidate_source_refusal)?;
    if presence.is_none() {
        if required_file {
            return Err(SourceCommandError::Unsupported(
                "required candidate Artifact replay member is absent",
            ));
        }
        return Ok(());
    }
    if presence != Some(tos_source_store::SourcePresenceV1::File) {
        return Err(SourceCommandError::Conflict(
            "candidate Artifact replay member is not a regular source file",
        ));
    }

    let mut selected: Option<SourceFile> = None;
    let mut observed_bytes = 0u64;
    let mut retained_state = 0usize;
    source
        .with_current_member(
            file_path,
            max_member_bytes,
            deadline,
            cancelled,
            &mut |meta, raw| {
                if input.input_identity() != &input_identity
                    || selected.is_some()
                    || meta.path != file_path
                    || raw.len() > max_member_bytes
                    || u64::try_from(raw.len()).ok() != Some(meta.size_bytes)
                {
                    return Err(tos_validation::item_rules::ItemRefusal::Source(
                        "candidate Artifact replay current member binding changed".into(),
                    ));
                }
                let next_read = total_bytes
                    .checked_add(meta.size_bytes)
                    .ok_or(tos_validation::item_rules::ItemRefusal::Budget)?;
                if next_read > max_total_bytes {
                    return Err(tos_validation::item_rules::ItemRefusal::Budget);
                }
                if expected_digest.is_some_and(|digest| {
                    Digest256::of_bytes(raw).to_prefixed() != digest
                }) {
                    return Err(tos_validation::item_rules::ItemRefusal::Source(
                        "candidate Artifact replay member digest changed".into(),
                    ));
                }
                let next_state = raw
                    .len()
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(file_path.len().checked_mul(3)?))
                    .and_then(|n| n.checked_add(CANDIDATE_REPLAY_FILE_NODE_UPPER))
                    .and_then(|n| n.checked_add(size_of::<SourceFile>() + 1024))
                    .and_then(|n| n.checked_add(*state_bytes))
                    .ok_or(tos_validation::item_rules::ItemRefusal::Budget)?;
                if next_state > max_state_bytes {
                    return Err(tos_validation::item_rules::ItemRefusal::Budget);
                }
                let mut copy = Vec::new();
                copy.try_reserve_exact(raw.len())
                    .map_err(|_| tos_validation::item_rules::ItemRefusal::Budget)?;
                copy.extend_from_slice(raw);
                *selected = Some(SourceFile {
                    path: relative.clone(),
                    raw: copy,
                });
                observed_bytes = meta.size_bytes;
                retained_state = next_state - *state_bytes;
                Ok(())
            },
        )
        .map_err(candidate_source_refusal)?;
    let file = selected.ok_or(SourceCommandError::Conflict(
        "candidate Artifact replay input omitted a present member",
    ))?;
    *total_bytes = total_bytes
        .checked_add(observed_bytes)
        .ok_or(SourceCommandError::Invalid(
            "candidate Artifact replay source byte overflow",
        ))?;
    *state_bytes = state_bytes
        .checked_add(retained_state)
        .ok_or(SourceCommandError::Invalid(
            "candidate Artifact replay state overflow",
        ))?;
    files.insert(file_path.to_owned(), file);
    Ok(())
}

fn candidate_insert_owned_package_file(
    files: &mut BTreeMap<String, SourceFile>,
    file_path: String,
    raw: &[u8],
    max_state_bytes: usize,
    state_bytes: &mut usize,
) -> SourceCommandResult<()> {
    if files.contains_key(&file_path) {
        return Err(SourceCommandError::Conflict(
            "candidate replay package repeats a source path",
        ));
    }
    let charge = raw
        .len()
        .checked_mul(2)
        .and_then(|n| n.checked_add(file_path.len().checked_mul(3)?))
        .and_then(|n| n.checked_add(CANDIDATE_REPLAY_FILE_NODE_UPPER))
        .and_then(|n| n.checked_add(size_of::<SourceFile>() + 1024))
        .and_then(|n| n.checked_add(*state_bytes))
        .ok_or(SourceCommandError::Invalid(
            "candidate Artifact replay package state overflow",
        ))?;
    if charge > max_state_bytes {
        return Err(SourceCommandError::Invalid(
            "candidate Artifact replay package state budget",
        ));
    }
    let relative = path(&file_path)?;
    let mut copy = Vec::new();
    copy.try_reserve_exact(raw.len())
        .map_err(|_| SourceCommandError::Invalid("candidate Artifact replay allocation"))?;
    copy.extend_from_slice(raw);
    files.insert(file_path, SourceFile { path: relative, raw: copy });
    *state_bytes = charge;
    Ok(())
}

fn candidate_replay_source_files<I: Copy + Eq>(
    input: &dyn SourceCutInputWithIdentity<I>,
    native_history: &CandidateNativeRecordHistoryReadObservation<I>,
    max_member_bytes: usize,
    max_total_bytes: u64,
    max_state_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(Vec<SourceFile>, u64, usize)> {
    let input_identity = *input.input_identity();
    let mut files = BTreeMap::<String, SourceFile>::new();
    let mut source_bytes = 0u64;
    let mut state_bytes = 0usize;
    let parent = native_history
        .record_path()
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or(SourceCommandError::Invalid(
            "candidate Artifact replay owner path",
        ))?;
    for (name, raw) in native_history.selected_package() {
        if Instant::now() >= deadline || cancelled.load(Ordering::Relaxed) {
            return Err(SourceCommandError::Unsupported(
                "candidate Artifact replay package deadline",
            ));
        }
        candidate_insert_owned_package_file(
            &mut files,
            format!("{parent}/{name}"),
            raw,
            max_state_bytes,
            &mut state_bytes,
        )?;
    }

    for (index, read) in native_history.reads().iter().enumerate() {
        if index % 64 == 0 && (Instant::now() >= deadline || cancelled.load(Ordering::Relaxed)) {
            return Err(SourceCommandError::Unsupported(
                "candidate Artifact replay history-read deadline",
            ));
        }
        if let PredicateRead::ExactPath { path: read_path, digest } = read {
            candidate_current_member_file(
                input,
                input_identity,
                &mut files,
                read_path,
                true,
                Some(digest),
                max_member_bytes,
                max_total_bytes,
                &mut source_bytes,
                max_state_bytes,
                &mut state_bytes,
                deadline,
                cancelled,
            )?;
        }
    }

    candidate_current_member_file(
        input,
        input_identity,
        &mut files,
        CONTROL_PATH,
        false,
        None,
        max_member_bytes,
        max_total_bytes,
        &mut source_bytes,
        max_state_bytes,
        &mut state_bytes,
        deadline,
        cancelled,
    )?;

    for (index, transaction) in native_history.transactions().iter().enumerate() {
        if index % 32 == 0 && (Instant::now() >= deadline || cancelled.load(Ordering::Relaxed)) {
            return Err(SourceCommandError::Unsupported(
                "candidate Artifact publication manifest deadline",
            ));
        }
        let id = transaction.transaction_id();
        let directory = format!(
            "ToS/source-witnesses/.metadata-transactions/{}",
            &id[7..]
        );
        let manifest_path = format!("{directory}/manifest.json");
        candidate_current_member_file(
            input,
            input_identity,
            &mut files,
            &manifest_path,
            true,
            Some(transaction.manifest_sha256()),
            max_member_bytes,
            max_total_bytes,
            &mut source_bytes,
            max_state_bytes,
            &mut state_bytes,
            deadline,
            cancelled,
        )?;
        candidate_current_member_file(
            input,
            input_identity,
            &mut files,
            &format!("{directory}/completion.json"),
            false,
            None,
            max_member_bytes,
            max_total_bytes,
            &mut source_bytes,
            max_state_bytes,
            &mut state_bytes,
            deadline,
            cancelled,
        )?;
        let manifest_raw = &files
            .get(&manifest_path)
            .ok_or(SourceCommandError::Conflict(
                "candidate Artifact replay manifest disappeared",
            ))?
            .raw;
        if manifest_raw.len() > 524_288 {
            return Err(SourceCommandError::Invalid(
                "candidate Artifact replay manifest byte budget",
            ));
        }
        let manifest = cmd::parse(manifest_raw)?;
        let plan = cmd::field(&manifest, "plan")?;
        for entry in cmd::array(plan, "files")? {
            for side in ["before", "after"] {
                let binding = cmd::field(entry, side)?;
                if binding == &JsonValue::Null {
                    continue;
                }
                let digest = digest_text(cmd::field(binding, "sha256")?)?;
                let blob_path = format!("{directory}/{}.blob", &digest[7..]);
                candidate_current_member_file(
                    input,
                    input_identity,
                    &mut files,
                    &blob_path,
                    true,
                    Some(digest),
                    max_member_bytes,
                    max_total_bytes,
                    &mut source_bytes,
                    max_state_bytes,
                    &mut state_bytes,
                    deadline,
                    cancelled,
                )?;
            }
        }
    }
    if input.input_identity() != &input_identity {
        return Err(SourceCommandError::Conflict(
            "candidate Artifact replay input fence changed",
        ));
    }
    Ok((files.into_values().collect(), source_bytes, state_bytes))
}

/// Replay the same maintained Artifact correction history from a borrowed
/// candidate source input and completed streamed Records report. The typed
/// schema identity, opaque input identity, and Records membership must agree
/// before any retained publication is consumed.
#[allow(clippy::too_many_arguments)]
pub(crate) fn replay_artifact_corrections_from_candidate<I: Copy + Eq>(
    input: &dyn SourceCutInputWithIdentity<I>,
    records: &tos_validation::source_foundation_records::SourceFoundationRecordsStreamedReport<'_, I>,
    source_root: &str,
    effective_uid: u64,
    native_history: &CandidateNativeRecordHistoryReadObservation<I>,
    worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    max_member_bytes: usize,
    max_total_bytes: u64,
    max_state_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(CandidateArtifactCorrectionReplayObservation<I>, u64, usize)> {
    if Instant::now() >= deadline || cancelled.load(Ordering::Relaxed) {
        return Err(SourceCommandError::Unsupported(
            "candidate Artifact correction replay deadline or cancellation",
        ));
    }
    let identity = *input.input_identity();
    let schema = records
        .candidate_schema_identity()
        .ok_or(SourceCommandError::Conflict(
            "candidate Artifact replay lacks the prepared Records schema identity",
        ))?;
    if records.input_identity() != &identity
        || native_history.input_identity() != &identity
        || worker.input_identity() != &identity
        || *records.source_membership() != native_history.current_membership()
        || schema.profile() != worker.profile()
        || schema.schema_set_digest() != worker.schema_set_digest()
        || schema.contract_selection_digest() != worker.contract_selection_digest()
        || schema.prepared_execution_binding() != worker.prepared_execution_binding()
    {
        return Err(SourceCommandError::Conflict(
            "candidate Artifact replay input, Records, history, or schema bindings differ",
        ));
    }
    let (files, source_bytes, retained_state) = candidate_replay_source_files(
        input,
        native_history,
        max_member_bytes,
        max_total_bytes,
        max_state_bytes,
        deadline,
        cancelled,
    )?;
    let ctx = CandidateRecordVersionReadInput {
        input,
        input_identity: identity,
        files,
        schema_source_path: native_history.record_path().to_owned(),
        effective_uid,
    };
    let replay = replay_artifact_corrections_with_context(
        &ctx,
        source_root,
        None,
        *records.source_membership(),
        native_history,
        worker,
        deadline,
        cancelled,
    )?;
    Ok((replay, source_bytes, retained_state))
}

fn artifact_correction_observation_state_bytes<I>(
    observation: &ArtifactCorrectionReplayObservation<I>,
) -> SourceCommandResult<usize> {
    let mut state = std::mem::size_of::<ArtifactCorrectionReplayObservation<I>>()
        .checked_add(observation.source_root.capacity())
        .and_then(|state| state.checked_add(observation.source_path.capacity()))
        .and_then(|state| state.checked_add(observation.record_id.capacity()))
        .and_then(|state| state.checked_add(observation.origin_record_sha256.capacity()))
        .and_then(|state| {
            state.checked_add(
                observation
                    .history_sha256
                    .as_ref()
                    .map_or(0, String::capacity),
            )
        })
        .and_then(|state| {
            state.checked_add(observation.transactions.capacity().checked_mul(
                std::mem::size_of::<ArtifactCorrectionReplayTransactionObservation>(),
            )?)
        })
        .ok_or(SourceCommandError::Unsupported(
            "Artifact correction observation state overflow",
        ))?;
    for transaction in &observation.transactions {
        state = state
            .checked_add(transaction.transaction_id.capacity())
            .and_then(|state| state.checked_add(transaction.manifest_sha256.capacity()))
            .and_then(|state| state.checked_add(transaction.receipt_sha256.capacity()))
            .ok_or(SourceCommandError::Unsupported(
                "Artifact correction observation state overflow",
            ))?;
    }
    Ok(state)
}

/// Prepare the exact maintained record handler selected by protected owner
/// bytes. `publication` is mandatory for selected handlers, including their
/// legacy token=None baseline. It contains observations, not authority.
pub fn prepare_record_revision(
    ctx: &CommandContext,
    publication: Option<&RevisionPublication>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedCommand> {
    prepare_record_revision_inner(ctx, publication, None, worker, deadline, cancelled)
}

/// Complete native identity reservation uses the anchored manifest membership.
/// Every selected native packet must also be present byte-for-byte in `ctx`, so
/// the proposed plan retains its exact read dependencies.
pub fn prepare_record_revision_with_profile_cut(
    ctx: &CommandContext,
    publication: Option<&RevisionPublication>,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedCommand> {
    ctx.check()?;
    if cut.current().revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "profile cut and command revision differ",
        ));
    }
    for selected in &ctx.files {
        if !selected.path.as_str().starts_with("ToS/") {
            return Err(SourceCommandError::Unsupported(
                "profile cut input requires separately selected software capture",
            ));
        }
        let member =
            cut.current()
                .member(&selected.path)
                .ok_or(SourceCommandError::Unsupported(
                    "selected profile input outside anchored cut",
                ))?;
        if member.sha256 != Digest256::of_bytes(&selected.raw)
            || member.size_bytes != selected.raw.len() as u64
        {
            return Err(SourceCommandError::Conflict(
                "selected profile input differs from anchored cut member",
            ));
        }
    }
    prepare_record_revision_inner(ctx, publication, Some(cut), worker, deadline, cancelled)
}

/// Authenticate authored and implementation inputs through their independently
/// selected carriers. Software components cannot supply source membership or
/// native identity absence; the complete source cut still owns that inventory.
/// Protected configuration/account observations remain outside both captures.
pub fn prepare_record_revision_from_captures(
    ctx: &CommandContext,
    publication: Option<&RevisionPublication>,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedCommand> {
    ctx.check_from_selected_captures(cut, software, components, deadline, cancelled)?;
    prepare_record_revision_inner(ctx, publication, Some(cut), worker, deadline, cancelled)
}

pub(crate) fn prepare_record_revision_inner(
    ctx: &CommandContext,
    publication: Option<&RevisionPublication>,
    cut: Option<&CorpusCutReader>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedCommand> {
    if worker.source_revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "schema worker and source command cut differ",
        ));
    }
    let (config, family) = configuration(ctx)?;
    let request_value = cmd::parse(&ctx.request_raw)?;
    let operation = request(&request_value, family)?;
    let configuration = cmd::record_digest(&config)?.to_prefixed();
    if family.selected() {
        let publication = publication.ok_or(SourceCommandError::Unsupported(
            "selected publication observation required",
        ))?;
        let ids = publication
            .transactions
            .iter()
            .map(|t| t.transaction_id.as_str())
            .collect::<Vec<_>>();
        if read_record_revision_publication(ctx, &ids)? != *publication {
            return Err(SourceCommandError::Conflict(
                "publication observation differs from exact selected owner carriers",
            ));
        }
        if let Some(token) = &publication.token {
            digest_text(&cmd::string(token))?;
        }
        let ids = publication
            .transactions
            .iter()
            .map(|t| &t.transaction_id)
            .collect::<BTreeSet<_>>();
        if ids.len() != publication.transactions.len() || publication.transactions.len() > 128 {
            return Err(SourceCommandError::Invalid(
                "publication transaction identity or budget",
            ));
        }
        let pending = publication
            .transactions
            .iter()
            .filter(|t| t.status == RevisionTransactionStatus::Pending)
            .collect::<Vec<_>>();
        if pending.len() > 1 {
            return Err(SourceCommandError::Conflict(
                "multiple pending selected transactions",
            ));
        }
        if let Some(transaction) = pending.first() {
            if operation != "record.revise" && operation != "record.recover" {
                return Err(SourceCommandError::Conflict(
                    "selected publication has pending transaction",
                ));
            }
            let recovery = operation == "record.recover";
            if recovery
                && (!has(&config, "allowed_operations", "record.recover")?
                    || cmd::text(&request_value, "expected_configuration")? != configuration
                    || cmd::text(&request_value, "transaction_id")? != transaction.transaction_id)
            {
                return Err(SourceCommandError::Denied(
                    "recovery exact transaction or current delegation",
                ));
            }
            let (original, before, after, receipt) = reconstruct_transaction(
                cut,
                worker,
                deadline,
                cancelled,
                ctx,
                &config,
                family,
                transaction,
                TransactionReconstructionMode::CurrentOwner,
                if recovery {
                    "record.recover"
                } else {
                    "record.revise"
                },
            )?;
            if !recovery && !cmd::same(&request_value, &original)? {
                return Err(SourceCommandError::Conflict(
                    "only exact original command may resume",
                ));
            }
            let rollback = recovery && cmd::text(&request_value, "decision")? == "rollback";
            let source_path = cmd::text(&config, "source_path")?;
            let (parent, _) = split(source_path)?;
            let mut changes = Vec::new();
            for name in names(source_path)? {
                let path = path(&format!("{parent}/{name}"))?;
                let current = ctx.file(&path)?;
                if current != before.files.get(&name).map(Vec::as_slice)
                    && current != after.files.get(&name).map(Vec::as_slice)
                {
                    return Err(SourceCommandError::Conflict(
                        "pending source member is neither retained before nor after",
                    ));
                }
                let target = if rollback {
                    &before.files
                } else {
                    &after.files
                };
                changes.push(SourceChange {
                    before: current.map(Digest256::of_bytes),
                    path,
                    after: target.get(&name).cloned(),
                });
            }
            let inspection = if rollback { &before } else { &after };
            let mut response = result(
                worker,
                deadline,
                cancelled,
                ctx,
                &config,
                family,
                inspection,
                Some(publication),
                if rollback { None } else { Some(&receipt) },
                false,
            )?;
            cmd::set(
                &mut response,
                "recovery",
                cmd::object(vec![
                    ("transaction_id", cmd::string(&transaction.transaction_id)),
                    (
                        "decision",
                        cmd::string(if rollback { "rollback" } else { "resume" }),
                    ),
                    ("status", cmd::string("prepared")),
                ]),
            )?;
            return ctx.plan(family.handler_id(), response, changes, false);
        }
        if operation == "record.recover" {
            return Err(SourceCommandError::Conflict(
                "no exact pending transaction selected for recovery",
            ));
        }
    }
    let inspection = inspect(cut, worker, deadline, cancelled, ctx, &config, family)?;
    let mut response = result(
        worker,
        deadline,
        cancelled,
        ctx,
        &config,
        family,
        &inspection,
        publication,
        None,
        false,
    )?;
    if operation == "describe" {
        return ctx.plan(family.handler_id(), response, vec![], false);
    }
    if operation == "inspect-version" {
        let requested = cmd::field(&request_value, "source")?;
        for receipt in cmd::array(&inspection.history, "receipts")? {
            if cmd::same(cmd::field(receipt, "previous_source")?, requested)? {
                let (files, locations) = read_archive(ctx, &config, receipt)?;
                let (_, base) = split(cmd::text(&config, "source_path")?)?;
                cmd::set(
                    &mut response,
                    "record",
                    cmd::parse(
                        files
                            .get(base)
                            .ok_or(SourceCommandError::Conflict("archived record missing"))?,
                    )?,
                )?;
                cmd::set(&mut response, "inspected_source", requested.clone())?;
                cmd::set(&mut response, "files", locations)?;
                return ctx.plan(family.handler_id(), response, vec![], false);
            }
        }
        return Err(SourceCommandError::Conflict(
            "requested exact version is not retained in committed history",
        ));
    }
    scope(&config, &request_value, "record.revise")?; // Revocation precedes replay.
    if operation == "prepare-revise" {
        if !family.selected() && cmd::array(&inspection.history, "receipts")?.len() >= 128 {
            return Err(SourceCommandError::Invalid(
                "source revision history capacity reached",
            ));
        }
        let (_, subject, _, views, refs) = proposal(
            cut,
            worker,
            deadline,
            cancelled,
            ctx,
            &config,
            family,
            &inspection,
            &request_value,
            "record.revise",
        )?;
        cmd::set(&mut response, "prepared_source", subject)?;
        cmd::set(&mut response, "prepared_forms", JsonValue::Array(refs))?;
        cmd::set(
            &mut response,
            "prepared_materializations",
            JsonValue::Array(views),
        )?;
        cmd::set(
            &mut response,
            "expected_dependencies",
            cmd::string(&dependencies(ctx, &config, family, &inspection)?),
        )?;
        if family.selected() {
            cmd::set(
                &mut response,
                "expected_publication",
                publication
                    .and_then(|p| p.token.as_deref())
                    .map(cmd::string)
                    .unwrap_or(JsonValue::Null),
            )?;
        }
        return ctx.plan(family.handler_id(), response, vec![], false);
    }
    for receipt in cmd::array(&inspection.history, "receipts")? {
        if cmd::field(receipt, "command_id")? == cmd::field(&request_value, "command_id")? {
            if cmd::text(receipt, "request_digest")?
                != cmd::record_digest(&request_value)?.to_prefixed()
            {
                return Err(SourceCommandError::Conflict(
                    "command identity reused for another request",
                ));
            }
            read_archive(ctx, &config, receipt)?;
            if family.selected() {
                if cmd::text(&request_value, "expected_configuration")? != configuration {
                    return Err(SourceCommandError::Conflict(
                        "selected replay requires current exact configuration",
                    ));
                }
                let id = cmd::text(cmd::field(receipt, "publication")?, "transaction_id")?;
                let retained = publication
                    .and_then(|p| p.transactions.iter().find(|t| t.transaction_id == id))
                    .ok_or(SourceCommandError::Unsupported(
                        "selected replay retained publication evidence absent",
                    ))?;
                if retained.status != RevisionTransactionStatus::Committed {
                    return Err(SourceCommandError::Conflict(
                        "selected history has no committed publication evidence",
                    ));
                }
                let (original, _, _, reconstructed) = reconstruct_transaction(
                    cut,
                    worker,
                    deadline,
                    cancelled,
                    ctx,
                    &config,
                    family,
                    retained,
                    TransactionReconstructionMode::CurrentOwner,
                    "record.revise",
                )?;
                if !cmd::same(&original, &request_value)? || !cmd::same(&reconstructed, receipt)? {
                    return Err(SourceCommandError::Conflict(
                        "retained publication differs from correction receipt",
                    ));
                }
            }
            response = result(
                worker,
                deadline,
                cancelled,
                ctx,
                &config,
                family,
                &inspection,
                publication,
                Some(receipt),
                true,
            )?;
            return ctx.plan(family.handler_id(), response, vec![], true);
        }
    }
    if cmd::text(&request_value, "expected_configuration")? != configuration
        || !cmd::same(
            cmd::field(&request_value, "expected_source")?,
            &inspection.subject,
        )?
        || cmd::text(&request_value, "expected_revision")? != revision(&inspection.files)?
        || cmd::text(&request_value, "expected_dependencies")?
            != dependencies(ctx, &config, family, &inspection)?
    {
        return Err(SourceCommandError::Conflict(
            "record revision source or dependency snapshot stale",
        ));
    }
    if family.selected()
        && cmd::field(&request_value, "expected_publication")?
            != &publication
                .and_then(|p| p.token.as_deref())
                .map(cmd::string)
                .unwrap_or(JsonValue::Null)
    {
        return Err(SourceCommandError::Conflict(
            "selected publication snapshot stale",
        ));
    }
    let (after, receipt) = successor(
        cut,
        worker,
        deadline,
        cancelled,
        ctx,
        &config,
        family,
        &inspection,
        &request_value,
        &ctx.recorded_at,
        "record.revise",
    )?;
    let mut changes = retain_archive(ctx, &config, family, &inspection)?;
    changes.extend(source_changes(ctx, &config, &after.files)?);
    response = result(
        worker,
        deadline,
        cancelled,
        ctx,
        &config,
        family,
        &after,
        publication,
        Some(&receipt),
        false,
    )?;
    ctx.plan(family.handler_id(), response, changes, false)
}

const CONTROL_PATH: &str = "ToS/source-witnesses/.metadata-publication.json";
pub(crate) fn state(value: &JsonValue) -> SourceCommandResult<()> {
    cmd::exact_keys(
        value,
        &[
            "schema_version",
            "generation",
            "transition_id",
            "phase",
            "transaction_id",
            "manifest_sha256",
            "outcome",
            "recovery_authorization",
            "token",
        ],
    )?;
    let generation = cmd::integer(value, "generation")?;
    let transition = cmd::text(value, "transition_id")?;
    let phase = cmd::text(value, "phase")?;
    if cmd::text(value, "schema_version")? != "tos_source_metadata_publication_v1"
        || !(1..=9_007_199_254_740_991).contains(&generation)
        || transition.len() != 32
        || !transition
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || !matches!(phase, "pending" | "ready")
        || phase == "pending"
            && (cmd::field(value, "outcome")? != &JsonValue::Null
                || cmd::field(value, "recovery_authorization")? != &JsonValue::Null)
        || phase == "ready" && !matches!(cmd::text(value, "outcome")?, "committed" | "rolled-back")
    {
        return Err(SourceCommandError::Invalid(
            "selected publication state grammar",
        ));
    }
    let recovery = cmd::field(value, "recovery_authorization")?;
    if recovery != &JsonValue::Null
        && (recovery.as_object().is_none() || cmd::canonical(recovery)?.len() > 4096)
    {
        return Err(SourceCommandError::Invalid("selected recovery binding"));
    }
    for field in ["transaction_id", "manifest_sha256", "token"] {
        digest_text(cmd::field(value, field)?)?;
    }
    let fields = value
        .as_object()
        .ok_or(SourceCommandError::Invalid("publication state object"))?
        .iter()
        .filter(|(key, _)| key.as_str() != Some("token"))
        .cloned()
        .collect();
    if cmd::text(value, "token")? != cmd::record_digest(&JsonValue::Object(fields))?.to_prefixed() {
        return Err(SourceCommandError::Conflict(
            "publication control token does not bind exact state",
        ));
    }
    Ok(())
}
fn control(ctx: &impl RecordRead) -> SourceCommandResult<Option<JsonValue>> {
    let Some(raw) = ctx.file(&path(CONTROL_PATH)?)? else {
        return Ok(None);
    };
    if raw.len() > 8192 {
        return Err(SourceCommandError::Invalid(
            "publication control byte budget",
        ));
    }
    let value = cmd::parse(raw)?;
    state(&value)?;
    Ok(Some(value))
}
/// Read publication observations from explicitly selected owner control,
/// manifest, completion and immutable blob bytes. IDs select retained carriers;
/// they grant no admission. The status is derived from those exact carriers;
/// the public current-owner entrypoint also includes the selected pending control.
pub fn read_record_revision_publication(
    ctx: &CommandContext,
    selected_ids: &[&str],
) -> SourceCommandResult<RevisionPublication> {
    Ok(read_record_revision_publication_inner(
        ctx,
        ctx.effective_uid,
        selected_ids,
        None,
        None,
        true,
    )?)
}

fn read_record_revision_publication_inner(
    ctx: &impl RecordRead,
    effective_uid: u64,
    selected_ids: &[&str],
    expected_manifest_sha256: Option<&BTreeMap<&str, &str>>,
    operation: Option<(Instant, &AtomicBool)>,
    include_current_pending: bool,
) -> SourceCommandResult<RevisionPublication> {
    ctx.check()?;
    if selected_ids.len() > 128 {
        return Err(SourceCommandError::Invalid(
            "retained publication count budget",
        ));
    }
    let control = control(ctx)?;
    let mut ids = selected_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<BTreeSet<_>>();
    if ids.len() != selected_ids.len() {
        return Err(SourceCommandError::Invalid(
            "duplicate retained transaction selection",
        ));
    }
    if include_current_pending && let Some(control) = &control {
        if cmd::text(control, "phase")? == "pending" {
            ids.insert(cmd::text(control, "transaction_id")?.into());
        }
    }
    if ids.len() > 128 {
        return Err(SourceCommandError::Invalid(
            "retained publication count budget",
        ));
    }
    let mut transactions = Vec::new();
    for id in ids {
        if operation.is_some_and(|(deadline, cancelled)| {
            Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed)
        }) {
            return Err(SourceCommandError::Unsupported(
                "Artifact history publication verification deadline or cancellation",
            ));
        }
        digest_text(&cmd::string(&id))?;
        let directory = format!("ToS/source-witnesses/.metadata-transactions/{}", &id[7..]);
        let manifest_raw = required(ctx, &format!("{directory}/manifest.json"))?;
        if manifest_raw.len() > 524_288 {
            return Err(SourceCommandError::Invalid(
                "transaction manifest byte budget",
            ));
        }
        let manifest = cmd::parse(manifest_raw)?;
        cmd::exact_keys(
            &manifest,
            &[
                "schema_version",
                "transaction_id",
                "base_publication",
                "plan",
                "parents",
            ],
        )?;
        if cmd::text(&manifest, "schema_version")? != "tos_selected_metadata_transaction_v1"
            || cmd::text(&manifest, "transaction_id")? != id
        {
            return Err(SourceCommandError::Unsupported(
                "retained transaction manifest is not selected record revision grammar",
            ));
        }
        let base = cmd::field(&manifest, "base_publication")?;
        cmd::exact_keys(base, &["token", "generation"])?;
        let generation = cmd::integer(base, "generation")?;
        if generation > 9_007_199_254_740_989
            || (cmd::field(base, "token")? == &JsonValue::Null) != (generation == 0)
        {
            return Err(SourceCommandError::Invalid(
                "retained publication predecessor",
            ));
        }
        if generation > 0 {
            digest_text(cmd::field(base, "token")?)?;
        }
        let plan = cmd::field(&manifest, "plan")?;
        cmd::exact_keys(plan, &["authorization", "files", "new_directories"])?;
        if !cmd::array(plan, "new_directories")?.is_empty() || cmd::array(plan, "files")?.len() != 3
        {
            return Err(SourceCommandError::Denied(
                "record revision transaction selects other paths or directories",
            ));
        }
        let authorization = cmd::field(plan, "authorization")?;
        let authorization_raw = cmd::canonical(authorization)?;
        if authorization.as_object().is_none() || authorization_raw.len() > 65_536 {
            return Err(SourceCommandError::Invalid(
                "retained transaction authorization budget",
            ));
        }
        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut paths = BTreeSet::new();
        let mut parents = BTreeSet::new();
        let mut sums = [0u64; 2];
        let mut changed = false;
        let mut last = "";
        for item in cmd::array(plan, "files")? {
            if operation.is_some_and(|(deadline, cancelled)| {
                Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed)
            }) {
                return Err(SourceCommandError::Unsupported(
                    "Artifact history publication verification deadline or cancellation",
                ));
            }
            cmd::exact_keys(item, &["path", "before", "after"])?;
            let source_path = cmd::text(item, "path")?;
            path(source_path)?;
            if !source_path.starts_with("ToS/source-witnesses/")
                || source_path.split('/').any(|p| {
                    p.starts_with('.')
                        || matches!(
                            p,
                            "payload" | "private" | "local-content" | "owner-local" | "catalog"
                        )
                })
                || !source_path.ends_with(".json")
                || source_path.len() > 1024
                || source_path.split('/').count() > 24
                || source_path <= last
                || !paths.insert(source_path)
            {
                return Err(SourceCommandError::Denied(
                    "retained revision canonical selected path grammar",
                ));
            }
            last = source_path;
            let mut parent = split(source_path)?.0;
            loop {
                parents.insert(parent);
                if parent == "ToS/source-witnesses" {
                    break;
                }
                parent = split(parent)?.0;
            }
            changed |= !cmd::same(cmd::field(item, "before")?, cmd::field(item, "after")?)?;
            if cmd::field(item, "before")? == &JsonValue::Null
                && cmd::field(item, "after")? == &JsonValue::Null
            {
                return Err(SourceCommandError::Invalid(
                    "retained transaction absent-to-absent member",
                ));
            }
            for (index, side) in ["before", "after"].into_iter().enumerate() {
                let binding = cmd::field(item, side)?;
                if binding == &JsonValue::Null {
                    continue;
                }
                cmd::exact_keys(binding, &["sha256", "bytes"])?;
                let digest = digest_text(cmd::field(binding, "sha256")?)?;
                let length = cmd::integer(binding, "bytes")?;
                sums[index] = sums[index]
                    .checked_add(length)
                    .ok_or(SourceCommandError::Invalid("retained blob budget overflow"))?;
                if length > 8_388_608 || sums[index] > 8_388_608 {
                    return Err(SourceCommandError::Invalid(
                        "retained selected side byte budget",
                    ));
                }
                let raw = required(ctx, &format!("{directory}/{}.blob", &digest[7..]))?;
                if raw.len() as u64 != length || Digest256::of_bytes(raw).to_prefixed() != digest {
                    return Err(SourceCommandError::Conflict(
                        "retained transaction immutable blob differs",
                    ));
                }
                let file = SourceFile {
                    path: path(source_path)?,
                    raw: raw.to_vec(),
                };
                if side == "before" {
                    before.push(file)
                } else {
                    after.push(file)
                }
            }
        }
        if !changed {
            return Err(SourceCommandError::Invalid(
                "retained revision transaction has no byte change",
            ));
        }
        let bindings =
            cmd::field(&manifest, "parents")?
                .as_object()
                .ok_or(SourceCommandError::Invalid(
                    "retained source parent bindings",
                ))?;
        if bindings.len() != parents.len()
            || bindings
                .iter()
                .any(|(key, _)| !key.as_str().is_some_and(|key| parents.contains(key)))
        {
            return Err(SourceCommandError::Conflict(
                "retained source parent closure differs",
            ));
        }
        let current = control
            .as_ref()
            .filter(|state| cmd::text(state, "transaction_id").ok() == Some(id.as_str()));
        for (_, binding) in bindings {
            cmd::exact_keys(binding, &["device", "inode", "mode", "uid"])?;
            cmd::integer(binding, "device")?;
            cmd::integer(binding, "inode")?;
            let mode = cmd::integer(binding, "mode")?;
            let uid = cmd::integer(binding, "uid")?;
            if mode & 0o170000 != 0o040000
                || mode & 0o022 != 0
                || current.is_some_and(|c| cmd::text(c, "phase").ok() == Some("pending"))
                    && uid != 0
                    && uid != effective_uid
            {
                return Err(SourceCommandError::Denied(
                    "retained protected source parent binding",
                ));
            }
        }
        let digest = Digest256::of_bytes(manifest_raw).to_prefixed();
        if expected_manifest_sha256.is_some_and(|manifests| {
            manifests
                .get(id.as_str())
                .is_none_or(|expected| *expected != digest.as_str())
        }) {
            return Err(SourceCommandError::Conflict(
                "retained publication manifest differs from native history observation",
            ));
        }
        let completion_raw = ctx.file(&path(&format!("{directory}/completion.json"))?)?;
        if completion_raw.is_some_and(|raw| raw.len() > 8192) {
            return Err(SourceCommandError::Invalid(
                "retained completion byte budget",
            ));
        }
        let completion = completion_raw.map(cmd::parse).transpose()?;
        let mut terminal = None;
        if let Some(completion) = &completion {
            cmd::exact_keys(completion, &["schema_version", "publication"])?;
            if cmd::text(completion, "schema_version")? != "tos_selected_metadata_completion_v1" {
                return Err(SourceCommandError::Invalid("retained completion schema"));
            }
            let completed = cmd::field(completion, "publication")?;
            state(completed)?;
            if cmd::text(completed, "phase")? != "ready"
                || cmd::text(completed, "transaction_id")? != id
                || cmd::text(completed, "manifest_sha256")? != digest
                || cmd::integer(completed, "generation")? != generation + 2
            {
                return Err(SourceCommandError::Conflict(
                    "retained completion does not bind exact manifest",
                ));
            }
            terminal = Some(completed);
        }
        let status = if let Some(current) = current {
            if cmd::text(current, "manifest_sha256")? != digest {
                return Err(SourceCommandError::Conflict(
                    "current publication manifest digest differs",
                ));
            }
            if cmd::text(current, "phase")? == "pending" {
                if cmd::integer(current, "generation")? != generation + 1 || terminal.is_some() {
                    return Err(SourceCommandError::Conflict(
                        "pending transaction predecessor or terminal state differs",
                    ));
                }
                RevisionTransactionStatus::Pending
            } else {
                if cmd::integer(current, "generation")? != generation + 2
                    || terminal.is_some_and(|t| t != current)
                {
                    return Err(SourceCommandError::Conflict(
                        "terminal current publication differs from completion",
                    ));
                }
                if cmd::text(current, "outcome")? == "committed" {
                    RevisionTransactionStatus::Committed
                } else {
                    RevisionTransactionStatus::RolledBack
                }
            }
        } else if let Some(terminal) = terminal {
            if cmd::text(terminal, "outcome")? == "committed" {
                RevisionTransactionStatus::Committed
            } else {
                RevisionTransactionStatus::RolledBack
            }
        } else {
            return Err(SourceCommandError::Unsupported(
                "unselected orphan transaction has no terminal publication evidence",
            ));
        };
        transactions.push(RetainedRevisionTransaction {
            transaction_id: id,
            status,
            authorization_raw,
            before,
            after,
        });
    }
    let token = control
        .as_ref()
        .map(|c| cmd::text(c, "token").map(String::from))
        .transpose()?;
    Ok(RevisionPublication {
        token,
        transactions,
    })
}
fn allowed_fields<'a>(
    family: RevisionFamily,
    config: &JsonValue,
) -> SourceCommandResult<Vec<&'a str>> {
    Ok(match family {
        RevisionFamily::Historical | RevisionFamily::PublicProfile => BASE_FIELDS.to_vec(),
        RevisionFamily::PublicProfileScope => BASE_FIELDS
            .iter()
            .copied()
            .chain(["semantic_scope"])
            .collect(),
        RevisionFamily::CorpusFlat
        | RevisionFamily::CorpusSelectedV2
        | RevisionFamily::CorpusSelectedV3 => CORPUS_FIELDS.to_vec(),
        RevisionFamily::NativeSelected => match cmd::text(config, "record_type")? {
            "artifact" => vec![
                "path_identity",
                "physical_description",
                "find_context",
                "bibliography",
            ],
            "composite" => vec!["preferred_label", "editorial_object"],
            "link" => vec![
                "preferred_label",
                "variant_labels",
                "notes",
                "source_refs",
                "provider_label",
            ],
            _ => return Err(SourceCommandError::Denied("native revision record type")),
        },
    })
}
fn request(v: &JsonValue, family: RevisionFamily) -> SourceCommandResult<&str> {
    let operation = cmd::text(v, "operation")?;
    let mut keys = vec!["schema_version", "operation"];
    match operation {
        "describe" => {}
        "inspect-version" => keys.push("source"),
        "prepare-revise" | "record.revise" => {
            keys.extend(["fields", "forms", "reason"]);
            if operation == "record.revise" {
                keys.extend([
                    "command_id",
                    "expected_configuration",
                    "expected_source",
                    "expected_revision",
                    "expected_dependencies",
                ]);
                if family.selected() {
                    keys.push("expected_publication");
                }
            }
        }
        "record.recover" if family.selected() => {
            keys.extend(["transaction_id", "decision", "expected_configuration"])
        }
        _ => return Err(SourceCommandError::Unsupported("record revision operation")),
    }
    cmd::exact_keys(v, &keys)?;
    if cmd::text(v, "schema_version")? != "tos_local_source_command_v1" {
        return Err(SourceCommandError::Invalid("command request schema"));
    }
    if operation == "record.revise" {
        let id = cmd::text(v, "command_id")?;
        if id.is_empty() || id.chars().count() > 256 {
            return Err(SourceCommandError::Invalid("revision command identity"));
        }
        for k in [
            "expected_configuration",
            "expected_revision",
            "expected_dependencies",
        ] {
            digest_text(cmd::field(v, k)?)?;
        }
        exact_ref(cmd::field(v, "expected_source")?)?;
        if family.selected() && cmd::field(v, "expected_publication")? != &JsonValue::Null {
            digest_text(cmd::field(v, "expected_publication")?)?;
        }
    }
    if operation == "inspect-version" {
        exact_ref(cmd::field(v, "source")?)?;
    }
    if operation == "record.recover" {
        digest_text(cmd::field(v, "transaction_id")?)?;
        digest_text(cmd::field(v, "expected_configuration")?)?;
        if !matches!(cmd::text(v, "decision")?, "resume" | "rollback") {
            return Err(SourceCommandError::Invalid("recovery decision"));
        }
    }
    Ok(operation)
}
fn exact_ref(v: &JsonValue) -> SourceCommandResult<()> {
    cmd::exact_keys(v, &["id", "version", "digest"])?;
    if cmd::text(v, "id")?.is_empty() || cmd::integer(v, "version")? == 0 {
        return Err(SourceCommandError::Invalid("exact source reference"));
    }
    digest_text(cmd::field(v, "digest")?)?;
    Ok(())
}
fn scope(config: &JsonValue, request: &JsonValue, operation: &str) -> SourceCommandResult<()> {
    if !has(config, "allowed_operations", operation)? {
        return Err(SourceCommandError::Denied(
            "current record revision scope revoked",
        ));
    }
    let fields = cmd::field(request, "fields")?
        .as_object()
        .ok_or(SourceCommandError::Invalid("revision fields object"))?;
    if fields.is_empty() {
        return Err(SourceCommandError::Denied("empty correction"));
    }
    for (key, _) in fields {
        if !has(
            config,
            "allowed_fields",
            key.as_str()
                .ok_or(SourceCommandError::Invalid("field name"))?,
        )? {
            return Err(SourceCommandError::Denied(
                "field outside current delegated scope",
            ));
        }
    }
    let forms = cmd::array(request, "forms")?;
    if !(1..=32).contains(&forms.len()) {
        return Err(SourceCommandError::Invalid(
            "revision requires bounded source forms",
        ));
    }
    let mut seen = BTreeSet::new();
    for form in forms {
        cmd::exact_keys(form, &["form_id", "field_id"])?;
        let id = cmd::text(form, "form_id")?;
        cmd::text(form, "field_id")?;
        if !has(config, "allowed_form_ids", id)? {
            return Err(SourceCommandError::Denied(
                "form outside current delegated scope",
            ));
        }
        if !seen.insert(id) {
            return Err(SourceCommandError::Invalid("duplicate revision form"));
        }
    }
    if !(1..=4096).contains(&strip(cmd::text(request, "reason")?)?.chars().count()) {
        return Err(SourceCommandError::Invalid("authored correction reason"));
    }
    Ok(())
}
pub(crate) fn package(
    ctx: &impl RecordRead,
    source_path: &str,
    selected: bool,
    remaining_state_bytes: Option<usize>,
) -> SourceCommandResult<Package> {
    let (parent, base) = split(source_path)?;
    let selected_names = names(source_path)?;
    let prefix = format!("{parent}/");
    let mut result = Package::new();
    let mut total_bytes = 0usize;
    let mut state_bytes = std::mem::size_of::<Package>();
    for file in ctx.files() {
        if let Some(name) = file.path.as_str().strip_prefix(&prefix) {
            if ctx.writer().is_none() && name != base && name != HISTORY {
                continue;
            }
            if !name.contains('/') && (!selected || selected_names.iter().any(|s| s == name)) {
                total_bytes = total_bytes
                    .checked_add(file.raw.len())
                    .filter(|total| *total <= 8_388_608)
                    .ok_or(SourceCommandError::Unsupported(
                        "record package state or byte budget",
                    ))?;
                state_bytes = state_bytes
                    .checked_add(file.raw.len())
                    .and_then(|total| total.checked_add(name.len()))
                    .and_then(|total| total.checked_add(std::mem::size_of::<(String, Vec<u8>)>()))
                    .filter(|total| {
                        remaining_state_bytes.is_none_or(|remaining| *total <= remaining)
                    })
                    .ok_or(SourceCommandError::Unsupported(
                        "record package state or byte budget",
                    ))?;
                if result.len() >= 64 || file.raw.len() > 2_097_152 {
                    return Err(SourceCommandError::Invalid(
                        "record package presence or budget",
                    ));
                }
                result.insert(name.into(), file.raw.clone());
            } else if !selected && name.contains('/') {
                return Err(SourceCommandError::Denied(
                    "flat revision package has descendants",
                ));
            }
        }
    }
    if !result.contains_key(base)
        || result.is_empty()
        || result.len() > 64
        || result.values().any(|b| b.len() > 2_097_152)
        || total_bytes > 8_388_608
    {
        return Err(SourceCommandError::Invalid(
            "record package presence or budget",
        ));
    }
    Ok(result)
}
pub(crate) fn file_refs(files: &Package, blobs: bool) -> JsonValue {
    JsonValue::Object(
        files
            .iter()
            .map(|(name, raw)| {
                let digest = Digest256::of_bytes(raw);
                let mut value = cmd::object(vec![
                    ("sha256", cmd::string(&digest.to_prefixed())),
                    ("bytes", cmd::number(raw.len() as u64)),
                ]);
                if blobs {
                    cmd::set(
                        &mut value,
                        "blob",
                        cmd::string(&format!("{}.blob", digest.to_hex())),
                    )
                    .expect("constructed object");
                }
                (JsonString::from_utf8(name), value)
            })
            .collect(),
    )
}
pub(crate) fn revision(files: &Package) -> SourceCommandResult<String> {
    Ok(cmd::record_digest(&file_refs(files, false))?.to_prefixed())
}
pub(crate) fn archive_path(config: &JsonValue, revision: &str) -> SourceCommandResult<String> {
    Ok(format!(
        "ToS/source-witnesses/.record-revisions/{}-{}",
        Digest256::of_bytes(cmd::text(config, "record_id")?.as_bytes()).to_hex(),
        revision
            .strip_prefix("sha256:")
            .ok_or(SourceCommandError::Invalid("revision digest"))?
    ))
}

pub(crate) fn schema(
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &impl RecordRead<Identity = tos_foundation::SourceRevision>,
    refs: &[String],
    root: &str,
    instance: &JsonValue,
) -> SourceCommandResult<()> {
    schema_at(
        worker, deadline, cancelled, ctx, refs, root, instance, None, false,
    )
}

// Reuse is limited to an actual scalar result from this selected worker operation.
// The caller still reads and checks the current source/resource dependencies.
fn schema_at<W, C>(
    worker: &mut W,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &C,
    refs: &[String],
    root: &str,
    instance: &JsonValue,
    source_path: Option<&str>,
    reuse_scalar: bool,
) -> SourceCommandResult<()>
where
    W: RecordSchemaWorker,
    C: RecordRead<Identity = W::Identity>,
{
    if worker.input_identity() != ctx.input_identity() {
        return Err(SourceCommandError::Conflict(
            "schema worker and selected source input differ",
        ));
    }
    for name in refs {
        if worker.resource_closure_bound_to_identity() {
            if worker.contract_digest(name).is_none() {
                return Err(SourceCommandError::Conflict(
                    "candidate schema resource is outside the prepared source closure",
                ));
            }
            continue;
        }
        let raw = required(ctx, name)?;
        let source = cmd::parse(raw)?;
        let uri = cmd::text(&source, "$id")?;
        if uri != format!("https://tree-of-sophia.local/{name}")
            && uri != format!("https://treeofsophia.local/{name}")
        {
            return Err(SourceCommandError::Invalid(
                "source schema identity differs",
            ));
        }
        if worker.contract_digest(name) != Some(Digest256::of_bytes(raw)) {
            return Err(SourceCommandError::Conflict(
                "schema worker resources and selected source bytes differ",
            ));
        }
    }
    let fallback_path;
    let source_path = match source_path {
        Some(path) => path,
        None => {
            fallback_path = ctx.schema_source_path()?;
            &fallback_path
        }
    };
    let raw = cmd::canonical(instance)?;
    let result = if reuse_scalar {
        worker.check_reusing_scalar(source_path, &raw, root, deadline, cancelled)
    } else {
        worker.check(source_path, &raw, root, deadline, cancelled)
    };
    match result {
        Ok(true) => Ok(()),
        Ok(false) => Err(SourceCommandError::Invalid(
            "source violates selected schema",
        )),
        Err(reason) => Err(SourceCommandError::SchemaExecution {
            path: source_path.to_owned(),
            root: root.to_owned(),
            reason,
        }),
    }
}

fn profile<W, C>(
    cut: Option<&CorpusCutReader>,
    worker: &mut W,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &C,
    config: &JsonValue,
    family: RevisionFamily,
    record: &JsonValue,
) -> SourceCommandResult<(JsonValue, Vec<String>, Option<String>, Option<String>)>
where
    W: RecordSchemaWorker,
    C: RecordRead<Identity = W::Identity>,
{
    let source_path = cmd::text(config, "source_path")?;
    let (_, base) = split(source_path)?;
    let (kind, id_field, schema_ref) = if family == RevisionFamily::NativeSelected {
        let kind = cmd::text(config, "record_type")?;
        let (subtree, basename, identity, source_schema) =
            match (kind, cmd::text(record, "schema_version")?) {
                ("artifact", "tos_artifact_source_witness_v1") => (
                    "artifacts",
                    "artifact-witness.json",
                    "artifact_id",
                    "ToS/contracts/artifact-source-witness.schema.json",
                ),
                ("artifact", "tos_artifact_source_witness_v2") => (
                    "artifacts",
                    "artifact-witness.json",
                    "artifact_id",
                    "ToS/contracts/artifact-source-witness-v2.schema.json",
                ),
                ("composite", "tos_scholarly_composite_witness_v1") => (
                    "scholarly-composites",
                    "composite-witness.json",
                    "composite_id",
                    "ToS/contracts/scholarly-composite-witness.schema.json",
                ),
                ("link", "tos_source_link_v1") => (
                    "links",
                    "link.json",
                    "record_id",
                    "ToS/contracts/source-link.schema.json",
                ),
                _ => return Err(SourceCommandError::Denied("exact native record schema")),
            };
        if base != basename
            || !source_path.starts_with(&format!("ToS/source-witnesses/{subtree}/"))
            || cmd::text(config, "record_schema_version")? != cmd::text(record, "schema_version")?
        {
            return Err(SourceCommandError::Denied(
                "exact native owner path or schema",
            ));
        }
        (kind, identity, source_schema)
    } else if family == RevisionFamily::Historical {
        if cmd::text(record, "schema_version")? != "tos_historical_record_v1" {
            return Err(SourceCommandError::Denied("historical schema required"));
        }
        (
            cmd::text(record, "record_type")?,
            "record_id",
            "ToS/contracts/historical-record.schema.json",
        )
    } else if family.profile() {
        return worker.public_profile(cut, deadline, cancelled, ctx, config, record);
    } else {
        let kind = cmd::text(config, "record_type")?;
        let mut allowed = vec!["agent", "place", "organization", "work"];
        if matches!(
            family,
            RevisionFamily::CorpusSelectedV2 | RevisionFamily::CorpusSelectedV3
        ) {
            allowed.push("expression");
        }
        if family == RevisionFamily::CorpusSelectedV3 {
            allowed.extend(["edition", "collection", "item"]);
        }
        if !allowed.contains(&kind)
            || cmd::text(record, "record_type")? != kind
            || cmd::text(record, "schema_version")? != "tos_corpus_record_v1"
            || base != format!("{kind}.json")
        {
            return Err(SourceCommandError::Denied(
                "native grant version or corpus type",
            ));
        }
        (kind, "record_id", CORPUS_SCHEMA)
    };
    if cmd::text(record, id_field)? != cmd::text(config, "record_id")?
        || !valid_id(
            cmd::text(config, "record_id")?,
            &format!("tos.{kind}."),
            false,
        )
    {
        return Err(SourceCommandError::Denied("exact record identity"));
    }
    let mut schemas = vec![schema_ref.into()];
    if family == RevisionFamily::Historical {
        schemas.push(CORPUS_SCHEMA.into());
    }
    schema_at(
        worker,
        deadline,
        cancelled,
        ctx,
        &schemas,
        schema_ref,
        record,
        Some(source_path),
        false,
    )?;
    if family == RevisionFamily::Historical
        && !matches!(
            cmd::text(record, "visibility")?,
            "public" | "public_metadata_only"
        )
    {
        return Err(SourceCommandError::Denied("record visibility"));
    }
    let profile = cmd::object(vec![
        ("record_type", cmd::string(kind)),
        ("id_prefix", cmd::string(&format!("tos.{kind}."))),
        ("source_basename", cmd::string(base)),
        ("schema_ref", cmd::string(schema_ref)),
        (
            "schema_version",
            cmd::string(cmd::text(record, "schema_version")?),
        ),
        ("source_scope", cmd::string("public_metadata_only")),
    ]);
    Ok((profile, schemas, None, None))
}

fn ancestry<'a>(
    entities: &BTreeMap<&'a str, &'a JsonValue>,
    id: &'a str,
    visiting: &mut BTreeSet<&'a str>,
    out: &mut BTreeSet<&'a str>,
) -> SourceCommandResult<()> {
    if visiting.len() > 128 || !visiting.insert(id) {
        return Err(SourceCommandError::Invalid(
            "entity ancestry cycle or depth budget",
        ));
    }
    let entry = entities
        .get(id)
        .ok_or(SourceCommandError::Invalid("entity parent missing"))?;
    out.insert(id);
    for parent in cmd::array(entry, "parent_type_ids")? {
        ancestry(
            entities,
            parent
                .as_str()
                .ok_or(SourceCommandError::Invalid("entity parent identity"))?,
            visiting,
            out,
        )?;
    }
    visiting.remove(id);
    Ok(())
}
/// Match the maintained native inventory discovery against complete anchored
/// membership. A caller-selected list alone can never prove absence.
fn native_identity_inventory(
    cut: Option<&CorpusCutReader>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &impl RecordRead<Identity = tos_foundation::SourceRevision>,
    record_id: &str,
) -> SourceCommandResult<(Option<String>, bool)> {
    if !["occurrence", "lexeme", "sense", "sign", "concept"]
        .iter()
        .any(|kind| record_id.starts_with(&format!("tos.{kind}.")))
    {
        return Ok((None, false));
    }
    let cut = cut.ok_or(SourceCommandError::Unsupported(
        "profile native identity reservation needs complete anchored source cut",
    ))?;
    let writer = ctx.writer().ok_or(SourceCommandError::Unsupported(
        "profile native identity reservation needs complete anchored source cut",
    ))?;
    let (_, snapshot, schema) = selected_native_identity_inventory(
        writer,
        cut,
        worker,
        deadline,
        cancelled,
        Some(record_id),
    )?;
    Ok((Some(snapshot), schema))
}

/// Exact native identity membership for the maintained creation catalog.
/// It reserves identities; it never projects private semantic packet contents.
pub(crate) fn native_identity_inventory_from_cut(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(BTreeMap<String, Vec<String>>, String, bool)> {
    selected_native_identity_inventory(ctx, cut, worker, deadline, cancelled, None)
}

fn selected_native_identity_inventory(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    reserved_id: Option<&str>,
) -> SourceCommandResult<(BTreeMap<String, Vec<String>>, String, bool)> {
    if cut.current().revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "native inventory source cut differs",
        ));
    }
    let mut entries = BTreeMap::new();
    let mut identities: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut remaining = 8_388_608usize;
    let contract = "ToS/contracts/semantic-annotation-packet-v2.schema.json";
    for member in cut.current().members() {
        let name = member.path.as_str();
        if name == "ToS/source-witnesses/owner-local"
            || name.starts_with("ToS/source-witnesses/owner-local/")
        {
            return Err(SourceCommandError::Denied(
                "reserved owner-local namespace cannot enter public native inventory",
            ));
        }
        let basename = name.rsplit('/').next().unwrap_or(name);
        if !name.starts_with("ToS/source-witnesses/")
            || !basename.starts_with("semantic-annotation")
            || !basename.ends_with(".json")
            || name
                .split('/')
                .any(|part| matches!(part, "payload" | "local-content" | "catalog"))
        {
            continue;
        }
        if entries.len() >= 1024 || member.size_bytes > remaining.min(1_048_576) as u64 {
            return Err(SourceCommandError::Invalid(
                "native identity inventory metadata budget",
            ));
        }
        let observed = cut
            .read_member(
                ctx.base_revision,
                &member.path,
                remaining.min(1_048_576) as u64,
                deadline,
                cancelled,
            )
            .map_err(|_| {
                SourceCommandError::Unsupported("exact native inventory member read unavailable")
            })?;
        let raw = required(ctx, name)?;
        if raw != observed.raw.as_slice() {
            return Err(SourceCommandError::Conflict(
                "selected native inventory bytes differ from anchored cut",
            ));
        }
        remaining = remaining
            .checked_sub(raw.len())
            .ok_or(SourceCommandError::Invalid("native inventory byte budget"))?;
        let packet = cmd::parse(raw)?;
        if cmd::text(&packet, "schema_version")? != "tos_semantic_annotation_packet_v2" {
            return Err(SourceCommandError::Unsupported(
                "native identity packet contract",
            ));
        }
        schema_at(
            worker,
            deadline,
            cancelled,
            ctx,
            &[contract.into()],
            contract,
            &packet,
            Some(name),
            true,
        )?;
        for entity in cmd::array(&packet, "entities")? {
            let entity_id = cmd::text(entity, "entity_id")?;
            if reserved_id == Some(entity_id) {
                return Err(SourceCommandError::Denied(
                    "subject identity belongs to native packet; explicit owner migration required",
                ));
            }
            identities
                .entry(entity_id.to_owned())
                .or_default()
                .push(name.to_owned());
        }
        entries.insert(
            name.to_string(),
            Digest256::of_bytes(raw).to_prefixed()[7..].to_string(),
        );
    }
    let used_schema = !entries.is_empty();
    let value = JsonValue::Object(
        entries
            .into_iter()
            .map(|(name, digest)| (JsonString::from_utf8(&name), cmd::string(&digest)))
            .collect(),
    );
    // Python inventory json.dumps uses ensure_ascii=True, like binding snapshots.
    Ok((identities, python_ascii_digest(&value)?, used_schema))
}

struct NativeBindingReader<'a> {
    ctx: &'a CommandContext,
    cut: &'a CorpusCutReader,
    worker: &'a mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &'a AtomicBool,
    inputs: BTreeMap<String, String>,
    contracts: BTreeSet<String>,
    remaining: usize,
}
impl NativeBindingReader<'_> {
    fn route(name: &str, support: bool, contract: bool, content: bool) -> SourceCommandResult<()> {
        path(name)?;
        let home = if contract {
            "ToS/contracts/"
        } else if support {
            "ToS/"
        } else {
            "ToS/source-witnesses/"
        };
        if !name.starts_with(home)
            || name.starts_with("ToS/source-witnesses/owner-local/")
            || name.split('/').any(|part| {
                part == "catalog" || !content && matches!(part, "payload" | "local-content")
            })
        {
            return Err(SourceCommandError::Denied(
                "native binding reference leaves declared public owner home",
            ));
        }
        Ok(())
    }
    fn read(
        &mut self,
        name: &str,
        expected: Option<&str>,
        support: bool,
        contract: bool,
    ) -> SourceCommandResult<Vec<u8>> {
        Self::route(name, support, contract, false)?;
        let raw = required(self.ctx, name)?;
        let digest = Digest256::of_bytes(raw).to_hex();
        if expected.is_some_and(|expected| expected != digest) {
            return Err(SourceCommandError::Conflict(
                "native binding exact input digest differs",
            ));
        }
        if !self.inputs.contains_key(name) {
            if self.inputs.len() >= 128 || raw.len() > self.remaining.min(1_048_576) {
                return Err(SourceCommandError::Invalid(
                    "native binding metadata dependency budget",
                ));
            }
            let member = self
                .cut
                .read_member(
                    self.ctx.base_revision,
                    &path(name)?,
                    self.remaining.min(1_048_576) as u64,
                    self.deadline,
                    self.cancelled,
                )
                .map_err(|_| {
                    SourceCommandError::Unsupported(
                        "native binding exact source member unavailable",
                    )
                })?;
            if raw != member.raw.as_slice() {
                return Err(SourceCommandError::Conflict(
                    "native binding selected input differs from cut",
                ));
            }
            self.remaining -= raw.len();
            self.inputs.insert(name.into(), digest);
        }
        if contract {
            self.contracts.insert(name.into());
        }
        Ok(raw.to_vec())
    }
    fn record(
        &mut self,
        name: &str,
        expected: Option<&str>,
    ) -> SourceCommandResult<(JsonValue, Vec<u8>)> {
        let raw = self.read(name, expected, false, false)?;
        let value = cmd::parse(&raw)?;
        if value.as_object().is_none() {
            return Err(SourceCommandError::Invalid(
                "native binding metadata object",
            ));
        }
        Ok((value, raw))
    }
    fn validate(
        &mut self,
        value: &JsonValue,
        basename: &str,
        locator: &str,
    ) -> SourceCommandResult<()> {
        let name = format!("ToS/contracts/{basename}");
        let grammar = cmd::parse(&self.read(&name, None, false, true)?)?;
        if cmd::text(&grammar, "$id")? != format!("https://tree-of-sophia.local/{name}") {
            return Err(SourceCommandError::Invalid(
                "native schema identity differs from owner path",
            ));
        }
        native_schema_refs(&grammar, 0)?;
        schema_at(
            self.worker,
            self.deadline,
            self.cancelled,
            self.ctx,
            &[name.clone()],
            &name,
            value,
            Some(locator),
            true,
        )
    }
    fn metadata(&self, raw: &[u8], name: &str, kind: &str) -> SourceCommandResult<()> {
        use tos_validation::text_metadata_rules::{
            self as rules, TextMetadataLimits, TextMetadataState,
        };
        let limits = TextMetadataLimits {
            max_packet_bytes: 1_048_576,
            max_state_bytes: 8_388_608,
            max_issues: 128,
            deadline: self.deadline,
        };
        let report = match kind {
            "unit" => {
                rules::inspect_source_text_unit_v1_metadata(raw, name, limits, self.cancelled)
            }
            "layer" => rules::inspect_source_text_layer_metadata(raw, name, limits, self.cancelled),
            "anchor" => rules::inspect_source_anchor_v2_metadata(raw, name, limits, self.cancelled),
            _ => {
                return Err(SourceCommandError::Unsupported(
                    "native metadata predicate route",
                ));
            }
        }
        .map_err(|_| {
            SourceCommandError::Unsupported("native metadata owner predicate execution unavailable")
        })?;
        if report.packet_digest != Digest256::of_bytes(raw).to_hex()
            || report.scope != "owner-metadata-predicates-only"
        {
            return Err(SourceCommandError::Conflict(
                "native metadata predicate input binding differs",
            ));
        }
        match report.state {
            TextMetadataState::CheckedMetadata if report.issues.is_empty() => Ok(()),
            TextMetadataState::Unsupported => Err(SourceCommandError::Unsupported(
                "native metadata owner predicates need unsupported scalar representation",
            )),
            _ => Err(SourceCommandError::Invalid(
                "native metadata violates owner predicates",
            )),
        }
    }
    fn layer_dependencies(
        &mut self,
        layer: &JsonValue,
        raw: &[u8],
        name: &str,
        visiting: &mut BTreeSet<String>,
    ) -> SourceCommandResult<()> {
        let id = cmd::text(layer, "layer_id")?;
        if visiting.len() >= 16 || !visiting.insert(id.into()) {
            return Err(SourceCommandError::Invalid(
                "native text layer lineage cycle or depth",
            ));
        }
        self.metadata(raw, name, "layer")?;
        let policy = cmd::field(layer, "editorial_policy")?;
        self.read(
            cmd::text(policy, "policy_ref")?,
            Some(cmd::text(policy, "policy_sha256")?),
            true,
            false,
        )?;
        let derivation = cmd::field(layer, "derivation")?;
        let maker = cmd::field(derivation, "maker")?;
        if let Some(configuration) = maker
            .object_get("configuration_ref")
            .filter(|v| **v != JsonValue::Null)
        {
            self.read(
                configuration.as_str().ok_or(SourceCommandError::Invalid(
                    "native maker configuration path",
                ))?,
                Some(cmd::text(maker, "configuration_digest")?),
                true,
                false,
            )?;
        }
        for target in cmd::array(derivation, "input_layers")? {
            let name = cmd::text(target, "record_ref")?;
            let (previous, raw) = self.record(name, Some(cmd::text(target, "record_sha256")?))?;
            self.validate(&previous, "source-text-layer.schema.json", name)?;
            if cmd::field(&previous, "layer_id")? != cmd::field(target, "layer_id")?
                || cmd::field(cmd::field(&previous, "representation")?, "content_sha256")?
                    != cmd::field(target, "content_sha256")?
            {
                return Err(SourceCommandError::Conflict(
                    "native predecessor layer identity or content differs",
                ));
            }
            self.layer_dependencies(&previous, &raw, name, visiting)?;
        }
        visiting.remove(id);
        Ok(())
    }
    fn source_scope(
        &mut self,
        binding: &JsonValue,
        packet: &JsonValue,
        layer: &JsonValue,
    ) -> SourceCommandResult<JsonValue> {
        let scope = cmd::field(packet, "source_scope")?;
        let layer_scope = cmd::field(layer, "source_binding")?;
        let refs = cmd::field(binding, "source_record_refs")?;
        let mut records = BTreeMap::new();
        for kind in ["work", "expression", "edition", "item"] {
            let name = cmd::text(refs, kind)?;
            if name.rsplit('/').next() != Some(format!("{kind}.json").as_str()) {
                return Err(SourceCommandError::Denied(
                    "native source scope metadata kind locator",
                ));
            }
            let (record, _) = self.record(name, None)?;
            self.validate(&record, "corpus-record.schema.json", name)?;
            let key = format!("{kind}_ref");
            if cmd::text(&record, "record_type")? != kind
                || cmd::field(&record, "record_id")? != cmd::field(scope, &key)?
                || cmd::field(layer_scope, &key)? != cmd::field(scope, &key)?
            {
                return Err(SourceCommandError::Conflict(
                    "native source scope identity differs",
                ));
            }
            records.insert(kind, record);
        }
        if cmd::field(&records["expression"], "work_ref")? != cmd::field(scope, "work_ref")?
            || !cmd::array(&records["edition"], "embodies_expression_refs")?
                .contains(cmd::field(scope, "expression_ref")?)
        {
            return Err(SourceCommandError::Conflict(
                "native source scope bibliographic topology differs",
            ));
        }
        let name = cmd::text(&records["item"], "item_manifest_ref")?;
        if split(name)?.0 != split(cmd::text(refs, "item")?)?.0
            || split(name)?.1 != "item.manifest.json"
        {
            return Err(SourceCommandError::Denied(
                "native item manifest leaves exact item",
            ));
        }
        let (manifest, _) = self.record(name, None)?;
        self.validate(&manifest, "source-item-manifest.schema.json", name)?;
        if cmd::field(&manifest, "item_id")? != cmd::field(scope, "item_ref")?
            || cmd::field(&manifest, "embodiment_ref")? != cmd::field(scope, "edition_ref")?
            || cmd::field(layer_scope, "source_file_ref")? != cmd::field(scope, "file_ref")?
            || cmd::field(layer_scope, "source_file_sha256")? != cmd::field(scope, "file_sha256")?
        {
            return Err(SourceCommandError::Conflict(
                "native item or source file binding differs",
            ));
        }
        let matches = cmd::array(&manifest, "payload_files")?
            .iter()
            .filter(|row| row.object_get("file_id") == scope.object_get("file_ref"))
            .collect::<Vec<_>>();
        if matches.len() != 1
            || cmd::field(matches[0], "sha256")? != cmd::field(scope, "file_sha256")?
        {
            return Err(SourceCommandError::Conflict(
                "native source file not unique in exact item manifest",
            ));
        }
        Ok(manifest)
    }
    fn snapshot(self) -> SourceCommandResult<(String, Vec<String>)> {
        let value = JsonValue::Array(
            self.inputs
                .into_iter()
                .map(|(name, digest)| {
                    JsonValue::Array(vec![
                        cmd::string(&name),
                        cmd::string("metadata"),
                        cmd::string(&digest),
                    ])
                })
                .collect(),
        );
        Ok((
            python_ascii_digest(&value)?,
            self.contracts.into_iter().collect(),
        ))
    }
}

// The maintained native resolver has no external-resource retrieval route for
// these self-contained contracts. A worker's wider resource inventory must not
// silently make an undeclared native grammar dependency usable.
fn native_schema_refs(value: &JsonValue, depth: usize) -> SourceCommandResult<()> {
    if depth > 128 {
        return Err(SourceCommandError::Invalid("native schema depth budget"));
    }
    match value {
        JsonValue::Object(fields) => {
            for (name, child) in fields {
                if [Some("$ref"), Some("$dynamicRef"), Some("$recursiveRef")]
                    .contains(&name.as_str())
                    && !child
                        .as_str()
                        .is_some_and(|reference| reference.starts_with('#'))
                {
                    return Err(SourceCommandError::Unsupported(
                        "native binding schema selects undeclared resource",
                    ));
                }
                native_schema_refs(child, depth + 1)?;
            }
        }
        JsonValue::Array(values) => {
            for child in values {
                native_schema_refs(child, depth + 1)?;
            }
        }
        _ => (),
    }
    Ok(())
}

pub(crate) fn python_ascii_digest(value: &JsonValue) -> SourceCommandResult<String> {
    let compact = cmd::canonical(value)?;
    let compact = std::str::from_utf8(&compact)
        .map_err(|_| SourceCommandError::Invalid("native snapshot UTF-8"))?;
    let mut ascii = String::new();
    for ch in compact.chars() {
        if ch.is_ascii() {
            ascii.push(ch);
        } else {
            let mut units = [0u16; 2];
            for unit in ch.encode_utf16(&mut units).iter() {
                use std::fmt::Write;
                write!(&mut ascii, "\\u{unit:04x}")
                    .map_err(|_| SourceCommandError::Invalid("native snapshot emission"))?;
            }
        }
    }
    Ok(Digest256::of_bytes(ascii.as_bytes()).to_prefixed())
}

/// Exact maintained metadata-only adapter. Public gate means recorded posture,
/// not verified content, an authenticated producer, rights judgment or admission.
fn native_text_binding(
    cut: Option<&CorpusCutReader>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &CommandContext,
    binding: &JsonValue,
) -> SourceCommandResult<(String, Vec<String>)> {
    let cut = cut.ok_or(SourceCommandError::Unsupported(
        "native text binding requires anchored source cut",
    ))?;
    if cut.current().revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict("native binding cut differs"));
    }
    let mut reader = NativeBindingReader {
        ctx,
        cut,
        worker,
        deadline,
        cancelled,
        inputs: BTreeMap::new(),
        contracts: BTreeSet::new(),
        remaining: 8_388_608,
    };
    let binding_locator =
        cmd::text(&cmd::parse(&ctx.configuration_raw)?, "source_path")?.to_owned();
    reader.validate(
        binding,
        "native-text-unit-binding.schema.json",
        &binding_locator,
    )?;
    let packet_path = cmd::text(binding, "packet_ref")?;
    let (packet, packet_raw) =
        reader.record(packet_path, Some(cmd::text(binding, "packet_sha256")?))?;
    reader.validate(
        &packet,
        "source-text-unit-packet-v1.schema.json",
        packet_path,
    )?;
    if cmd::text(&packet, "content_posture")? != "source_bound"
        || cmd::field(&packet, "packet_id")? != cmd::field(binding, "packet_id")?
        || cmd::field(&packet, "packet_version")? != cmd::field(binding, "packet_version")?
    {
        return Err(SourceCommandError::Conflict(
            "native packet identity version or posture differs",
        ));
    }
    let layer_binding = cmd::field(binding, "text_layer")?;
    let packet_layer = cmd::field(&packet, "source_layer")?;
    let layer_path = cmd::text(layer_binding, "record_ref")?;
    if cmd::text(packet_layer, "text_layer_ref")? != layer_path {
        return Err(SourceCommandError::Conflict(
            "native packet addresses another text layer",
        ));
    }
    let (layer, layer_raw) =
        reader.record(layer_path, Some(cmd::text(layer_binding, "record_sha256")?))?;
    reader.validate(&layer, "source-text-layer.schema.json", layer_path)?;
    if cmd::field(&layer, "layer_id")? != cmd::field(layer_binding, "layer_id")?
        || cmd::field(&layer, "layer_version")? != cmd::field(layer_binding, "layer_version")?
    {
        return Err(SourceCommandError::Conflict(
            "native text layer identity or version differs",
        ));
    }
    reader.metadata(&packet_raw, packet_path, "unit")?;
    reader.layer_dependencies(&layer, &layer_raw, layer_path, &mut BTreeSet::new())?;
    let manifest = reader.source_scope(binding, &packet, &layer)?;
    let rep = cmd::field(&layer, "representation")?;
    let normalization = cmd::text(rep, "character_normalization")?;
    let unicode_form = if normalization == "none" {
        "source_preserved"
    } else {
        normalization
    };
    if cmd::field(packet_layer, "text_layer_sha256")? != cmd::field(rep, "content_sha256")?
        || cmd::field(packet_layer, "language")? != cmd::field(rep, "language")?
        || cmd::text(packet_layer, "unicode_form")? != unicode_form
        || cmd::field(packet_layer, "visibility")? != cmd::field(rep, "content_visibility")?
        || cmd::field(packet_layer, "publication_authorized")?
            != cmd::field(rep, "publication_authorized")?
        || !["text/plain", "text/plain; charset=utf-8"].contains(&cmd::text(rep, "media_type")?)
    {
        return Err(SourceCommandError::Conflict(
            "native packet and UTF-8 layer declarations differ",
        ));
    }
    let scope = cmd::field(rep, "text_scope")?;
    for anchor in cmd::array(&packet, "anchors")? {
        let selector = cmd::field(anchor, "selector")?;
        if cmd::field(cmd::field(anchor, "source_return")?, "locator_ref")?
            != cmd::field(rep, "content_ref")?
            || !(cmd::integer(scope, "start")? <= cmd::integer(selector, "start")?
                && cmd::integer(selector, "start")? <= cmd::integer(selector, "end")?
                && cmd::integer(selector, "end")? <= cmd::integer(scope, "end")?)
        {
            return Err(SourceCommandError::Conflict(
                "native unit anchor leaves exact representation scope",
            ));
        }
    }
    let source_scope = cmd::field(&packet, "source_scope")?;
    for target in cmd::array(cmd::field(&layer, "source_binding")?, "anchors")? {
        let name = cmd::text(target, "anchor_record_ref")?;
        let (anchor, raw) =
            reader.record(name, Some(cmd::text(target, "anchor_record_sha256")?))?;
        reader.validate(&anchor, "source-anchor-v2.schema.json", name)?;
        reader.metadata(&raw, name, "anchor")?;
        let anchor_target = cmd::field(&anchor, "target")?;
        if cmd::field(&anchor, "anchor_id")? != cmd::field(target, "anchor_id")?
            || cmd::field(anchor_target, "item_id")? != cmd::field(source_scope, "item_ref")?
            || cmd::field(anchor_target, "file_id")? != cmd::field(source_scope, "file_ref")?
            || cmd::field(anchor_target, "file_sha256")? != cmd::field(source_scope, "file_sha256")?
        {
            return Err(SourceCommandError::Conflict(
                "native text layer source anchor binding differs",
            ));
        }
    }
    let rights = cmd::field(&packet, "rights_and_visibility")?;
    let rights_refs = texts(rights, "rights_record_refs", 128)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let declared = cmd::array(rep, "rights_record_refs")?
        .iter()
        .map(|row| cmd::text(row, "ref").map(String::from))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if declared.is_empty()
        || rights_refs != declared
        || !declared.contains(cmd::text(&manifest, "rights_ref")?)
    {
        return Err(SourceCommandError::Conflict(
            "native packet layer and item rights closure differs",
        ));
    }
    let exact_scope = [
        cmd::text(&layer, "layer_id")?,
        cmd::text(rep, "content_file_id")?,
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let mut relevant = exact_scope.clone();
    for (key, value) in source_scope
        .as_object()
        .ok_or(SourceCommandError::Invalid("native source scope"))?
    {
        if key.as_str().is_some_and(|name| name.ends_with("_ref")) {
            relevant.insert(
                value
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("native source scope reference"))?,
            );
        }
    }
    let mut rights_records = Vec::new();
    for target in cmd::array(rep, "rights_record_refs")? {
        let name = cmd::text(target, "ref")?;
        let (record, _) = reader.record(name, Some(cmd::text(target, "sha256")?))?;
        reader.validate(&record, "rights-record.schema.json", name)?;
        let scope = texts(&record, "scope_refs", 4096)?
            .into_iter()
            .collect::<BTreeSet<_>>();
        if !relevant.iter().any(|id| scope.contains(*id))
            || name == cmd::text(&manifest, "rights_ref")?
                && ![
                    cmd::text(source_scope, "item_ref")?,
                    cmd::text(source_scope, "file_ref")?,
                ]
                .iter()
                .all(|id| scope.contains(*id))
        {
            return Err(SourceCommandError::Denied(
                "native rights record lacks exact relevant source scope",
            ));
        }
        rights_records.push((record, scope));
    }
    for target in cmd::array(rep, "publication_authority_refs")? {
        reader.read(
            cmd::text(target, "ref")?,
            Some(cmd::text(target, "sha256")?),
            true,
            false,
        )?;
    }
    let units = cmd::array(&packet, "units")?
        .iter()
        .filter(|row| row.object_get("unit_id") == binding.object_get("unit_id"))
        .collect::<Vec<_>>();
    let segments = cmd::array(&packet, "segmentations")?
        .iter()
        .filter(|row| row.object_get("segmentation_id") == binding.object_get("segmentation_id"))
        .collect::<Vec<_>>();
    if units.len() != 1 || segments.len() != 1 {
        return Err(SourceCommandError::Conflict(
            "native unit or segmentation is not unique",
        ));
    }
    let unit = units[0];
    let segment = segments[0];
    if cmd::field(unit, "unit_version")? != cmd::field(binding, "unit_version")?
        || cmd::field(unit, "ordered_anchor_refs")? != cmd::field(binding, "ordered_anchor_refs")?
        || cmd::text(unit, "surface_posture")? != "source_bearing"
        || cmd::field(segment, "segmentation_version")?
            != cmd::field(binding, "segmentation_version")?
        || !cmd::array(segment, "ordered_unit_refs")?.contains(cmd::field(unit, "unit_id")?)
    {
        return Err(SourceCommandError::Conflict(
            "native unit membership version or ordered anchors differs",
        ));
    }
    if cmd::text(rep, "content_visibility")? != "public"
        || cmd::field(rep, "publication_authorized")? != &JsonValue::Bool(true)
        || cmd::text(rights, "packet_visibility")? != "public"
        || cmd::text(rights, "effective_visibility")? != "public"
        || cmd::field(rights, "publication_authorized")? != &JsonValue::Bool(true)
        || cmd::field(rights, "private_source_used")? != &JsonValue::Bool(false)
    {
        return Err(SourceCommandError::Denied(
            "public profile native binding has no declared public content gate",
        ));
    }
    let narrower = rights_records
        .iter()
        .any(|(_, scope)| exact_scope.iter().any(|id| scope.contains(*id)));
    for (record, scope) in &rights_records {
        if narrower && !exact_scope.iter().any(|id| scope.contains(*id)) {
            continue;
        }
        if !["public_domain_reviewed", "licensed", "permission_granted"]
            .contains(&cmd::text(record, "assessment_status")?)
            || cmd::text(record, "visibility")? != "public_payload"
            || !["authorized", "authorized_with_conditions"]
                .contains(&cmd::text(record, "redistribution_posture")?)
            || !["allowed", "allowed_with_conditions"]
                .contains(&cmd::text(record, "derivative_posture")?)
            || ["legal_review_requested", "superseded"]
                .contains(&cmd::text(record, "review_status")?)
        {
            return Err(SourceCommandError::Denied(
                "native public content declaration conflicts with recorded rights gate",
            ));
        }
    }
    // Validate navigation without opening, executing, hashing or projecting text.
    NativeBindingReader::route(cmd::text(rep, "content_ref")?, false, false, true)?;
    reader.snapshot()
}

/// Reuse the maintained registry constructor law without loading unused profile
/// schemas or confusing a profile declaration with writer authority.
pub(crate) fn validate_source_profile_registry(
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &impl RecordRead<Identity = tos_foundation::SourceRevision>,
) -> SourceCommandResult<JsonValue> {
    let contract = "ToS/contracts/semantic-entity-type-registry.schema.json";
    let registry = cmd::parse(required(ctx, REGISTRY)?)?;
    schema_at(
        worker,
        deadline,
        cancelled,
        ctx,
        &[contract.into()],
        contract,
        &registry,
        Some(REGISTRY),
        true,
    )?;
    let entries = cmd::array(&registry, "types")?;
    let mut entities = BTreeMap::new();
    for entry in entries {
        if entities
            .insert(cmd::text(entry, "type_id")?, entry)
            .is_some()
        {
            return Err(SourceCommandError::Invalid("duplicate entity type"));
        }
    }
    let reserved = [
        "agent",
        "place",
        "organization",
        "work",
        "expression",
        "edition",
        "collection",
        "item",
        "link",
        "artifact",
        "composite",
    ];
    let mut unique: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for entry in entries {
        let Some(profile) = entry.object_get("source_record_profile") else {
            continue;
        };
        let kind = cmd::text(profile, "record_type")?;
        let type_id = cmd::text(entry, "type_id")?;
        let reader = cmd::text(profile, "reader")?;
        let (role, family) = match reader {
            "corpus-metadata-v1" => ("identity", "tos.entity.identity"),
            "semantic-metadata-v1" => ("semantic", "tos.entity.semantic-object"),
            _ => return Err(SourceCommandError::Unsupported("profile reader")),
        };
        let mut parents = BTreeSet::new();
        ancestry(&entities, type_id, &mut BTreeSet::new(), &mut parents)?;
        let retained = kind == "composite"
            && type_id == "tos.entity.composite"
            && cmd::text(profile, "retained_native_adapter").ok() == Some("scholarly-composite-v1")
            && reader == "corpus-metadata-v1"
            && cmd::text(profile, "catalog_filename")? == "composites.jsonl";
        let sign =
            kind == "sign" && type_id == "tos.entity.sign" && reader == "semantic-metadata-v1";
        if cmd::field(entry, "abstract")? != &JsonValue::Bool(false)
            || cmd::text(entry, "object_role")? != role
            || !parents.contains(family)
            || type_id == family
            || !retained
                && (reserved.contains(&kind)
                    || reserved.iter().any(|k| {
                        cmd::text(profile, "source_basename").ok()
                            == Some(format!("{k}.json").as_str())
                    })
                    || reserved.iter().any(|k| {
                        cmd::text(profile, "catalog_filename").ok()
                            == Some(format!("{k}s.jsonl").as_str())
                    })
                    || cmd::text(profile, "catalog_filename")? == "claims.jsonl")
            || cmd::text(profile, "id_prefix")? != format!("tos.{kind}.")
            || cmd::text(profile, "source_basename")? != format!("{kind}.json")
            || profile.object_get("retained_native_adapter").is_some() && !retained
            || profile.object_get("creation_gate").is_some() && !sign
            || sign && cmd::text(profile, "creation_gate").ok() != Some("sign-promotion-v1")
            || (kind == "sign" || type_id == "tos.entity.sign") && !sign
            || profile.object_get("native_binding_adapter").is_some()
                && (reader != "semantic-metadata-v1"
                    || cmd::text(profile, "native_binding_adapter")? != "source-text-unit-v1")
            || profile.object_get("identity_proposal_adapter").is_some()
                && (role != "semantic"
                    || cmd::text(profile, "identity_proposal_adapter")?
                        != "exact-semantic-metadata-v1"
                    || cmd::text(profile, "graph_layer")? != "source-profile"
                    || ["claim", "literal", "temporal-assertion"].contains(&kind))
        {
            return Err(SourceCommandError::Denied(
                "source record profile role, ancestry or reserved adapter collision",
            ));
        }
        for graph in ["source-claims", "source-navigation"] {
            let mappings = cmd::array(entry, "source_mappings")?
                .iter()
                .filter(|mapping| {
                    cmd::text(mapping, "source_graph").ok() == Some(graph)
                        && cmd::text(mapping, "source_kind_id").ok() == Some(kind)
                })
                .count();
            if mappings != 1
                || entries
                    .iter()
                    .filter(|other| cmd::text(other, "type_id").ok() != Some(type_id))
                    .any(|other| {
                        cmd::array(other, "source_mappings").is_ok_and(|mappings| {
                            mappings.iter().any(|mapping| {
                                cmd::text(mapping, "source_graph").ok() == Some(graph)
                                    && cmd::text(mapping, "source_kind_id").ok() == Some(kind)
                            })
                        })
                    })
            {
                return Err(SourceCommandError::Denied(
                    "source profile mapping has another type owner",
                ));
            }
        }
        for field in [
            "record_type",
            "id_prefix",
            "source_basename",
            "catalog_filename",
        ] {
            if !unique
                .entry(field)
                .or_default()
                .insert(cmd::text(profile, field)?)
            {
                return Err(SourceCommandError::Invalid(
                    "duplicate source record profile identity",
                ));
            }
        }
        let mut routes = BTreeSet::new();
        for route in cmd::array(profile, "schemas")? {
            if !routes.insert(cmd::text(route, "schema_version")?) {
                return Err(SourceCommandError::Invalid(
                    "duplicate source profile schema route",
                ));
            }
        }
    }
    Ok(registry)
}

pub(crate) fn public_profile(
    cut: Option<&CorpusCutReader>,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
    ctx: &impl RecordRead<Identity = tos_foundation::SourceRevision>,
    config: &JsonValue,
    record: &JsonValue,
) -> SourceCommandResult<(JsonValue, Vec<String>, Option<String>, Option<String>)> {
    let contract = "ToS/contracts/semantic-entity-type-registry.schema.json";
    let registry = validate_source_profile_registry(worker, deadline, cancelled, ctx)?;
    let entities = cmd::array(&registry, "types")?
        .iter()
        .map(|entry| Ok((cmd::text(entry, "type_id")?, entry)))
        .collect::<SourceCommandResult<BTreeMap<_, _>>>()?;
    let entry = entities
        .get(cmd::text(config, "profile_type_id")?)
        .ok_or(SourceCommandError::Denied("profile type absent"))?;
    let profile = cmd::field(entry, "source_record_profile")?;
    let kind = cmd::text(profile, "record_type")?;
    let source_path = cmd::text(config, "source_path")?;
    let (_, base) = split(source_path)?;
    if base != cmd::text(profile, "source_basename")?
        || cmd::text(record, "record_type")? != kind
        || cmd::text(record, "record_id")? != cmd::text(config, "record_id")?
        || !valid_id(
            cmd::text(record, "record_id")?,
            cmd::text(profile, "id_prefix")?,
            false,
        )
        || !matches!(
            cmd::text(record, "visibility")?,
            "public" | "public_metadata_only"
        )
        || !matches!(
            cmd::text(record, "identity_status")?,
            "provisional" | "verified" | "disputed" | "superseded"
        )
        || strip(cmd::text(record, "preferred_label")?)?.is_empty()
        || cmd::integer(record, "record_version")? == 0
    {
        return Err(SourceCommandError::Denied(
            "public profile exact identity, visibility or metadata",
        ));
    }
    if kind == "composite"
        && (!source_path.starts_with("ToS/source-witnesses/scholarly-composites/")
            || source_path.split('/').count() < 7)
    {
        return Err(SourceCommandError::Denied(
            "retained composite profile path",
        ));
    }
    // Read evidence validates the declared shape only, matching MetadataVersionReader.
    // Writer preparation retains its stronger identity and native binding checks.
    let (native_identity_snapshot, inventory_schema, native_text) =
        if let Some(writer) = ctx.writer() {
            let (snapshot, inventory_schema) = native_identity_inventory(
                cut,
                worker,
                deadline,
                cancelled,
                writer,
                cmd::text(record, "record_id")?,
            )?;
            let native_text = if profile.object_get("native_binding_adapter").is_some() {
                Some(native_text_binding(
                    cut,
                    worker,
                    deadline,
                    cancelled,
                    writer,
                    cmd::field(record, "native_text_binding")?,
                )?)
            } else {
                None
            };
            (snapshot, inventory_schema, native_text)
        } else {
            (None, false, None)
        };
    if profile.object_get("native_binding_adapter").is_none()
        && record.object_get("native_text_binding").is_some()
    {
        return Err(SourceCommandError::Denied(
            "native text binding has no explicit profile adapter",
        ));
    }
    let routes = cmd::array(profile, "schemas")?
        .iter()
        .filter(|route| {
            cmd::text(route, "schema_version").ok() == cmd::text(record, "schema_version").ok()
        })
        .collect::<Vec<_>>();
    if routes.len() != 1 {
        return Err(SourceCommandError::Unsupported(
            "profile exact schema version unavailable",
        ));
    }
    let root = cmd::text(routes[0], "schema_ref")?;
    let mut resources = vec![CORPUS_SCHEMA.to_string()];
    resources.extend(texts(routes[0], "schema_dependencies", 128)?);
    resources.push(root.into());
    let mut seen = BTreeSet::new();
    resources.retain(|r| seen.insert(r.clone()));
    schema_at(
        worker,
        deadline,
        cancelled,
        ctx,
        &resources,
        root,
        record,
        Some(source_path),
        true,
    )?;
    // This exact source contract supplies the shared Corpus property checks
    // through the same disposable worker, without a synthetic schema issuer.
    let shared = "ToS/contracts/source-metadata-record.schema.json";
    if !resources.iter().any(|path| path == shared) {
        return Err(SourceCommandError::Unsupported(
            "profile shared metadata worker contract absent from exact declared route",
        ));
    }
    schema_at(
        worker,
        deadline,
        cancelled,
        ctx,
        &resources,
        shared,
        record,
        Some(source_path),
        true,
    )?;
    if inventory_schema {
        resources.push("ToS/contracts/semantic-annotation-packet-v2.schema.json".into());
    }
    let native_text_snapshot = native_text.map(|(snapshot, contracts)| {
        for name in contracts {
            if !resources.contains(&name) {
                resources.push(name);
            }
        }
        snapshot
    });
    resources.insert(0, contract.into());
    resources.insert(0, REGISTRY.into());
    if resources.len() > 128
        || resources
            .iter()
            .map(|name| required(ctx, name).map(<[u8]>::len))
            .collect::<SourceCommandResult<Vec<_>>>()?
            .into_iter()
            .sum::<usize>()
            > 8_388_608
    {
        return Err(SourceCommandError::Invalid(
            "source profile contract snapshot budget",
        ));
    }
    Ok((
        profile.clone(),
        resources,
        native_identity_snapshot,
        native_text_snapshot,
    ))
}

/// Resolve an exact metadata version through its selected current source home
/// and complete retained record-revision predecessor closure. This is a read
/// seam for typed Claim owners, with the same concrete source-cut worker.
/// Compound parent receipts require their operation-specific verifier and
/// currently refuse explicitly; transport evidence cannot substitute for it.
pub fn resolve_record_version(
    ctx: &CommandContext,
    exact: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, String)> {
    let resolved =
        resolve_record_version_selected(ctx, None, None, None, exact, worker, deadline, cancelled)?;
    Ok((resolved.record, resolved.source_path))
}

pub(crate) fn resolve_record_version_from_cut(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    exact: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, String)> {
    if cut.current().revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "metadata resolver selected cut differs",
        ));
    }
    let resolved = resolve_record_version_selected(
        ctx,
        Some(cut),
        None,
        None,
        exact,
        worker,
        deadline,
        cancelled,
    )?;
    Ok((resolved.record, resolved.source_path))
}

/// Evidence produced by the same exact-version read and retained archive
/// verification. It describes a source version; it grants no current use.
pub struct ResolvedRecordVersion {
    pub record: JsonValue,
    pub current_record: Option<JsonValue>,
    pub source_path: String,
    pub current_ref: JsonValue,
    pub version_status: &'static str,
    pub source: JsonValue,
    pub history: JsonValue,
    pub transition: JsonValue,
    pub route_profile: JsonValue,
    /// Present only after the readonly owner-registry descriptor check.
    pub descriptor: Option<JsonValue>,
    pub bytes_read: u64,
    pub returned_state_bytes: usize,
    pub reads: Vec<PredicateRead>,
}

impl ResolvedRecordVersion {
    fn readonly_descriptor(&self, registry: &JsonValue) -> SourceCommandResult<JsonValue> {
        let kind = cmd::text(&self.route_profile, "record_type")?;
        let schema_version = cmd::text(&self.record, "schema_version")?;
        let native_witness = matches!(
            schema_version,
            "tos_artifact_source_witness_v1"
                | "tos_artifact_source_witness_v2"
                | "tos_scholarly_composite_witness_v1"
        );
        let native_link = schema_version == "tos_source_link_v1";
        let native_corpus = schema_version == "tos_corpus_record_v1";
        let (adapter, identity_field) = if native_witness {
            (
                "native-witness",
                if kind == "artifact" {
                    "artifact_id"
                } else {
                    "composite_id"
                },
            )
        } else if native_link {
            ("native-link", "record_id")
        } else if native_corpus {
            ("native-corpus", "record_id")
        } else {
            ("declared-profile", "record_id")
        };
        let entries = cmd::array(registry, "types")?
            .iter()
            .filter(|entry| {
                if native_witness || native_link || native_corpus {
                    cmd::array(entry, "source_mappings").is_ok_and(|mappings| {
                        mappings.iter().any(|mapping| {
                            cmd::text(mapping, "source_graph").ok() == Some("source-navigation")
                                && cmd::text(mapping, "source_kind_id").ok() == Some(kind)
                        })
                    })
                } else {
                    entry
                        .object_get("source_record_profile")
                        .is_some_and(|profile| cmd::text(profile, "record_type").ok() == Some(kind))
                }
            })
            .collect::<Vec<_>>();
        if entries.len() != 1 {
            return Err(SourceCommandError::Conflict(
                "metadata kind lacks exact owner-registry mapping",
            ));
        }
        let type_id = cmd::text(entries[0], "type_id")?;
        let schema_ref = if adapter == "declared-profile" {
            let routes = cmd::array(cmd::field(entries[0], "source_record_profile")?, "schemas")?
                .iter()
                .filter(|route| cmd::text(route, "schema_version").ok() == Some(schema_version))
                .collect::<Vec<_>>();
            if routes.len() != 1 {
                return Err(SourceCommandError::Unsupported(
                    "metadata source schema not supported",
                ));
            }
            cmd::text(routes[0], "schema_ref")?
        } else {
            cmd::text(&self.route_profile, "schema_ref")?
        };
        // Native Corpus keeps its metadata-only reader contract. Other
        // adapters preserve the selected record's declared public visibility,
        // matching the access consumer's descriptor consistency law.
        let source_scope = if native_corpus {
            "public_metadata_only"
        } else if self.record.object_get("visibility").is_some() {
            let visibility = cmd::text(&self.record, "visibility")?;
            if !matches!(visibility, "public" | "public_metadata_only") {
                return Err(SourceCommandError::Denied("metadata descriptor visibility"));
            }
            visibility
        } else {
            "public_metadata_only"
        };
        Ok(cmd::object(vec![
            ("adapter", cmd::string(adapter)),
            ("record_type", cmd::string(kind)),
            ("profile_type_id", cmd::string(type_id)),
            ("source_schema_ref", cmd::string(schema_ref)),
            ("source_schema_version", cmd::string(schema_version)),
            ("source_scope", cmd::string(source_scope)),
            ("record_kind", cmd::string("subject")),
            ("identity_field", cmd::string(identity_field)),
            (
                "source_basename",
                cmd::field(&self.route_profile, "source_basename")?.clone(),
            ),
            ("schema_version", cmd::string(schema_version)),
            ("schema_ref", cmd::string(schema_ref)),
            ("type_id", cmd::string(type_id)),
        ]))
    }

    /// Build the maintained MetadataVersionReader provenance envelope from
    /// verified observations. The transport supplies its checked catalog provenance.
    pub fn readonly_provenance(&self, catalog: &JsonValue) -> SourceCommandResult<JsonValue> {
        let descriptor = self
            .descriptor
            .as_ref()
            .ok_or(SourceCommandError::Unsupported(
                "readonly descriptor absent",
            ))?;
        let mut source = self.source.clone();
        let raw_bytes = cmd::field(&source, "record_bytes")?.clone();
        // Select exact Python fields without changing internal writer evidence.
        source = cmd::object(
            [
                "source_ref",
                "record_sha256",
                "archive_blob_ref",
                "archive_manifest_ref",
                "archive_manifest_sha256",
                "package_revision",
            ]
            .iter()
            .map(|key| Ok((*key, cmd::field(&source, key)?.clone())))
            .collect::<SourceCommandResult<Vec<_>>>()?,
        );
        cmd::set(&mut source, "record_bytes", raw_bytes)?;
        let history = cmd::object(
            [
                "source_ref",
                "sha256",
                "receipt_count",
                "retained_record_chain_verified",
                "retained_baseline_ref",
            ]
            .iter()
            .map(|key| Ok((*key, cmd::field(&self.history, key)?.clone())))
            .collect::<SourceCommandResult<Vec<_>>>()?,
        );
        Ok(cmd::object(vec![
            ("verification_scope", cmd::string("selected-record-chain")),
            ("all_package_bytes_verified", JsonValue::Bool(false)),
            ("catalog", catalog.clone()),
            ("descriptor", descriptor.clone()),
            ("history", history),
            ("source", source),
            ("transition", self.transition.clone()),
        ]))
    }
}

pub(crate) fn resolve_record_version_evidence(
    ctx: &CommandContext,
    exact: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ResolvedRecordVersion> {
    resolve_record_version_selected(ctx, None, None, None, exact, worker, deadline, cancelled)
}

pub(crate) fn resolve_record_version_evidence_from_cut(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    exact: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ResolvedRecordVersion> {
    if cut.current().revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "metadata evidence selected cut differs",
        ));
    }
    resolve_record_version_selected(
        ctx,
        Some(cut),
        None,
        None,
        exact,
        worker,
        deadline,
        cancelled,
    )
}

/// The Claim inventory has already proved global selected identity uniqueness.
/// Read the exact borrowed owner path without cloning or rescanning its cut.
pub(crate) fn resolve_record_version_evidence_at_from_cut(
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    owner_path: &str,
    collection_limits: ItemLimits,
    exact: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ResolvedRecordVersion> {
    if cut.current().revision() != ctx.base_revision {
        return Err(SourceCommandError::Conflict(
            "metadata owner selected cut differs",
        ));
    }
    resolve_record_version_selected(
        ctx,
        Some(cut),
        Some(owner_path),
        Some(collection_limits),
        exact,
        worker,
        deadline,
        cancelled,
    )
}

/// The Claim inventory supplies an exact selected physical owner path and
/// rejects duplicate identities before it returns. Keep bounded history/copy
/// observations without another global selected-file discovery pass.
pub(crate) fn resolve_record_version_evidence_at_selected(
    ctx: &CommandContext,
    owner_path: &str,
    limits: ItemLimits,
    exact: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ResolvedRecordVersion> {
    resolve_record_version_selected(
        ctx,
        None,
        Some(owner_path),
        Some(limits),
        exact,
        worker,
        deadline,
        cancelled,
    )
}

/// Resolve the exact current or retained metadata version at a catalog-selected
/// owner path. The caller owns live source-root publication/currentness checks.
/// This returns evidence only and never prepares a writer operation.
pub fn resolve_record_version_readonly(
    input: &RecordVersionReadInput<'_>,
    owner_path: &str,
    limits: ItemLimits,
    exact: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ResolvedRecordVersion> {
    input.check()?;
    if input.schema_source_path != owner_path {
        return Err(SourceCommandError::Conflict(
            "read schema locator differs from selected owner",
        ));
    }
    let mut resolved = resolve_record_version_selected(
        input,
        None,
        Some(owner_path),
        Some(limits),
        exact,
        worker,
        deadline,
        cancelled,
    )?;
    let registry = validate_source_profile_registry(worker, deadline, cancelled, input)?;
    resolved.descriptor = Some(resolved.readonly_descriptor(&registry)?);
    Ok(resolved)
}

fn resolve_record_version_selected(
    ctx: &impl RecordRead<Identity = tos_foundation::SourceRevision>,
    cut: Option<&CorpusCutReader>,
    selected_path: Option<&str>,
    collection_limits: Option<ItemLimits>,
    exact: &JsonValue,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ResolvedRecordVersion> {
    // The selected-owner route is reached only through a whole Claim
    // inventory which rejects duplicate identities before returning. Rechecking
    // its entire file vector for every participant would turn one bounded
    // preparation into N full scans.
    if selected_path.is_none() {
        ctx.check()?;
    }
    exact_ref(exact)?;
    if worker.source_revision() != ctx.input_identity() {
        return Err(SourceCommandError::Conflict(
            "metadata resolver worker and source cut differ",
        ));
    }
    let (location, record, subject) = if let Some(location) = selected_path {
        if ctx.writer().is_some() {
            if !location.starts_with("ToS/source-witnesses/")
                || !location.ends_with(".json")
                || location.split('/').any(|part| {
                    part.starts_with('.')
                        || matches!(
                            part,
                            "owner-local" | "catalog" | "payload" | "local-content" | "private"
                        )
                })
            {
                return Err(SourceCommandError::Denied("selected metadata owner path"));
            }
        } else {
            selected_metadata_path(location)?;
        }
        let record = cmd::parse(required(ctx, location)?)?;
        let subject = source_forms::metadata_subject(&record)?;
        if cmd::field(&subject, "id")? != cmd::field(exact, "id")? {
            return Err(SourceCommandError::Conflict(
                "selected metadata owner identity",
            ));
        }
        (location, record, subject)
    } else {
        let mut matches = Vec::new();
        for file in ctx.files() {
            let location = file.path.as_str();
            if !location.starts_with("ToS/source-witnesses/")
                || !location.ends_with(".json")
                || location.ends_with(".human-forms.json")
                || location.ends_with(HISTORY)
                || location.split('/').any(|part| {
                    part.starts_with('.')
                        || matches!(
                            part,
                            "owner-local" | "catalog" | "payload" | "local-content" | "private"
                        )
                })
            {
                continue;
            }
            let record = cmd::parse(&file.raw)?;
            if let Ok(subject) = source_forms::metadata_subject(&record) {
                if cmd::field(&subject, "id")? == cmd::field(exact, "id")? {
                    matches.push((location, record, subject));
                }
            }
        }
        if matches.len() != 1 {
            return Err(SourceCommandError::Conflict(
                "exact metadata identity has no unique selected current owner",
            ));
        }
        matches
            .pop()
            .ok_or(SourceCommandError::Conflict("selected metadata missing"))?
    };
    let schema_version = cmd::text(&record, "schema_version")?;
    let mut descriptor = cmd::object(vec![
        ("source_path", cmd::string(location)),
        ("record_id", cmd::field(&subject, "id")?.clone()),
    ]);
    let family = match schema_version {
        "tos_historical_record_v1" => RevisionFamily::Historical,
        "tos_corpus_record_v1" => {
            cmd::set(
                &mut descriptor,
                "record_type",
                cmd::field(&record, "record_type")?.clone(),
            )?;
            RevisionFamily::CorpusSelectedV3
        }
        "tos_artifact_source_witness_v1"
        | "tos_artifact_source_witness_v2"
        | "tos_scholarly_composite_witness_v1"
        | "tos_source_link_v1" => {
            let kind = if schema_version.starts_with("tos_artifact_") {
                "artifact"
            } else if schema_version == "tos_scholarly_composite_witness_v1" {
                "composite"
            } else {
                "link"
            };
            cmd::set(&mut descriptor, "record_type", cmd::string(kind))?;
            cmd::set(
                &mut descriptor,
                "record_schema_version",
                cmd::string(schema_version),
            )?;
            RevisionFamily::NativeSelected
        }
        _ => {
            let registry = cmd::parse(required(ctx, REGISTRY)?)?;
            let entries = cmd::array(&registry, "types")?
                .iter()
                .filter(|entry| {
                    entry
                        .object_get("source_record_profile")
                        .and_then(|p| cmd::text(p, "record_type").ok())
                        == cmd::text(&record, "record_type").ok()
                })
                .collect::<Vec<_>>();
            if entries.len() != 1 {
                return Err(SourceCommandError::Unsupported(
                    "exact metadata schema has no unique declared profile",
                ));
            }
            cmd::set(
                &mut descriptor,
                "profile_type_id",
                cmd::field(entries[0], "type_id")?.clone(),
            )?;
            RevisionFamily::PublicProfile
        }
    };
    // This descriptor routes a read. It is never evaluated as a protected
    // configuration or used to manufacture a grant or preparation plan.
    let (route_profile, _, _, _) = profile(
        cut,
        worker,
        deadline,
        cancelled,
        ctx,
        &descriptor,
        family,
        &record,
    )?;
    if family == RevisionFamily::CorpusSelectedV3
        && cmd::text(&record, "record_type")? == "collection"
        && let Some(cut) = cut
    {
        let exact_raw = cmd::canonical(exact)?;
        let exact_serde: serde_json::Value = serde_json::from_slice(&exact_raw)
            .map_err(|_| SourceCommandError::Invalid("Collection exact reference conversion"))?;
        let limits = collection_limits.unwrap_or(ItemLimits {
            max_member_bytes: 8_388_608,
            max_total_bytes: 33_554_432,
            max_state_bytes: 33_554_432,
            max_issues: 256,
            deadline,
        });
        let observed = tos_validation::native_compound::verify_collection_version_from_cut(
            cut,
            worker,
            location,
            &exact_serde,
            limits,
            cancelled,
        )
        .map_err(|reason| SourceCommandError::SchemaExecution {
            path: location.to_owned(),
            root: "selected Collection historical version".to_owned(),
            reason,
        })?;
        if observed.source_path != location {
            return Err(SourceCommandError::Conflict(
                "Collection version verifier selected another owner",
            ));
        }
        // The verifier's returned objects remain live while the Foundation
        // representation and short serde transport buffers are constructed.
        let mut returned_state_bytes = observed.returned_state_bytes;
        let mut account_conversion = |bytes: usize| -> SourceCommandResult<()> {
            returned_state_bytes = returned_state_bytes
                .checked_add(bytes)
                .and_then(|total| total.checked_add(bytes))
                .filter(|total| *total <= limits.max_state_bytes)
                .ok_or(SourceCommandError::Unsupported(
                    "Collection conversion state budget",
                ))?;
            Ok(())
        };
        let current_ref_wire = serde_json::to_vec(&observed.current_ref)
            .map_err(|_| SourceCommandError::Invalid("Collection current reference conversion"))?;
        account_conversion(current_ref_wire.len())?;
        let current_ref = cmd::parse(&current_ref_wire)?;
        let transition = match observed.transition {
            Some(receipt) => {
                let receipt_wire = serde_json::to_vec(&receipt)
                    .map_err(|_| SourceCommandError::Invalid("Collection transition conversion"))?;
                account_conversion(receipt_wire.len())?;
                let receipt = cmd::parse(&receipt_wire)?;
                cmd::object(
                    [
                        "command_id",
                        "recorded_at",
                        "previous_source",
                        "source",
                        "request_digest",
                    ]
                    .iter()
                    .map(|key| Ok((*key, cmd::field(&receipt, key)?.clone())))
                    .collect::<SourceCommandResult<Vec<_>>>()?,
                )
            }
            None => JsonValue::Null,
        };
        let historical = observed.version_status == "historical";
        let source = cmd::object(vec![
            ("source_ref", cmd::string(location)),
            (
                "record_bytes",
                cmd::number(observed.record_raw.len() as u64),
            ),
            (
                "record_sha256",
                cmd::string(&Digest256::of_bytes(&observed.record_raw).to_prefixed()),
            ),
            (
                "archive_blob_ref",
                observed
                    .archive_blob_ref
                    .as_deref()
                    .map(cmd::string)
                    .unwrap_or(JsonValue::Null),
            ),
            (
                "archive_manifest_ref",
                observed
                    .archive_manifest_ref
                    .as_deref()
                    .map(cmd::string)
                    .unwrap_or(JsonValue::Null),
            ),
            (
                "archive_manifest_sha256",
                observed
                    .archive_manifest_sha256
                    .as_deref()
                    .map(cmd::string)
                    .unwrap_or(JsonValue::Null),
            ),
            (
                "package_revision",
                observed
                    .package_revision
                    .as_deref()
                    .map(cmd::string)
                    .unwrap_or(JsonValue::Null),
            ),
        ]);
        let baseline_wire = serde_json::to_vec(&observed.retained_baseline_ref)
            .map_err(|_| SourceCommandError::Invalid("Collection history baseline conversion"))?;
        account_conversion(baseline_wire.len())?;
        let history = cmd::object(vec![
            (
                "source_ref",
                observed
                    .history_ref
                    .as_deref()
                    .map(cmd::string)
                    .unwrap_or(JsonValue::Null),
            ),
            (
                "sha256",
                observed
                    .history_sha256
                    .as_deref()
                    .map(cmd::string)
                    .unwrap_or(JsonValue::Null),
            ),
            (
                "receipt_count",
                cmd::number(observed.history_receipt_count as u64),
            ),
            ("retained_record_chain_verified", JsonValue::Bool(true)),
            (
                "all_package_bytes_verified",
                JsonValue::Bool(ctx.writer().is_some()),
            ),
            ("retained_baseline_ref", cmd::parse(&baseline_wire)?),
        ]);
        account_conversion(observed.record_raw.len())?;
        let historical_record = cmd::parse(&observed.record_raw)?;
        return Ok(ResolvedRecordVersion {
            record: historical_record,
            current_record: historical.then_some(record),
            source_path: location.to_owned(),
            current_ref,
            version_status: observed.version_status,
            source,
            history,
            transition,
            route_profile,
            descriptor: None,
            bytes_read: observed.bytes_read,
            returned_state_bytes,
            reads: observed.reads,
        });
    }
    let files = package(
        ctx,
        location,
        true,
        collection_limits.map(|limits| limits.max_state_bytes.min(limits.max_total_bytes as usize)),
    )?;
    let selected_bytes_read = files
        .values()
        .try_fold(0u64, |sum, raw| sum.checked_add(raw.len() as u64))
        .ok_or(SourceCommandError::Unsupported(
            "selected metadata read byte overflow",
        ))?;
    let history_allowance = collection_limits
        .map(|limits| {
            limits
                .max_total_bytes
                .checked_sub(selected_bytes_read)
                .ok_or(SourceCommandError::Unsupported(
                    "selected metadata read budget",
                ))
        })
        .transpose()?;
    let (retained, history_bytes_read) = verify_history_accounted(
        ctx,
        &descriptor,
        &files,
        &record,
        history_allowance,
        collection_limits.map(|limits| {
            limits
                .max_state_bytes
                .saturating_sub(selected_bytes_read as usize)
        }),
        ctx.writer().is_none().then_some((deadline, cancelled)),
    )?;
    let bytes_read = selected_bytes_read.checked_add(history_bytes_read).ok_or(
        SourceCommandError::Unsupported("metadata read byte overflow"),
    )?;
    let (parent, base) = split(location)?;
    let history_raw = files.get(HISTORY);
    let history_receipts = cmd::array(&retained, "receipts")?;
    let baseline = history_receipts
        .first()
        .map(|receipt| cmd::field(receipt, "previous_source"))
        .transpose()?
        .unwrap_or(&subject);
    let history_evidence = cmd::object(vec![
        (
            "source_ref",
            history_raw
                .map(|_| cmd::string(&format!("{parent}/{HISTORY}")))
                .unwrap_or(JsonValue::Null),
        ),
        (
            "sha256",
            history_raw
                .map(|raw| cmd::string(&Digest256::of_bytes(raw).to_prefixed()))
                .unwrap_or(JsonValue::Null),
        ),
        ("receipt_count", cmd::number(history_receipts.len() as u64)),
        ("retained_record_chain_verified", JsonValue::Bool(true)),
        (
            "all_package_bytes_verified",
            JsonValue::Bool(ctx.writer().is_some()),
        ),
        ("retained_baseline_ref", baseline.clone()),
    ]);
    let source_evidence = |raw: &[u8], archive: Option<(&str, &str, &str)>| {
        cmd::object(vec![
            ("source_ref", cmd::string(location)),
            ("record_bytes", cmd::number(raw.len() as u64)),
            (
                "record_sha256",
                cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
            ),
            (
                "archive_blob_ref",
                archive
                    .map(|(blob, _, _)| cmd::string(blob))
                    .unwrap_or(JsonValue::Null),
            ),
            (
                "archive_manifest_ref",
                archive
                    .map(|(_, manifest, _)| cmd::string(manifest))
                    .unwrap_or(JsonValue::Null),
            ),
            (
                "archive_manifest_sha256",
                archive
                    .map(|(_, _, digest)| cmd::string(digest))
                    .unwrap_or(JsonValue::Null),
            ),
            ("package_revision", JsonValue::Null),
        ])
    };
    if cmd::same(&subject, exact)? {
        let raw = files
            .get(base)
            .ok_or(SourceCommandError::Conflict("current record absent"))?;
        return Ok(ResolvedRecordVersion {
            current_record: None,
            record,
            source_path: location.into(),
            current_ref: subject,
            version_status: "current",
            source: source_evidence(raw, None),
            history: history_evidence,
            transition: JsonValue::Null,
            route_profile,
            descriptor: None,
            bytes_read,
            returned_state_bytes: 0,
            reads: Vec::new(),
        });
    }
    for receipt in history_receipts {
        if cmd::same(cmd::field(receipt, "previous_source")?, exact)? {
            let (archived, locations) = read_archive_bounded(
                ctx,
                &descriptor,
                receipt,
                collection_limits.map(|limits| limits.max_total_bytes.saturating_sub(bytes_read)),
                collection_limits.map(|limits| {
                    limits
                        .max_state_bytes
                        .saturating_sub(selected_bytes_read as usize)
                }),
            )?;
            let raw = archived.get(base).ok_or(SourceCommandError::Conflict(
                "retained metadata record absent",
            ))?;
            let archived_record = cmd::parse(raw)?;
            let blob = cmd::text(cmd::field(&locations, base)?, "archive_path")?;
            let manifest = format!("{}/manifest.json", cmd::text(receipt, "archive_path")?);
            let manifest_digest = Digest256::of_bytes(required(ctx, &manifest)?).to_prefixed();
            let mut source = source_evidence(raw, Some((blob, &manifest, &manifest_digest)));
            cmd::set(
                &mut source,
                "package_revision",
                cmd::field(receipt, "previous_revision")?.clone(),
            )?;
            let transition = cmd::object(
                [
                    "command_id",
                    "recorded_at",
                    "previous_source",
                    "source",
                    "request_digest",
                ]
                .iter()
                .map(|key| Ok((*key, cmd::field(receipt, key)?.clone())))
                .collect::<SourceCommandResult<Vec<_>>>()?,
            );
            let selected_archive_bytes = archived
                .values()
                .try_fold(0u64, |sum, raw| sum.checked_add(raw.len() as u64))
                .ok_or(SourceCommandError::Unsupported(
                    "metadata archive read byte overflow",
                ))?;
            let manifest_bytes = required(ctx, &manifest)?.len() as u64;
            let selected_archive_read_bytes = bytes_read
                .checked_add(selected_archive_bytes)
                .and_then(|sum| sum.checked_add(manifest_bytes))
                .ok_or(SourceCommandError::Unsupported(
                    "metadata archive read byte overflow",
                ))?;
            return Ok(ResolvedRecordVersion {
                record: archived_record,
                current_record: Some(record.clone()),
                source_path: location.into(),
                current_ref: subject,
                version_status: "historical",
                source,
                history: history_evidence,
                transition,
                route_profile,
                descriptor: None,
                bytes_read: selected_archive_read_bytes,
                returned_state_bytes: 0,
                reads: Vec::new(),
            });
        }
    }
    if ctx.writer().is_none() {
        let requested_version = cmd::integer(exact, "version")?;
        let present = cmd::integer(&subject, "version")? == requested_version
            || history_receipts.iter().any(|receipt| {
                cmd::field(receipt, "previous_source")
                    .and_then(|reference| cmd::integer(reference, "version"))
                    .is_ok_and(|version| version == requested_version)
            });
        return Err(if present {
            SourceCommandError::Conflict("exact-version-digest-mismatch")
        } else {
            SourceCommandError::Unsupported("exact-version-not-retained")
        });
    }
    Err(SourceCommandError::Conflict(
        "requested metadata version is not retained in exact owner history",
    ))
}
