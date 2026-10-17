//! PostgreSQL owner preparation for Expression responsibility attachment.
//!
//! The selected compound verifier remains responsible for retained native
//! lineage. This adapter supplies the complete current responsibility union,
//! exact owner authorization, and the source-side publication proposal.

use super::*;
use crate::source_cohort::{IndexRows, SourceRevisionWrite};
use crate::source_command::{self as cmd, CommandContext, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{CreationFilesystem, SelectedSourceWitnessCatalog};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonString, JsonValue};
use tos_source_store::{SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::item_rules::{ItemLimits, ItemRefusal};
use tos_validation::record_biblio_cut::{
    SourceCutInput, SourceCutInputWithIdentity, SourceCutMemberMeta,
};
use tos_validation::source_cut::CandidateCutWorkerSchemaExecutor;

const CONTRACTS: &[&str] = &[
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    "ToS/contracts/corpus-record.schema.json",
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/semantic-relation-type-registry.schema.json",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
    "ToS/contracts/provenance-event-v2.schema.json",
];

/// Owner result and physical corpus fence. The common Growth writer keeps this
/// package alive through registration, recovery, and source-revision commit.
pub(crate) struct PgExpressionResponsibilityOwnerPackage<'a> {
    pub(crate) context: CommandContext,
    pub(crate) proposal: cmd::PreparedCommand,
    pub(crate) semantic_indexes: IndexRows,
    pub(crate) fence: work_transaction::WorkCorpusFence<'a>,
}

struct SelectedResponsibilityCatalog {
    catalog: ExpressionResponsibilityCatalog,
    agent: JsonValue,
    agent_raw: Vec<u8>,
}

/// Prepare one exact selected Expression responsibility attachment. All
/// retained native members are independently replayed from the selected input;
/// legacy responsibility carriers are accepted only when their exact source
/// event and output binding are present in that same input.
pub(crate) fn prepare_pg_expression_responsibility<'a, I: Copy + Eq>(
    fs: &'a CreationFilesystem,
    context: &CommandContext,
    input: &dyn SourceCutInputWithIdentity<I>,
    selected_catalog: &SelectedSourceWitnessCatalog,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PgExpressionResponsibilityOwnerPackage<'a>> {
    context.check_selected_software_inputs(software, components, limits.deadline, cancelled)?;
    for path in CONTRACTS {
        if context.file(&relative(path)?)?.is_none() {
            return Err(SourceCommandError::Unsupported(
                "selected Expression Responsibility registry/schema set incomplete",
            ));
        }
    }

    let fence = work_transaction::WorkCorpusFence::hold_existing(fs, limits.deadline, cancelled)?;
    let owner =
        ExpressionResponsibilityOwner::select(fs, context, false, limits.deadline, cancelled)?;
    let snapshot = work_transaction::PublicationSnapshot::select_from_input(
        input.source_input(),
        limits.deadline,
        cancelled,
    )?;
    check_requested_publication(&owner, &snapshot)?;
    snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled)?;
    let prior_id = selected_prior_publication(input, &snapshot, limits.deadline, cancelled)?;
    if let Some(id) = prior_id.as_deref() {
        selected_prior_completion(
            input.source_input(),
            &snapshot,
            id,
            snapshot.token.as_deref(),
            limits.deadline,
            cancelled,
        )?;
    }
    let before = selected_before(
        &owner,
        input.source_input(),
        &snapshot,
        limits.deadline,
        cancelled,
    )?;
    let selected = current_catalog_from_selected_input(
        &owner,
        &before,
        input,
        selected_catalog,
        worker,
        limits,
        cancelled,
    )?;
    let directories = owner.new_directories(fs, limits.deadline, cancelled)?;
    let claim_home = owner
        .claim_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid(
            "ExpressionResponsibility selected Claim home",
        ))?
        .0;
    verify_empty_prefix(input.source_input(), claim_home, limits.deadline, cancelled)?;

    let scope = serde_value(&owner.scope()?)?;
    let mut request_raw = cmd::canonical(&owner.request)?;
    request_raw.push(b'\n');
    let mut authorization_error = None;
    let core = tos_validation::native_compound::prepare_selected_expression_responsibility_bytes(
        input,
        worker,
        &scope,
        &request_raw,
        &before,
        &context.recorded_at,
        remaining(&selected.catalog, limits)?,
        cancelled,
        |grammar| {
            let result = (|| {
                let dependencies = dependency_bindings(&selected.catalog, grammar, context)?;
                if cmd::record_digest(&dependencies)?.to_prefixed()
                    != cmd::text(&owner.request, "expected_dependencies")?
                {
                    return Err(SourceCommandError::Conflict(
                        "ExpressionResponsibility current dependency closure changed",
                    ));
                }
                serde_value(&owner.authorization(dependencies)?)
            })();
            result.map_err(|error| {
                authorization_error = Some(error);
                ItemRefusal::Source(
                    "ExpressionResponsibility owner authorization rejected".to_owned(),
                )
            })
        },
    );
    let core = match core {
        Ok(core) => core,
        Err(error) => return Err(authorization_error.unwrap_or_else(|| item_error(error))),
    };
    if core.transaction_id() != owner.transaction_id()? {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility transaction identity differs from request",
        ));
    }
    let authorization = foundation_value(core.authorization())?;
    let archive_path = core.archive_path().to_owned();
    let outputs = core.outputs().map_err(item_error)?;
    let borrowed = outputs
        .iter()
        .map(|(path, raw)| (path.as_str(), *raw))
        .collect::<Vec<_>>();
    let capture = crate::source_serialization::capture_expression_responsibility(
        &owner.request,
        cmd::text(&owner.configuration, "provenance_event_id")?,
        claim_home,
        &archive_path,
        &before,
        &borrowed,
        software,
        components,
        limits.deadline,
        cancelled,
    )?;
    let finished =
        tos_validation::native_compound::finish_selected_expression_responsibility_bytes(
            core,
            &capture.environment_raw,
            &capture.event_raw,
            worker,
        )
        .map_err(item_error)?;
    if finished.transaction_id != owner.transaction_id()? || finished.archive_path != archive_path {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility finished byte recipe differs from selected transaction",
        ));
    }
    let receipt = finished_receipt(&finished.child)?;
    let materializations = compound_views(&finished.parent, &finished.child, &owner.claim_id)?;
    let old_expression = cmd::parse(before.get("expression.json").ok_or(
        SourceCommandError::Conflict("ExpressionResponsibility selected parent absent"),
    )?)?;
    let archive = work_transaction::expression_archive_package(
        owner.expression_path.as_str(),
        &old_expression,
        &before,
        cmd::text(&owner.request, "expected_revision")?,
    )?;
    if archive.path != archive_path {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility retained archive locator differs from recipe",
        ));
    }
    let mut plan = owner.plan(
        authorization.clone(),
        &before,
        finished.parent,
        finished.child,
        directories,
    )?;
    append_archive_package(input.source_input(), &mut plan, &archive, limits, cancelled)?;
    include_missing_output_directories(input.source_input(), &mut plan, limits, cancelled)?;
    let commit_members = work_transaction::pg_selected_commit_members(
        input.source_input(),
        &snapshot,
        &plan,
        limits.deadline,
        cancelled,
    )?;
    for (path, after) in commit_members {
        let before = selected_optional_member(
            input.source_input(),
            &path,
            16_777_216,
            limits.deadline,
            cancelled,
        )?;
        if path != "ToS/source-witnesses/.metadata-publication.json" && before.is_some() {
            return Err(SourceCommandError::Conflict(
                "ExpressionResponsibility PG transaction journal path already occupied",
            ));
        }
        plan.files.push(work_transaction::SelectedFile {
            path: relative(&path)?,
            before,
            after: Some(after),
        });
    }
    verify_new_paths_absent(input.source_input(), &plan, limits.deadline, cancelled)?;

    worker
        .finish(limits.deadline, cancelled)
        .map_err(item_error)?;
    software_current(fs, context, limits.deadline, cancelled)?;
    fs.current_context(context, limits.deadline, cancelled)?;
    snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled)?;
    fence.verify(limits.deadline, cancelled)?;

    let projected_outputs = output_bytes(&plan)?;
    let source_profiles = selected_source_profiles(context)?;
    let expression_raw = projected_outputs
        .get(owner.expression_path.as_str())
        .ok_or(SourceCommandError::Conflict(
            "ExpressionResponsibility output record absent",
        ))?;
    let expression = cmd::parse(expression_raw)?;
    let response = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_expression_responsibility_result_v1"),
        ),
        ("authentication", cmd::string("local-unix-account")),
        (
            "owner_configuration",
            cmd::string(&owner.configuration_digest),
        ),
        ("operation", cmd::string(OPERATION)),
        (
            "command_operations",
            JsonValue::Array(
                ["describe", "prepare-attach", OPERATION, RECOVERY]
                    .into_iter()
                    .map(cmd::string)
                    .collect(),
            ),
        ),
        (
            "allowed_operations",
            cmd::field(&owner.configuration, "allowed_operations")?.clone(),
        ),
        (
            "expression_source_path",
            cmd::string(owner.expression_path.as_str()),
        ),
        ("claim_source_path", cmd::string(owner.claim_path.as_str())),
        (
            "source",
            cmd::reference(&old_expression, "record_id", "record_version")?,
        ),
        (
            "revision",
            cmd::string(&crate::source_revisions::revision(&before)?),
        ),
        (
            "publication_snapshot",
            snapshot
                .token
                .as_deref()
                .map(cmd::string)
                .unwrap_or(JsonValue::Null),
        ),
        ("agent_record", selected.agent.clone()),
        (
            "agent_source_binding",
            cmd::object(vec![
                (
                    "source_path",
                    cmd::field(&owner.configuration, "agent_source_path")?.clone(),
                ),
                (
                    "source",
                    cmd::reference(&selected.agent, "record_id", "record_version")?,
                ),
                (
                    "source_sha256",
                    cmd::string(&Digest256::of_bytes(&selected.agent_raw).to_prefixed()),
                ),
                ("source_bytes", cmd::number(selected.agent_raw.len() as u64)),
            ]),
        ),
        ("scope", owner.scope()?),
        ("receipt", receipt),
        ("replayed", JsonValue::Bool(false)),
        ("recovery", JsonValue::Null),
        ("materializations", materializations),
        ("source_profiles", source_profiles),
        (
            "source_fields",
            JsonValue::Array(
                crate::source_forms::metadata_fields(&expression)?
                    .iter()
                    .map(|field| field.public())
                    .collect(),
            ),
        ),
        ("transaction_id", cmd::string(&owner.transaction_id()?)),
        ("grants_admission", JsonValue::Bool(false)),
    ]);
    let changes = plan
        .files
        .iter()
        .map(|file| {
            Ok(cmd::SourceChange {
                path: file.path.clone(),
                before: file.before.as_ref().map(|raw| Digest256::of_bytes(raw)),
                after: file.after.clone(),
            })
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let proposal = context.plan("native-expression-responsibility", response, changes, false)?;
    let semantic_indexes = semantic_index_rows(
        &projected_outputs,
        &owner.expression_path,
        &owner.expression_id,
        &owner.claim_path,
        &owner.claim_id,
        cmd::text(&owner.configuration, "provenance_event_id")?,
    )?;
    owner_contributions_from_members(
        projected_outputs
            .iter()
            .map(|(path, raw)| (path.as_str(), raw.as_slice())),
        owner.expression_path.as_str(),
        owner.claim_path.as_str(),
        &owner.expression_id,
        &owner.claim_id,
        cmd::text(&owner.configuration, "provenance_event_id")?,
        CORPUS_RECORD_SCHEMA,
        &context.files,
    )?;
    Ok(PgExpressionResponsibilityOwnerPackage {
        context: context.clone(),
        proposal,
        semantic_indexes,
        fence,
    })
}

/// Prepare the existing descriptive attach proposal from one exact selected
/// PG source epoch. This path never constructs a transaction plan or writes.
pub(crate) fn prepare_pg_expression_responsibility_preview<I: Copy + Eq>(
    fs: &CreationFilesystem,
    context: &CommandContext,
    input: &dyn SourceCutInputWithIdentity<I>,
    selected_catalog: &SelectedSourceWitnessCatalog,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ExpressionResponsibilityPreparation> {
    context.check_selected_software_inputs(software, components, limits.deadline, cancelled)?;
    for path in CONTRACTS {
        if context.file(&relative(path)?)?.is_none() {
            return Err(SourceCommandError::Unsupported(
                "selected Expression Responsibility registry/schema set incomplete",
            ));
        }
    }

    let mut owner =
        ExpressionResponsibilityOwner::select(fs, context, true, limits.deadline, cancelled)?;
    let snapshot = work_transaction::PublicationSnapshot::select_from_input(
        input.source_input(),
        limits.deadline,
        cancelled,
    )?;
    snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled)?;
    let prior_id = selected_prior_publication(input, &snapshot, limits.deadline, cancelled)?;
    if let Some(id) = prior_id.as_deref() {
        selected_prior_completion(
            input.source_input(),
            &snapshot,
            id,
            snapshot.token.as_deref(),
            limits.deadline,
            cancelled,
        )?;
    }
    let before = selected_before(
        &owner,
        input.source_input(),
        &snapshot,
        limits.deadline,
        cancelled,
    )?;
    let selected = current_catalog_from_selected_input(
        &owner,
        &before,
        input,
        selected_catalog,
        worker,
        limits,
        cancelled,
    )?;
    owner.new_directories(fs, limits.deadline, cancelled)?;
    let claim_home = owner
        .claim_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid(
            "ExpressionResponsibility preview Claim home",
        ))?
        .0
        .to_owned();
    verify_empty_prefix(
        input.source_input(),
        &claim_home,
        limits.deadline,
        cancelled,
    )?;

    let expression = cmd::parse(before.get("expression.json").ok_or(
        SourceCommandError::Conflict("ExpressionResponsibility preview selected record absent"),
    )?)?;
    let mut refs = cmd::array(&expression, "responsibility_claim_refs")?.to_vec();
    if refs.len() >= 128
        || refs
            .iter()
            .any(|value| value.as_str() == Some(owner.claim_id.as_str()))
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility preview Claim append invalid",
        ));
    }
    refs.push(cmd::string(&owner.claim_id));
    let claim = cmd::field(&owner.request, "claim")?;
    let mut claim_raw = cmd::canonical(claim)?;
    claim_raw.push(b'\n');
    let scope = serde_value(&owner.scope()?)?;
    let remaining_limits = remaining(&selected.catalog, limits)?;
    let mut prepared_request = None;
    let mut authorization_error = None;
    let core =
        tos_validation::native_compound::prepare_selected_expression_responsibility_preview_bytes(
            input,
            worker,
            &scope,
            &claim_raw,
            &before,
            &context.recorded_at,
            remaining_limits,
            cancelled,
            |grammar| {
                let result = (|| {
                    let dependencies = dependency_bindings(&selected.catalog, grammar, context)?;
                    let mut request = owner.request.clone();
                    cmd::set(&mut request, "operation", cmd::string(OPERATION))?;
                    cmd::set(
                        &mut request,
                        "command_id",
                        cmd::string("preview:uncommitted"),
                    )?;
                    cmd::set(
                        &mut request,
                        "fields",
                        cmd::object(vec![("responsibility_claim_refs", JsonValue::Array(refs))]),
                    )?;
                    cmd::set(
                        &mut request,
                        "expected_configuration",
                        cmd::string(&owner.configuration_digest),
                    )?;
                    cmd::set(
                        &mut request,
                        "expected_source",
                        cmd::reference(&expression, "record_id", "record_version")?,
                    )?;
                    cmd::set(
                        &mut request,
                        "expected_revision",
                        cmd::string(&crate::source_revisions::revision(&before)?),
                    )?;
                    cmd::set(
                        &mut request,
                        "expected_dependencies",
                        cmd::string(&cmd::record_digest(&dependencies)?.to_prefixed()),
                    )?;
                    cmd::set(
                        &mut request,
                        "expected_publication",
                        snapshot
                            .token
                            .as_ref()
                            .map_or(JsonValue::Null, |value| cmd::string(value)),
                    )?;
                    owner.request = request.clone();
                    let authorization = serde_value(&owner.authorization(dependencies)?)?;
                    let mut raw = cmd::canonical(&request)?;
                    raw.push(b'\n');
                    prepared_request = Some(request);
                    Ok((raw, authorization))
                })();
                result.map_err(|error| {
                    authorization_error = Some(error);
                    ItemRefusal::Source(
                        "ExpressionResponsibility preview authorization rejected".to_owned(),
                    )
                })
            },
        );
    let core = match core {
        Ok(core) => core,
        Err(error) => return Err(authorization_error.unwrap_or_else(|| item_error(error))),
    };
    let request = prepared_request.ok_or(SourceCommandError::Invalid(
        "ExpressionResponsibility preview full request absent",
    ))?;
    let authorization = foundation_value(core.authorization())?;
    if cmd::text(&authorization, "owner_configuration")? != owner.configuration_digest
        || !cmd::same(
            cmd::field(&authorization, "scope")?,
            &foundation_value(&scope)?,
        )?
        || cmd::record_digest(cmd::field(&authorization, "dependency_bindings")?)?.to_prefixed()
            != cmd::text(&request, "expected_dependencies")?
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility preview authorization differs from prepared request",
        ));
    }
    let outputs = core.outputs().map_err(item_error)?;
    for (path, _) in &outputs {
        if path.starts_with(&format!("{claim_home}/"))
            && input
                .source_input()
                .path_presence(path, limits.deadline, cancelled)
                .map_err(|_| {
                    SourceCommandError::Conflict("ExpressionResponsibility preview output presence")
                })?
                .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "ExpressionResponsibility preview Claim output path is occupied",
            ));
        }
    }
    drop(outputs);
    let projected_outputs = core.into_prepared_outputs().map_err(item_error)?;
    worker
        .finish(limits.deadline, cancelled)
        .map_err(item_error)?;
    fs.current_context(context, limits.deadline, cancelled)?;
    software_current(fs, context, limits.deadline, cancelled)?;
    snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled)?;
    Ok(ExpressionResponsibilityPreparation {
        request,
        projected_outputs,
    })
}

/// Build the current Expression/Agent result fields from one exact selected
/// PG source epoch. This is the `describe` surface; it does not prepare Claims
/// or materialize a responsibility proposal.
pub(crate) fn current_result_fields_from_selected_pg<I: Copy + Eq>(
    fs: &CreationFilesystem,
    context: &CommandContext,
    input: &dyn SourceCutInputWithIdentity<I>,
    catalog: &SelectedSourceWitnessCatalog,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let deadline = limits.deadline;
    context.check_selected_software_inputs(software, components, deadline, cancelled)?;
    let request = cmd::parse(&context.request_raw)?;
    cmd::exact_keys(&request, &["schema_version", "operation"])?;
    if cmd::text(&request, "schema_version")? != REQUEST
        || cmd::text(&request, "operation")? != "describe"
    {
        return Err(SourceCommandError::Invalid(
            "ExpressionResponsibility selected describe request",
        ));
    }

    for path in CONTRACTS {
        let expected = context
            .file(&relative(path)?)?
            .ok_or(SourceCommandError::Unsupported(
                "selected Expression Responsibility registry/schema set incomplete",
            ))?;
        let selected = read_selected_source_member(
            input.source_input(),
            path,
            2_097_152,
            deadline,
            cancelled,
        )?;
        if selected.as_slice() != expected {
            return Err(SourceCommandError::Conflict(
                "ExpressionResponsibility describe schema differs from selected context",
            ));
        }
    }

    let grant =
        super::ExpressionResponsibilityGrant::select(fs, context, None, deadline, cancelled)?;
    let config = grant.configuration;
    let expression_id = cmd::text(&config, "expression_id")?;
    let expression_path = grant.expression_path.as_str();
    let agent_id = cmd::text(&config, "agent_id")?;
    let agent_path = grant.agent_path.as_str();
    let records = selected_entries(catalog, "record")?;
    let expression_entry = records
        .get(expression_id)
        .ok_or(SourceCommandError::Conflict(
            "PG ExpressionResponsibility current Expression is absent from catalog",
        ))?;
    let agent_entry = records.get(agent_id).ok_or(SourceCommandError::Conflict(
        "PG ExpressionResponsibility current Agent is absent from catalog",
    ))?;
    if cmd::text(expression_entry, "source_record_ref")? != expression_path
        || cmd::text(agent_entry, "source_record_ref")? != agent_path
    {
        return Err(SourceCommandError::Conflict(
            "PG ExpressionResponsibility current endpoint differs from catalog",
        ));
    }

    let snapshot = work_transaction::PublicationSnapshot::select_from_input(
        input.source_input(),
        deadline,
        cancelled,
    )?;
    if let Some(id) = selected_prior_publication(input, &snapshot, deadline, cancelled)? {
        selected_prior_completion(
            input.source_input(),
            &snapshot,
            &id,
            snapshot.token.as_deref(),
            deadline,
            cancelled,
        )?;
    }

    let mut digests = catalog.digests.clone();
    let mut total = catalog.bytes_read;
    let (expression, expression_raw) = selected_record(
        input.source_input(),
        expression_entry,
        &mut digests,
        &mut total,
        deadline,
        cancelled,
    )?;
    if cmd::text(&expression, "record_type")? != "expression"
        || cmd::text(&expression, "record_id")? != expression_id
    {
        return Err(SourceCommandError::Conflict(
            "PG ExpressionResponsibility current Expression identity differs",
        ));
    }
    let (agent, agent_raw) = selected_record(
        input.source_input(),
        agent_entry,
        &mut digests,
        &mut total,
        deadline,
        cancelled,
    )?;
    if cmd::text(&agent, "record_type")? != "agent" || cmd::text(&agent, "record_id")? != agent_id {
        return Err(SourceCommandError::Conflict(
            "PG ExpressionResponsibility current Agent identity differs",
        ));
    }

    let home = expression_path
        .rsplit_once('/')
        .map(|(home, _)| home)
        .ok_or(SourceCommandError::Invalid(
            "PG ExpressionResponsibility current Expression home",
        ))?;
    let mut package = BTreeMap::from([("expression.json".to_owned(), expression_raw.clone())]);
    for name in [
        "expression.human-forms.json",
        "source-revision-history.json",
    ] {
        let path = format!("{home}/{name}");
        if let Some(raw) =
            selected_optional_member(input.source_input(), &path, 2_097_152, deadline, cancelled)?
        {
            package.insert(name.to_owned(), raw);
        }
    }
    tos_validation::native_compound::inspect_record_history(
        &package,
        &expression_raw,
        deadline,
        cancelled,
    )
    .map_err(item_error)?;

    crate::source_claims::work_expression_source_descriptors_candidate(
        context, input, worker, limits, cancelled,
    )?;
    let source_profiles = selected_source_profiles(context)?;
    let scope = JsonValue::Object(
        config
            .as_object()
            .ok_or(SourceCommandError::Invalid(
                "PG ExpressionResponsibility current owner grant",
            ))?
            .iter()
            .filter(|(key, _)| key.as_str().is_some_and(|key| SCOPE_KEYS.contains(&key)))
            .cloned()
            .collect(),
    );
    let result = cmd::object(vec![
        (
            "source",
            cmd::reference(&expression, "record_id", "record_version")?,
        ),
        (
            "revision",
            cmd::string(&crate::source_revisions::revision(&package)?),
        ),
        (
            "publication_snapshot",
            snapshot
                .token
                .as_deref()
                .map(cmd::string)
                .unwrap_or(JsonValue::Null),
        ),
        ("agent_record", agent.clone()),
        (
            "agent_source_binding",
            cmd::object(vec![
                (
                    "source_path",
                    cmd::field(&config, "agent_source_path")?.clone(),
                ),
                (
                    "source",
                    cmd::reference(&agent, "record_id", "record_version")?,
                ),
                (
                    "source_sha256",
                    cmd::string(&Digest256::of_bytes(&agent_raw).to_prefixed()),
                ),
                ("source_bytes", cmd::number(agent_raw.len() as u64)),
            ]),
        ),
        ("scope", scope),
        ("materializations", JsonValue::Null),
        ("source_profiles", source_profiles),
        (
            "source_fields",
            JsonValue::Array(
                crate::source_forms::metadata_fields(&expression)?
                    .iter()
                    .map(|field| field.public())
                    .collect(),
            ),
        ),
    ]);
    worker.finish(deadline, cancelled).map_err(item_error)?;
    software_current(fs, context, deadline, cancelled)?;
    fs.current_context(context, deadline, cancelled)?;
    snapshot.verify_from_input(input.source_input(), deadline, cancelled)?;
    Ok(result)
}

/// Re-derive mixed-inventory contributions from the exact writes in a sealed
/// PG attempt. This is used at registration and recovery, where a prep-time
/// map is not an authority source.
pub(crate) fn expected_inventory_contributions_from_writes(
    writes: &[SourceRevisionWrite],
    context: &CommandContext,
    response: &JsonValue,
) -> crate::DurableResult<BTreeMap<String, JsonValue>> {
    expected_revision_indexes_from_writes(writes)?;
    let scope = cmd::field(response, "scope").map_err(crate::DurableError::Source)?;
    let expression_path =
        cmd::text(response, "expression_source_path").map_err(crate::DurableError::Source)?;
    let claim_path =
        cmd::text(response, "claim_source_path").map_err(crate::DurableError::Source)?;
    let expression_id = cmd::text(scope, "expression_id").map_err(crate::DurableError::Source)?;
    let claim_id = cmd::text(scope, "claim_id").map_err(crate::DurableError::Source)?;
    let event_id = cmd::text(scope, "provenance_event_id").map_err(crate::DurableError::Source)?;
    let members = writes
        .iter()
        .map(|write| (write.subject.as_str(), write.exact_bytes.as_slice()));
    owner_contributions_from_members(
        members,
        expression_path,
        claim_path,
        expression_id,
        claim_id,
        event_id,
        CORPUS_RECORD_SCHEMA,
        &context.files,
    )
    .map_err(crate::DurableError::Source)
}

/// Re-derive semantic and path-index rows from a sealed owner write set.
/// Recovery does not trust the index rows serialized in the attempt record.
pub(crate) fn expected_revision_indexes_from_writes(
    writes: &[SourceRevisionWrite],
) -> crate::DurableResult<IndexRows> {
    use crate::DurableError;

    if writes.is_empty() || writes.len() > 512 {
        return Err(DurableError::Corrupt(
            "PG ExpressionResponsibility write count differs",
        ));
    }
    let mut outputs = BTreeMap::new();
    let mut indexes = writes
        .iter()
        .filter(|write| write.expected_predecessor.is_none())
        .map(|write| {
            (
                "path".to_owned(),
                write.subject.clone(),
                write.subject.clone(),
            )
        })
        .collect::<IndexRows>();
    for write in writes {
        if !write.subject.starts_with("ToS/source-witnesses/")
            || outputs
                .insert(write.subject.clone(), write.exact_bytes.clone())
                .is_some()
        {
            return Err(DurableError::Corrupt(
                "PG ExpressionResponsibility writes duplicate or leave source homes",
            ));
        }
    }

    let mut expression = None;
    for (path, raw) in &outputs {
        if path.rsplit('/').next() != Some("expression.json") {
            continue;
        }
        let record = cmd::parse(raw).map_err(DurableError::Source)?;
        if cmd::text(&record, "record_type").map_err(DurableError::Source)? != "expression" {
            return Err(DurableError::Corrupt(
                "PG ExpressionResponsibility metadata kind differs",
            ));
        }
        let id = cmd::text(&record, "record_id")
            .map_err(DurableError::Source)?
            .to_owned();
        if writes
            .iter()
            .filter(|write| write.subject.as_str() == path.as_str())
            .count()
            != 1
            || expression.replace((path.clone(), id)).is_some()
        {
            return Err(DurableError::Corrupt(
                "PG ExpressionResponsibility metadata path is ambiguous",
            ));
        }
    }
    let (expression_path, expression_id) = expression.ok_or(DurableError::Corrupt(
        "PG ExpressionResponsibility successor metadata absent",
    ))?;
    if !writes.iter().any(|write| {
        write.subject.as_str() == expression_path.as_str() && write.expected_predecessor.is_some()
    }) {
        return Err(DurableError::Corrupt(
            "PG ExpressionResponsibility metadata is not a predecessor update",
        ));
    }

    let claim_streams = outputs
        .iter()
        .filter(|(path, _)| path.rsplit('/').next() == Some("source-claims.jsonl"))
        .collect::<Vec<_>>();
    if claim_streams.len() != 1 {
        return Err(DurableError::Corrupt(
            "PG ExpressionResponsibility Claim stream cardinality differs",
        ));
    }
    let (claim_path, claim_raw) = claim_streams[0];
    let claims = catalog_lines(claim_raw)
        .filter(|line| !crate::source_claims::python_bytes_blank(line))
        .map(|line| cmd::parse(line).map_err(DurableError::Source))
        .collect::<crate::DurableResult<Vec<_>>>()?;
    if claims.len() != 1
        || cmd::text(&claims[0], "predicate").map_err(DurableError::Source)? != "translated_by"
        || cmd::text(&claims[0], "subject_ref").map_err(DurableError::Source)? != expression_id
        || !writes.iter().any(|write| {
            write.subject.as_str() == claim_path.as_str() && write.expected_predecessor.is_none()
        })
    {
        return Err(DurableError::Corrupt(
            "PG ExpressionResponsibility Claim identity or write class differs",
        ));
    }
    let claim_id = cmd::text(&claims[0], "claim_id")
        .map_err(DurableError::Source)?
        .to_owned();
    let event_id = cmd::text(&claims[0], "provenance_event_ref")
        .map_err(DurableError::Source)?
        .to_owned();
    let claim_home = claim_path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or(DurableError::Corrupt(
            "PG ExpressionResponsibility Claim home absent",
        ))?;
    let event_path = format!("{claim_home}/source-create-provenance.jsonl");
    let event_raw = outputs.get(&event_path).ok_or(DurableError::Corrupt(
        "PG ExpressionResponsibility event stream absent",
    ))?;
    let events = catalog_lines(event_raw)
        .filter(|line| !crate::source_claims::python_bytes_blank(line))
        .map(|line| cmd::parse(line).map_err(DurableError::Source))
        .collect::<crate::DurableResult<Vec<_>>>()?;
    if events.len() != 1
        || cmd::text(&events[0], "event_id").map_err(DurableError::Source)? != event_id
    {
        return Err(DurableError::Corrupt(
            "PG ExpressionResponsibility event identity differs",
        ));
    }
    let semantic = semantic_index_rows(
        &outputs,
        &tos_foundation::RelativePath::parse(&expression_path)
            .map_err(|_| DurableError::Corrupt("PG ExpressionResponsibility path is invalid"))?,
        &expression_id,
        &tos_foundation::RelativePath::parse(claim_path)
            .map_err(|_| DurableError::Corrupt("PG ExpressionResponsibility Claim path invalid"))?,
        &claim_id,
        &event_id,
    )
    .map_err(DurableError::Source)?;
    indexes.extend(semantic);
    Ok(indexes)
}

/// Compare exact PG reads and absences with the protected physical source tree
/// while the retained owner fence is held by the caller.
pub(crate) fn verify_pg_expression_responsibility_context(
    fs: &CreationFilesystem,
    context: &CommandContext,
    absences: &BTreeSet<String>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    fs.current_context(context, deadline, cancelled)?;
    software_current(fs, context, deadline, cancelled)?;
    for path in absences {
        let (parent, leaf) = path.rsplit_once('/').ok_or(SourceCommandError::Invalid(
            "ExpressionResponsibility exact absence path",
        ))?;
        let Some(directory) = work_transaction::read_existing_parent(fs, parent)? else {
            continue;
        };
        if work_transaction::read_at(&directory, leaf, fs.uid, 16_777_216, deadline, cancelled)?
            .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "ExpressionResponsibility exact path absence differs from protected corpus",
            ));
        }
    }
    Ok(())
}

fn check_requested_publication(
    owner: &ExpressionResponsibilityOwner,
    snapshot: &work_transaction::PublicationSnapshot,
) -> SourceCommandResult<()> {
    let requested = cmd::field(&owner.request, "expected_publication")?;
    if requested.as_str() != snapshot.token.as_deref()
        && !(requested == &JsonValue::Null && snapshot.token.is_none())
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility requested publication snapshot changed",
        ));
    }
    Ok(())
}

fn selected_before(
    owner: &ExpressionResponsibilityOwner,
    input: &dyn SourceCutInput,
    snapshot: &work_transaction::PublicationSnapshot,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let home = owner
        .expression_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid(
            "ExpressionResponsibility selected parent",
        ))?
        .0;
    let mut before = BTreeMap::new();
    let mut total = 0usize;
    for name in [
        "expression.json",
        "expression.human-forms.json",
        "source-revision-history.json",
    ] {
        let path = format!("{home}/{name}");
        let raw = selected_optional_member(input, &path, 2_097_152, deadline, cancelled)?;
        if name == "expression.json" && raw.is_none() {
            return Err(SourceCommandError::Conflict(
                "ExpressionResponsibility selected parent absent",
            ));
        }
        if let Some(raw) = raw {
            total = total
                .checked_add(raw.len())
                .filter(|bytes| *bytes <= 8_388_608)
                .ok_or(SourceCommandError::Invalid(
                    "selected ExpressionResponsibility package byte budget",
                ))?;
            before.insert(name.to_owned(), raw);
        }
    }
    snapshot.verify_from_input(input, deadline, cancelled)?;
    Ok(before)
}

fn current_catalog_from_selected_input<I: Copy + Eq>(
    owner: &ExpressionResponsibilityOwner,
    before: &BTreeMap<String, Vec<u8>>,
    input: &dyn SourceCutInputWithIdentity<I>,
    selected: &SelectedSourceWitnessCatalog,
    worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<SelectedResponsibilityCatalog> {
    let deadline = limits.deadline;
    let records = selected_entries(selected, "record")?;
    let claims = selected_entries(selected, "claim")?;
    let mut digests = selected.digests.clone();
    let mut total = selected.bytes_read;
    let expression_raw = before
        .get("expression.json")
        .ok_or(SourceCommandError::Conflict(
            "ExpressionResponsibility selected predecessor absent",
        ))?;
    let expression = cmd::parse(expression_raw)?;
    let history = tos_validation::native_compound::inspect_record_history(
        before,
        expression_raw,
        deadline,
        cancelled,
    )
    .map_err(item_error)?;
    let parent_entry = records
        .get(&owner.expression_id)
        .ok_or(SourceCommandError::Conflict(
            "ExpressionResponsibility catalog parent absent",
        ))?;
    if cmd::text(&expression, "record_type")? != "expression"
        || cmd::text(&expression, "record_id")? != owner.expression_id
        || cmd::text(parent_entry, "source_record_ref")? != owner.expression_path.as_str()
        || cmd::text(parent_entry, "record_sha256")? != cmd::record_digest(&expression)?.to_hex()
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility catalog exact parent stale",
        ));
    }
    if selected.record_ids.contains(&owner.claim_id)
        || selected.claim_ids.contains(&owner.claim_id)
        || claims.values().any(|entry| {
            entry.object_get("provenance_event_ref")
                == owner.configuration.object_get("provenance_event_id")
        })
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility new Claim/event identity occupied",
        ));
    }
    let agent_entry = records
        .get(&owner.agent_id)
        .ok_or(SourceCommandError::Conflict(
            "ExpressionResponsibility existing Agent absent",
        ))?;
    if cmd::text(agent_entry, "record_type")? != "agent"
        || cmd::text(agent_entry, "source_record_ref")? != owner.agent_path.as_str()
    {
        return Err(SourceCommandError::Denied(
            "ExpressionResponsibility existing exact Agent endpoint",
        ));
    }
    let (agent, agent_raw) = selected_record(
        input.source_input(),
        agent_entry,
        &mut digests,
        &mut total,
        deadline,
        cancelled,
    )?;
    if !cmd::same(&agent, cmd::field(&owner.request, "agent")?)? {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility caller Agent differs from current source",
        ));
    }

    let mut agents = BTreeMap::from([(owner.agent_id.clone(), agent.clone())]);
    let mut selected_ids = BTreeSet::new();
    let mut retained_transactions = BTreeMap::new();
    let mut native_reads = Vec::new();
    let mut native_bytes = 0u64;
    let mut native_state = 0usize;
    let mut legacy_events: BTreeMap<String, Vec<JsonValue>> = BTreeMap::new();
    for (claim_id, entry) in claims.iter().filter(|(_, entry)| {
        entry.object_get("predicate").and_then(JsonValue::as_str) == Some("translated_by")
            && entry.object_get("subject_ref").and_then(JsonValue::as_str)
                == Some(owner.expression_id.as_str())
    }) {
        active(deadline, cancelled)?;
        let claim = selected_claim(
            input.source_input(),
            entry,
            &mut digests,
            &mut total,
            deadline,
            cancelled,
        )?;
        let target = cmd::text(&claim, "object")?;
        let target_entry = records.get(target).ok_or(SourceCommandError::Conflict(
            "ExpressionResponsibility member Agent absent",
        ))?;
        if cmd::text(target_entry, "record_type")? != "agent" {
            return Err(SourceCommandError::Conflict(
                "ExpressionResponsibility member endpoint is not Agent",
            ));
        }
        if !agents.contains_key(target) {
            let (value, _) = selected_record(
                input.source_input(),
                target_entry,
                &mut digests,
                &mut total,
                deadline,
                cancelled,
            )?;
            agents.insert(target.to_owned(), value);
        }
        let path = cmd::text(entry, "source_claim_file_ref")?;
        if path.ends_with("/source-claims.jsonl") {
            let mut remaining_limits = limits;
            remaining_limits.max_total_bytes = limits
                .max_total_bytes
                .checked_sub(native_bytes)
                .and_then(|value| value.checked_sub(total as u64))
                .ok_or(SourceCommandError::Invalid(
                    "ExpressionResponsibility cumulative native bytes",
                ))?;
            remaining_limits.max_state_bytes =
                limits.max_state_bytes.checked_sub(native_state).ok_or(
                    SourceCommandError::Invalid("ExpressionResponsibility cumulative native state"),
                )?;
            let observation = tos_validation::native_compound::verify_expression_responsibility_from_selected_input(
                input,
                worker,
                path,
                &serde_value(&claim)?,
                remaining_limits,
                cancelled,
            )
            .map_err(item_error)?;
            let (manifest, authorization) = selected_committed_authorization(
                input.source_input(),
                &observation.transaction_id,
                &observation.manifest_sha256,
                &mut digests,
                &mut total,
                deadline,
                cancelled,
            )?;
            let _ = manifest;
            if observation.transport
                != tos_validation::native_compound::NativeTransportState::Committed
                || observation.claim_id != *claim_id
                || cmd::text(&authorization, "schema_version")? != AUTHORIZATION
                || cmd::text(cmd::field(&authorization, "scope")?, "expression_id")?
                    != owner.expression_id
                || !history["receipts"]
                    .as_array()
                    .ok_or(SourceCommandError::Invalid(
                        "ExpressionResponsibility retained history",
                    ))?
                    .iter()
                    .any(|receipt| {
                        receipt["publication"]["transaction_id"].as_str()
                            == Some(observation.transaction_id.as_str())
                    })
            {
                return Err(SourceCommandError::Conflict(
                    "ExpressionResponsibility member lacks exact committed parent lineage",
                ));
            }
            if let Some(previous) = retained_transactions.insert(
                observation.transaction_id.clone(),
                observation.manifest_sha256.clone(),
            ) && previous != observation.manifest_sha256
            {
                return Err(SourceCommandError::Conflict(
                    "ExpressionResponsibility retained transaction changed",
                ));
            }
            native_bytes = native_bytes.checked_add(observation.bytes_read).ok_or(
                SourceCommandError::Invalid("ExpressionResponsibility native byte overflow"),
            )?;
            native_state = native_state
                .checked_add(observation.returned_state_bytes)
                .ok_or(SourceCommandError::Invalid(
                    "ExpressionResponsibility native state overflow",
                ))?;
            native_reads.extend(observation.reads);
        } else if path.ends_with("/responsibility-claims.jsonl") {
            let provenance = format!(
                "{}/provenance.jsonl",
                path.rsplit_once('/')
                    .ok_or(SourceCommandError::Invalid(
                        "ExpressionResponsibility legacy carrier",
                    ))?
                    .0
            );
            if !legacy_events.contains_key(&provenance) {
                let raw = selected_source_dependency(
                    input.source_input(),
                    &provenance,
                    &mut digests,
                    &mut total,
                    deadline,
                    cancelled,
                )?;
                let mut events = Vec::new();
                for line in catalog_lines(&raw) {
                    if !crate::source_claims::python_bytes_blank(line) {
                        events.push(cmd::parse(line)?);
                    }
                }
                legacy_events.insert(provenance.clone(), events);
            }
            let matches = legacy_events
                .get(&provenance)
                .ok_or(SourceCommandError::Conflict(
                    "ExpressionResponsibility legacy provenance absent",
                ))?
                .iter()
                .filter(|event| {
                    event.object_get("event_id") == claim.object_get("provenance_event_ref")
                })
                .collect::<Vec<_>>();
            if cmd::text(&claim, "claim_type")? != "bibliographic"
                || !matches!(
                    cmd::text(&claim, "assertion_layer")?,
                    "bibliographic_assertion" | "scholarly_report"
                )
                || matches.len() != 1
                || !cmd::array(matches[0], "outputs")?.iter().any(|output| {
                    output.object_get("ref").and_then(JsonValue::as_str) == Some(path)
                        && output.object_get("sha256").and_then(JsonValue::as_str)
                            == digests.get(path).map(String::as_str)
                })
            {
                return Err(SourceCommandError::Conflict(
                    "ExpressionResponsibility legacy responsibility retained batch differs",
                ));
            }
        } else {
            return Err(SourceCommandError::Denied(
                "ExpressionResponsibility member has no declared evidence carrier",
            ));
        }
        selected_ids.insert(claim_id.clone());
    }
    let declared = cmd::array(&expression, "responsibility_claim_refs")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or(SourceCommandError::Invalid(
                    "ExpressionResponsibility reference list",
                ))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    if declared.len() != selected_ids.len()
        || declared.iter().cloned().collect::<BTreeSet<_>>() != selected_ids
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility complete responsibility union differs",
        ));
    }
    let proposed_claim = cmd::field(&owner.request, "claim")?;
    for key in ["evidence_refs", "counterevidence_refs"] {
        if let Some(values) = proposed_claim.object_get(key) {
            for reference in values.as_array().ok_or(SourceCommandError::Invalid(
                "ExpressionResponsibility evidence list",
            ))? {
                let reference = reference.as_str().ok_or(SourceCommandError::Invalid(
                    "ExpressionResponsibility evidence reference",
                ))?;
                if reference.starts_with("ToS/") && reference != owner.expression_path.as_str() {
                    selected_source_dependency(
                        input.source_input(),
                        reference,
                        &mut digests,
                        &mut total,
                        deadline,
                        cancelled,
                    )?;
                }
            }
        }
    }
    if let Some(values) = proposed_claim.object_get("alternative_claim_refs") {
        if values
            .as_array()
            .ok_or(SourceCommandError::Invalid(
                "ExpressionResponsibility alternative Claim list",
            ))?
            .iter()
            .any(|value| !value.as_str().is_some_and(|id| claims.contains_key(id)))
        {
            return Err(SourceCommandError::Conflict(
                "ExpressionResponsibility alternative Claim lacks catalog identity",
            ));
        }
    }
    let catalog = ExpressionResponsibilityCatalog {
        digests,
        retained_transactions,
        native_reads,
        native_bytes,
        native_state,
        source_bytes: total as u64,
    };
    Ok(SelectedResponsibilityCatalog {
        catalog,
        agent,
        agent_raw,
    })
}

fn selected_entries(
    selected: &SelectedSourceWitnessCatalog,
    kind: &str,
) -> SourceCommandResult<BTreeMap<String, JsonValue>> {
    let mut result = BTreeMap::new();
    for (entry_kind, entry) in &selected.entries {
        if entry_kind != kind {
            continue;
        }
        let id = if kind == "claim" {
            cmd::text(entry, "claim_id")?
        } else {
            cmd::text(entry, "record_id")?
        };
        if result.insert(id.to_owned(), entry.clone()).is_some() {
            return Err(SourceCommandError::Conflict(
                "ExpressionResponsibility catalog identity repeats",
            ));
        }
    }
    Ok(result)
}

fn selected_record(
    input: &dyn SourceCutInput,
    entry: &JsonValue,
    digests: &mut BTreeMap<String, String>,
    total: &mut usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, Vec<u8>)> {
    let path = cmd::text(entry, "source_record_ref")?;
    let record_type = cmd::text(entry, "record_type")?;
    if path.rsplit_once('/').map(|(_, name)| name) != Some(format!("{record_type}.json").as_str()) {
        return Err(SourceCommandError::Invalid(
            "ExpressionResponsibility catalog record path",
        ));
    }
    let raw = selected_source_dependency(input, path, digests, total, deadline, cancelled)?;
    if raw.len() > 1_048_576 {
        return Err(SourceCommandError::Invalid(
            "ExpressionResponsibility typed record byte budget",
        ));
    }
    let record = cmd::parse(&raw)?;
    if cmd::field(&record, "record_id")? != cmd::field(entry, "record_id")?
        || cmd::field(&record, "record_type")? != cmd::field(entry, "record_type")?
        || cmd::record_digest(&record)?.to_hex() != cmd::text(entry, "record_sha256")?
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility current catalog record stale",
        ));
    }
    Ok((record, raw))
}

fn selected_claim(
    input: &dyn SourceCutInput,
    entry: &JsonValue,
    digests: &mut BTreeMap<String, String>,
    total: &mut usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let path = cmd::text(entry, "source_claim_file_ref")?;
    let raw = selected_source_dependency(input, path, digests, total, deadline, cancelled)?;
    let line = usize::try_from(cmd::integer(entry, "source_claim_line")?)
        .map_err(|_| SourceCommandError::Invalid("ExpressionResponsibility Claim line"))?
        .checked_sub(1)
        .ok_or(SourceCommandError::Invalid(
            "ExpressionResponsibility Claim line",
        ))?;
    let claim = cmd::parse(
        catalog_lines(&raw)
            .nth(line)
            .ok_or(SourceCommandError::Conflict(
                "ExpressionResponsibility Claim line absent",
            ))?,
    )?;
    for key in [
        "claim_id",
        "predicate",
        "object",
        "subject_ref",
        "provenance_event_ref",
    ] {
        if cmd::field(&claim, key)? != cmd::field(entry, key)? {
            return Err(SourceCommandError::Conflict(
                "ExpressionResponsibility catalog Claim endpoint stale",
            ));
        }
    }
    if cmd::record_digest(&claim)?.to_hex() != cmd::text(entry, "claim_sha256")? {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility catalog Claim digest stale",
        ));
    }
    Ok(claim)
}

fn selected_source_dependency(
    input: &dyn SourceCutInput,
    path: &str,
    digests: &mut BTreeMap<String, String>,
    total: &mut usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let raw = read_selected_source_member(input, path, 2_097_152, deadline, cancelled)?;
    *total = total
        .checked_add(raw.len())
        .ok_or(SourceCommandError::Invalid(
            "ExpressionResponsibility current source byte overflow",
        ))?;
    if *total > 33_554_432 || digests.len() >= 512 {
        return Err(SourceCommandError::Invalid(
            "ExpressionResponsibility current source dependency budget",
        ));
    }
    let digest = raw_hex(&raw);
    if let Some(prior) = digests.insert(path.to_owned(), digest.clone())
        && prior != digest
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility current dependency changed",
        ));
    }
    Ok(raw)
}

fn selected_committed_authorization(
    input: &dyn SourceCutInput,
    id: &str,
    expected_manifest_sha256: &str,
    digests: &mut BTreeMap<String, String>,
    total: &mut usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, JsonValue)> {
    if id.len() != 71 || !id.starts_with("sha256:") {
        return Err(SourceCommandError::Invalid(
            "ExpressionResponsibility retained transaction identity",
        ));
    }
    let path = format!(
        "ToS/source-witnesses/.metadata-transactions/{}/manifest.json",
        &id[7..]
    );
    let raw = selected_source_dependency(input, &path, digests, total, deadline, cancelled)?;
    if Digest256::of_bytes(&raw).to_prefixed() != expected_manifest_sha256 {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility retained manifest digest differs",
        ));
    }
    let manifest = cmd::parse(&raw)?;
    let plan = cmd::field(&manifest, "plan")?;
    let authorization = cmd::field(plan, "authorization")?.clone();
    Ok((manifest, authorization))
}

fn selected_prior_publication<I: Copy + Eq>(
    input: &dyn SourceCutInputWithIdentity<I>,
    snapshot: &work_transaction::PublicationSnapshot,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<String>> {
    let path = "ToS/source-witnesses/.metadata-publication.json";
    if input
        .path_presence(path, deadline, cancelled)
        .map_err(|_| {
            SourceCommandError::Conflict("ExpressionResponsibility prior publication presence")
        })?
        .is_none()
    {
        if snapshot.token.is_some() {
            return Err(SourceCommandError::Conflict(
                "ExpressionResponsibility prior publication absent",
            ));
        }
        return Ok(None);
    }
    let state = cmd::parse(&read_selected_source_member(
        input.source_input(),
        path,
        8192,
        deadline,
        cancelled,
    )?)?;
    work_transaction::state(&state)?;
    if cmd::text(&state, "phase")? != "ready"
        || Some(cmd::text(&state, "token")?) != snapshot.token.as_deref()
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility prior publication differs from protected head",
        ));
    }
    Ok(Some(cmd::text(&state, "transaction_id")?.to_owned()))
}

fn selected_prior_completion(
    input: &dyn SourceCutInput,
    snapshot: &work_transaction::PublicationSnapshot,
    id: &str,
    expected_token: Option<&str>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let token = expected_token.ok_or(SourceCommandError::Conflict(
        "ExpressionResponsibility prior publication token absent",
    ))?;
    let state = snapshot
        .selected_state()
        .ok_or(SourceCommandError::Conflict(
            "ExpressionResponsibility prior publication state absent",
        ))?;
    if snapshot.token.as_deref() != Some(token) || cmd::text(state, "transaction_id")? != id {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility prior publication does not bind selected head",
        ));
    }
    let journal = format!(
        "ToS/source-witnesses/.metadata-transactions/{}",
        id.strip_prefix("sha256:")
            .ok_or(SourceCommandError::Invalid(
                "ExpressionResponsibility prior transaction identity",
            ))?
    );
    let manifest_path = format!("{journal}/manifest.json");
    let manifest_raw =
        selected_optional_member(input, &manifest_path, 524_288, deadline, cancelled)?.ok_or(
            SourceCommandError::Conflict("ExpressionResponsibility prior manifest absent"),
        )?;
    let manifest = cmd::parse(&manifest_raw)?;
    let schema = cmd::text(&manifest, "schema_version")?;
    let common = [
        "schema_version",
        "transaction_id",
        "base_publication",
        "plan",
        "parents",
    ];
    match schema {
        "tos_selected_metadata_transaction_v1"
        | "tos_selected_metadata_transaction_v2"
        | "tos_selected_metadata_transaction_v3" => cmd::exact_keys(&manifest, &common)?,
        "tos_selected_metadata_transaction_v4" => cmd::exact_keys(
            &manifest,
            &[
                "schema_version",
                "transaction_id",
                "base_publication",
                "plan",
                "parents",
                "source_readset",
                "source_successor",
            ],
        )?,
        work_transaction::PG_SOURCE_MANIFEST_SCHEMA => {
            cmd::exact_keys(
                &manifest,
                &[
                    "schema_version",
                    "transaction_id",
                    "base_publication",
                    "plan",
                    "parents",
                    "parent_binding_profile",
                ],
            )?;
            if cmd::text(&manifest, "parent_binding_profile")?
                != "tos_source_revision_parent_bindings_v1"
            {
                return Err(SourceCommandError::Conflict(
                    "ExpressionResponsibility prior PG parent binding profile differs",
                ));
            }
        }
        _ => {
            return Err(SourceCommandError::Conflict(
                "ExpressionResponsibility prior manifest schema",
            ));
        }
    }
    let base = cmd::field(&manifest, "base_publication")?;
    cmd::exact_keys(base, &["token", "generation"])?;
    let base_generation = cmd::integer(base, "generation")?;
    let base_token_valid = match cmd::field(base, "token")? {
        JsonValue::Null => base_generation == 0,
        JsonValue::String(value) => {
            base_generation > 0 && value.as_str().is_some_and(work_transaction::is_hash)
        }
        _ => false,
    };
    if cmd::text(&manifest, "transaction_id")? != id
        || Digest256::of_bytes(&manifest_raw).to_prefixed() != cmd::text(state, "manifest_sha256")?
        || base_generation.checked_add(2) != Some(cmd::integer(state, "generation")?)
        || !base_token_valid
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility prior publication manifest differs",
        ));
    }
    let completion_raw = selected_optional_member(
        input,
        &format!("{journal}/completion.json"),
        8192,
        deadline,
        cancelled,
    )?
    .ok_or(SourceCommandError::Conflict(
        "ExpressionResponsibility prior completion absent",
    ))?;
    let completion = cmd::parse(&completion_raw)?;
    cmd::exact_keys(&completion, &["schema_version", "publication"])?;
    if cmd::text(&completion, "schema_version")? != "tos_selected_metadata_completion_v1"
        || !cmd::same(cmd::field(&completion, "publication")?, state)?
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility prior completion differs from selected publication",
        ));
    }
    Ok(())
}

fn read_selected_source_member(
    input: &dyn SourceCutInput,
    path: &str,
    max_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let canonical = relative(path)?;
    if canonical.as_str() != path
        || input
            .path_presence(path, deadline, cancelled)
            .map_err(|_| {
                SourceCommandError::Conflict("ExpressionResponsibility selected member presence")
            })?
            .is_none()
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility selected member absent",
        ));
    }
    let mut selected = None;
    input
        .with_current_member(
            path,
            max_bytes.min(8_388_608),
            deadline,
            cancelled,
            &mut |meta: SourceCutMemberMeta<'_>, raw| {
                if meta.path != path || raw.len() as u64 != meta.size_bytes {
                    return Err(ItemRefusal::Source(
                        "ExpressionResponsibility selected member binding differs".into(),
                    ));
                }
                selected = Some(raw.to_vec());
                Ok(())
            },
        )
        .map_err(|_| {
            SourceCommandError::Conflict("ExpressionResponsibility selected member read")
        })?;
    selected.ok_or(SourceCommandError::Conflict(
        "ExpressionResponsibility selected member absent",
    ))
}

fn selected_optional_member(
    input: &dyn SourceCutInput,
    path: &str,
    max_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<Vec<u8>>> {
    if input
        .path_presence(path, deadline, cancelled)
        .map_err(|_| {
            SourceCommandError::Conflict("ExpressionResponsibility selected member presence")
        })?
        .is_some()
    {
        Ok(Some(read_selected_source_member(
            input, path, max_bytes, deadline, cancelled,
        )?))
    } else {
        Ok(None)
    }
}

fn verify_empty_prefix(
    input: &dyn SourceCutInput,
    prefix: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let mut found = None;
    input
        .for_each_current_member_meta_under(prefix, deadline, cancelled, &mut |meta| {
            found = Some(meta.path.to_owned());
            Err(ItemRefusal::Source(
                "ExpressionResponsibility new output home occupied".into(),
            ))
        })
        .map_err(|_| {
            if found.is_some() {
                SourceCommandError::Conflict("ExpressionResponsibility new Claim home is not empty")
            } else {
                SourceCommandError::Conflict(
                    "ExpressionResponsibility new Claim home enumeration incomplete",
                )
            }
        })?;
    Ok(())
}

fn append_archive_package(
    input: &dyn SourceCutInput,
    plan: &mut work_transaction::WorkPlan,
    archive: &work_transaction::WorkArchivePackage,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let expected_paths = archive
        .members
        .keys()
        .map(|name| format!("{}/{name}", archive.path))
        .collect::<BTreeSet<_>>();
    let mut selected_paths = BTreeSet::new();
    input
        .for_each_current_member_meta_under(
            &archive.path,
            limits.deadline,
            cancelled,
            &mut |meta| {
                selected_paths.insert(meta.path.to_owned());
                Ok(())
            },
        )
        .map_err(|_| {
            SourceCommandError::Conflict("ExpressionResponsibility archive prefix incomplete")
        })?;
    if expected_paths.is_empty() || !selected_paths.is_empty() && selected_paths != expected_paths {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility archive member set incomplete or unexpected",
        ));
    }
    for (name, expected) in &archive.members {
        let path = format!("{}/{name}", archive.path);
        if let Some(selected) =
            selected_optional_member(input, &path, 16_777_216, limits.deadline, cancelled)?
        {
            if &selected != expected {
                return Err(SourceCommandError::Conflict(
                    "ExpressionResponsibility retained archive bytes changed",
                ));
            }
        } else {
            plan.files.push(work_transaction::SelectedFile {
                path: relative(&path)?,
                before: None,
                after: Some(expected.clone()),
            });
        }
    }
    Ok(())
}

fn prefix_has_member(
    input: &dyn SourceCutInput,
    prefix: &str,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<bool> {
    let mut found = false;
    input
        .for_each_current_member_meta_under(prefix, limits.deadline, cancelled, &mut |_| {
            found = true;
            Ok(())
        })
        .map_err(|_| {
            SourceCommandError::Conflict("ExpressionResponsibility output prefix incomplete")
        })?;
    Ok(found)
}

fn include_missing_output_directories(
    input: &dyn SourceCutInput,
    plan: &mut work_transaction::WorkPlan,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let existing = plan
        .files
        .iter()
        .filter(|file| file.before.is_some())
        .map(|file| file.path.as_str().to_owned())
        .collect::<Vec<_>>();
    let mut directories = plan
        .new_directories
        .iter()
        .map(|path| path.as_str().to_owned())
        .collect::<BTreeSet<_>>();
    for path in plan
        .files
        .iter()
        .filter(|file| file.before.is_none())
        .map(|file| file.path.as_str().to_owned())
    {
        let mut parent = path
            .rsplit_once('/')
            .map(|(parent, _)| parent.to_owned())
            .ok_or(SourceCommandError::Invalid(
                "ExpressionResponsibility output parent",
            ))?;
        while parent.starts_with("ToS/source-witnesses/") {
            if existing
                .iter()
                .any(|member| member.starts_with(&format!("{parent}/")))
            {
                break;
            }
            if !prefix_has_member(input, &parent, limits, cancelled)? {
                directories.insert(parent.clone());
            } else {
                break;
            }
            if parent == "ToS/source-witnesses" {
                break;
            }
            parent = parent
                .rsplit_once('/')
                .map(|(parent, _)| parent.to_owned())
                .ok_or(SourceCommandError::Invalid(
                    "ExpressionResponsibility output ancestor",
                ))?;
        }
    }
    plan.new_directories = directories
        .into_iter()
        .map(|path| relative(&path))
        .collect::<SourceCommandResult<Vec<_>>>()?;
    Ok(())
}

fn verify_new_paths_absent(
    input: &dyn SourceCutInput,
    plan: &work_transaction::WorkPlan,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    for file in &plan.files {
        if file.before.is_some() {
            continue;
        }
        if input
            .path_presence(file.path.as_str(), deadline, cancelled)
            .map_err(|_| {
                SourceCommandError::Conflict("ExpressionResponsibility output presence read")
            })?
            .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "ExpressionResponsibility output path occupied",
            ));
        }
    }
    Ok(())
}

fn semantic_index_rows(
    outputs: &BTreeMap<String, Vec<u8>>,
    expression_path: &tos_foundation::RelativePath,
    expression_id: &str,
    claim_path: &tos_foundation::RelativePath,
    claim_id: &str,
    event_id: &str,
) -> SourceCommandResult<IndexRows> {
    let claim_home = claim_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid(
            "ExpressionResponsibility Claim home",
        ))?
        .0;
    let event_path = format!("{claim_home}/source-create-provenance.jsonl");
    let mut rows = BTreeSet::from([
        (
            "metadata".to_owned(),
            expression_id.to_owned(),
            expression_path.as_str().to_owned(),
        ),
        (
            "claim".to_owned(),
            claim_id.to_owned(),
            claim_path.as_str().to_owned(),
        ),
        ("event".to_owned(), event_id.to_owned(), event_path),
    ]);
    let claim_stream = outputs
        .get(claim_path.as_str())
        .ok_or(SourceCommandError::Conflict(
            "ExpressionResponsibility output Claim stream absent",
        ))?;
    let claims = catalog_lines(claim_stream)
        .filter(|line| !crate::source_claims::python_bytes_blank(line))
        .map(cmd::parse)
        .collect::<SourceCommandResult<Vec<_>>>()?;
    if claims.len() != 1
        || cmd::text(&claims[0], "claim_id")? != claim_id
        || cmd::text(&claims[0], "predicate")? != "translated_by"
        || cmd::text(&claims[0], "subject_ref")? != expression_id
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility output Claim identity differs",
        ));
    }
    for (path, raw) in outputs {
        if !path.ends_with(".human-forms.json") {
            continue;
        }
        let set = cmd::parse(raw)?;
        for section in ["forms", "prior_forms"] {
            for form in cmd::array(&set, section)? {
                rows.insert((
                    "form".to_owned(),
                    cmd::text(form, "form_id")?.to_owned(),
                    path.clone(),
                ));
            }
        }
    }
    Ok(rows)
}

fn owner_contributions_from_members<'a>(
    members: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    expression_path: &str,
    claim_path: &str,
    expression_id: &str,
    claim_id: &str,
    event_id: &str,
    expression_schema: &str,
    context_files: &[cmd::SourceFile],
) -> SourceCommandResult<BTreeMap<String, JsonValue>> {
    let event_path = format!(
        "{}/source-create-provenance.jsonl",
        claim_path
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid(
                "ExpressionResponsibility Claim contribution home",
            ))?
            .0
    );
    let source_profiles = inventory_source_profiles(context_files)?;
    let mut exact = BTreeMap::<String, Vec<u8>>::new();
    for (path, raw) in members {
        if exact.insert(path.to_owned(), raw.to_vec()).is_some() {
            return Err(SourceCommandError::Conflict(
                "ExpressionResponsibility duplicate contribution path",
            ));
        }
    }
    if !exact.contains_key(expression_path)
        || !exact.contains_key(claim_path)
        || !exact.contains_key(&event_path)
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility contribution owner writes absent",
        ));
    }
    let expression = cmd::parse(&exact[expression_path])?;
    if cmd::text(&expression, "record_type")? != "expression"
        || cmd::text(&expression, "record_id")? != expression_id
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility contribution record identity differs",
        ));
    }
    let catalog_entry = crate::source_claims::pg_catalogue_record_value(
        &expression,
        expression_path,
        Some(expression_schema),
    )?;
    let mut contribution_by_path = BTreeMap::new();
    for (path, raw) in &exact {
        if !path.starts_with("ToS/source-witnesses/") {
            return Err(SourceCommandError::Invalid(
                "ExpressionResponsibility contribution path outside source home",
            ));
        }
        let mut records = crate::source_claims::MIXED_INVENTORY_RECORD_KINDS
            .iter()
            .map(|kind| (JsonString::from_utf8(kind), JsonValue::Array(Vec::new())))
            .collect::<Vec<_>>();
        if path == expression_path {
            let (_, value) = records
                .iter_mut()
                .find(|(kind, _)| kind.as_str() == Some("expression"))
                .ok_or(SourceCommandError::Invalid(
                    "ExpressionResponsibility expression inventory kind",
                ))?;
            *value = JsonValue::Array(vec![catalog_entry.clone()]);
        }
        let events = if path == &event_path {
            event_projection(path, raw, event_id)?
        } else {
            empty_object()
        };
        let form = if path.ends_with(".human-forms.json") {
            form_projection(raw)?
        } else {
            JsonValue::Null
        };
        let claim_owner = if path == claim_path {
            expression_responsibility_claim_owner(&exact, claim_path, claim_id)?
        } else {
            JsonValue::Null
        };
        contribution_by_path.insert(
            path.clone(),
            cmd::object(vec![
                (
                    "schema_version",
                    cmd::string("tos_pg_source_inventory_member_contribution_v1"),
                ),
                ("records", JsonValue::Object(records)),
                ("source_profiles", source_profiles.clone()),
                ("events", events),
                ("anchors", empty_object()),
                ("form", form),
                ("claim_owner", claim_owner),
            ]),
        );
    }
    Ok(contribution_by_path)
}

fn inventory_source_profiles(context_files: &[cmd::SourceFile]) -> SourceCommandResult<JsonValue> {
    let mut profiles = Vec::new();
    let mut paths = BTreeSet::new();
    for file in context_files {
        let path = file.path.as_str();
        let selected = path.starts_with("ToS/contracts/") && path.ends_with(".schema.json")
            || matches!(
                path,
                "ToS/doctrine/semantic-interchange/entity-types.v1.json"
                    | "ToS/doctrine/semantic-interchange/relation-types.v1.json"
            );
        if selected {
            if !paths.insert(path.to_owned()) {
                return Err(SourceCommandError::Conflict(
                    "ExpressionResponsibility source profile repeats",
                ));
            }
            profiles.push((
                JsonString::from_utf8(path),
                cmd::string(&Digest256::of_bytes(&file.raw).to_hex()),
            ));
        }
    }
    if profiles.len() < CONTRACTS.len() {
        return Err(SourceCommandError::Unsupported(
            "ExpressionResponsibility source profile set incomplete",
        ));
    }
    profiles.sort_by(|(left, _), (right, _)| left.as_str().cmp(&right.as_str()));
    Ok(JsonValue::Object(profiles))
}

fn selected_source_profiles(context: &CommandContext) -> SourceCommandResult<JsonValue> {
    let entity = serde_value(&cmd::parse(
        context
            .file(&relative(CONTRACTS[0])?)?
            .ok_or(SourceCommandError::Unsupported(
                "ExpressionResponsibility entity registry absent",
            ))?,
    )?)?;
    let relation = serde_value(&cmd::parse(
        context
            .file(&relative(CONTRACTS[1])?)?
            .ok_or(SourceCommandError::Unsupported(
                "ExpressionResponsibility relation registry absent",
            ))?,
    )?)?;
    let corpus = serde_value(&cmd::parse(
        context
            .file(&relative(CONTRACTS[2])?)?
            .ok_or(SourceCommandError::Unsupported(
                "ExpressionResponsibility corpus schema absent",
            ))?,
    )?)?;
    let entity_types = entity["types"]
        .as_array()
        .ok_or(SourceCommandError::Invalid(
            "ExpressionResponsibility entity registry shape",
        ))?;
    let mut profiles = Vec::new();
    for kind in ["expression", "agent"] {
        let matching = entity_types
            .iter()
            .filter(|entry| {
                entry["source_mappings"].as_array().is_some_and(|rows| {
                    rows.iter().any(|row| {
                        row["source_graph"] == "source-claims" && row["source_kind_id"] == kind
                    })
                })
            })
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return Err(SourceCommandError::Unsupported(
                "ExpressionResponsibility descriptor mapping ambiguous",
            ));
        }
        let profile = serde_json::json!({
            "type_id": matching[0]["type_id"],
            "record_type": kind,
            "schema_ref": CORPUS_RECORD_SCHEMA,
            "schema_version": corpus["properties"]["schema_version"]["const"],
            "source_basename": format!("{kind}.json")
        });
        profiles.push((kind, foundation_value(&profile)?));
    }
    let relations = relation["relations"]
        .as_array()
        .ok_or(SourceCommandError::Invalid(
            "ExpressionResponsibility relation registry shape",
        ))?;
    let matching = relations
        .iter()
        .filter(|entry| {
            entry["source_mappings"].as_array().is_some_and(|rows| {
                rows.iter().any(|row| {
                    row["source_graph"] == "source-claims"
                        && row["source_predicate_id"] == "translated_by"
                        && row["scope"] == "claim-predicate"
                })
            })
        })
        .collect::<Vec<_>>();
    if matching.len() != 1 {
        return Err(SourceCommandError::Unsupported(
            "ExpressionResponsibility relation descriptor ambiguous",
        ));
    }
    let profile = &matching[0]["source_claim_profile"];
    let routes = profile["schemas"]
        .as_array()
        .ok_or(SourceCommandError::Invalid(
            "ExpressionResponsibility relation schema routes",
        ))?
        .iter()
        .filter(|route| route["schema_version"] == "tos_source_relation_claim_v1")
        .collect::<Vec<_>>();
    if routes.len() != 1 {
        return Err(SourceCommandError::Unsupported(
            "ExpressionResponsibility Claim descriptor ambiguous",
        ));
    }
    profiles.push((
        "translated_by",
        foundation_value(&serde_json::json!({
            "relation_type_id": matching[0]["relation_type_id"],
            "predicate": "translated_by",
            "reader": profile["reader"],
            "schema_ref": routes[0]["schema_ref"],
            "schema_version": routes[0]["schema_version"],
            "assertion_layers": profile["assertion_layers"],
            "source_basename": "source-claims.jsonl"
        }))?,
    ));
    Ok(cmd::object(profiles))
}

fn expression_responsibility_claim_owner(
    members: &BTreeMap<String, Vec<u8>>,
    source_path: &str,
    claim_id: &str,
) -> SourceCommandResult<JsonValue> {
    let home = source_path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or(SourceCommandError::Invalid(
            "ExpressionResponsibility Claim owner home",
        ))?;
    let stream = members
        .get(source_path)
        .ok_or(SourceCommandError::Conflict(
            "ExpressionResponsibility Claim owner stream absent",
        ))?;
    let claims = catalog_lines(stream)
        .filter(|line| !crate::source_claims::python_bytes_blank(line))
        .map(cmd::parse)
        .collect::<SourceCommandResult<Vec<_>>>()?;
    if claims.len() != 1 || cmd::text(&claims[0], "claim_id")? != claim_id {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility Claim owner stream differs",
        ));
    }
    let mut direct_members = Vec::new();
    let mut roles = BTreeMap::<&'static str, usize>::new();
    for (path, raw) in members {
        let Some((parent, basename)) = path.rsplit_once('/') else {
            continue;
        };
        if parent != home {
            continue;
        }
        let role = match basename {
            "source-claims.jsonl" => "claim-stream",
            "source-create-request.json" => "creation-request",
            "source-create-environment.json" => "creation-environment",
            "source-create-provenance.jsonl" => "creation-event",
            "responsibility-attachment-receipt.json" => "creation-receipt",
            name if name.starts_with("source-claims.") && name.ends_with(".human-forms.json") => {
                "human-form-set"
            }
            _ => {
                return Err(SourceCommandError::Conflict(
                    "ExpressionResponsibility Claim owner member role unsupported",
                ));
            }
        };
        *roles.entry(role).or_default() += 1;
        direct_members.push(cmd::object(vec![
            ("ref", cmd::string(path)),
            ("role", cmd::string(role)),
            ("bytes", cmd::number(raw.len() as u64)),
            (
                "sha256",
                cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
            ),
        ]));
    }
    direct_members.sort_by(|left, right| {
        cmd::text(left, "ref")
            .unwrap_or_default()
            .as_bytes()
            .cmp(cmd::text(right, "ref").unwrap_or_default().as_bytes())
    });
    for role in [
        "claim-stream",
        "creation-request",
        "creation-environment",
        "creation-event",
        "creation-receipt",
        "human-form-set",
    ] {
        if roles.get(role).copied() != Some(1) {
            return Err(SourceCommandError::Conflict(
                "ExpressionResponsibility Claim owner member set differs",
            ));
        }
    }
    if direct_members.len() != 6 {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility Claim owner member set incomplete",
        ));
    }
    let mut owner = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_managed_expression_responsibility_claim_owner_inventory_v1"),
        ),
        ("package_home", cmd::string(home)),
        ("source_path", cmd::string(source_path)),
        (
            "stream_sha256",
            cmd::string(&Digest256::of_bytes(stream).to_prefixed()),
        ),
        ("direct_members", JsonValue::Array(direct_members)),
        ("claims", JsonValue::Array(claims)),
    ]);
    let closure = cmd::record_digest(&owner)?.to_prefixed();
    cmd::set(&mut owner, "closure_sha256", cmd::string(&closure))?;
    Ok(owner)
}

fn event_projection(path: &str, raw: &[u8], expected_id: &str) -> SourceCommandResult<JsonValue> {
    let lines = catalog_lines(raw)
        .filter(|line| !crate::source_claims::python_bytes_blank(line))
        .collect::<Vec<_>>();
    if lines.len() != 1 {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility output event stream is not singular",
        ));
    }
    let event = cmd::parse(lines[0])?;
    let id = cmd::text(&event, "event_id")?;
    if id != expected_id {
        return Err(SourceCommandError::Conflict(
            "ExpressionResponsibility output event identity differs",
        ));
    }
    Ok(cmd::object(vec![(
        id,
        cmd::object(vec![
            ("payload", event.clone()),
            ("source_ref", cmd::string(path)),
            ("source_line", cmd::number(1)),
            (
                "source_sha256",
                cmd::string(&cmd::record_digest(&event)?.to_hex()),
            ),
        ]),
    )]))
}

fn form_projection(raw: &[u8]) -> SourceCommandResult<JsonValue> {
    let set = cmd::parse(raw)?;
    let ids = crate::source_forms::projection_form_ids(&set)?;
    Ok(cmd::object(vec![
        (
            "raw_sha256",
            cmd::string(&Digest256::of_bytes(raw).to_prefixed()),
        ),
        ("form_ids", JsonValue::Array(ids)),
    ]))
}

fn output_bytes(
    plan: &work_transaction::WorkPlan,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let mut outputs = BTreeMap::new();
    for file in &plan.files {
        if let Some(raw) = &file.after {
            outputs.insert(file.path.as_str().to_owned(), raw.clone());
        }
    }
    Ok(outputs)
}

fn empty_object() -> JsonValue {
    JsonValue::Object(Vec::new())
}

const CORPUS_RECORD_SCHEMA: &str = "ToS/contracts/corpus-record.schema.json";
