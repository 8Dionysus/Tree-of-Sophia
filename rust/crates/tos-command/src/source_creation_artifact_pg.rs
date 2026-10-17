//! Bounded PostgreSQL preparation for one initial native Artifact record.
//!
//! Artifact meaning and byte serialization remain with the maintained source
//! creation owner. This adapter replaces only the full-cut absence and
//! collision observations with exact PG readset predicates and a versioned
//! dependency commitment.

use super::{CreationFamily, PreparedCreation, configuration, prepare_source_forms};
use crate::source_artifact_native;
use crate::source_cohort::creation::{self, PgDependencyCommitment};
use crate::source_cohort::revision::{
    PgRecordRevisionIdentity, PgSourceHomeObservation, PgSourceUniqueIndexObservation,
};
use crate::source_command::{
    self as cmd, CommandContext, CommandPlan, SourceCommandError, SourceCommandResult,
};
use crate::source_revisions::{self as revisions, PgRecordRevisionCommandContext};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_source_store::{SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::source_cut::CandidateCutWorkerSchemaExecutor;

pub(crate) const ARTIFACT_RECORD_TYPE: &str = "artifact";
const ARTIFACT_DEPENDENCY_PROFILE: &str = "tos_initial_artifact_dependency_commitment_v1";
pub(crate) const ARTIFACT_FAMILY_TAG: &str =
    "tos-pg-artifact-create-v1:tos_local_artifact_create_owner_v1:source.create";
const REQUIRED_SOURCE_INPUTS: &[&str] = &[
    "ToS/contracts/artifact-source-witness-v2.schema.json",
    "ToS/contracts/rights-record.schema.json",
    "ToS/contracts/material-discovery-record.schema.json",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
    "ToS/contracts/provenance-event-v2.schema.json",
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
];
pub(crate) const INITIAL_PACKAGE_FILES: [&str; 6] = [
    "artifact-witness.json",
    "artifact-witness.human-forms.json",
    "source-create-request.json",
    "source-create-environment.json",
    "source-create-provenance.jsonl",
    "source-create-receipt.json",
];

/// Shared recovery uses this exact owner identity check after decoding the
/// durable package. Semantic admission remains with `initial_with_schema`.
pub(crate) fn recovery_source_identity_matches(
    record_type: &str,
    record_id: &str,
    source_path: &str,
    body: &[u8],
) -> bool {
    if record_type != ARTIFACT_RECORD_TYPE
        || !revisions::valid_id(record_id, "tos.artifact.", false)
    {
        return false;
    }
    let Ok(path) = RelativePath::parse(source_path) else {
        return false;
    };
    let parts = path.as_str().split('/').collect::<Vec<_>>();
    if parts.len() != 7
        || parts[..3] != ["ToS", "source-witnesses", "artifacts"]
        || parts[6] != "artifact-witness.json"
        || parts[3..6]
            .iter()
            .any(|part| part.eq_ignore_ascii_case("cdli"))
    {
        return false;
    }
    let Ok(record) = cmd::parse(body) else {
        return false;
    };
    cmd::canonical(&record).is_ok_and(|canonical| canonical == body)
        && cmd::text(&record, "schema_version").ok() == Some("tos_artifact_source_witness_v2")
        && cmd::text(&record, "artifact_id").ok() == Some(record_id)
        && cmd::integer(&record, "record_version").ok() == Some(1)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PgArtifactUniqueIndexRequirement {
    pub(crate) kind: String,
    pub(crate) token: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PgArtifactCreationRequirements {
    pub(crate) home: RelativePath,
    pub(crate) artifact_id: String,
    pub(crate) source_refs: Vec<RelativePath>,
    pub(crate) required_source_inputs: Vec<RelativePath>,
    pub(crate) unique_tokens: Vec<PgArtifactUniqueIndexRequirement>,
}

#[derive(Clone, Debug)]
pub(crate) struct PgArtifactCreationObservations {
    pub(crate) artifact_id: String,
    pub(crate) home: PgSourceHomeObservation,
    pub(crate) unique: Vec<PgSourceUniqueIndexObservation>,
    pub(crate) dependency_generation: u64,
}

/// Resolve only the exact inputs needed to prepare one grant-bound Artifact.
/// The source indexes supply identity, event and form absence; the three
/// owner-bound references remain exact selected source members.
pub(crate) fn requirements(
    context: &CommandContext,
) -> SourceCommandResult<PgArtifactCreationRequirements> {
    context.check()?;
    let (family, config, home) = configuration(context)?;
    if family != CreationFamily::Artifact {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL initial Artifact owner",
        ));
    }
    let request = cmd::parse(&context.request_raw)?;
    let operation = cmd::text(&request, "operation")?;
    let mut keys = vec![
        "schema_version",
        "operation",
        "record",
        "forms",
        "source_bindings",
    ];
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
            "PostgreSQL initial Artifact creation operation",
        ));
    }
    cmd::exact_keys(&request, &keys)?;
    if cmd::text(&request, "schema_version")? != "tos_local_source_command_v1"
        || !cmd::array(&config, "allowed_operations")?
            .iter()
            .any(|value| value.as_str() == Some(family.operation()))
        || !cmd::same(
            cmd::field(&request, "source_bindings")?,
            cmd::field(&config, "source_bindings")?,
        )?
    {
        return Err(SourceCommandError::Denied(
            "PostgreSQL Artifact creation operation/input grant",
        ));
    }
    let record = cmd::field(&request, "record")?;
    let artifact_id = cmd::text(record, "artifact_id")?.to_owned();
    if artifact_id != cmd::text(&config, "record_id")? {
        return Err(SourceCommandError::Denied(
            "PostgreSQL Artifact identity grant",
        ));
    }
    let mut source_refs = BTreeSet::new();
    let bindings = cmd::field(&config, "source_bindings")?;
    for field in ["rights_ref", "discovery_ref", "research_ref"] {
        let reference = cmd::text(cmd::field(bindings, field)?, "ref")?;
        let path = RelativePath::parse(reference)
            .map_err(|_| SourceCommandError::Invalid("Artifact input reference path"))?;
        source_refs.insert(path);
    }
    if source_refs.len() != 3 {
        return Err(SourceCommandError::Invalid(
            "Artifact distinct exact input bindings",
        ));
    }

    let selections = cmd::array(&request, "forms")?;
    if selections.is_empty() || selections.len() > 32 {
        return Err(SourceCommandError::Invalid(
            "Artifact creation form selections budget",
        ));
    }
    let mut unique = BTreeSet::new();
    unique.insert(("metadata".to_owned(), artifact_id.clone()));
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
            "duplicate PostgreSQL Artifact unique identity token",
        ));
    }
    Ok(PgArtifactCreationRequirements {
        home,
        artifact_id,
        source_refs: source_refs.into_iter().collect(),
        required_source_inputs: REQUIRED_SOURCE_INPUTS
            .iter()
            .map(|path| {
                RelativePath::parse(path)
                    .map_err(|_| SourceCommandError::Invalid("Artifact schema input path"))
            })
            .collect::<SourceCommandResult<Vec<_>>>()?,
        unique_tokens: unique
            .into_iter()
            .map(|(kind, token)| PgArtifactUniqueIndexRequirement { kind, token })
            .collect(),
    })
}

/// Semantic Artifact preparation over an exact PG selection. This is a
/// proposal only; the shared creation writer verifies the same readset and
/// observations before registering a durable attempt.
pub(crate) fn prepare_pg_artifact_creation(
    context: CommandContext,
    identity: PgRecordRevisionIdentity,
    observations: PgArtifactCreationObservations,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CandidateCutWorkerSchemaExecutor<PgRecordRevisionIdentity>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedPgArtifactCreation> {
    if context.base_revision != identity.bootstrap_revision
        || worker.input_identity() != &identity
        || components.capture() != software.selection()
    {
        return Err(SourceCommandError::Conflict(
            "PG Artifact source epoch/software selection differs",
        ));
    }
    context.check()?;
    let requirements = requirements(&context)?;
    for path in &requirements.required_source_inputs {
        if context.file(path)?.is_none() {
            return Err(SourceCommandError::Unsupported(
                "required selected Artifact creation bytes absent",
            ));
        }
    }
    for path in &requirements.source_refs {
        if context.file(path)?.is_none() {
            return Err(SourceCommandError::Conflict(
                "Artifact exact bound input unavailable",
            ));
        }
    }
    if !observations_match(&context, &requirements, &observations)? {
        return Err(SourceCommandError::Conflict(
            "PG Artifact source home or identity observations differ",
        ));
    }

    let (family, config, home) = configuration(&context)?;
    let schema_context = PgRecordRevisionCommandContext::new(context.clone(), identity);
    revisions::validate_source_profile_registry_candidate(
        worker,
        deadline,
        cancelled,
        &schema_context,
    )?;
    let request = cmd::parse(&cmd::canonical(&cmd::parse(&context.request_raw)?)?)?;
    let record = cmd::field(&request, "record")?;
    {
        let mut validate = |refs: &[String], root: &str, instance: &JsonValue| {
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
        source_artifact_native::initial_with_schema(&context, &config, record, &mut validate)?;
    }
    let selections = cmd::array(&request, "forms")?;
    let mut validate_forms = |refs: &[String], root: &str, instance: &JsonValue| {
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
    let forms = prepare_source_forms(&config, record, selections, &mut validate_forms)?;

    let dependency_commitment = artifact_dependency_commitment(
        &context,
        identity,
        &observations,
        &config,
        record,
        selections,
        components,
    )?;
    let source_path = cmd::text(&config, "source_path")?;
    if source_path != format!("{}/artifact-witness.json", home.as_str()) {
        return Err(SourceCommandError::Denied(
            "PG Artifact source home is not exact",
        ));
    }
    let files = BTreeMap::from([
        ("artifact-witness.json".to_owned(), cmd::published(record)?),
        (
            "artifact-witness.human-forms.json".to_owned(),
            cmd::published(&forms.set)?,
        ),
    ]);
    let prepared = PreparedCreation {
        context,
        managed_basis: None,
        managed_observations: None,
        managed_inventory: None,
        family,
        home,
        subject: forms.subject,
        dependencies: dependency_commitment.to_prefixed(),
        files,
        components: components.clone(),
    };
    Ok(PreparedPgArtifactCreation {
        prepared,
        identity,
        observations,
        artifact_id: requirements.artifact_id,
        dependency_commitment,
    })
}

fn observations_match(
    context: &CommandContext,
    required: &PgArtifactCreationRequirements,
    observations: &PgArtifactCreationObservations,
) -> SourceCommandResult<bool> {
    if observations.artifact_id != required.artifact_id
        || observations.home.parent != required.home
        || !observations.home.children.is_empty()
    {
        return Ok(false);
    }
    let expected = required
        .unique_tokens
        .iter()
        .map(|item| (item.kind.clone(), item.token.clone()))
        .collect::<BTreeSet<_>>();
    let observed = observations
        .unique
        .iter()
        .map(|item| (item.kind.clone(), item.token.clone()))
        .collect::<BTreeSet<_>>();
    if observed != expected
        || observations.unique.len() != expected.len()
        || observations.unique.iter().any(|item| !item.rows.is_empty())
    {
        return Ok(false);
    }
    let fresh_requirements = requirements(context)?;
    Ok(fresh_requirements.home == required.home
        && fresh_requirements.artifact_id == required.artifact_id
        && fresh_requirements.unique_tokens == required.unique_tokens)
}

fn artifact_dependency_commitment(
    context: &CommandContext,
    identity: PgRecordRevisionIdentity,
    observations: &PgArtifactCreationObservations,
    config: &JsonValue,
    record: &JsonValue,
    forms: &[JsonValue],
    components: &SoftwareComponentSelectionV1,
) -> SourceCommandResult<Digest256> {
    let mut selected_inputs = BTreeMap::new();
    for file in &context.files {
        if selected_inputs
            .insert(
                file.path.as_str().to_owned(),
                (file.raw.len(), Digest256::of_bytes(&file.raw)),
            )
            .is_some()
        {
            return Err(SourceCommandError::Invalid(
                "duplicate PG Artifact selected input path",
            ));
        }
    }
    let input_rows = selected_inputs
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
    let binding = cmd::object(vec![
        ("profile", cmd::string(ARTIFACT_DEPENDENCY_PROFILE)),
        ("family_tag", cmd::string(ARTIFACT_FAMILY_TAG)),
        ("identity", artifact_identity_value(identity)),
        (
            "configuration_sha256",
            cmd::string(&cmd::record_digest(config)?.to_prefixed()),
        ),
        (
            "artifact_sha256",
            cmd::string(&cmd::record_digest(record)?.to_prefixed()),
        ),
        (
            "forms_sha256",
            cmd::string(&cmd::record_digest(&JsonValue::Array(forms.to_vec()))?.to_prefixed()),
        ),
        (
            "source_bindings_sha256",
            cmd::string(&cmd::record_digest(cmd::field(config, "source_bindings")?)?.to_prefixed()),
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
        ("software_components", JsonValue::Array(software_members)),
        ("selected_inputs", JsonValue::Array(input_rows)),
        (
            "home_observation",
            cmd::object(vec![
                ("path", cmd::string(requirements(context)?.home.as_str())),
                ("generation", cmd::number(observations.home.generation)),
                (
                    "children",
                    cmd::number(observations.home.children.len() as u64),
                ),
            ]),
        ),
        ("unique_observations", JsonValue::Array(unique)),
        (
            "dependency_generation",
            cmd::number(observations.dependency_generation),
        ),
    ]);
    cmd::record_digest(&binding)
}

fn artifact_identity_value(identity: PgRecordRevisionIdentity) -> JsonValue {
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

/// Semantic package before the shared durable writer issues a registration
/// capability. Construction is private to this owner adapter.
pub(crate) struct PreparedPgArtifactCreation {
    prepared: PreparedCreation,
    identity: PgRecordRevisionIdentity,
    observations: PgArtifactCreationObservations,
    artifact_id: String,
    dependency_commitment: Digest256,
}

impl PreparedPgArtifactCreation {
    pub(crate) fn preview(&self) -> SourceCommandResult<JsonValue> {
        self.prepared.preview()
    }

    pub(crate) fn serialize(
        self,
        software: &SoftwareCaptureReader,
        worker: &mut CandidateCutWorkerSchemaExecutor<PgRecordRevisionIdentity>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<PgSerializedArtifactCreation> {
        if worker.input_identity() != &self.identity {
            return Err(SourceCommandError::Conflict(
                "PG Artifact schema worker identity changed",
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
        if plan.operation != "source.create"
            || plan.handler_id != "native-artifact-create"
            || prepared.family() != CreationFamily::Artifact
        {
            return Err(SourceCommandError::Conflict(
                "PG Artifact creation serializer profile changed",
            ));
        }
        if prepared
            .files()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>()
            != INITIAL_PACKAGE_FILES.into_iter().collect()
        {
            return Err(SourceCommandError::Conflict(
                "PG Artifact serialized package member closure changed",
            ));
        }
        let request = cmd::parse(&prepared.context.request_raw)?;
        let config = cmd::parse(&prepared.context.configuration_raw)?;
        let record = cmd::field(&request, "record")?;
        if cmd::text(record, "artifact_id")? != self.artifact_id {
            return Err(SourceCommandError::Conflict(
                "PG Artifact identity changed after serialization",
            ));
        }
        let home = prepared.home().clone();
        let mut indexes = BTreeSet::new();
        indexes.insert((
            "metadata".to_owned(),
            self.artifact_id.clone(),
            format!("{}/artifact-witness.json", home.as_str()),
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
                format!("{}/artifact-witness.human-forms.json", home.as_str()),
            ));
        }
        let dependency_commitment =
            PgDependencyCommitment::new(creation::COMMITMENT_VERSION, self.dependency_commitment)
                .map_err(|_| SourceCommandError::Invalid("PG Artifact dependency commitment"))?;
        Ok(PgSerializedArtifactCreation {
            context: prepared.context().clone(),
            plan,
            home,
            files: prepared.files().clone(),
            indexes,
            dependency_commitment,
            identity: self.identity,
            observations: self.observations,
        })
    }
}

/// Exact owner serialization accepted by the shared PG creation writer.
pub(crate) struct PgSerializedArtifactCreation {
    context: CommandContext,
    plan: CommandPlan,
    home: RelativePath,
    files: BTreeMap<String, Vec<u8>>,
    indexes: BTreeSet<(String, String, String)>,
    dependency_commitment: PgDependencyCommitment,
    identity: PgRecordRevisionIdentity,
    observations: PgArtifactCreationObservations,
}

impl PgSerializedArtifactCreation {
    pub(crate) fn context(&self) -> &CommandContext {
        &self.context
    }
    pub(crate) fn plan(&self) -> &CommandPlan {
        &self.plan
    }
    pub(crate) fn home(&self) -> &RelativePath {
        &self.home
    }
    pub(crate) fn files(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.files
    }
    pub(crate) fn indexes(&self) -> &BTreeSet<(String, String, String)> {
        &self.indexes
    }
    pub(crate) fn dependency_commitment(&self) -> PgDependencyCommitment {
        self.dependency_commitment
    }
    pub(crate) fn identity(&self) -> PgRecordRevisionIdentity {
        self.identity
    }
    pub(crate) fn home_observation(&self) -> &PgSourceHomeObservation {
        &self.observations.home
    }
    pub(crate) fn unique_observations(&self) -> &[PgSourceUniqueIndexObservation] {
        &self.observations.unique
    }
}
