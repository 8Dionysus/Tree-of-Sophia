//! Bounded PostgreSQL preparation for one initial native Corpus record.
//!
//! This module reuses the maintained creation semantics and serializer while
//! replacing only the complete-inventory absence/identity checks with typed
//! PG observations from the exact source read session.

use super::{
    CreationFamily, PreparedCreation, configuration, initial_corpus_with_schema,
    prepare_source_forms,
};
use crate::source_cohort::revision::{
    PgRecordRevisionIdentity, PgSourceHomeObservation, PgSourceUniqueIndexObservation,
};
use crate::source_cohort::{IndexRows, creation};
use crate::source_command::{
    self as cmd, CommandContext, CommandPlan, SourceCommandError, SourceCommandResult,
};
use crate::source_revisions::{self as revisions, PgRecordRevisionCommandContext};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, RelativePath};
use tos_source_store::{SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::source_cut::CandidateCutWorkerSchemaExecutor;

const CORPUS_DEPENDENCY_PROFILE: &str = "tos_initial_corpus_dependency_commitment_v2";
const AGENT_DEPENDENCY_PROFILE_COMPAT: &str = "tos_initial_agent_dependency_commitment_v2";
pub(crate) const PG_CORPUS_V1_FAMILY_TAG: &str =
    "tos-pg-corpus-create-v1:tos_local_corpus_create_owner_v1:source.create";
pub(crate) const PG_CORPUS_V2_FAMILY_TAG: &str =
    "tos-pg-corpus-create-v2:tos_local_corpus_create_owner_v2:source.create";
const CORPUS_REQUIRED_SOURCE_INPUTS: &[&str] = &[
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/corpus-record.schema.json",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/knowledge-assessment.schema.json",
    "ToS/contracts/human-form-template.schema.json",
    "ToS/contracts/provenance-event-v2.schema.json",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PgSourceUniqueIndexRequirement {
    pub(crate) kind: String,
    pub(crate) token: String,
}

/// Bounded source observations requested before semantic preparation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PgCorpusCreationRequirements {
    pub(crate) home: RelativePath,
    pub(crate) record_type: String,
    /// Literal ToS source references only; non-ToS bibliographic references
    /// remain authored values and are not turned into transport paths.
    pub(crate) source_refs: Vec<RelativePath>,
    pub(crate) required_source_inputs: Vec<RelativePath>,
    pub(crate) unique_tokens: Vec<PgSourceUniqueIndexRequirement>,
}

// Retained for the existing parent-module wrapper while callers move to the
// family-aware requirement type.
pub(crate) type PgAgentCreationRequirements = PgCorpusCreationRequirements;

#[derive(Clone, Debug)]
pub(crate) struct PgCorpusCreationObservations {
    pub(crate) record_type: String,
    pub(crate) home: PgSourceHomeObservation,
    pub(crate) unique: Vec<PgSourceUniqueIndexObservation>,
    pub(crate) dependency_generation: u64,
}

/// Parse the request and owner grant far enough to select the exact bounded
/// source observations needed for the requested initial Corpus record.
pub(crate) fn requirements(
    context: &CommandContext,
) -> SourceCommandResult<PgCorpusCreationRequirements> {
    let (family, config, home) = configuration(context)?;
    if !matches!(family, CreationFamily::CorpusV1 | CreationFamily::CorpusV2) {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL initial Corpus owner",
        ));
    }
    let request = cmd::parse(&context.request_raw)?;
    let operation = cmd::text(&request, "operation")?;
    let mut keys = vec!["schema_version", "operation", "record", "forms"];
    if operation == family.operation() {
        keys.extend([
            "command_id",
            "expected_configuration",
            "expected_source",
            "expected_revision",
            "expected_dependencies",
        ]);
    } else if operation != "prepare-create" {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL initial Corpus creation operation",
        ));
    }
    cmd::exact_keys(&request, &keys)?;
    if cmd::text(&request, "schema_version")? != "tos_local_source_command_v1"
        || !cmd::array(&config, "allowed_operations")?
            .iter()
            .any(|value| value.as_str() == Some(family.operation()))
    {
        return Err(SourceCommandError::Denied(
            "PostgreSQL initial Corpus operation grant",
        ));
    }
    let record = cmd::field(&request, "record")?;
    // Delegate type, path, identity, assertion and schema-profile gates to
    // the maintained semantic Corpus owner before selecting PG observations.
    initial_corpus_with_schema(&config, family, record, |_, _, _| Ok(()))?;
    let record_type = cmd::text(record, "record_type")?.to_owned();
    if cmd::text(&config, "source_path")? != format!("{}/{}.json", home.as_str(), record_type) {
        return Err(SourceCommandError::Denied(
            "PostgreSQL initial Corpus source home is not exact",
        ));
    }
    let mut source_refs = BTreeSet::new();
    for reference in cmd::array(record, "source_refs")? {
        let value = reference
            .as_str()
            .ok_or(SourceCommandError::Invalid("Corpus source reference"))?;
        if value.starts_with("ToS/") {
            if value
                .split('/')
                .any(|part| ["owner-local", "payload", "local-content"].contains(&part))
            {
                return Err(SourceCommandError::Denied(
                    "PG Corpus source reference addresses private content",
                ));
            }
            source_refs.insert(
                RelativePath::parse(value)
                    .map_err(|_| SourceCommandError::Invalid("Corpus source reference path"))?,
            );
        }
    }
    let selections = cmd::array(&request, "forms")?;
    if selections.is_empty() || selections.len() > 32 {
        return Err(SourceCommandError::Invalid(
            "creation form selections budget",
        ));
    }
    let mut unique = BTreeSet::new();
    unique.insert((
        "metadata".to_owned(),
        cmd::text(record, "record_id")?.to_owned(),
    ));
    unique.insert((
        "event".to_owned(),
        cmd::text(&config, "provenance_event_id")?.to_owned(),
    ));
    for selection in selections {
        cmd::exact_keys(selection, &["form_id", "field_id"])?;
        unique.insert((
            "form".to_owned(),
            cmd::text(selection, "form_id")?.to_owned(),
        ));
    }
    if unique.len() != selections.len() + 2 {
        return Err(SourceCommandError::Invalid(
            "duplicate PostgreSQL Corpus unique identity token",
        ));
    }
    Ok(PgCorpusCreationRequirements {
        home,
        record_type,
        source_refs: source_refs.into_iter().collect(),
        required_source_inputs: CORPUS_REQUIRED_SOURCE_INPUTS
            .iter()
            .map(|path| {
                RelativePath::parse(path)
                    .map_err(|_| SourceCommandError::Invalid("Corpus schema input path"))
            })
            .collect::<SourceCommandResult<Vec<_>>>()?,
        unique_tokens: unique
            .into_iter()
            .map(|(kind, token)| PgSourceUniqueIndexRequirement { kind, token })
            .collect(),
    })
}

/// Sealed semantic preparation; it carries no durable registration authority.
pub(crate) struct PreparedPgCorpusCreation {
    prepared: PreparedCreation,
    identity: PgRecordRevisionIdentity,
    observations: PgCorpusCreationObservations,
    record_type: String,
    commitment: Digest256,
}

impl PreparedPgCorpusCreation {
    pub(crate) fn preview(&self) -> SourceCommandResult<tos_foundation::JsonValue> {
        self.prepared.preview()
    }

    pub(crate) fn serialize(
        self,
        software: &SoftwareCaptureReader,
        worker: &mut CandidateCutWorkerSchemaExecutor<PgRecordRevisionIdentity>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<PgSerializedCorpusCreation> {
        if worker.input_identity() != &self.identity {
            return Err(SourceCommandError::Conflict(
                "PG creation schema worker identity changed",
            ));
        }
        let schema_context =
            PgRecordRevisionCommandContext::new(self.prepared.context().clone(), self.identity);
        let components = self.prepared.components().clone();
        let (prepared, plan) = self.prepared.serialize_content_with_schema(
            software,
            &components,
            deadline,
            cancelled,
            None,
            |_, refs, root, instance| {
                revisions::schema_candidate(
                    worker,
                    deadline,
                    cancelled,
                    &schema_context,
                    refs,
                    root,
                    instance,
                )
            },
        )?;
        if plan.operation != "source.create" || plan.handler_id != "native-corpus-create" {
            return Err(SourceCommandError::Conflict(
                "PG Corpus creation serializer profile changed",
            ));
        }
        let request = cmd::parse(&prepared.context.request_raw)?;
        let record = cmd::field(&request, "record")?;
        let config = cmd::parse(&prepared.context.configuration_raw)?;
        let family_tag = match prepared.family() {
            CreationFamily::CorpusV1 => PG_CORPUS_V1_FAMILY_TAG,
            CreationFamily::CorpusV2 => PG_CORPUS_V2_FAMILY_TAG,
            _ => {
                return Err(SourceCommandError::Conflict(
                    "PG Corpus creation serializer family changed",
                ));
            }
        };
        let home = prepared.home.clone();
        let record_type = cmd::text(record, "record_type")?;
        if record_type != self.record_type {
            return Err(SourceCommandError::Conflict(
                "PG Corpus record type changed after preparation",
            ));
        }
        let record_filename = format!("{record_type}.json");
        let forms_filename = format!("{record_type}.human-forms.json");
        let mut indexes = IndexRows::new();
        indexes.insert((
            "metadata".to_owned(),
            cmd::text(record, "record_id")?.to_owned(),
            format!("{}/{}", home.as_str(), record_filename),
        ));
        indexes.insert((
            "event".to_owned(),
            cmd::text(&config, "provenance_event_id")?.to_owned(),
            format!("{}/source-create-provenance.jsonl", home.as_str()),
        ));
        for selection in cmd::array(&request, "forms")? {
            indexes.insert((
                "form".to_owned(),
                cmd::text(selection, "form_id")?.to_owned(),
                format!("{}/{}", home.as_str(), forms_filename),
            ));
        }
        Ok(PgSerializedCorpusCreation {
            context: prepared.context.clone(),
            plan,
            home,
            files: prepared.files().clone(),
            indexes,
            commitment: creation::PgDependencyCommitment::new(
                creation::COMMITMENT_VERSION,
                self.commitment,
            )
            .map_err(|_| SourceCommandError::Invalid("PG Corpus dependency commitment"))?,
            identity: self.identity,
            observations: self.observations,
            record_type: self.record_type,
            family_tag,
        })
    }
}

/// Prepare the common creation semantics over exactly the caller-selected PG
/// readset. The full source inventory is intentionally not materialized.
pub(crate) fn prepare_pg_corpus_creation(
    context: CommandContext,
    identity: PgRecordRevisionIdentity,
    observations: PgCorpusCreationObservations,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CandidateCutWorkerSchemaExecutor<PgRecordRevisionIdentity>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedPgCorpusCreation> {
    if context.base_revision != identity.bootstrap_revision
        || worker.input_identity() != &identity
        || components.capture() != software.selection()
    {
        return Err(SourceCommandError::Conflict(
            "PG Corpus source epoch/software selection differs",
        ));
    }
    context.check()?;
    for path in CORPUS_REQUIRED_SOURCE_INPUTS {
        if context
            .file(
                &RelativePath::parse(path)
                    .map_err(|_| SourceCommandError::Invalid("Corpus schema input path"))?,
            )?
            .is_none()
        {
            return Err(SourceCommandError::Unsupported(
                "required selected creation bytes absent",
            ));
        }
    }
    let (family, config, home) = configuration(&context)?;
    if !matches!(family, CreationFamily::CorpusV1 | CreationFamily::CorpusV2)
        || observations.home.parent != home
        || !observations.home.children.is_empty()
    {
        return Err(SourceCommandError::Conflict(
            "PG Corpus creation family or home observation differs",
        ));
    }
    let schema_context = PgRecordRevisionCommandContext::new(context.clone(), identity);
    let registry = revisions::validate_source_profile_registry_candidate(
        worker,
        deadline,
        cancelled,
        &schema_context,
    )?;
    let _ = registry;
    let request = cmd::parse(&cmd::canonical(&cmd::parse(&context.request_raw)?)?)?;
    let operation = cmd::text(&request, "operation")?;
    if !["prepare-create", family.operation()].contains(&operation) {
        return Err(SourceCommandError::Invalid(
            "creation package preparation operation",
        ));
    }
    let mut keys = vec!["schema_version", "operation", "record", "forms"];
    if operation == family.operation() {
        keys.extend([
            "command_id",
            "expected_configuration",
            "expected_source",
            "expected_revision",
            "expected_dependencies",
        ]);
    }
    cmd::exact_keys(&request, &keys)?;
    if cmd::text(&request, "schema_version")? != "tos_local_source_command_v1"
        || !cmd::array(&config, "allowed_operations")?
            .iter()
            .any(|value| value.as_str() == Some(family.operation()))
    {
        return Err(SourceCommandError::Denied(
            "creation operation not delegated",
        ));
    }
    let record = cmd::field(&request, "record")?;
    let record_type = cmd::text(record, "record_type")?;
    if record_type != observations.record_type
        || !observations_match(&context, &home, &config, record, &request, &observations)?
    {
        return Err(SourceCommandError::Conflict(
            "PG Corpus source home or identity observations differ",
        ));
    }
    let mut validate_record_schema =
        |refs: &[String], root: &str, instance: &tos_foundation::JsonValue| {
            revisions::schema_candidate(
                worker,
                deadline,
                cancelled,
                &schema_context,
                refs,
                root,
                instance,
            )
        };
    initial_corpus_with_schema(&config, family, record, &mut validate_record_schema)?;
    drop(validate_record_schema);

    let selections = cmd::array(&request, "forms")?;
    let mut validate_form_schema =
        |refs: &[String], root: &str, instance: &tos_foundation::JsonValue| {
            revisions::schema_candidate(
                worker,
                deadline,
                cancelled,
                &schema_context,
                refs,
                root,
                instance,
            )
        };
    let forms = prepare_source_forms(&config, record, selections, &mut validate_form_schema)?;
    drop(validate_form_schema);
    let source_path = cmd::text(&config, "source_path")?;
    if source_path != format!("{}/{}.json", home.as_str(), record_type) {
        return Err(SourceCommandError::Denied(
            "PG initial Corpus source home is not exact",
        ));
    }
    let filename = source_path
        .rsplit('/')
        .next()
        .ok_or(SourceCommandError::Invalid("creation source filename"))?;
    let files = BTreeMap::from([
        (filename.to_owned(), cmd::published(record)?),
        (
            format!("{record_type}.human-forms.json"),
            cmd::published(&forms.set)?,
        ),
    ]);
    let commitment = dependency_commitment(
        &context,
        identity,
        &observations,
        &config,
        record,
        selections,
        components,
    )?;
    let prepared = PreparedCreation {
        context,
        managed_basis: None,
        managed_observations: None,
        managed_inventory: None,
        family,
        home,
        subject: forms.subject,
        dependencies: commitment.to_prefixed(),
        files,
        components: components.clone(),
    };
    Ok(PreparedPgCorpusCreation {
        prepared,
        identity,
        observations,
        record_type: record_type.to_owned(),
        commitment,
    })
}

fn observations_match(
    context: &CommandContext,
    home: &RelativePath,
    config: &tos_foundation::JsonValue,
    record: &tos_foundation::JsonValue,
    request: &tos_foundation::JsonValue,
    observations: &PgCorpusCreationObservations,
) -> SourceCommandResult<bool> {
    if observations.record_type != cmd::text(record, "record_type")?
        || observations.home.parent != *home
        || !observations.home.children.is_empty()
    {
        return Ok(false);
    }
    let mut required = BTreeSet::new();
    required.insert((
        "metadata".to_owned(),
        cmd::text(record, "record_id")?.to_owned(),
    ));
    required.insert((
        "event".to_owned(),
        cmd::text(config, "provenance_event_id")?.to_owned(),
    ));
    for selection in cmd::array(request, "forms")? {
        required.insert((
            "form".to_owned(),
            cmd::text(selection, "form_id")?.to_owned(),
        ));
    }
    let observed = observations
        .unique
        .iter()
        .map(|item| (item.kind.clone(), item.token.clone()))
        .collect::<BTreeSet<_>>();
    if observed != required
        || observations.unique.len() != required.len()
        || observations.unique.iter().any(|item| !item.rows.is_empty())
    {
        return Ok(false);
    }
    let requirements = requirements(context)?;
    if requirements.home != *home || requirements.record_type != observations.record_type {
        return Ok(false);
    }
    Ok(true)
}

fn dependency_commitment(
    context: &CommandContext,
    identity: PgRecordRevisionIdentity,
    observations: &PgCorpusCreationObservations,
    config: &tos_foundation::JsonValue,
    record: &tos_foundation::JsonValue,
    forms: &[tos_foundation::JsonValue],
    components: &SoftwareComponentSelectionV1,
) -> SourceCommandResult<Digest256> {
    let record_type = cmd::text(record, "record_type")?;
    let mut inputs = BTreeMap::new();
    for file in &context.files {
        if inputs
            .insert(
                file.path.as_str().to_owned(),
                (file.raw.len(), Digest256::of_bytes(&file.raw)),
            )
            .is_some()
        {
            return Err(SourceCommandError::Invalid(
                "duplicate PG Corpus selected input path",
            ));
        }
    }
    let input_rows = inputs
        .into_iter()
        .map(|(path, (bytes, digest))| {
            cmd::object(vec![
                ("path", cmd::string(&path)),
                ("bytes", cmd::number(bytes as u64)),
                ("sha256", cmd::string(&digest.to_prefixed())),
            ])
        })
        .collect();
    let capture = components.capture();
    let software_members = components
        .members()
        .map(|member| {
            cmd::object(vec![
                ("path", cmd::string(member.path.as_str())),
                ("bytes", cmd::number(member.size_bytes)),
                ("sha256", cmd::string(&member.sha256.to_prefixed())),
            ])
        })
        .collect();
    let unique = observations
        .unique
        .iter()
        .map(|item| {
            cmd::object(vec![
                ("kind", cmd::string(&item.kind)),
                ("token", cmd::string(&item.token)),
                ("generation", cmd::number(item.generation)),
            ])
        })
        .collect();
    let identity = identity_value(identity);
    let binding = cmd::object(vec![
        ("profile", cmd::string(dependency_profile(record_type))),
        ("identity", identity),
        (
            "configuration_sha256",
            cmd::string(&cmd::record_digest(config)?.to_prefixed()),
        ),
        (
            "record_sha256",
            cmd::string(&cmd::record_digest(record)?.to_prefixed()),
        ),
        (
            "forms_sha256",
            cmd::string(
                &cmd::record_digest(&tos_foundation::JsonValue::Array(forms.to_vec()))?
                    .to_prefixed(),
            ),
        ),
        (
            "software_capture",
            cmd::object(vec![
                ("source_git_commit", cmd::string(&capture.source_git_commit)),
                ("source_git_tree", cmd::string(&capture.source_git_tree)),
                (
                    "capture_manifest_sha256",
                    cmd::string(&capture.capture_manifest_sha256.to_prefixed()),
                ),
            ]),
        ),
        (
            "software_components",
            tos_foundation::JsonValue::Array(software_members),
        ),
        (
            "selected_inputs",
            tos_foundation::JsonValue::Array(input_rows),
        ),
        (
            "home_observation",
            cmd::object(vec![
                ("path", cmd::string(context_home(context)?.as_str())),
                ("generation", cmd::number(observations.home.generation)),
                (
                    "children",
                    cmd::number(observations.home.children.len() as u64),
                ),
            ]),
        ),
        (
            "unique_observations",
            tos_foundation::JsonValue::Array(unique),
        ),
        (
            "dependency_generation",
            cmd::number(observations.dependency_generation),
        ),
    ]);
    cmd::record_digest(&binding)
}

fn context_home(context: &CommandContext) -> SourceCommandResult<RelativePath> {
    Ok(configuration(context)?.2)
}

fn dependency_profile(record_type: &str) -> &'static str {
    if record_type == "agent" {
        AGENT_DEPENDENCY_PROFILE_COMPAT
    } else {
        CORPUS_DEPENDENCY_PROFILE
    }
}

fn identity_value(identity: PgRecordRevisionIdentity) -> tos_foundation::JsonValue {
    cmd::object(vec![
        (
            "domain_digest",
            cmd::string(&identity.domain_digest.to_prefixed()),
        ),
        ("store_id", cmd::string(&hex(&identity.store_id))),
        (
            "bootstrap_revision",
            cmd::string(&identity.bootstrap_revision.0.to_prefixed()),
        ),
        (
            "bootstrap_membership_count",
            cmd::number(identity.bootstrap_membership.count),
        ),
        (
            "bootstrap_membership_digest",
            cmd::string(&identity.bootstrap_membership.digest.to_prefixed()),
        ),
        (
            "cohort_digest",
            cmd::string(&identity.cohort_digest.to_prefixed()),
        ),
        ("cohort_epoch", cmd::number(identity.cohort_epoch)),
        (
            "cohort_definition",
            cmd::string(&identity.cohort_definition.to_prefixed()),
        ),
        ("cohort_generation", cmd::number(identity.cohort_generation)),
        (
            "job_id_digest",
            cmd::string(&identity.job_id_digest.to_prefixed()),
        ),
        ("job_fence", cmd::number(identity.job_fence)),
        ("epoch", cmd::number(identity.epoch)),
        ("source_generation", cmd::number(identity.source_generation)),
        ("head_seq", cmd::number(identity.head_seq)),
        ("audit_generation", cmd::number(identity.audit_generation)),
        ("rights_version", cmd::number(identity.rights_version)),
        ("rule_version", cmd::number(identity.rule_version)),
        ("contract", cmd::string(&identity.contract.to_prefixed())),
    ])
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

/// Concrete output accepted by the PostgreSQL adapter. Fields are private;
/// construction requires the shared semantic preparation and serializer.
pub(crate) struct PgSerializedCorpusCreation {
    context: CommandContext,
    plan: CommandPlan,
    home: RelativePath,
    files: BTreeMap<String, Vec<u8>>,
    indexes: IndexRows,
    commitment: creation::PgDependencyCommitment,
    identity: PgRecordRevisionIdentity,
    observations: PgCorpusCreationObservations,
    record_type: String,
    family_tag: &'static str,
}

impl PgSerializedCorpusCreation {
    pub(crate) fn context(&self) -> &CommandContext {
        &self.context
    }
    pub(crate) fn proposal(&self) -> &CommandPlan {
        &self.plan
    }
    pub(crate) fn files(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.files
    }
}

impl creation::PgCreationInput for PgSerializedCorpusCreation {
    fn context(&self) -> &CommandContext {
        &self.context
    }
    fn plan(&self) -> &CommandPlan {
        &self.plan
    }
    fn family_tag(&self) -> &str {
        self.family_tag
    }
    fn record_type(&self) -> &str {
        &self.record_type
    }
    fn home(&self) -> &RelativePath {
        &self.home
    }
    fn files(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.files
    }
    fn indexes(&self) -> &IndexRows {
        &self.indexes
    }
    fn dependency_commitment(&self) -> creation::PgDependencyCommitment {
        self.commitment
    }
    fn identity(&self) -> PgRecordRevisionIdentity {
        self.identity
    }
    fn home_observation(&self) -> &PgSourceHomeObservation {
        &self.observations.home
    }
    fn unique_observations(&self) -> &[PgSourceUniqueIndexObservation] {
        &self.observations.unique
    }
}
