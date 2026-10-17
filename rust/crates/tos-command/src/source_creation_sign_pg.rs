//! Bounded PostgreSQL preparation for a maintained Sign promotion package.
//!
//! Sign semantics remain with `source_creation`; this adapter supplies only
//! the exact selected PG identity/absence observations and schema worker.

use super::{
    CreationFamily, PreparedCreation, configuration, initial_identity, prepare_source_forms,
};
use crate::source_cohort::revision::{
    PgRecordRevisionIdentity, PgSourceHomeObservation, PgSourceUniqueIndexObservation,
};
use crate::source_cohort::{IndexRows, SourceRevisionWrite};
use crate::source_command::{
    self as cmd, CommandContext, PreparedCommand, SourceCommandError, SourceCommandResult,
};
use crate::source_revisions::{self as revisions, PgRecordRevisionCommandContext};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_source_store::{SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::source_cut::CandidateCutWorkerSchemaExecutor;

const SIGN_FAMILY_TAG: &str = "tos-pg-maintained-growth-v1:sign-promotion";
const REQUIRED_SCHEMA_INPUTS: &[&str] = &[
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/corpus-record.schema.json",
    "ToS/contracts/source-metadata-record.schema.json",
    "ToS/contracts/sign-description-record.schema.json",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
    "ToS/contracts/provenance-event-v2.schema.json",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PgSignCreationUniqueRequirement {
    pub(crate) kind: String,
    pub(crate) token: String,
}

/// Exact output identity and source-home probes needed before owner semantics
/// are evaluated. The Sign-candidate and annotation-identity namespaces use
/// their dedicated typed owner indexes, not the ordinary unique table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PgSignCreationRequirements {
    pub(crate) home: RelativePath,
    pub(crate) required_schema_inputs: Vec<RelativePath>,
    pub(crate) unique_tokens: Vec<PgSignCreationUniqueRequirement>,
    pub(crate) annotation_identity_token: String,
    pub(crate) candidate_token: String,
}

#[derive(Clone, Debug)]
pub(crate) struct PgSignCreationObservations {
    pub(crate) home: PgSourceHomeObservation,
    pub(crate) unique: Vec<PgSourceUniqueIndexObservation>,
    pub(crate) identities: crate::source_sign::pg::PgSignCreationIdentityObservations,
}

/// Parse only the fixed, owner-granted creation identities needed to capture
/// absence observations before Sign promotion work begins.
pub(crate) fn requirements(
    context: &CommandContext,
) -> SourceCommandResult<PgSignCreationRequirements> {
    let (family, config, home) = configuration(context)?;
    if family != CreationFamily::Sign
        || !cmd::text(&config, "source_path")?.ends_with("/sign.json")
        || cmd::text(&config, "profile_type_id")? != "tos.entity.sign"
    {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL maintained Sign creation owner",
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
            "PostgreSQL Sign creation operation",
        ));
    }
    cmd::exact_keys(&request, &keys)?;
    if cmd::text(&request, "schema_version")? != "tos_local_source_command_v1"
        || !cmd::array(&config, "allowed_operations")?
            .iter()
            .any(|value| value.as_str() == Some(family.operation()))
    {
        return Err(SourceCommandError::Denied(
            "PostgreSQL Sign operation grant",
        ));
    }
    let record = cmd::field(&request, "record")?;
    let promotion_basis = cmd::field(record, "promotion_basis")?;
    let record_id = cmd::text(record, "record_id")?.to_owned();
    let candidate_id = cmd::text(&config, "promotion_candidate_id")?.to_owned();
    if record_id != cmd::text(&config, "record_id")?
        || !revisions::valid_id(&record_id, "tos.sign.", false)
        || !revisions::valid_id(&candidate_id, "tos.claim.", false)
        || cmd::text(record, "record_type")? != "sign"
        || cmd::text(record, "schema_version")? != "tos_sign_description_record_v1"
        || cmd::text(promotion_basis, "schema_version")? != "tos_sign_promotion_basis_v1"
    {
        return Err(SourceCommandError::Denied(
            "PostgreSQL Sign record/candidate identity grant",
        ));
    }
    if cmd::text(&config, "source_path")? != format!("{}/sign.json", home.as_str()) {
        return Err(SourceCommandError::Denied(
            "PostgreSQL Sign source home is not exact",
        ));
    }
    let forms = cmd::array(&request, "forms")?;
    if forms.is_empty() || forms.len() > 32 {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL Sign form selections budget",
        ));
    }
    let mut unique = BTreeSet::new();
    unique.insert(("metadata".to_owned(), record_id.clone()));
    unique.insert((
        "event".to_owned(),
        cmd::text(&config, "provenance_event_id")?.to_owned(),
    ));
    for selection in forms {
        cmd::exact_keys(selection, &["form_id", "field_id"])?;
        unique.insert((
            "form".to_owned(),
            cmd::text(selection, "form_id")?.to_owned(),
        ));
    }
    if unique.len() != forms.len() + 2 {
        return Err(SourceCommandError::Invalid(
            "duplicate PostgreSQL Sign unique identity token",
        ));
    }
    Ok(PgSignCreationRequirements {
        home,
        required_schema_inputs: REQUIRED_SCHEMA_INPUTS
            .iter()
            .map(|path| {
                RelativePath::parse(path)
                    .map_err(|_| SourceCommandError::Invalid("Sign schema input path"))
            })
            .collect::<SourceCommandResult<Vec<_>>>()?,
        unique_tokens: unique
            .into_iter()
            .map(|(kind, token)| PgSignCreationUniqueRequirement { kind, token })
            .collect(),
        annotation_identity_token: record_id,
        candidate_token: candidate_id,
    })
}

/// A bounded owner-prepared Sign package prior to durable registration.
pub(crate) struct PreparedPgSignCreation {
    prepared: PreparedCreation,
    identity: PgRecordRevisionIdentity,
    observations: PgSignCreationObservations,
    commitment: Digest256,
    owner_snapshot: Digest256,
}

impl PreparedPgSignCreation {
    pub(crate) fn preview(&self) -> SourceCommandResult<JsonValue> {
        self.prepared.preview()
    }

    pub(crate) fn serialize(
        self,
        software: &SoftwareCaptureReader,
        worker: &mut CandidateCutWorkerSchemaExecutor<PgRecordRevisionIdentity>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<PgSerializedSignCreation> {
        if worker.input_identity() != &self.identity {
            return Err(SourceCommandError::Conflict(
                "PG Sign schema worker identity changed",
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
        if plan.operation != "sign.promote" || plan.handler_id != "sign-promotion" {
            return Err(SourceCommandError::Conflict(
                "PG Sign creation serializer profile changed",
            ));
        }
        let writes = plan
            .changes
            .iter()
            .map(|change| {
                Ok(SourceRevisionWrite {
                    subject: change.path.as_str().to_owned(),
                    expected_predecessor: None,
                    proposed_revision: 1,
                    exact_bytes: change.after.clone().ok_or(SourceCommandError::Invalid(
                        "PG Sign creation emitted deletion",
                    ))?,
                })
            })
            .collect::<SourceCommandResult<Vec<_>>>()?;
        let indexes = crate::source_sign::pg::expected_semantic_indexes_from_writes(&writes)?;
        let proposal = plan.into_v1(prepared.context.base_revision);
        crate::source_sign::pg::PgSignRecoverableBasis::from_writes(&writes)?;
        Ok(PgSerializedSignCreation {
            context: prepared.context.clone(),
            proposal,
            home: prepared.home.clone(),
            files: prepared.files().clone(),
            indexes,
            writes,
            commitment: self.commitment,
            identity: self.identity,
            observations: self.observations,
            owner_snapshot: self.owner_snapshot,
            family_tag: SIGN_FAMILY_TAG,
        })
    }
}

/// Build the maintained Sign `describe` or `prepare` response from a bounded
/// PG-selected context. This keeps the native creation response shape and
/// initial-profile validation with the Sign owner while avoiding a corpus cut.
pub(crate) fn selected_information(
    context: &CommandContext,
    identity: PgRecordRevisionIdentity,
    worker: &mut CandidateCutWorkerSchemaExecutor<PgRecordRevisionIdentity>,
    target_exists: bool,
    promotion: Option<&JsonValue>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    if worker.input_identity() != &identity {
        return Err(SourceCommandError::Conflict(
            "PG Sign information worker identity changed",
        ));
    }
    let schema_context = PgRecordRevisionCommandContext::new(context.clone(), identity);
    let (family, config, _) = configuration(context)?;
    if family != CreationFamily::Sign {
        return Err(SourceCommandError::Unsupported(
            "PG Sign information owner family",
        ));
    }
    let request = cmd::parse(&cmd::canonical(&cmd::parse(&context.request_raw)?)?)?;
    let operation = cmd::text(&request, "operation")?;
    if !["describe", "prepare"].contains(&operation)
        || cmd::text(&request, "schema_version")? != "tos_local_source_command_v1"
    {
        return Err(SourceCommandError::Invalid("PG Sign information request"));
    }
    cmd::exact_keys(
        &request,
        if operation == "describe" {
            &["schema_version", "operation"]
        } else {
            &["schema_version", "operation", "record"]
        },
    )?;
    revisions::validate_source_profile_registry_candidate(
        worker,
        deadline,
        cancelled,
        &schema_context,
    )?;
    let mut response =
        super::creation_information(context, family, target_exists, JsonValue::Null, false)?;
    if operation == "describe" {
        cmd::set(
            &mut response,
            "promotion",
            promotion.cloned().unwrap_or(JsonValue::Null),
        )?;
        return Ok(response);
    }
    if !cmd::array(&config, "allowed_operations")?
        .iter()
        .any(|value| value.as_str() == Some(family.operation()))
    {
        return Err(SourceCommandError::Denied(
            "PG Sign creation operation not delegated",
        ));
    }
    let record = cmd::field(&request, "record")?;
    initial_identity(&config, record)?;
    validate_initial_sign_profile(
        context, identity, &config, record, worker, deadline, cancelled,
    )?;
    cmd::set(
        &mut response,
        "prepared_source",
        crate::source_forms::metadata_subject(record)?,
    )?;
    cmd::set(
        &mut response,
        "source_fields",
        JsonValue::Array(
            crate::source_forms::metadata_fields(record)?
                .into_iter()
                .map(|field| field.public())
                .collect(),
        ),
    )?;
    Ok(response)
}

/// Output handed to the shared Growth revision runner. The common writer
/// still derives structural rows and verifies owner rows from exact writes.
pub(crate) struct PgSerializedSignCreation {
    pub(crate) context: CommandContext,
    pub(crate) proposal: PreparedCommand,
    pub(crate) home: RelativePath,
    pub(crate) files: BTreeMap<String, Vec<u8>>,
    pub(crate) indexes: IndexRows,
    pub(crate) writes: Vec<SourceRevisionWrite>,
    pub(crate) commitment: Digest256,
    pub(crate) identity: PgRecordRevisionIdentity,
    pub(crate) observations: PgSignCreationObservations,
    pub(crate) owner_snapshot: Digest256,
    pub(crate) family_tag: &'static str,
}

/// Prepare Sign owner semantics from selected exact inputs. There is no
/// complete-corpus inventory or all-Sign scan here: typed output identities,
/// the selected public-source snapshot, and the source readset carry those
/// bounded absence/freshness guarantees.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_pg_sign_creation(
    context: CommandContext,
    identity: PgRecordRevisionIdentity,
    observations: PgSignCreationObservations,
    basis: JsonValue,
    owner_snapshot: Digest256,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CandidateCutWorkerSchemaExecutor<PgRecordRevisionIdentity>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedPgSignCreation> {
    if context.base_revision != identity.bootstrap_revision
        || worker.input_identity() != &identity
        || components.capture() != software.selection()
    {
        return Err(SourceCommandError::Conflict(
            "PG Sign source epoch/software selection differs",
        ));
    }
    context.check()?;
    for path in REQUIRED_SCHEMA_INPUTS {
        if context
            .file(
                &RelativePath::parse(path)
                    .map_err(|_| SourceCommandError::Invalid("Sign schema input path"))?,
            )?
            .is_none()
        {
            return Err(SourceCommandError::Unsupported(
                "required selected Sign schema bytes absent",
            ));
        }
    }
    let (family, config, home) = configuration(&context)?;
    if family != CreationFamily::Sign
        || observations.home.parent != home
        || !observations.home.children.is_empty()
    {
        return Err(SourceCommandError::Conflict(
            "PG Sign creation family or home observation differs",
        ));
    }
    let request = cmd::parse(&cmd::canonical(&cmd::parse(&context.request_raw)?)?)?;
    let operation = cmd::text(&request, "operation")?;
    if !["prepare-create", family.operation()].contains(&operation) {
        return Err(SourceCommandError::Invalid(
            "PG Sign creation preparation operation",
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
        return Err(SourceCommandError::Denied("PG Sign operation grant"));
    }
    let record = cmd::field(&request, "record")?;
    initial_identity(&config, record)?;
    if cmd::text(&config, "profile_type_id")? != "tos.entity.sign"
        || cmd::text(&config, "source_path")? != format!("{}/sign.json", home.as_str())
        || cmd::text(record, "record_type")? != "sign"
        || cmd::text(record, "schema_version")? != "tos_sign_description_record_v1"
        || !cmd::same(cmd::field(record, "promotion_basis")?, &basis)?
    {
        return Err(SourceCommandError::Conflict(
            "PG Sign provisional record or current basis differs",
        ));
    }
    validate_basis_binding(&basis, &config, owner_snapshot)?;
    validate_initial_sign_profile(
        &context, identity, &config, record, worker, deadline, cancelled,
    )?;

    let requirements = requirements(&context)?;
    if requirements.home != home
        || !observations_match(&config, record, &request, &requirements, &observations)?
    {
        return Err(SourceCommandError::Conflict(
            "PG Sign source home or identity observations differ",
        ));
    }

    let schema_context = PgRecordRevisionCommandContext::new(context.clone(), identity);
    let mut validate_form_schema = |refs: &[String], root: &str, instance: &JsonValue| {
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
    let forms = prepare_source_forms(
        &config,
        record,
        cmd::array(&request, "forms")?,
        &mut validate_form_schema,
    )?;
    if cancelled.load(std::sync::atomic::Ordering::Relaxed) || Instant::now() >= deadline {
        return Err(SourceCommandError::Denied(
            "PG Sign creation deadline/cancellation",
        ));
    }
    let record_file = cmd::published(record)?;
    let form_file = cmd::published(&forms.set)?;
    let files = BTreeMap::from([
        ("sign.json".to_owned(), record_file),
        ("sign.human-forms.json".to_owned(), form_file),
    ]);
    let commitment = dependency_commitment(
        &context,
        identity,
        &observations,
        &config,
        record,
        cmd::array(&request, "forms")?,
        components,
        owner_snapshot,
    )?;
    if operation == family.operation()
        && cmd::text(&request, "expected_dependencies")? != commitment.to_prefixed()
    {
        return Err(SourceCommandError::Conflict(
            "PG Sign selected promotion dependency commitment changed",
        ));
    }
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
    Ok(PreparedPgSignCreation {
        prepared,
        identity,
        observations,
        commitment,
        owner_snapshot,
    })
}

fn validate_basis_binding(
    basis: &JsonValue,
    config: &JsonValue,
    owner_snapshot: Digest256,
) -> SourceCommandResult<()> {
    cmd::exact_keys(
        basis,
        &[
            "schema_version",
            "candidate",
            "policy",
            "required_sources",
            "assessment_refs",
            "owner_snapshot",
            "journal_revision",
            "status",
            "use",
            "limits",
            "grants_current_use",
        ],
    )?;
    let candidate = cmd::field(basis, "candidate")?;
    let policy = cmd::field(basis, "policy")?;
    if cmd::text(basis, "schema_version")? != "tos_sign_promotion_basis_v1"
        || cmd::text(basis, "use")? != "sign-promotion"
        || cmd::field(basis, "grants_current_use")? != &JsonValue::Bool(false)
        || !matches!(
            cmd::text(basis, "status")?,
            "admitted" | "admitted-with-limits"
        )
        || !revisions::valid_id(cmd::text(candidate, "id")?, "tos.claim.", false)
        || cmd::text(candidate, "id")? != cmd::text(config, "promotion_candidate_id")?
        || !revisions::valid_id(cmd::text(policy, "id")?, "tos.policy.", false)
        || cmd::integer(candidate, "version")? == 0
        || cmd::integer(policy, "version")? == 0
        || cmd::array(basis, "required_sources")?.is_empty()
        || cmd::array(basis, "assessment_refs")?.is_empty()
    {
        return Err(SourceCommandError::Invalid("PG Sign current basis profile"));
    }
    let parsed_snapshot = Digest256::from_prefixed(cmd::text(basis, "owner_snapshot")?)
        .map_err(|_| SourceCommandError::Invalid("PG Sign basis owner snapshot"))?;
    Digest256::from_hex(cmd::text(basis, "journal_revision")?)
        .map_err(|_| SourceCommandError::Invalid("PG Sign basis journal revision"))?;
    Digest256::from_prefixed(cmd::text(policy, "digest")?)
        .map_err(|_| SourceCommandError::Invalid("PG Sign basis policy digest"))?;
    if parsed_snapshot != owner_snapshot {
        return Err(SourceCommandError::Conflict(
            "PG Sign current basis binding differs",
        ));
    }
    Ok(())
}

fn validate_initial_sign_profile(
    context: &CommandContext,
    identity: PgRecordRevisionIdentity,
    config: &JsonValue,
    record: &JsonValue,
    worker: &mut CandidateCutWorkerSchemaExecutor<PgRecordRevisionIdentity>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let schema_context = PgRecordRevisionCommandContext::new(context.clone(), identity);
    let registry = revisions::validate_source_profile_registry_candidate(
        worker,
        deadline,
        cancelled,
        &schema_context,
    )?;
    let matching = cmd::array(&registry, "types")?
        .iter()
        .filter(|entry| {
            entry.object_get("type_id").and_then(JsonValue::as_str) == Some("tos.entity.sign")
        })
        .collect::<Vec<_>>();
    let entry = match matching.as_slice() {
        [entry] => *entry,
        _ => return Err(SourceCommandError::Denied("Sign profile registry owner")),
    };
    let profile = cmd::field(entry, "source_record_profile")?;
    let source_path = cmd::text(config, "source_path")?;
    let basename = source_path
        .rsplit_once('/')
        .map(|(_, basename)| basename)
        .ok_or(SourceCommandError::Invalid("Sign profile source basename"))?;
    if cmd::field(entry, "abstract")? != &JsonValue::Bool(false)
        || cmd::text(profile, "record_type")? != "sign"
        || cmd::text(profile, "reader")? != "semantic-metadata-v1"
        || cmd::text(profile, "creation_gate")? != "sign-promotion-v1"
        || cmd::text(profile, "source_basename")? != basename
        || cmd::text(record, "record_type")? != "sign"
        || cmd::text(record, "record_id")? != cmd::text(config, "record_id")?
        || !revisions::valid_id(
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
        || tos_foundation::python_strip_unicode16_v1(
            cmd::text(record, "preferred_label")?,
            1_048_576,
        )
        .map_err(|_| SourceCommandError::Invalid("Sign preferred label budget"))?
        .is_empty()
        || cmd::integer(record, "record_version")? == 0
        || profile.object_get("native_binding_adapter").is_some()
        || record.object_get("native_text_binding").is_some()
    {
        return Err(SourceCommandError::Denied(
            "Sign initial public profile identity/metadata",
        ));
    }
    let routes = cmd::array(profile, "schemas")?
        .iter()
        .filter(|route| route.object_get("schema_version") == record.object_get("schema_version"))
        .collect::<Vec<_>>();
    let route = match routes.as_slice() {
        [route] => *route,
        _ => return Err(SourceCommandError::Unsupported("Sign profile schema route")),
    };
    let root = cmd::text(route, "schema_ref")?;
    let mut refs = vec!["ToS/contracts/corpus-record.schema.json".to_owned()];
    refs.extend(
        cmd::array(route, "schema_dependencies")?
            .iter()
            .map(|dependency| {
                dependency
                    .as_str()
                    .map(str::to_owned)
                    .ok_or(SourceCommandError::Invalid(
                        "Sign profile schema dependency",
                    ))
            })
            .collect::<SourceCommandResult<Vec<_>>>()?,
    );
    refs.push(root.to_owned());
    refs.sort();
    refs.dedup();
    if !refs
        .iter()
        .any(|path| path == "ToS/contracts/source-metadata-record.schema.json")
    {
        return Err(SourceCommandError::Unsupported(
            "Sign profile shared metadata contract absent",
        ));
    }
    revisions::schema_candidate(
        worker,
        deadline,
        cancelled,
        &schema_context,
        &refs,
        root,
        record,
    )?;
    revisions::schema_candidate(
        worker,
        deadline,
        cancelled,
        &schema_context,
        &refs,
        "ToS/contracts/source-metadata-record.schema.json",
        record,
    )
}

fn observations_match(
    config: &JsonValue,
    record: &JsonValue,
    request: &JsonValue,
    requirements: &PgSignCreationRequirements,
    observations: &PgSignCreationObservations,
) -> SourceCommandResult<bool> {
    if observations.home.parent != requirements.home || !observations.home.children.is_empty() {
        return Ok(false);
    }
    if observations.identities.sign_id != requirements.annotation_identity_token
        || !observations
            .identities
            .annotation_identity_owners
            .is_empty()
        || observations.identities.candidate_id != requirements.candidate_token
        || observations.identities.prior_candidate_path.is_some()
    {
        return Ok(false);
    }
    if cmd::text(record, "record_id")? != requirements.annotation_identity_token
        || cmd::text(
            cmd::field(cmd::field(record, "promotion_basis")?, "candidate")?,
            "id",
        )? != requirements.candidate_token
    {
        return Ok(false);
    }
    let required = requirements
        .unique_tokens
        .iter()
        .map(|item| (item.kind.clone(), item.token.clone()))
        .collect::<BTreeSet<_>>();
    let observed = observations
        .unique
        .iter()
        .map(|item| (item.kind.clone(), item.token.clone()))
        .collect::<BTreeSet<_>>();
    if observed != required
        || observations.unique.len() != required.len()
        || observations.unique.iter().any(|item| !item.rows.is_empty())
        || cmd::text(config, "provenance_event_id")?
            != required
                .iter()
                .find(|(kind, _)| kind == "event")
                .map(|(_, token)| token.as_str())
                .unwrap_or_default()
        || cmd::array(request, "forms")?.len() + 2 != required.len()
    {
        return Ok(false);
    }
    Ok(true)
}

fn dependency_commitment(
    context: &CommandContext,
    identity: PgRecordRevisionIdentity,
    observations: &PgSignCreationObservations,
    config: &JsonValue,
    record: &JsonValue,
    forms: &[JsonValue],
    components: &SoftwareComponentSelectionV1,
    owner_snapshot: Digest256,
) -> SourceCommandResult<Digest256> {
    let mut input_rows = BTreeMap::new();
    for file in &context.files {
        if input_rows
            .insert(
                file.path.as_str().to_owned(),
                (file.raw.len(), Digest256::of_bytes(&file.raw)),
            )
            .is_some()
        {
            return Err(SourceCommandError::Invalid(
                "duplicate PG Sign selected input",
            ));
        }
    }
    let inputs = input_rows
        .into_iter()
        .map(|(path, (bytes, digest))| {
            cmd::object(vec![
                ("path", cmd::string(&path)),
                ("bytes", cmd::number(bytes as u64)),
                ("sha256", cmd::string(&digest.to_prefixed())),
            ])
        })
        .collect();
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
    let capture = components.capture();
    cmd::record_digest(&cmd::object(vec![
        (
            "profile",
            cmd::string("tos_pg_sign_promotion_dependencies_v1"),
        ),
        (
            "input_identity",
            cmd::string(&identity.digest().to_prefixed()),
        ),
        ("owner_snapshot", cmd::string(&owner_snapshot.to_prefixed())),
        (
            "configuration",
            cmd::string(&cmd::record_digest(config)?.to_prefixed()),
        ),
        (
            "record",
            cmd::string(&cmd::record_digest(record)?.to_prefixed()),
        ),
        (
            "forms",
            cmd::string(&cmd::record_digest(&JsonValue::Array(forms.to_vec()))?.to_prefixed()),
        ),
        ("selected_inputs", JsonValue::Array(inputs)),
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
        (
            "home_observation",
            cmd::object(vec![
                ("path", cmd::string(observations.home.parent.as_str())),
                ("generation", cmd::number(observations.home.generation)),
                (
                    "child_count",
                    cmd::number(observations.home.children.len() as u64),
                ),
            ]),
        ),
        ("unique_observations", JsonValue::Array(unique)),
    ]))
}
