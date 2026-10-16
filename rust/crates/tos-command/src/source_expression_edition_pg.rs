//! Bounded PostgreSQL preparation for maintained Expression/Edition growth.
//!
//! The existing ExpressionEdition owner remains authoritative for grant
//! selection, the typed capture recipe, output bytes, and source package shape.
//! This adapter supplies its exact selected-input counterpart and leaves
//! registration and publication to the shared PG Growth writer.

use super::work_transaction::{self, WorkCorpusFence, WorkPlan};
use super::{
    ExpressionEditionCatalog, ExpressionEditionOwner, catalog_lines, dependency_bindings,
    finished_receipt, foundation_value, item_error, relative, serde_value, software_current,
};
use crate::source_cohort::{IndexRows, SourceRevisionWrite};
use crate::source_command::{self as cmd, CommandContext, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{CreationFilesystem, SelectedSourceWitnessCatalog};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue};
use tos_source_store::{SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::item_rules::{ItemLimits, ItemRefusal};
use tos_validation::record_biblio_cut::{SourceCutInput, SourceCutInputWithIdentity};
use tos_validation::source_cut::{CandidateCutWorkerSchemaExecutor, CutSchemaExecutor};

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
const CLAIM_OWNER_SCHEMA: &str = "tos_managed_expression_edition_claim_owner_inventory_v1";

/// Prepared Edition owner bytes and the physical corpus fence. Keep the fence
/// alive through the shared PG writer's registration/commit boundary.
pub(crate) struct PgExpressionEditionOwnerPackage<'a> {
    pub(crate) context: CommandContext,
    pub(crate) proposal: cmd::PreparedCommand,
    pub(crate) semantic_indexes: IndexRows,
    pub(crate) owner_contributions: BTreeMap<String, JsonValue>,
    pub(crate) fence: WorkCorpusFence<'a>,
}

/// Prepare one exact selected Work/Expression/Edition operation. `catalog` is
/// the publication-bound catalog selected by `PgExpressionEditionInputs`;
/// the PG caller binds the returned context to its final readset afterwards.
pub(crate) fn prepare_pg_expression_edition<'a, I: Copy + Eq>(
    fs: &'a CreationFilesystem,
    context: &CommandContext,
    input: &dyn SourceCutInputWithIdentity<I>,
    catalog: &SelectedSourceWitnessCatalog,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PgExpressionEditionOwnerPackage<'a>> {
    context.check_selected_software_inputs(software, components, limits.deadline, cancelled)?;
    verify_selected_contracts(context, input.source_input(), limits, cancelled)?;

    let fence = work_transaction::WorkCorpusFence::hold_existing(fs, limits.deadline, cancelled)?;
    let owner = ExpressionEditionOwner::select(fs, context, false, limits.deadline, cancelled)?;
    let snapshot = work_transaction::PublicationSnapshot::select_from_input(
        input.source_input(),
        limits.deadline,
        cancelled,
    )?;
    let expected_publication = cmd::field(&owner.request, "expected_publication")?;
    if expected_publication.as_str() != snapshot.token.as_deref()
        && !(expected_publication == &JsonValue::Null && snapshot.token.is_none())
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition requested publication snapshot changed",
        ));
    }
    snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled)?;
    let prior_id = selected_prior_publication(input, &snapshot, limits, cancelled)?;
    if let Some(id) = prior_id.as_deref() {
        selected_prior_completion(
            input.source_input(),
            &snapshot,
            id,
            snapshot.token.as_deref(),
            limits,
            cancelled,
        )?;
    }

    let before =
        selected_before_from_input(&owner, input.source_input(), &snapshot, limits, cancelled)?;
    let source_catalog = current_catalog_from_selected_input(
        fs,
        &owner,
        &before,
        input,
        catalog,
        worker,
        limits,
        || snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled),
        cancelled,
    )?;
    let profiles = expression_edition_source_profiles(context, input, worker, limits, cancelled)?;
    let directories = owner.new_directories(fs, limits.deadline, cancelled)?;
    let scope = serde_value(&owner.scope()?)?;
    let mut request_raw = cmd::canonical(&owner.request)?;
    request_raw.push(b'\n');
    let mut authorization_error = None;
    let core = tos_validation::native_compound::prepare_selected_expression_edition_bytes(
        input,
        worker,
        &scope,
        &request_raw,
        &before,
        &context.recorded_at,
        remaining(&source_catalog, limits)?,
        cancelled,
        |grammar| {
            let result = (|| {
                let dependencies = dependency_bindings(&source_catalog, grammar, context)?;
                if cmd::record_digest(&dependencies)?.to_prefixed()
                    != cmd::text(&owner.request, "expected_dependencies")?
                {
                    return Err(SourceCommandError::Conflict(
                        "ExpressionEdition current dependency closure changed",
                    ));
                }
                serde_value(&owner.authorization(dependencies)?)
            })();
            result.map_err(|error| {
                authorization_error = Some(error);
                ItemRefusal::Source("ExpressionEdition owner authorization rejected".to_owned())
            })
        },
    );
    let core = match core {
        Ok(core) => core,
        Err(error) => return Err(authorization_error.unwrap_or_else(|| item_error(error))),
    };
    if core.transaction_id() != owner.transaction_id()? {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition transaction identity differs from request",
        ));
    }
    let authorization = foundation_value(core.authorization())?;
    let archive_path = core.archive_path().to_owned();
    let outputs = core.outputs().map_err(item_error)?;
    let borrowed = outputs
        .iter()
        .map(|(path, raw)| (path.as_str(), *raw))
        .collect::<Vec<_>>();
    let capture = crate::source_serialization::capture_expression_edition(
        &owner.request,
        cmd::text(&owner.configuration, "provenance_event_id")?,
        owner
            .claim_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("ExpressionEdition Claim home"))?
            .0,
        &archive_path,
        &before,
        &borrowed,
        software,
        components,
        limits.deadline,
        cancelled,
    )?;
    let finished = tos_validation::native_compound::finish_selected_expression_edition_bytes(
        core,
        &capture.environment_raw,
        &capture.event_raw,
        worker,
    )
    .map_err(item_error)?;
    if finished.transaction_id != owner.transaction_id()? || finished.archive_path != archive_path {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition finished byte recipe differs from selected transaction",
        ));
    }
    let receipt = finished_receipt(&finished.child)?;
    let mut plan = owner.plan(
        authorization,
        &before,
        finished.parent,
        finished.child,
        directories,
    )?;
    let old = cmd::parse(
        before
            .get("expression.json")
            .ok_or(SourceCommandError::Conflict(
                "ExpressionEdition parent absent",
            ))?,
    )?;
    let archive = work_transaction::expression_archive_package(
        owner.expression_path.as_str(),
        &old,
        &before,
        cmd::text(&owner.request, "expected_revision")?,
    )?;
    if archive.path != archive_path {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition retained archive locator differs from recipe",
        ));
    }
    append_archive_package(input.source_input(), &mut plan, &archive, limits, cancelled)?;
    let commit_members = work_transaction::pg_selected_commit_members(
        input.source_input(),
        &snapshot,
        &plan,
        limits.deadline,
        cancelled,
    )?;
    for (path, after) in commit_members {
        let before =
            selected_optional_member(input.source_input(), &path, 16_777_216, limits, cancelled)?;
        if path != "ToS/source-witnesses/.metadata-publication.json" && before.is_some() {
            return Err(SourceCommandError::Conflict(
                "ExpressionEdition PG transaction journal path already occupied",
            ));
        }
        plan.files.push(work_transaction::SelectedFile {
            path: relative(&path)?,
            before,
            after: Some(after),
        });
    }
    include_missing_output_directories(input.source_input(), &mut plan, limits, cancelled)?;
    verify_new_paths_absent(input.source_input(), &plan, limits, cancelled)?;

    worker
        .finish(limits.deadline, cancelled)
        .map_err(item_error)?;
    software_current(fs, context, limits.deadline, cancelled)?;
    fs.current_context(context, limits.deadline, cancelled)?;
    snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled)?;
    fence.verify(limits.deadline, cancelled)?;

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
    let edition_raw = plan
        .files
        .iter()
        .find(|file| file.path == owner.edition_path)
        .and_then(|file| file.after.as_ref())
        .ok_or(SourceCommandError::Conflict(
            "ExpressionEdition output record absent",
        ))?;
    let edition = cmd::parse(edition_raw)?;
    let response = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_expression_edition_result_v1"),
        ),
        ("authentication", cmd::string("local-unix-account")),
        (
            "owner_configuration",
            cmd::string(&owner.configuration_digest),
        ),
        ("operation", cmd::string(super::OPERATION)),
        (
            "command_operations",
            JsonValue::Array(
                [
                    "describe",
                    "prepare-create",
                    super::OPERATION,
                    super::RECOVERY,
                ]
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
            "source",
            cmd::reference(&edition, "record_id", "record_version")?,
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
        ("scope", owner.scope()?),
        ("receipt", receipt),
        ("replayed", JsonValue::Bool(false)),
        ("recovery", JsonValue::Null),
        ("materializations", JsonValue::Null),
        ("source_profiles", profiles),
        (
            "source_fields",
            JsonValue::Array(
                crate::source_forms::metadata_fields(&edition)?
                    .iter()
                    .map(|field| field.public())
                    .collect(),
            ),
        ),
        ("grants_admission", JsonValue::Bool(false)),
    ]);
    let mut proposal_context = context.clone();
    for file in &plan.files {
        let Some(raw) = file.before.as_ref() else {
            continue;
        };
        if let Some(existing) = proposal_context
            .files
            .iter()
            .find(|existing| existing.path == file.path)
        {
            if existing.raw != *raw {
                return Err(SourceCommandError::Conflict(
                    "ExpressionEdition selected predecessor differs from context",
                ));
            }
        } else {
            proposal_context.files.push(cmd::SourceFile {
                path: file.path.clone(),
                raw: raw.clone(),
            });
        }
    }
    proposal_context
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    proposal_context.check()?;
    let proposal = proposal_context.plan("native-expression-edition", response, changes, false)?;
    let projected_writes = projected_writes(&plan)?;
    let semantic_indexes = expected_expression_edition_revision_indexes(&projected_writes)
        .map_err(|_| SourceCommandError::Conflict("ExpressionEdition semantic index projection"))?;
    let owner_contributions = expected_expression_edition_inventory_contributions(
        &projected_writes,
        &proposal_context,
        &proposal.response,
    )
    .map_err(|_| SourceCommandError::Conflict("ExpressionEdition inventory projection"))?;
    software_current(fs, &proposal_context, limits.deadline, cancelled)?;
    fs.current_context(&proposal_context, limits.deadline, cancelled)?;
    Ok(PgExpressionEditionOwnerPackage {
        context: proposal_context,
        proposal,
        semantic_indexes,
        owner_contributions,
        fence,
    })
}

/// Prepare the existing descriptive Expression/Edition proposal against an
/// exact PG-selected source input. This path returns projected owner bytes
/// only; it creates no work plan, journal, directory, or write intent.
pub(crate) fn prepare_pg_expression_edition_preview<I: Copy + Eq>(
    fs: &CreationFilesystem,
    context: &CommandContext,
    input: &dyn SourceCutInputWithIdentity<I>,
    catalog: &SelectedSourceWitnessCatalog,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<super::ExpressionEditionPreparation> {
    context.check_selected_software_inputs(software, components, limits.deadline, cancelled)?;
    verify_selected_contracts(context, input.source_input(), limits, cancelled)?;
    let snapshot = work_transaction::PublicationSnapshot::select_from_input(
        input.source_input(),
        limits.deadline,
        cancelled,
    )?;
    selected_prior_publication(input, &snapshot, limits, cancelled)?
        .map(|id| {
            selected_prior_completion(
                input.source_input(),
                &snapshot,
                &id,
                snapshot.token.as_deref(),
                limits,
                cancelled,
            )
        })
        .transpose()?;
    let mut owner = ExpressionEditionOwner::select(fs, context, true, limits.deadline, cancelled)?;
    let claim_home = owner
        .edition_path
        .as_str()
        .rsplit_once('/')
        .map(|(home, _)| home.to_owned())
        .ok_or(SourceCommandError::Invalid(
            "ExpressionEdition preview Claim home",
        ))?;
    verify_empty_prefix(
        input.source_input(),
        &claim_home,
        limits.deadline,
        cancelled,
    )?;
    let before =
        selected_before_from_input(&owner, input.source_input(), &snapshot, limits, cancelled)?;
    let source_catalog = current_catalog_from_selected_input(
        fs,
        &owner,
        &before,
        input,
        catalog,
        worker,
        limits,
        || snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled),
        cancelled,
    )?;
    let expression = cmd::parse(before.get("expression.json").ok_or(
        SourceCommandError::Conflict("ExpressionEdition preview selected record absent"),
    )?)?;
    let mut refs = cmd::array(&expression, "embodiment_claim_refs")?.to_vec();
    if refs.len() >= 128
        || refs
            .iter()
            .any(|value| value.as_str() == Some(owner.claim_id.as_str()))
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition preview Claim append invalid",
        ));
    }
    refs.push(cmd::string(&owner.claim_id));
    let mut claim_raw = cmd::canonical(cmd::field(&owner.request, "claim")?)?;
    claim_raw.push(b'\n');
    let scope = serde_value(&owner.scope()?)?;
    let mut prepared_request = None;
    let mut authorization_error = None;
    let core = tos_validation::native_compound::prepare_selected_expression_edition_preview_bytes(
        input,
        worker,
        &scope,
        &claim_raw,
        &before,
        &context.recorded_at,
        remaining(&source_catalog, limits)?,
        cancelled,
        |grammar| {
            let result = (|| {
                let dependencies = dependency_bindings(&source_catalog, grammar, context)?;
                let mut request = owner.request.clone();
                cmd::set(&mut request, "operation", cmd::string("prepare-create"))?;
                cmd::set(
                    &mut request,
                    "command_id",
                    cmd::string("preview:uncommitted"),
                )?;
                cmd::set(
                    &mut request,
                    "fields",
                    cmd::object(vec![("embodiment_claim_refs", JsonValue::Array(refs))]),
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
                        .as_deref()
                        .map(cmd::string)
                        .unwrap_or(JsonValue::Null),
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
                ItemRefusal::Source("ExpressionEdition preview authorization rejected".to_owned())
            })
        },
    );
    let core = match core {
        Ok(core) => core,
        Err(error) => return Err(authorization_error.unwrap_or_else(|| item_error(error))),
    };
    let request = prepared_request.ok_or(SourceCommandError::Invalid(
        "ExpressionEdition preview full request absent",
    ))?;
    let outputs = core.outputs().map_err(item_error)?;
    for (path, _) in &outputs {
        if path.starts_with(&format!("{claim_home}/"))
            && input
                .source_input()
                .path_presence(path, limits.deadline, cancelled)
                .map_err(|_| {
                    SourceCommandError::Conflict("ExpressionEdition preview output presence")
                })?
                .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "ExpressionEdition preview output path is occupied",
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
    Ok(super::ExpressionEditionPreparation {
        request,
        projected_outputs,
    })
}

/// Build the current ExpressionEdition result fields from one selected PG
/// source epoch. This is the `describe` surface: it reads the configured
/// Expression and its history, but does not prepare Claims or materialize a
/// Work/Claim proposal.
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
    context.check_selected_software_inputs(software, components, limits.deadline, cancelled)?;
    let request = cmd::parse(&context.request_raw)?;
    cmd::exact_keys(&request, &["schema_version", "operation"])?;
    if cmd::text(&request, "schema_version")? != super::REQUEST
        || cmd::text(&request, "operation")? != "describe"
    {
        return Err(SourceCommandError::Invalid(
            "ExpressionEdition selected describe request",
        ));
    }
    verify_selected_contracts(context, input.source_input(), limits, cancelled)?;
    let grant =
        super::ExpressionEditionGrant::select(fs, context, None, limits.deadline, cancelled)?;
    let config = grant.configuration;
    let path = grant.expression_path.as_str();
    let expression_id = cmd::text(&config, "expression_id")?;
    let mut matching = catalog.entries.iter().filter(|(kind, entry)| {
        kind == "expression"
            && entry.object_get("record_id").and_then(JsonValue::as_str) == Some(expression_id)
    });
    let entry = matching
        .next()
        .map(|(_, entry)| entry)
        .ok_or(SourceCommandError::Conflict(
            "PG ExpressionEdition current Expression is absent from catalog",
        ))?;
    if matching.next().is_some() || cmd::text(entry, "source_record_ref")? != path {
        return Err(SourceCommandError::Conflict(
            "PG ExpressionEdition current Expression endpoint differs",
        ));
    }
    let work_id = cmd::text(&config, "work_id")?;
    let work_path = cmd::text(&config, "work_source_path")?;
    let mut matching_work = catalog.entries.iter().filter(|(kind, entry)| {
        kind == "work" && entry.object_get("record_id").and_then(JsonValue::as_str) == Some(work_id)
    });
    let work_entry =
        matching_work
            .next()
            .map(|(_, entry)| entry)
            .ok_or(SourceCommandError::Conflict(
                "PG ExpressionEdition current Work is absent from catalog",
            ))?;
    if matching_work.next().is_some() || cmd::text(work_entry, "source_record_ref")? != work_path {
        return Err(SourceCommandError::Conflict(
            "PG ExpressionEdition current Work endpoint differs",
        ));
    }
    let snapshot = work_transaction::PublicationSnapshot::select_from_input(
        input.source_input(),
        limits.deadline,
        cancelled,
    )?;
    if let Some(id) = selected_prior_publication(input, &snapshot, limits, cancelled)? {
        selected_prior_completion(
            input.source_input(),
            &snapshot,
            &id,
            snapshot.token.as_deref(),
            limits,
            cancelled,
        )?;
    }
    let expression_raw = read_selected(input.source_input(), path, 2_097_152, limits, cancelled)?;
    let expression = cmd::parse(&expression_raw)?;
    verify_record_entry(entry, &expression, path)?;
    if cmd::text(&expression, "work_ref")? != work_id {
        return Err(SourceCommandError::Conflict(
            "PG ExpressionEdition current Expression has another Work parent",
        ));
    }
    let work_raw = read_selected(
        input.source_input(),
        work_path,
        2_097_152,
        limits,
        cancelled,
    )?;
    let work = cmd::parse(&work_raw)?;
    verify_record_entry(work_entry, &work, work_path)?;
    let home = path
        .rsplit_once('/')
        .map(|(home, _)| home)
        .ok_or(SourceCommandError::Invalid(
            "PG ExpressionEdition current Expression home",
        ))?;
    let mut package = BTreeMap::from([("expression.json".to_owned(), expression_raw.clone())]);
    for name in [
        "expression.human-forms.json",
        "source-revision-history.json",
    ] {
        let member_path = format!("{home}/{name}");
        if let Some(raw) = selected_optional_member(
            input.source_input(),
            &member_path,
            2_097_152,
            limits,
            cancelled,
        )? {
            package.insert(name.to_owned(), raw);
        }
    }
    tos_validation::native_compound::inspect_record_history(
        &package,
        &expression_raw,
        limits.deadline,
        cancelled,
    )
    .map_err(item_error)?;
    let profiles = expression_edition_source_profiles(context, input, worker, limits, cancelled)?;
    let scope = JsonValue::Object(
        config
            .as_object()
            .ok_or(SourceCommandError::Invalid(
                "PG ExpressionEdition current owner grant",
            ))?
            .iter()
            .filter(|(key, _)| {
                key.as_str()
                    .is_some_and(|key| super::SCOPE_KEYS.contains(&key))
            })
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
        ("scope", scope),
        ("materializations", JsonValue::Null),
        ("source_profiles", profiles),
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
    worker
        .finish(limits.deadline, cancelled)
        .map_err(item_error)?;
    software_current(fs, context, limits.deadline, cancelled)?;
    fs.current_context(context, limits.deadline, cancelled)?;
    snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled)?;
    Ok(result)
}

fn verify_selected_contracts(
    context: &CommandContext,
    input: &dyn SourceCutInput,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    for path in CONTRACTS {
        let expected = context
            .file(&relative(path)?)?
            .ok_or(SourceCommandError::Unsupported(
                "selected ExpressionEdition schema/registry set incomplete",
            ))?;
        let selected = read_selected(input, path, 2_097_152, limits, cancelled)?;
        if selected.as_slice() != expected {
            return Err(SourceCommandError::Conflict(
                "ExpressionEdition schema differs from selected context",
            ));
        }
    }
    Ok(())
}

/// Compare the exact PG context and absence predicates with the protected
/// physical source tree while the package's corpus fence is held.
pub(crate) fn verify_pg_expression_edition_context(
    fs: &CreationFilesystem,
    context: &CommandContext,
    absences: &BTreeSet<String>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    fs.current_context(context, deadline, cancelled)?;
    software_current(fs, context, deadline, cancelled)?;
    for path in absences {
        let relative_path = relative(path)?;
        if context.file(&relative_path)?.is_some() {
            return Err(SourceCommandError::Conflict(
                "ExpressionEdition path is both selected and absent",
            ));
        }
        let (parent_path, name) = path.rsplit_once('/').ok_or(SourceCommandError::Invalid(
            "ExpressionEdition exact absence path",
        ))?;
        let Some(parent) = work_transaction::read_existing_parent(fs, parent_path)? else {
            continue;
        };
        if work_transaction::read_at(&parent, name, fs.uid, 16_777_216, deadline, cancelled)?
            .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "ExpressionEdition exact path absence differs from protected corpus",
            ));
        }
    }
    Ok(())
}

fn remaining(
    catalog: &ExpressionEditionCatalog,
    mut limits: ItemLimits,
) -> SourceCommandResult<ItemLimits> {
    limits.max_total_bytes = limits
        .max_total_bytes
        .checked_sub(catalog.native_bytes)
        .and_then(|n| n.checked_sub(catalog.source_bytes))
        .ok_or(SourceCommandError::Invalid(
            "ExpressionEdition whole read budget",
        ))?;
    limits.max_state_bytes = limits
        .max_state_bytes
        .checked_sub(catalog.native_state)
        .ok_or(SourceCommandError::Invalid(
            "ExpressionEdition whole state budget",
        ))?;
    Ok(limits)
}

fn selected_before_from_input(
    owner: &ExpressionEditionOwner,
    input: &dyn SourceCutInput,
    snapshot: &work_transaction::PublicationSnapshot,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let parent = owner
        .expression_path
        .as_str()
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or(SourceCommandError::Invalid("ExpressionEdition parent"))?;
    let mut before = BTreeMap::new();
    let mut bytes = 0usize;
    for name in [
        "expression.json",
        "expression.human-forms.json",
        "source-revision-history.json",
    ] {
        let path = format!("{parent}/{name}");
        let value = selected_optional_member(input, &path, 2_097_152, limits, cancelled)?;
        if name == "expression.json" && value.is_none() {
            return Err(SourceCommandError::Conflict(
                "ExpressionEdition parent record absent",
            ));
        }
        if let Some(raw) = value {
            bytes = bytes
                .checked_add(raw.len())
                .filter(|size| *size <= 8_388_608)
                .ok_or(SourceCommandError::Invalid(
                    "ExpressionEdition parent byte budget",
                ))?;
            before.insert(name.to_owned(), raw);
        }
    }
    snapshot.verify_from_input(input, limits.deadline, cancelled)?;
    Ok(before)
}

fn current_catalog_from_selected_input<I: Copy + Eq>(
    fs: &CreationFilesystem,
    owner: &ExpressionEditionOwner,
    before: &BTreeMap<String, Vec<u8>>,
    input: &dyn SourceCutInputWithIdentity<I>,
    selected: &SelectedSourceWitnessCatalog,
    worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    limits: ItemLimits,
    check_publication: impl FnMut() -> SourceCommandResult<()>,
    cancelled: &AtomicBool,
) -> SourceCommandResult<ExpressionEditionCatalog> {
    let mut check_publication = check_publication;
    check_publication()?;
    let mut digests = selected.digests.clone();
    let mut total = selected.bytes_read;
    let expression_entry = selected
        .entries
        .iter()
        .find(|(kind, entry)| {
            kind == "expression"
                && entry.object_get("record_id").and_then(JsonValue::as_str)
                    == Some(owner.expression_id.as_str())
        })
        .map(|(_, entry)| entry)
        .ok_or(SourceCommandError::Conflict(
            "ExpressionEdition catalog parent absent",
        ))?;
    let work_entry = selected
        .entries
        .iter()
        .find(|(kind, entry)| {
            kind == "work"
                && entry.object_get("record_id").and_then(JsonValue::as_str)
                    == Some(owner.work_id.as_str())
        })
        .map(|(_, entry)| entry)
        .ok_or(SourceCommandError::Conflict(
            "ExpressionEdition Work absent",
        ))?;
    if cmd::text(expression_entry, "source_record_ref")? != owner.expression_path.as_str()
        || cmd::text(work_entry, "source_record_ref")? != owner.work_path.as_str()
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition exact catalog endpoint differs",
        ));
    }
    let expression_raw = before
        .get("expression.json")
        .ok_or(SourceCommandError::Conflict(
            "ExpressionEdition selected parent absent",
        ))?;
    add_dependency(
        &mut digests,
        &mut total,
        owner.expression_path.as_str(),
        expression_raw,
    )?;
    let expression = cmd::parse(expression_raw)?;
    if cmd::text(&expression, "record_type")? != "expression"
        || cmd::text(&expression, "record_id")? != owner.expression_id
        || cmd::text(&expression, "work_ref")? != owner.work_id
        || cmd::text(expression_entry, "record_sha256")?
            != cmd::record_digest(&expression)?.to_hex()
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition parent/Work binding stale",
        ));
    }
    let work_raw = read_selected(
        input.source_input(),
        owner.work_path.as_str(),
        2_097_152,
        limits,
        cancelled,
    )?;
    let work = cmd::parse(&work_raw)?;
    verify_record_entry(work_entry, &work, owner.work_path.as_str())?;
    add_dependency(
        &mut digests,
        &mut total,
        owner.work_path.as_str(),
        &work_raw,
    )?;

    let claims = selected
        .entries
        .iter()
        .filter(|(kind, entry)| {
            kind == "claim"
                && entry.object_get("predicate").and_then(JsonValue::as_str)
                    == Some("has_expression")
                && entry.object_get("object").and_then(JsonValue::as_str)
                    == Some(owner.expression_id.as_str())
        })
        .collect::<Vec<_>>();
    if claims.len() != 1 || cmd::text(&claims[0].1, "subject_ref")? != owner.work_id {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition unique Work origin absent",
        ));
    }
    let origin_id = cmd::text(&claims[0].1, "claim_id")?;
    if cmd::array(&work, "expression_claim_refs")?
        .iter()
        .filter(|value| value.as_str() == Some(origin_id))
        .count()
        != 1
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition Work origin reference differs",
        ));
    }
    let origin_path = cmd::text(&claims[0].1, "source_claim_file_ref")?;
    if !origin_path.ends_with("/source-claims.jsonl") {
        return Err(SourceCommandError::Unsupported(
            "legacy Work origin closure is not selectable for PG Edition growth",
        ));
    }
    let origin = read_selected_claim(
        input.source_input(),
        &claims[0].1,
        &mut digests,
        &mut total,
        limits,
        cancelled,
    )?;
    let origin_observation =
        tos_validation::native_compound::verify_work_expression_from_selected_input(
            input,
            worker,
            origin_path,
            &serde_value(&origin)?,
            remaining_source_limits(limits, total, 0, 0)?,
            cancelled,
        )
        .map_err(item_error)?;
    let (origin_manifest, origin_plan, _, _) = work_transaction::inspect_committed(
        fs,
        &origin_observation.transaction_id,
        limits.deadline,
        cancelled,
    )?;
    if origin_observation.transport
        != tos_validation::native_compound::NativeTransportState::Committed
        || origin_manifest != origin_observation.manifest_sha256
        || cmd::text(&origin_plan.authorization, "schema_version")?
            != "tos_work_expression_authorization_v1"
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition native Work origin is not committed",
        ));
    }

    let history = tos_validation::native_compound::inspect_record_history(
        before,
        expression_raw,
        limits.deadline,
        cancelled,
    )
    .map_err(item_error)?;
    let editions = selected
        .entries
        .iter()
        .filter(|(kind, entry)| {
            kind == "edition"
                && entry
                    .object_get("links")
                    .and_then(|links| links.object_get("embodies_expression_refs"))
                    .and_then(JsonValue::as_array)
                    .is_some_and(|refs| {
                        refs.iter()
                            .any(|value| value.as_str() == Some(owner.expression_id.as_str()))
                    })
        })
        .collect::<Vec<_>>();
    let edition_ids = editions
        .iter()
        .map(|(_, entry)| cmd::text(entry, "record_id").map(str::to_owned))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    let edition_claims = selected
        .entries
        .iter()
        .filter(|(kind, entry)| {
            kind == "claim"
                && entry.object_get("predicate").and_then(JsonValue::as_str) == Some("embodied_by")
                && entry.object_get("subject_ref").and_then(JsonValue::as_str)
                    == Some(owner.expression_id.as_str())
        })
        .collect::<Vec<_>>();
    let declared = cmd::array(&expression, "embodiment_claim_refs")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or(SourceCommandError::Invalid(
                    "ExpressionEdition embodiment reference",
                ))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let declared_ids = declared.iter().cloned().collect::<BTreeSet<_>>();
    let claim_ids = edition_claims
        .iter()
        .map(|(_, entry)| cmd::text(entry, "claim_id").map(str::to_owned))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    let targets = edition_claims
        .iter()
        .map(|(_, entry)| cmd::text(entry, "object").map(str::to_owned))
        .collect::<SourceCommandResult<BTreeSet<_>>>()?;
    if declared.len() != declared_ids.len()
        || declared_ids != claim_ids
        || edition_ids != targets
        || declared.len() != edition_claims.len()
        || edition_ids.len() != editions.len()
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition complete forward/backlink closure differs",
        ));
    }
    let mut retained_transactions = BTreeMap::new();
    let mut native_reads = origin_observation.reads.clone();
    let mut native_bytes = origin_observation.bytes_read;
    let mut native_state = origin_observation.returned_state_bytes;
    retain_transaction(
        &mut retained_transactions,
        &origin_observation.transaction_id,
        &origin_manifest,
    )?;
    for (_, entry) in &editions {
        let edition_path = cmd::text(entry, "source_record_ref")?;
        let edition_id = cmd::text(entry, "record_id")?;
        let mut matching_claims = edition_claims
            .iter()
            .filter(|candidate| {
                let claim = &candidate.1;
                claim.object_get("object").and_then(JsonValue::as_str) == Some(edition_id)
            })
            .copied();
        let claim_entry = matching_claims.next().ok_or(SourceCommandError::Conflict(
            "ExpressionEdition retained Edition Claim absent",
        ))?;
        if matching_claims.next().is_some() {
            return Err(SourceCommandError::Conflict(
                "ExpressionEdition retained Edition Claim duplicated",
            ));
        }
        let raw = read_selected(
            input.source_input(),
            edition_path,
            2_097_152,
            limits,
            cancelled,
        )?;
        let edition = cmd::parse(&raw)?;
        verify_record_entry(entry, &edition, edition_path)?;
        add_dependency(&mut digests, &mut total, edition_path, &raw)?;
        if !cmd::array(&edition, "embodies_expression_refs")?
            .iter()
            .any(|value| value.as_str() == Some(owner.expression_id.as_str()))
        {
            return Err(SourceCommandError::Conflict(
                "ExpressionEdition backlink absent",
            ));
        }
        let claim_path = cmd::text(&claim_entry.1, "source_claim_file_ref")?;
        if !claim_path.ends_with("/source-claims.jsonl") {
            return Err(SourceCommandError::Unsupported(
                "legacy Edition claim closure is not selectable",
            ));
        }
        let claim = read_selected_claim(
            input.source_input(),
            &claim_entry.1,
            &mut digests,
            &mut total,
            limits,
            cancelled,
        )?;
        let remaining = remaining_source_limits(limits, total, native_bytes, native_state)?;
        let observed =
            tos_validation::native_compound::verify_expression_edition_from_selected_input(
                input,
                worker,
                claim_path,
                &serde_value(&claim)?,
                remaining,
                cancelled,
            )
            .map_err(item_error)?;
        let (manifest, plan, _, _) = work_transaction::inspect_committed(
            fs,
            &observed.transaction_id,
            limits.deadline,
            cancelled,
        )?;
        if observed.transport != tos_validation::native_compound::NativeTransportState::Committed
            || manifest != observed.manifest_sha256
            || cmd::text(&plan.authorization, "schema_version")? != super::AUTHORIZATION
            || cmd::text(cmd::field(&plan.authorization, "scope")?, "expression_id")?
                != owner.expression_id
            || !history_receipt_has_transaction(&history, &observed.transaction_id)
        {
            return Err(SourceCommandError::Conflict(
                "ExpressionEdition retained Claim lacks exact parent lineage",
            ));
        }
        retain_transaction(
            &mut retained_transactions,
            &observed.transaction_id,
            &manifest,
        )?;
        native_bytes =
            native_bytes
                .checked_add(observed.bytes_read)
                .ok_or(SourceCommandError::Invalid(
                    "ExpressionEdition native byte overflow",
                ))?;
        native_state = native_state
            .checked_add(observed.returned_state_bytes)
            .ok_or(SourceCommandError::Invalid(
                "ExpressionEdition native state overflow",
            ))?;
        native_reads.extend(observed.reads);
    }
    check_publication()?;
    Ok(ExpressionEditionCatalog {
        digests,
        retained_transactions,
        native_reads,
        native_bytes,
        native_state,
        source_bytes: total as u64,
    })
}

fn history_receipt_has_transaction(history: &serde_json::Value, transaction_id: &str) -> bool {
    history
        .get("receipts")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|rows| {
            rows.iter().any(|row| {
                row.get("publication")
                    .and_then(|value| value.get("transaction_id"))
                    .and_then(serde_json::Value::as_str)
                    == Some(transaction_id)
            })
        })
}

fn retain_transaction(
    transactions: &mut BTreeMap<String, String>,
    id: &str,
    manifest: &str,
) -> SourceCommandResult<()> {
    if transactions
        .insert(id.to_owned(), manifest.to_owned())
        .is_some_and(|old| old != manifest)
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition retained transaction changed",
        ));
    }
    Ok(())
}

fn remaining_source_limits(
    limits: ItemLimits,
    source_bytes: usize,
    native_bytes: u64,
    native_state: usize,
) -> SourceCommandResult<ItemLimits> {
    let mut remaining = limits;
    remaining.max_total_bytes = remaining
        .max_total_bytes
        .checked_sub(source_bytes as u64)
        .and_then(|bytes| bytes.checked_sub(native_bytes))
        .ok_or(SourceCommandError::Invalid(
            "ExpressionEdition selected source budget",
        ))?;
    remaining.max_state_bytes =
        remaining
            .max_state_bytes
            .checked_sub(native_state)
            .ok_or(SourceCommandError::Invalid(
                "ExpressionEdition selected state budget",
            ))?;
    Ok(remaining)
}

fn verify_record_entry(
    entry: &JsonValue,
    record: &JsonValue,
    path: &str,
) -> SourceCommandResult<()> {
    if cmd::text(entry, "source_record_ref")? != path
        || cmd::text(entry, "record_type")? != cmd::text(record, "record_type")?
        || cmd::text(entry, "record_id")? != cmd::text(record, "record_id")?
        || cmd::text(entry, "record_sha256")? != cmd::record_digest(record)?.to_hex()
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition selected catalog record stale",
        ));
    }
    Ok(())
}

fn read_selected(
    input: &dyn SourceCutInput,
    path: &str,
    cap: usize,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<u8>> {
    let mut result = None;
    input
        .with_current_member(path, cap, limits.deadline, cancelled, &mut |meta, raw| {
            if meta.path != path || raw.len() as u64 != meta.size_bytes {
                return Err(ItemRefusal::Source(
                    "ExpressionEdition selected member metadata differs".into(),
                ));
            }
            result = Some(raw.to_vec());
            Ok(())
        })
        .map_err(|_| {
            SourceCommandError::Conflict("ExpressionEdition exact selected member read")
        })?;
    result.ok_or(SourceCommandError::Conflict(
        "ExpressionEdition selected member absent",
    ))
}

fn read_selected_claim(
    input: &dyn SourceCutInput,
    entry: &JsonValue,
    digests: &mut BTreeMap<String, String>,
    total: &mut usize,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    let path = cmd::text(entry, "source_claim_file_ref")?;
    let raw = read_selected(input, path, 16_777_216, limits, cancelled)?;
    add_dependency(digests, total, path, &raw)?;
    let line = usize::try_from(cmd::integer(entry, "source_claim_line")?)
        .map_err(|_| SourceCommandError::Invalid("ExpressionEdition Claim line"))?
        .checked_sub(1)
        .ok_or(SourceCommandError::Invalid("ExpressionEdition Claim line"))?;
    let claim = cmd::parse(
        tos_validation::native_compound::claim_lines(&raw)
            .nth(line)
            .ok_or(SourceCommandError::Conflict(
                "ExpressionEdition Claim line absent",
            ))?
            .0,
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
                "ExpressionEdition catalog Claim endpoint stale",
            ));
        }
    }
    if cmd::record_digest(&claim)?.to_hex() != cmd::text(entry, "claim_sha256")? {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition catalog Claim digest stale",
        ));
    }
    Ok(claim)
}

fn add_dependency(
    digests: &mut BTreeMap<String, String>,
    total: &mut usize,
    path: &str,
    raw: &[u8],
) -> SourceCommandResult<()> {
    let digest = Digest256::of_bytes(raw).to_hex();
    if let Some(old) = digests.get(path) {
        if old != &digest {
            return Err(SourceCommandError::Conflict(
                "ExpressionEdition selected dependency changed",
            ));
        }
        return Ok(());
    }
    digests.insert(path.to_owned(), digest);
    *total = total
        .checked_add(raw.len())
        .filter(|bytes| *bytes <= 16_777_216)
        .ok_or(SourceCommandError::Invalid(
            "ExpressionEdition selected source byte budget",
        ))?;
    Ok(())
}

fn expression_edition_source_profiles<I: Copy + Eq>(
    context: &CommandContext,
    input: &dyn SourceCutInputWithIdentity<I>,
    worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<JsonValue> {
    crate::source_claims::work_expression_source_descriptors_candidate(
        context, input, worker, limits, cancelled,
    )?;
    let entity_path = relative("ToS/doctrine/semantic-interchange/entity-types.v1.json")?;
    let relation_path = relative("ToS/doctrine/semantic-interchange/relation-types.v1.json")?;
    let corpus_path = relative("ToS/contracts/corpus-record.schema.json")?;
    let entities = cmd::parse(context.file(&entity_path)?.ok_or(
        SourceCommandError::Unsupported("ExpressionEdition entity registry absent"),
    )?)?;
    let relations = cmd::parse(context.file(&relation_path)?.ok_or(
        SourceCommandError::Unsupported("ExpressionEdition relation registry absent"),
    )?)?;
    let corpus = cmd::parse(context.file(&corpus_path)?.ok_or(
        SourceCommandError::Unsupported("ExpressionEdition corpus schema absent"),
    )?)?;
    let schema_version = corpus
        .object_get("properties")
        .and_then(|value| value.object_get("schema_version"))
        .and_then(|value| value.object_get("const"))
        .cloned()
        .ok_or(SourceCommandError::Invalid(
            "ExpressionEdition corpus schema version",
        ))?;
    let mut profiles = Vec::new();
    for kind in ["expression", "edition"] {
        let entity = cmd::array(&entities, "types")?
            .iter()
            .filter(|row| {
                row.object_get("source_mappings")
                    .and_then(JsonValue::as_array)
                    .is_some_and(|rows| {
                        rows.iter().any(|mapping| {
                            mapping
                                .object_get("source_graph")
                                .and_then(JsonValue::as_str)
                                == Some("source-claims")
                                && mapping
                                    .object_get("source_kind_id")
                                    .and_then(JsonValue::as_str)
                                    == Some(kind)
                        })
                    })
            })
            .collect::<Vec<_>>();
        if entity.len() != 1 {
            return Err(SourceCommandError::Unsupported(
                "ExpressionEdition entity profile mapping ambiguous",
            ));
        }
        profiles.push((
            kind,
            cmd::object(vec![
                ("type_id", cmd::field(entity[0], "type_id")?.clone()),
                ("record_type", cmd::string(kind)),
                (
                    "schema_ref",
                    cmd::string("ToS/contracts/corpus-record.schema.json"),
                ),
                ("schema_version", schema_version.clone()),
                ("source_basename", cmd::string(&format!("{kind}.json"))),
            ]),
        ));
    }
    let relation = cmd::array(&relations, "relations")?
        .iter()
        .filter(|row| {
            row.object_get("source_mappings")
                .and_then(JsonValue::as_array)
                .is_some_and(|rows| {
                    rows.iter().any(|mapping| {
                        mapping
                            .object_get("source_graph")
                            .and_then(JsonValue::as_str)
                            == Some("source-claims")
                            && mapping
                                .object_get("source_predicate_id")
                                .and_then(JsonValue::as_str)
                                == Some("embodied_by")
                            && mapping.object_get("scope").and_then(JsonValue::as_str)
                                == Some("claim-predicate")
                    })
                })
        })
        .collect::<Vec<_>>();
    if relation.len() != 1 {
        return Err(SourceCommandError::Unsupported(
            "ExpressionEdition source profile mapping ambiguous",
        ));
    }
    let profile =
        relation[0]
            .object_get("source_claim_profile")
            .ok_or(SourceCommandError::Invalid(
                "ExpressionEdition relation profile",
            ))?;
    let route = profile
        .object_get("schemas")
        .and_then(JsonValue::as_array)
        .ok_or(SourceCommandError::Invalid(
            "ExpressionEdition relation schema routes",
        ))?
        .iter()
        .filter(|row| {
            row.object_get("schema_version").and_then(JsonValue::as_str)
                == Some("tos_source_relation_claim_v1")
        })
        .collect::<Vec<_>>();
    if route.len() != 1 {
        return Err(SourceCommandError::Unsupported(
            "ExpressionEdition embodied_by schema route ambiguous",
        ));
    }
    profiles.push((
        "embodied_by",
        cmd::object(vec![
            (
                "relation_type_id",
                cmd::field(relation[0], "relation_type_id")?.clone(),
            ),
            ("predicate", cmd::string("embodied_by")),
            ("reader", cmd::field(profile, "reader")?.clone()),
            ("schema_ref", cmd::field(route[0], "schema_ref")?.clone()),
            (
                "schema_version",
                cmd::field(route[0], "schema_version")?.clone(),
            ),
            (
                "assertion_layers",
                cmd::field(profile, "assertion_layers")?.clone(),
            ),
            ("source_basename", cmd::string("source-claims.jsonl")),
        ]),
    ));
    Ok(cmd::object(profiles))
}

fn selected_prior_publication<I: Copy + Eq>(
    input: &dyn SourceCutInputWithIdentity<I>,
    snapshot: &work_transaction::PublicationSnapshot,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<String>> {
    let path = "ToS/source-witnesses/.metadata-publication.json";
    if input
        .path_presence(path, limits.deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("ExpressionEdition prior publication presence"))?
        .is_none()
    {
        if snapshot.token.is_some() {
            return Err(SourceCommandError::Conflict(
                "ExpressionEdition prior publication absent",
            ));
        }
        return Ok(None);
    }
    let raw = read_selected(input.source_input(), path, 8192, limits, cancelled)?;
    let state = cmd::parse(&raw)?;
    work_transaction::state(&state)?;
    if cmd::text(&state, "phase")? != "ready"
        || Some(cmd::text(&state, "token")?) != snapshot.token.as_deref()
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition prior publication differs from selected head",
        ));
    }
    Ok(Some(cmd::text(&state, "transaction_id")?.to_owned()))
}

fn selected_prior_completion(
    input: &dyn SourceCutInput,
    snapshot: &work_transaction::PublicationSnapshot,
    id: &str,
    expected_token: Option<&str>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let Some(token) = expected_token else {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition prior completion has no token",
        ));
    };
    let state = snapshot
        .selected_state()
        .ok_or(SourceCommandError::Conflict(
            "ExpressionEdition prior state absent",
        ))?;
    if snapshot.token.as_deref() != Some(token) || cmd::text(state, "transaction_id")? != id {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition prior state does not bind selected head",
        ));
    }
    let journal = format!("ToS/source-witnesses/.metadata-transactions/{}", &id[7..]);
    let manifest_raw = selected_optional_member(
        input,
        &format!("{journal}/manifest.json"),
        524_288,
        limits,
        cancelled,
    )?
    .ok_or(SourceCommandError::Conflict(
        "ExpressionEdition prior manifest absent",
    ))?;
    let manifest = cmd::parse(&manifest_raw)?;
    let schema = cmd::text(&manifest, "schema_version")?;
    if schema == work_transaction::PG_SOURCE_MANIFEST_SCHEMA {
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
                "ExpressionEdition prior PG parent binding profile differs",
            ));
        }
    } else if matches!(
        schema,
        "tos_selected_metadata_transaction_v1"
            | "tos_selected_metadata_transaction_v2"
            | "tos_selected_metadata_transaction_v3"
    ) {
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
    } else if schema == "tos_selected_metadata_transaction_v4" {
        cmd::exact_keys(
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
        )?;
    } else {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition prior manifest schema",
        ));
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
            "ExpressionEdition prior manifest differs",
        ));
    }
    let completion_raw = selected_optional_member(
        input,
        &format!("{journal}/completion.json"),
        8192,
        limits,
        cancelled,
    )?
    .ok_or(SourceCommandError::Conflict(
        "ExpressionEdition prior completion absent",
    ))?;
    let completion = cmd::parse(&completion_raw)?;
    cmd::exact_keys(&completion, &["schema_version", "publication"])?;
    if cmd::text(&completion, "schema_version")? != "tos_selected_metadata_completion_v1"
        || !cmd::same(cmd::field(&completion, "publication")?, state)?
    {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition prior completion differs from selected publication",
        ));
    }
    Ok(())
}

fn selected_optional_member(
    input: &dyn SourceCutInput,
    path: &str,
    cap: usize,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<Vec<u8>>> {
    if input
        .path_presence(path, limits.deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("ExpressionEdition selected member presence"))?
        .is_some()
    {
        Ok(Some(read_selected(input, path, cap, limits, cancelled)?))
    } else {
        Ok(None)
    }
}

fn append_archive_package(
    input: &dyn SourceCutInput,
    plan: &mut WorkPlan,
    archive: &work_transaction::WorkArchivePackage,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let expected = archive
        .members
        .keys()
        .map(|name| format!("{}/{name}", archive.path))
        .collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    input
        .for_each_current_member_meta_under(
            &archive.path,
            limits.deadline,
            cancelled,
            &mut |meta| {
                selected.insert(meta.path.to_owned());
                Ok(())
            },
        )
        .map_err(|_| {
            SourceCommandError::Conflict("ExpressionEdition archive prefix selection incomplete")
        })?;
    if expected.is_empty() || !selected.is_empty() && selected != expected {
        return Err(SourceCommandError::Conflict(
            "ExpressionEdition archive member set differs",
        ));
    }
    for (name, expected_raw) in &archive.members {
        let path = format!("{}/{name}", archive.path);
        let before = selected_optional_member(input, &path, 16_777_216, limits, cancelled)?;
        if let Some(raw) = &before {
            if raw != expected_raw {
                return Err(SourceCommandError::Conflict(
                    "ExpressionEdition archive bytes changed",
                ));
            }
        } else {
            plan.files.push(work_transaction::SelectedFile {
                path: relative(&path)?,
                before: None,
                after: Some(expected_raw.clone()),
            });
        }
    }
    Ok(())
}

fn include_missing_output_directories(
    input: &dyn SourceCutInput,
    plan: &mut WorkPlan,
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
        .map(|file| file.path.as_str())
    {
        let mut parent = path
            .rsplit_once('/')
            .map(|(parent, _)| parent.to_owned())
            .ok_or(SourceCommandError::Invalid(
                "ExpressionEdition output parent",
            ))?;
        while parent.starts_with("ToS/source-witnesses/") {
            if existing
                .iter()
                .any(|member| member.starts_with(&format!("{parent}/")))
            {
                break;
            }
            let mut any = false;
            input
                .for_each_current_member_meta_under(
                    &parent,
                    limits.deadline,
                    cancelled,
                    &mut |_| {
                        any = true;
                        Ok(())
                    },
                )
                .map_err(|_| {
                    SourceCommandError::Conflict(
                        "ExpressionEdition output parent enumeration incomplete",
                    )
                })?;
            if any {
                break;
            }
            directories.insert(parent.clone());
            if parent == "ToS/source-witnesses" {
                break;
            }
            parent = parent
                .rsplit_once('/')
                .map(|(parent, _)| parent.to_owned())
                .ok_or(SourceCommandError::Invalid(
                    "ExpressionEdition output ancestor",
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
    plan: &WorkPlan,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    for file in &plan.files {
        if file.before.is_none()
            && input
                .path_presence(file.path.as_str(), limits.deadline, cancelled)
                .map_err(|_| {
                    SourceCommandError::Conflict("ExpressionEdition output path presence")
                })?
                .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "ExpressionEdition output path occupied",
            ));
        }
    }
    Ok(())
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
                "ExpressionEdition PG output home is not empty".into(),
            ))
        })
        .map_err(|_| {
            if found.is_some() {
                SourceCommandError::Conflict("ExpressionEdition PG output home is not empty")
            } else {
                SourceCommandError::Conflict(
                    "ExpressionEdition PG output home enumeration incomplete",
                )
            }
        })?;
    Ok(())
}

fn projected_writes(plan: &WorkPlan) -> SourceCommandResult<Vec<SourceRevisionWrite>> {
    plan.files
        .iter()
        .filter_map(|file| {
            file.after.as_ref().map(|raw| {
                Ok(SourceRevisionWrite {
                    subject: file.path.as_str().to_owned(),
                    expected_predecessor: file
                        .before
                        .as_ref()
                        .map(|before| (1, Digest256::of_bytes(before))),
                    proposed_revision: 1,
                    exact_bytes: raw.clone(),
                })
            })
        })
        .collect()
}

/// Re-derive semantic index rows from the exact sealed PG writes. The parent
/// writer calls this on its commit/recovery write set, never on the prep map.
pub(crate) fn expected_expression_edition_revision_indexes(
    writes: &[SourceRevisionWrite],
) -> crate::DurableResult<IndexRows> {
    use crate::DurableError;

    if writes.is_empty() || writes.len() > 512 {
        return Err(DurableError::Corrupt(
            "PG ExpressionEdition write count differs",
        ));
    }
    let mut outputs = BTreeMap::<String, Vec<u8>>::new();
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
        let path = tos_foundation::RelativePath::parse(&write.subject)
            .map_err(|_| DurableError::Corrupt("PG ExpressionEdition write path is invalid"))?;
        if path.as_str() != write.subject
            || !write.subject.starts_with("ToS/source-witnesses/")
            || outputs
                .insert(write.subject.clone(), write.exact_bytes.clone())
                .is_some()
        {
            return Err(DurableError::Corrupt(
                "PG ExpressionEdition writes duplicated or outside source homes",
            ));
        }
    }
    let editions = outputs
        .iter()
        .filter(|(path, _)| path.ends_with("/edition.json"))
        .collect::<Vec<_>>();
    let expressions = outputs
        .iter()
        .filter(|(path, _)| path.ends_with("/expression.json"))
        .collect::<Vec<_>>();
    if editions.len() != 1 || expressions.len() != 1 {
        return Err(DurableError::Corrupt(
            "PG ExpressionEdition metadata successor cardinality differs",
        ));
    }
    let (edition_path, edition_raw) = (&editions[0].0, &editions[0].1);
    let edition_home = edition_path
        .rsplit_once('/')
        .map(|(home, _)| home)
        .ok_or(DurableError::Corrupt("PG ExpressionEdition home absent"))?;
    let (expression_path, expression_raw) = (&expressions[0].0, &expressions[0].1);
    let expression_home = expression_path
        .rsplit_once('/')
        .map(|(home, _)| home)
        .ok_or(DurableError::Corrupt(
            "PG ExpressionEdition parent home absent",
        ))?;
    let edition = cmd::parse(edition_raw).map_err(DurableError::Source)?;
    let expression = cmd::parse(expression_raw).map_err(DurableError::Source)?;
    if cmd::text(&edition, "record_type").map_err(DurableError::Source)? != "edition"
        || cmd::text(&expression, "record_type").map_err(DurableError::Source)? != "expression"
        || !writes.iter().any(|write| {
            write.subject.as_str() == edition_path.as_str() && write.expected_predecessor.is_none()
        })
        || !writes.iter().any(|write| {
            write.subject.as_str() == expression_path.as_str()
                && write.expected_predecessor.is_some()
        })
    {
        return Err(DurableError::Corrupt(
            "PG ExpressionEdition metadata write classes differ",
        ));
    }
    let edition_id = cmd::text(&edition, "record_id")
        .map_err(DurableError::Source)?
        .to_owned();
    let expression_id = cmd::text(&expression, "record_id")
        .map_err(DurableError::Source)?
        .to_owned();
    let edition_expression_refs =
        cmd::array(&edition, "embodies_expression_refs").map_err(DurableError::Source)?;
    if edition_expression_refs.len() != 1
        || edition_expression_refs[0].as_str() != Some(expression_id.as_str())
    {
        return Err(DurableError::Corrupt(
            "PG ExpressionEdition endpoint differs",
        ));
    }
    let claim_path = format!("{edition_home}/source-claims.jsonl");
    let claim_write = writes
        .iter()
        .find(|write| write.subject == claim_path)
        .ok_or(DurableError::Corrupt(
            "PG ExpressionEdition Claim write absent",
        ))?;
    if claim_write.expected_predecessor.is_some() {
        return Err(DurableError::Corrupt(
            "PG ExpressionEdition Claim write is not new",
        ));
    }
    let claim_raw = outputs.get(&claim_path).ok_or(DurableError::Corrupt(
        "PG ExpressionEdition Claim stream absent",
    ))?;
    let claims = catalog_lines(claim_raw)
        .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
        .map(|line| cmd::parse(line).map_err(DurableError::Source))
        .collect::<crate::DurableResult<Vec<_>>>()?;
    if claims.len() != 1 {
        return Err(DurableError::Corrupt(
            "PG ExpressionEdition Claim stream cardinality differs",
        ));
    }
    let claim = &claims[0];
    let claim_id = cmd::text(claim, "claim_id")
        .map_err(DurableError::Source)?
        .to_owned();
    let event_path = format!("{edition_home}/source-create-provenance.jsonl");
    let event_raw = outputs.get(&event_path).ok_or(DurableError::Corrupt(
        "PG ExpressionEdition event stream absent",
    ))?;
    let events = catalog_lines(event_raw)
        .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
        .map(|line| cmd::parse(line).map_err(DurableError::Source))
        .collect::<crate::DurableResult<Vec<_>>>()?;
    if events.len() != 1 {
        return Err(DurableError::Corrupt(
            "PG ExpressionEdition event cardinality differs",
        ));
    }
    let event_id = cmd::text(&events[0], "event_id")
        .map_err(DurableError::Source)?
        .to_owned();
    if cmd::text(claim, "claim_type").map_err(DurableError::Source)? != "relation"
        || cmd::text(claim, "predicate").map_err(DurableError::Source)? != "embodied_by"
        || cmd::text(claim, "subject_ref").map_err(DurableError::Source)? != expression_id
        || cmd::text(claim, "object").map_err(DurableError::Source)? != edition_id
        || cmd::text(claim, "provenance_event_ref").map_err(DurableError::Source)? != event_id
        || cmd::array(&expression, "embodiment_claim_refs")
            .map_err(DurableError::Source)?
            .iter()
            .filter(|value| value.as_str() == Some(claim_id.as_str()))
            .count()
            != 1
    {
        return Err(DurableError::Corrupt(
            "PG ExpressionEdition Claim binding differs",
        ));
    }
    let event_write = writes
        .iter()
        .find(|write| write.subject == event_path)
        .ok_or(DurableError::Corrupt(
            "PG ExpressionEdition event write absent",
        ))?;
    if event_write.expected_predecessor.is_some() {
        return Err(DurableError::Corrupt(
            "PG ExpressionEdition event write is not new",
        ));
    }
    indexes.insert(("metadata".to_owned(), edition_id, (*edition_path).clone()));
    indexes.insert(("claim".to_owned(), claim_id.clone(), claim_path));
    indexes.insert(("event".to_owned(), event_id, event_path));
    let claim_form_name = format!(
        "source-claims.{}.human-forms.json",
        Digest256::of_bytes(
            cmd::text(claim, "claim_id")
                .map_err(DurableError::Source)?
                .as_bytes(),
        )
        .to_hex()
    );
    let expression_forms = "expression.human-forms.json";
    for (home, name) in [
        (expression_home, expression_forms),
        (edition_home, "edition.human-forms.json"),
        (edition_home, claim_form_name.as_str()),
    ] {
        let path = format!("{home}/{name}");
        let write =
            writes
                .iter()
                .find(|write| write.subject == path)
                .ok_or(DurableError::Corrupt(
                    "PG ExpressionEdition form write absent",
                ))?;
        if home != expression_home && write.expected_predecessor.is_some() {
            return Err(DurableError::Corrupt(
                "PG ExpressionEdition form write class differs",
            ));
        }
        let raw = outputs.get(&path).ok_or(DurableError::Corrupt(
            "PG ExpressionEdition form set absent",
        ))?;
        let set = cmd::parse(raw).map_err(DurableError::Source)?;
        for section in ["forms", "prior_forms"] {
            for form in cmd::array(&set, section).map_err(DurableError::Source)? {
                indexes.insert((
                    "form".to_owned(),
                    cmd::text(form, "form_id")
                        .map_err(DurableError::Source)?
                        .to_owned(),
                    path.clone(),
                ));
            }
        }
    }
    indexes.insert((
        "metadata".to_owned(),
        expression_id,
        (*expression_path).clone(),
    ));
    Ok(indexes)
}

/// Re-derive mixed inventory contributions from sealed bytes for registration
/// and recovery. Owner output and per-write contribution receipt are never
/// trusted independently of the exact writes.
pub(crate) fn expected_expression_edition_inventory_contributions(
    writes: &[SourceRevisionWrite],
    context: &CommandContext,
    response: &JsonValue,
) -> crate::DurableResult<BTreeMap<String, JsonValue>> {
    expected_expression_edition_revision_indexes(writes)?;
    inventory_contributions_from_writes(writes, context, response)
}

fn inventory_contributions_from_writes(
    writes: &[SourceRevisionWrite],
    context: &CommandContext,
    response: &JsonValue,
) -> crate::DurableResult<BTreeMap<String, JsonValue>> {
    use crate::DurableError;
    let mut records = Vec::with_capacity(2);
    for write in writes {
        let kind = match write.subject.rsplit('/').next().unwrap_or(&write.subject) {
            "expression.json" => "expression",
            "edition.json" => "edition",
            _ => continue,
        };
        let record = cmd::parse(&write.exact_bytes).map_err(DurableError::Source)?;
        let profile = response
            .object_get("source_profiles")
            .and_then(|value| value.object_get(kind))
            .ok_or(DurableError::Corrupt(
                "PG ExpressionEdition source profile absent",
            ))?;
        records.push(crate::source_creation_store::PgGrowthRecordProjection {
            kind: kind.to_owned(),
            path: write.subject.clone(),
            record,
            schema_ref: Some(
                cmd::text(profile, "schema_ref")
                    .map_err(DurableError::Source)?
                    .to_owned(),
            ),
        });
    }
    if records.len() != 2
        || records
            .iter()
            .filter(|record| record.kind == "expression")
            .count()
            != 1
        || records
            .iter()
            .filter(|record| record.kind == "edition")
            .count()
            != 1
    {
        return Err(DurableError::Corrupt(
            "PG ExpressionEdition metadata projection coverage differs",
        ));
    }
    let home = records
        .iter()
        .find(|record| record.kind == "edition")
        .and_then(|record| record.path.rsplit_once('/').map(|(home, _)| home))
        .ok_or(DurableError::Corrupt("PG ExpressionEdition home absent"))?;
    let claim_path = format!("{home}/source-claims.jsonl");
    let event_path = format!("{home}/source-create-provenance.jsonl");
    crate::source_creation_store::pg_growth_owner_contributions(
        writes,
        &context.files,
        &records,
        &event_path,
        &claim_path,
        CLAIM_OWNER_SCHEMA,
    )
}
