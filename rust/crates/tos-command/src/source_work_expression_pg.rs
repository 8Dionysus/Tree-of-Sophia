//! PostgreSQL preparation for the maintained Work -> Expression owner.
//!
//! This uses the same typed owner grammar and capture recipe as the local
//! transaction path, but leaves the final source write to the fenced PG
//! revision writer.

use super::work_transaction::{self, WorkCorpusFence, WorkPlan};
use super::{
    WorkOwner, catalog_lines, current_catalog_from_selected_input, dependency_bindings,
    finished_receipt, foundation_value, item_error, read_selected_source_member, relative,
    serde_value, software_current, work_expression_materializations,
};
use crate::source_cohort::{IndexRows, SourceRevisionWrite};
use crate::source_command::{self as cmd, CommandContext, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::CreationFilesystem;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue};
use tos_source_store::{SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::item_rules::ItemLimits;
use tos_validation::record_biblio_cut::{SourceCutInput, SourceCutInputWithIdentity};
use tos_validation::source_cut::{CandidateCutWorkerSchemaExecutor, CutSchemaExecutor};

pub(crate) const PG_FAMILY_TAG: &str = "tos-pg-maintained-growth-v1:native-work-expression";

const WORK_CONTRACTS: &[&str] = &[
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

/// Prepared owner bytes and the protected local corpus fence. The caller must
/// keep this value alive through PG registration so the physical owner inputs
/// cannot move between preparation and the writer's atomic source commit.
pub(crate) struct PgWorkExpressionOwnerPackage<'a> {
    pub(crate) context: CommandContext,
    pub(crate) proposal: cmd::PreparedCommand,
    pub(crate) semantic_indexes: IndexRows,
    pub(crate) fence: WorkCorpusFence<'a>,
}

/// A selected-source Work preview. The fence remains held while the ordinary
/// CLI rechecks its invocation and returns the descriptive result.
pub(crate) struct PgWorkExpressionPreview<'a> {
    pub(crate) response: JsonValue,
    pub(crate) semantic_indexes: IndexRows,
    pub(crate) fence: WorkCorpusFence<'a>,
}

/// Prepare either the Work description or its non-mutating create preview from
/// the selected PG epoch. This deliberately uses the same catalog, retained
/// transaction, schema, Claim, and form kernels as the create path.
pub(crate) fn preview_pg_work_expression<'a, I: Copy + Eq>(
    fs: &'a CreationFilesystem,
    context: &CommandContext,
    input: &dyn SourceCutInputWithIdentity<I>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    operation: &str,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PgWorkExpressionPreview<'a>> {
    if !matches!(operation, "describe" | "prepare-create") {
        return Err(SourceCommandError::Unsupported("PG Work preview operation"));
    }
    context.check_selected_software_inputs(software, components, limits.deadline, cancelled)?;
    for path in WORK_CONTRACTS {
        if context.file(&relative(path)?)?.is_none() {
            return Err(SourceCommandError::Unsupported(
                "selected Work source registry/schema set incomplete",
            ));
        }
    }
    let fence = work_transaction::WorkCorpusFence::hold_existing(fs, limits.deadline, cancelled)?;
    let profiles = crate::source_claims::work_expression_source_descriptors_candidate(
        context, input, worker, limits, cancelled,
    )?;
    let mut owner = if operation == "prepare-create" {
        WorkOwner::select_proposal(fs, context, limits.deadline, cancelled)?
    } else {
        WorkOwner::select_configuration(fs, context, limits.deadline, cancelled)?
    };
    let snapshot = work_transaction::PublicationSnapshot::select_from_input(
        input.source_input(),
        limits.deadline,
        cancelled,
    )?;
    snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled)?;
    let prior_id = selected_prior_publication(input, &snapshot, limits.deadline, cancelled)?;
    let before = selected_before_from_input(
        &owner,
        input.source_input(),
        &snapshot,
        limits.deadline,
        cancelled,
    )?;
    let catalog = current_catalog_from_selected_input(
        fs,
        &owner,
        &before,
        input,
        worker,
        limits,
        snapshot.token.as_deref(),
        || snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled),
        limits.deadline,
        cancelled,
    )?;
    if let Some(prior_id) = prior_id.as_deref() {
        selected_prior_completion(
            input.source_input(),
            &snapshot,
            prior_id,
            snapshot.token.as_deref(),
            limits.deadline,
            cancelled,
        )?;
    }

    let work_raw = before
        .get("work.json")
        .ok_or(SourceCommandError::Conflict("Work preview source absent"))?;
    let work = cmd::parse(work_raw)?;
    tos_validation::native_compound::inspect_record_history(
        &before,
        work_raw,
        limits.deadline,
        cancelled,
    )
    .map_err(item_error)?;
    let response_value = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_work_expression_result_v1"),
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
        ("work_source_path", cmd::string(owner.work_path.as_str())),
        (
            "expression_source_path",
            cmd::string(owner.expression_path.as_str()),
        ),
        (
            "source",
            cmd::reference(&work, "record_id", "record_version")?,
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
        ("source_profiles", profiles),
        (
            "source_fields",
            JsonValue::Array(
                crate::source_forms::metadata_fields(&work)?
                    .iter()
                    .map(|field| field.public())
                    .collect(),
            ),
        ),
        ("scope", owner.scope()?),
        ("receipt", JsonValue::Null),
        ("replayed", JsonValue::Bool(false)),
        ("recovery", JsonValue::Null),
        ("materializations", JsonValue::Null),
        ("grants_admission", JsonValue::Bool(false)),
    ]);
    let mut response =
        serde_json::from_slice::<serde_json::Value>(&cmd::canonical(&response_value)?)
            .map_err(|_| SourceCommandError::Invalid("Work preview response JSON"))?;

    let mut semantic_indexes = IndexRows::new();
    if operation == "prepare-create" {
        let expression_home = owner
            .expression_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Work preview Expression home"))?
            .0;
        verify_empty_prefix(
            input.source_input(),
            expression_home,
            limits.deadline,
            cancelled,
        )?;
        let mut refs = cmd::array(&work, "expression_claim_refs")?.to_vec();
        if refs.len() >= 128
            || refs
                .iter()
                .any(|value| value.as_str() == Some(owner.claim_id.as_str()))
        {
            return Err(SourceCommandError::Conflict(
                "Work preview Claim append invalid",
            ));
        }
        refs.push(cmd::string(&owner.claim_id));
        let claim = cmd::field(&owner.request, "claim")?;
        let mut claim_raw = cmd::canonical(claim)?;
        claim_raw.push(b'\n');
        let scope = serde_value(&owner.scope()?)?;
        let mut prepared_request = None;
        let mut authorization_error = None;
        let core = tos_validation::native_compound::prepare_selected_work_expression_preview_bytes(
            input,
            worker,
            &scope,
            &claim_raw,
            &before,
            &context.recorded_at,
            limits,
            cancelled,
            |grammar| {
                let result = (|| {
                    let dependencies = dependency_bindings(&catalog, grammar, context)?;
                    let mut request = owner.request.clone();
                    cmd::set(&mut request, "operation", cmd::string(super::OPERATION))?;
                    cmd::set(
                        &mut request,
                        "command_id",
                        cmd::string("preview:uncommitted"),
                    )?;
                    cmd::set(
                        &mut request,
                        "fields",
                        cmd::object(vec![("expression_claim_refs", JsonValue::Array(refs))]),
                    )?;
                    cmd::set(
                        &mut request,
                        "expected_configuration",
                        cmd::string(&owner.configuration_digest),
                    )?;
                    cmd::set(
                        &mut request,
                        "expected_source",
                        cmd::reference(&work, "record_id", "record_version")?,
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
                            .map_or(JsonValue::Null, |token| cmd::string(token)),
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
                    tos_validation::item_rules::ItemRefusal::Source(
                        "Work preview authorization rejected".to_owned(),
                    )
                })
            },
        );
        let core = match core {
            Ok(core) => core,
            Err(error) => return Err(authorization_error.unwrap_or_else(|| item_error(error))),
        };
        let request =
            prepared_request.ok_or(SourceCommandError::Invalid("Work preview request absent"))?;
        let outputs = core.into_prepared_outputs().map_err(item_error)?;
        semantic_indexes = semantic_index_rows(
            &outputs,
            &owner.expression_path,
            &owner.expression_id,
            &owner.claim_id,
            &owner.event_id,
        )?;
        worker
            .finish(limits.deadline, cancelled)
            .map_err(item_error)?;
        fs.current_context(context, limits.deadline, cancelled)?;
        software_current(fs, context, limits.deadline, cancelled)?;
        snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled)?;
        fence.verify(limits.deadline, cancelled)?;

        let mut prepared_refs = BTreeMap::new();
        let mut prepared_forms = Vec::new();
        for (kind, path_key, form_field, id_key, version_key) in [
            (
                "work",
                "work_source_path",
                "forms",
                "record_id",
                "record_version",
            ),
            (
                "expression",
                "expression_source_path",
                "expression_forms",
                "record_id",
                "record_version",
            ),
            (
                "claim",
                "expression_source_path",
                "claim_forms",
                "claim_id",
                "claim_version",
            ),
        ] {
            let path = cmd::text(&owner.configuration, path_key)?;
            let home = path
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid("Work preview record home"))?
                .0;
            let record_path = if kind == "claim" {
                format!("{home}/source-claims.jsonl")
            } else {
                path.to_owned()
            };
            let record = cmd::parse(
                outputs
                    .get(&record_path)
                    .ok_or(SourceCommandError::Conflict("Work preview record absent"))?,
            )?;
            prepared_refs.insert(kind, cmd::reference(&record, id_key, version_key)?);
            let forms_name = if kind == "claim" {
                format!(
                    "source-claims.{}.human-forms.json",
                    Digest256::of_bytes(cmd::text(&owner.configuration, "claim_id")?.as_bytes())
                        .to_hex()
                )
            } else {
                format!("{kind}.human-forms.json")
            };
            let set = cmd::parse(
                outputs
                    .get(&format!("{home}/{forms_name}"))
                    .ok_or(SourceCommandError::Conflict("Work preview forms absent"))?,
            )?;
            let forms = cmd::array(&set, "forms")?;
            let refs = cmd::array(&request, form_field)?
                .iter()
                .map(|selection| {
                    let id = cmd::text(selection, "form_id")?;
                    let form = forms
                        .iter()
                        .find(|form| {
                            form.object_get("form_id").and_then(JsonValue::as_str) == Some(id)
                        })
                        .ok_or(SourceCommandError::Conflict(
                            "Work preview selected form absent",
                        ))?;
                    crate::source_forms::form_reference(form)
                })
                .collect::<SourceCommandResult<Vec<_>>>()?;
            prepared_forms.push((kind, JsonValue::Array(refs)));
        }
        let object = response
            .as_object_mut()
            .ok_or(SourceCommandError::Invalid("Work preview response"))?;
        object.insert(
            "prepared_fields".into(),
            growth_json(cmd::field(&request, "fields")?)?,
        );
        object.insert(
            "expected_dependencies".into(),
            growth_json(cmd::field(&request, "expected_dependencies")?)?,
        );
        object.insert(
            "expected_publication".into(),
            growth_json(cmd::field(&request, "expected_publication")?)?,
        );
        object.insert("prepared_work".into(), growth_json(&prepared_refs["work"])?);
        object.insert(
            "prepared_expression".into(),
            growth_json(&prepared_refs["expression"])?,
        );
        object.insert(
            "prepared_claim".into(),
            growth_json(&prepared_refs["claim"])?,
        );
        object.insert(
            "prepared_forms".into(),
            growth_json(&cmd::object(prepared_forms))?,
        );
        object.insert(
            "prepared_materializations".into(),
            growth_json(&work_expression_materializations(
                &owner.configuration,
                &outputs,
            )?)?,
        );
    } else {
        worker
            .finish(limits.deadline, cancelled)
            .map_err(item_error)?;
        fs.current_context(context, limits.deadline, cancelled)?;
        software_current(fs, context, limits.deadline, cancelled)?;
        snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled)?;
        fence.verify(limits.deadline, cancelled)?;
    }
    Ok(PgWorkExpressionPreview {
        response: super::foundation_value(&response)?,
        semantic_indexes,
        fence,
    })
}

fn growth_json(value: &JsonValue) -> SourceCommandResult<serde_json::Value> {
    serde_json::from_slice(&cmd::canonical(value)?)
        .map_err(|_| SourceCommandError::Invalid("Work preview response value"))
}

/// Prepare one exact selected Work/Expression package. The input must already
/// contain the selected catalog, Work endpoint, the nine contract/registry
/// members above, and the selected software component files in `context`.
/// The PG caller binds the resulting context to the final read snapshot after
/// dropping `input`, then reconstructs `proposal` from the returned response
/// and changes so every recorded exact read remains in the durable request.
pub(crate) fn prepare_pg_work_expression<'a, I: Copy + Eq>(
    fs: &'a CreationFilesystem,
    context: &CommandContext,
    input: &dyn SourceCutInputWithIdentity<I>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CandidateCutWorkerSchemaExecutor<I>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PgWorkExpressionOwnerPackage<'a>> {
    context.check_selected_software_inputs(software, components, limits.deadline, cancelled)?;
    for path in WORK_CONTRACTS {
        if context.file(&relative(path)?)?.is_none() {
            return Err(SourceCommandError::Unsupported(
                "selected Work source registry/schema set incomplete",
            ));
        }
    }

    let fence = work_transaction::WorkCorpusFence::hold_existing(fs, limits.deadline, cancelled)?;
    let profiles = crate::source_claims::work_expression_source_descriptors_candidate(
        context, input, worker, limits, cancelled,
    )?;
    let owner = WorkOwner::select(fs, context, limits.deadline, cancelled)?;
    let snapshot = work_transaction::PublicationSnapshot::select_from_input(
        input.source_input(),
        limits.deadline,
        cancelled,
    )?;
    let requested_publication = cmd::field(&owner.request, "expected_publication")?;
    if requested_publication.as_str() != snapshot.token.as_deref()
        && !(requested_publication == &JsonValue::Null && snapshot.token.is_none())
    {
        return Err(SourceCommandError::Conflict(
            "Work requested publication snapshot changed",
        ));
    }
    snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled)?;
    let prior_id = selected_prior_publication(input, &snapshot, limits.deadline, cancelled)?;
    let before = selected_before_from_input(
        &owner,
        input.source_input(),
        &snapshot,
        limits.deadline,
        cancelled,
    )?;
    let catalog = current_catalog_from_selected_input(
        fs,
        &owner,
        &before,
        input,
        worker,
        limits,
        snapshot.token.as_deref(),
        || snapshot.verify_from_input(input.source_input(), limits.deadline, cancelled),
        limits.deadline,
        cancelled,
    )?;
    if let Some(prior_id) = prior_id.as_deref() {
        selected_prior_completion(
            input.source_input(),
            &snapshot,
            prior_id,
            snapshot.token.as_deref(),
            limits.deadline,
            cancelled,
        )?;
    }
    let directories = Vec::new();
    verify_empty_prefix(
        input.source_input(),
        owner
            .expression_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Expression selected home"))?
            .0,
        limits.deadline,
        cancelled,
    )?;

    let scope = serde_value(&owner.scope()?)?;
    let mut request_raw = cmd::canonical(&owner.request)?;
    request_raw.push(b'\n');
    let mut authorization_error = None;
    let core = tos_validation::native_compound::prepare_selected_work_expression_bytes(
        input,
        worker,
        &scope,
        &request_raw,
        &before,
        &context.recorded_at,
        limits,
        cancelled,
        |grammar| {
            let result = (|| {
                let dependencies = dependency_bindings(&catalog, grammar, context)?;
                if cmd::record_digest(&dependencies)?.to_prefixed()
                    != cmd::text(&owner.request, "expected_dependencies")?
                {
                    return Err(SourceCommandError::Conflict(
                        "Work current dependency closure changed",
                    ));
                }
                serde_value(&owner.authorization(dependencies)?)
            })();
            result.map_err(|error| {
                authorization_error = Some(error);
                tos_validation::item_rules::ItemRefusal::Source(
                    "Work owner authorization rejected".to_owned(),
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
            "Work transaction identity differs from request",
        ));
    }
    let authorization = foundation_value(core.authorization())?;
    let archive_path = core.archive_path().to_owned();
    let outputs = core.outputs().map_err(item_error)?;
    let borrowed = outputs
        .iter()
        .map(|(path, raw)| (path.as_str(), *raw))
        .collect::<Vec<_>>();
    let capture = crate::source_serialization::capture_work_expression(
        &owner.request,
        &owner.event_id,
        owner
            .expression_path
            .as_str()
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("Expression selected home"))?
            .0,
        &archive_path,
        &before,
        &borrowed,
        software,
        components,
        limits.deadline,
        cancelled,
    )?;
    let finished = tos_validation::native_compound::finish_selected_compound_bytes(
        core,
        &capture.environment_raw,
        &capture.event_raw,
        worker,
    )
    .map_err(item_error)?;
    if finished.transaction_id != owner.transaction_id()? || finished.archive_path != archive_path {
        return Err(SourceCommandError::Conflict(
            "Work finished byte recipe differs from selected transaction",
        ));
    }
    let receipt = finished_receipt(&finished.child)?;
    let old_work = cmd::parse(
        before
            .get("work.json")
            .ok_or(SourceCommandError::Conflict("Work selected record absent"))?,
    )?;
    let expected_revision = cmd::text(&owner.request, "expected_revision")?;
    let archive = work_transaction::work_archive_package(
        owner.work_path.as_str(),
        &old_work,
        &before,
        expected_revision,
    )?;
    if archive.path != archive_path {
        return Err(SourceCommandError::Conflict(
            "Work retained archive locator differs from recipe",
        ));
    }
    let mut plan = owner.plan(
        authorization,
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
                "Work PG transaction journal path already occupied",
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

    let mut materialized = BTreeMap::new();
    let work_home = owner
        .work_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Work selected home"))?
        .0;
    let expression_home = owner
        .expression_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Expression selected home"))?
        .0;
    for file in &plan.files {
        if file.path.as_str().starts_with(&format!("{work_home}/"))
            || file
                .path
                .as_str()
                .starts_with(&format!("{expression_home}/"))
        {
            if let Some(after) = &file.after {
                materialized.insert(file.path.as_str().to_owned(), after.clone());
            }
        }
    }
    let source_profile_bytes = cmd::parse(
        before
            .get("work.json")
            .ok_or(SourceCommandError::Conflict("Work selected record absent"))?,
    )?;
    let response = cmd::object(vec![
        (
            "schema_version",
            cmd::string("tos_work_expression_result_v1"),
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
        ("work_source_path", cmd::string(owner.work_path.as_str())),
        (
            "expression_source_path",
            cmd::string(owner.expression_path.as_str()),
        ),
        (
            "source",
            cmd::reference(&source_profile_bytes, "record_id", "record_version")?,
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
        ("source_profiles", profiles),
        (
            "source_fields",
            JsonValue::Array(
                crate::source_forms::metadata_fields(&source_profile_bytes)?
                    .iter()
                    .map(|field| field.public())
                    .collect(),
            ),
        ),
        ("scope", owner.scope()?),
        ("receipt", receipt),
        ("replayed", JsonValue::Bool(false)),
        ("recovery", JsonValue::Null),
        (
            "materializations",
            work_expression_materializations(&owner.configuration, &materialized)?,
        ),
        ("grants_admission", JsonValue::Bool(false)),
    ]);
    let changes = plan
        .files
        .iter()
        .map(|file| {
            Ok(cmd::SourceChange {
                path: file.path.clone(),
                before: file.before.as_ref().map(|bytes| Digest256::of_bytes(bytes)),
                after: file.after.clone(),
            })
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let proposal = context.plan("native-work-expression", response, changes, false)?;
    let semantic_indexes = semantic_index_rows(
        &finished_output_bytes(&owner, &plan)?,
        &owner.expression_path,
        &owner.expression_id,
        &owner.claim_id,
        &owner.event_id,
    )?;
    Ok(PgWorkExpressionOwnerPackage {
        context: context.clone(),
        proposal,
        semantic_indexes,
        fence,
    })
}

/// Compare the final exact PG context reads and absences with the protected
/// physical source tree while `PgWorkExpressionOwnerPackage::fence` is held.
pub(crate) fn verify_pg_work_context(
    fs: &CreationFilesystem,
    context: &CommandContext,
    _absences: &BTreeSet<String>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    fs.current_context(context, deadline, cancelled)?;
    software_current(fs, context, deadline, cancelled)?;
    Ok(())
}

fn selected_before_from_input(
    owner: &WorkOwner,
    input: &dyn SourceCutInput,
    snapshot: &work_transaction::PublicationSnapshot,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let parent_ref = owner
        .work_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Work parent"))?
        .0;
    let mut before = BTreeMap::new();
    let mut total = 0usize;
    for name in [
        "work.json",
        "work.human-forms.json",
        "source-revision-history.json",
    ] {
        let path = format!("{parent_ref}/{name}");
        let selected = if input
            .path_presence(&path, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("Work PG parent presence read"))?
            .is_some()
        {
            Some(read_selected_source_member(
                input, &path, 2_097_152, deadline, cancelled,
            )?)
        } else {
            None
        };
        if name == "work.json" && selected.is_none() {
            return Err(SourceCommandError::Conflict(
                "Work selected parent record absent",
            ));
        }
        if let Some(raw) = selected {
            total = total
                .checked_add(raw.len())
                .filter(|bytes| *bytes <= 8_388_608)
                .ok_or(SourceCommandError::Invalid(
                    "selected Work package byte budget",
                ))?;
            before.insert(name.to_owned(), raw);
        }
    }
    snapshot.verify_from_input(input, deadline, cancelled)?;
    Ok(before)
}

fn selected_prior_publication<I: Copy + Eq>(
    input: &dyn SourceCutInputWithIdentity<I>,
    snapshot: &work_transaction::PublicationSnapshot,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<String>> {
    let path = "ToS/source-witnesses/.metadata-publication.json";
    let Some(_) = input
        .path_presence(path, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("Work prior publication presence"))?
    else {
        if snapshot.token.is_some() {
            return Err(SourceCommandError::Conflict(
                "Work prior publication absent",
            ));
        }
        return Ok(None);
    };
    let raw = read_selected_source_member(input.source_input(), path, 8192, deadline, cancelled)?;
    let state = cmd::parse(&raw)?;
    work_transaction::state(&state)?;
    if cmd::text(&state, "phase")? != "ready"
        || Some(cmd::text(&state, "token")?) != snapshot.token.as_deref()
    {
        return Err(SourceCommandError::Conflict(
            "Work prior publication differs from protected head",
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
    let Some(token) = expected_token else {
        return Err(SourceCommandError::Conflict(
            "Work prior completion has no publication token",
        ));
    };
    let Some(state) = snapshot.selected_state() else {
        return Err(SourceCommandError::Conflict(
            "Work prior publication state absent",
        ));
    };
    if snapshot.token.as_deref() != Some(token) || cmd::text(state, "transaction_id")? != id {
        return Err(SourceCommandError::Conflict(
            "Work prior publication does not bind selected head",
        ));
    }
    let journal = format!("ToS/source-witnesses/.metadata-transactions/{}", &id[7..]);
    let manifest_path = format!("{journal}/manifest.json");
    let manifest_raw =
        selected_optional_member(input, &manifest_path, 524_288, deadline, cancelled)?
            .ok_or(SourceCommandError::Conflict("Work prior manifest absent"))?;
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
                    "Work prior PG parent binding profile differs",
                ));
            }
        }
        _ => return Err(SourceCommandError::Conflict("Work prior manifest schema")),
    }
    let base = cmd::field(&manifest, "base_publication")?;
    cmd::exact_keys(base, &["token", "generation"])?;
    let base_generation = cmd::integer(base, "generation")?;
    let base_token_valid = match cmd::field(base, "token")? {
        JsonValue::Null => base_generation == 0,
        JsonValue::String(token) => {
            base_generation > 0 && token.as_str().is_some_and(work_transaction::is_hash)
        }
        _ => false,
    };
    if cmd::text(&manifest, "transaction_id")? != id
        || Digest256::of_bytes(&manifest_raw).to_prefixed() != cmd::text(state, "manifest_sha256")?
        || base_generation.checked_add(2) != Some(cmd::integer(state, "generation")?)
        || !base_token_valid
    {
        return Err(SourceCommandError::Conflict(
            "Work prior publication manifest differs",
        ));
    }
    let completion_path = format!("{journal}/completion.json");
    let completion = selected_optional_member(input, &completion_path, 8192, deadline, cancelled)?
        .ok_or(SourceCommandError::Conflict("Work prior completion absent"))?;
    let completion = cmd::parse(&completion)?;
    cmd::exact_keys(&completion, &["schema_version", "publication"])?;
    if cmd::text(&completion, "schema_version")? != "tos_selected_metadata_completion_v1"
        || !cmd::same(cmd::field(&completion, "publication")?, state)?
    {
        return Err(SourceCommandError::Conflict(
            "Work prior completion differs from selected publication",
        ));
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
            Err(tos_validation::item_rules::ItemRefusal::Source(
                "new PG Work output home is not empty".into(),
            ))
        })
        .map_err(|_| {
            if found.is_some() {
                SourceCommandError::Conflict("new PG Work output home is not empty")
            } else {
                SourceCommandError::Conflict("PG Work output home enumeration incomplete")
            }
        })?;
    Ok(())
}

fn append_archive_package(
    input: &dyn SourceCutInput,
    plan: &mut WorkPlan,
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
        .map_err(|_| SourceCommandError::Conflict("Work archive selected prefix incomplete"))?;
    if expected_paths.is_empty() || !selected_paths.is_empty() && selected_paths != expected_paths {
        return Err(SourceCommandError::Conflict(
            "Work archive selected member set is incomplete or unexpected",
        ));
    }

    for (name, expected) in &archive.members {
        let path = format!("{}/{name}", archive.path);
        let selected = if input
            .path_presence(&path, limits.deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("Work archive member presence"))?
            .is_some()
        {
            Some(read_selected_source_member(
                input,
                &path,
                16_777_216,
                limits.deadline,
                cancelled,
            )?)
        } else {
            None
        };
        if let Some(raw) = &selected {
            if raw != expected {
                return Err(SourceCommandError::Conflict("Work archive bytes changed"));
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

fn selected_optional_member(
    input: &dyn SourceCutInput,
    path: &str,
    max_bytes: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<Vec<u8>>> {
    if input
        .path_presence(path, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("selected PG member presence read"))?
        .is_some()
    {
        Ok(Some(read_selected_source_member(
            input, path, max_bytes, deadline, cancelled,
        )?))
    } else {
        Ok(None)
    }
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
        .map_err(|_| SourceCommandError::Conflict("selected PG prefix read incomplete"))?;
    Ok(found)
}

fn include_missing_output_directories(
    input: &dyn SourceCutInput,
    plan: &mut WorkPlan,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let existing_paths = plan
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
        add_missing_output_ancestors(
            input,
            &path,
            &existing_paths,
            &mut directories,
            limits,
            cancelled,
        )?;
    }
    plan.new_directories = directories
        .into_iter()
        .map(|path| relative(&path))
        .collect::<SourceCommandResult<Vec<_>>>()?;
    Ok(())
}

fn add_missing_output_ancestors(
    input: &dyn SourceCutInput,
    path: &str,
    existing_paths: &[String],
    directories: &mut BTreeSet<String>,
    limits: ItemLimits,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let mut parent = path
        .rsplit_once('/')
        .map(|(parent, _)| parent.to_owned())
        .ok_or(SourceCommandError::Invalid("selected PG output parent"))?;
    while parent.starts_with("ToS/source-witnesses/") {
        if existing_paths
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
            .ok_or(SourceCommandError::Invalid("selected PG output ancestor"))?;
    }
    Ok(())
}

fn verify_new_paths_absent(
    input: &dyn SourceCutInput,
    plan: &WorkPlan,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    for file in &plan.files {
        if file.before.is_some() {
            continue;
        }
        if input
            .path_presence(file.path.as_str(), deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("Work PG output presence read"))?
            .is_some()
        {
            return Err(SourceCommandError::Conflict("Work PG output path occupied"));
        }
    }
    Ok(())
}

fn semantic_index_rows(
    outputs: &BTreeMap<String, Vec<u8>>,
    expression_path: &tos_foundation::RelativePath,
    expression_id: &str,
    claim_id: &str,
    event_id: &str,
) -> SourceCommandResult<IndexRows> {
    let expression_home = expression_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Expression selected home"))?
        .0;
    let claim_path = format!("{expression_home}/source-claims.jsonl");
    let event_path = format!("{expression_home}/source-create-provenance.jsonl");
    let mut rows = BTreeSet::new();
    rows.insert((
        "metadata".to_owned(),
        expression_id.to_owned(),
        expression_path.as_str().to_owned(),
    ));
    let claims = outputs
        .get(&claim_path)
        .ok_or(SourceCommandError::Conflict(
            "Work generated Claim stream absent",
        ))?;
    let mut matching_claim = None;
    for line in catalog_lines(claims) {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let claim = cmd::parse(line)?;
        if cmd::text(&claim, "claim_id")? == claim_id {
            if matching_claim.replace(claim).is_some() {
                return Err(SourceCommandError::Conflict(
                    "duplicate generated Work Claim",
                ));
            }
        }
    }
    if matching_claim.is_none() {
        return Err(SourceCommandError::Conflict("generated Work Claim absent"));
    }
    rows.insert(("claim".to_owned(), claim_id.to_owned(), claim_path));
    rows.insert(("event".to_owned(), event_id.to_owned(), event_path));
    let forms_names = [
        "expression.human-forms.json".to_owned(),
        format!(
            "source-claims.{}.human-forms.json",
            Digest256::of_bytes(claim_id.as_bytes()).to_hex()
        ),
    ];
    for forms_name in forms_names {
        let path = format!("{expression_home}/{forms_name}");
        let raw = outputs.get(&path).ok_or(SourceCommandError::Conflict(
            "Work generated form set absent",
        ))?;
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

/// Re-derive the exact Work/Expression index contribution from sealed writes.
/// Recovery uses the same owner parser as fresh preparation; it does not trust
/// the indexes serialized in the attempt record.
pub(crate) fn expected_revision_indexes_from_writes(
    writes: &[SourceRevisionWrite],
) -> crate::DurableResult<IndexRows> {
    use crate::DurableError;

    if writes.is_empty() || writes.len() > 512 {
        return Err(DurableError::Corrupt(
            "PG Work/Expression write count differs",
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
                "PG Work/Expression writes are duplicated or outside source homes",
            ));
        }
    }

    let mut work = None;
    let mut expression = None;
    for (path, raw) in &outputs {
        let basename = path.rsplit('/').next().unwrap_or(path);
        if basename != "work.json" && basename != "expression.json" {
            continue;
        }
        let record = cmd::parse(raw).map_err(DurableError::Source)?;
        let record_type = cmd::text(&record, "record_type").map_err(DurableError::Source)?;
        let record_id = cmd::text(&record, "record_id")
            .map_err(DurableError::Source)?
            .to_owned();
        if (basename == "work.json" && record_type != "work")
            || (basename == "expression.json" && record_type != "expression")
        {
            return Err(DurableError::Corrupt(
                "PG Work/Expression metadata kind differs",
            ));
        }
        let slot = if basename == "work.json" {
            &mut work
        } else {
            &mut expression
        };
        if slot.replace((path.clone(), record_id)).is_some() {
            return Err(DurableError::Corrupt(
                "PG Work/Expression metadata path is ambiguous",
            ));
        }
    }
    let (work_path, _) = work.ok_or(DurableError::Corrupt(
        "PG Work/Expression predecessor metadata absent",
    ))?;
    let (expression_path, expression_id) = expression.ok_or(DurableError::Corrupt(
        "PG Work/Expression successor metadata absent",
    ))?;
    if !writes
        .iter()
        .any(|write| write.subject == work_path && write.expected_predecessor.is_some())
        || !writes
            .iter()
            .any(|write| write.subject == expression_path && write.expected_predecessor.is_none())
    {
        return Err(DurableError::Corrupt(
            "PG Work/Expression predecessor/successor write class differs",
        ));
    }
    let expression_parent = expression_path
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .ok_or(DurableError::Corrupt(
            "PG Work/Expression source home is absent",
        ))?;
    if work_path.rsplit_once('/').map(|(parent, _)| parent) != Some(expression_parent) {
        return Err(DurableError::Corrupt(
            "PG Work/Expression records are not siblings",
        ));
    }

    let claim_path = format!("{expression_parent}/source-claims.jsonl");
    let claim_raw = outputs.get(&claim_path).ok_or(DurableError::Corrupt(
        "PG Work/Expression Claim stream absent",
    ))?;
    let claims = catalog_lines(claim_raw)
        .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
        .map(|line| cmd::parse(line).map_err(DurableError::Source))
        .collect::<crate::DurableResult<Vec<_>>>()?;
    if claims.len() != 1 {
        return Err(DurableError::Corrupt(
            "PG Work/Expression Claim stream cardinality differs",
        ));
    }
    let claim_id = cmd::text(&claims[0], "claim_id")
        .map_err(DurableError::Source)?
        .to_owned();

    let event_path = format!("{expression_parent}/source-create-provenance.jsonl");
    let event_raw = outputs.get(&event_path).ok_or(DurableError::Corrupt(
        "PG Work/Expression event stream absent",
    ))?;
    let events = catalog_lines(event_raw)
        .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
        .map(|line| cmd::parse(line).map_err(DurableError::Source))
        .collect::<crate::DurableResult<Vec<_>>>()?;
    if events.len() != 1 {
        return Err(DurableError::Corrupt(
            "PG Work/Expression event cardinality differs",
        ));
    }
    let event_id = cmd::text(&events[0], "event_id")
        .map_err(DurableError::Source)?
        .to_owned();
    let semantic = semantic_index_rows(
        &outputs,
        &tos_foundation::RelativePath::parse(&expression_path)
            .map_err(|_| DurableError::Corrupt("PG Work/Expression path is invalid"))?,
        &expression_id,
        &claim_id,
        &event_id,
    )
    .map_err(DurableError::Source)?;
    indexes.extend(semantic);
    Ok(indexes)
}

/// Re-derive the full Growth member projection from the sealed Work package.
/// This is used by registration and recovery so the stored contribution is
/// never trusted independently of the exact source writes.
pub(crate) fn expected_inventory_contributions_from_writes(
    writes: &[SourceRevisionWrite],
    context: &CommandContext,
    response: &JsonValue,
) -> crate::DurableResult<BTreeMap<String, JsonValue>> {
    expected_revision_indexes_from_writes(writes)?;
    let mut records = Vec::with_capacity(2);
    let mut record_kinds = BTreeSet::new();
    for write in writes {
        let basename = write.subject.rsplit('/').next().unwrap_or(&write.subject);
        let kind = match basename {
            "work.json" => "work",
            "expression.json" => "expression",
            _ => continue,
        };
        let record = cmd::parse(&write.exact_bytes).map_err(crate::DurableError::Source)?;
        let profiles = response
            .object_get("source_profiles")
            .and_then(|value| value.object_get(kind))
            .ok_or(crate::DurableError::Corrupt(
                "PG Work source profile descriptor is absent",
            ))?;
        let schema_ref = cmd::text(profiles, "schema_ref")
            .map_err(crate::DurableError::Source)?
            .to_owned();
        if !record_kinds.insert(kind) {
            return Err(crate::DurableError::Corrupt(
                "PG Work metadata projection repeats",
            ));
        }
        records.push(crate::source_creation_store::PgGrowthRecordProjection {
            kind: kind.to_owned(),
            path: write.subject.clone(),
            record,
            schema_ref: Some(schema_ref),
        });
    }
    if records.len() != 2 {
        return Err(crate::DurableError::Corrupt(
            "PG Work metadata projection coverage differs",
        ));
    }
    let claim_path = writes
        .iter()
        .find(|write| write.subject.ends_with("/source-claims.jsonl"))
        .map(|write| write.subject.as_str())
        .ok_or(crate::DurableError::Corrupt(
            "PG Work Claim stream write absent",
        ))?;
    let event_path = writes
        .iter()
        .find(|write| write.subject.ends_with("/source-create-provenance.jsonl"))
        .map(|write| write.subject.as_str())
        .ok_or(crate::DurableError::Corrupt(
            "PG Work provenance event write absent",
        ))?;
    crate::source_creation_store::pg_growth_owner_contributions(
        writes,
        &context.files,
        &records,
        event_path,
        claim_path,
        "tos_managed_work_expression_claim_owner_inventory_v1",
    )
}

fn finished_output_bytes(
    owner: &WorkOwner,
    plan: &WorkPlan,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let work_home = owner
        .work_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Work selected home"))?
        .0;
    let expression_home = owner
        .expression_path
        .as_str()
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Expression selected home"))?
        .0;
    Ok(plan
        .files
        .iter()
        .filter_map(|file| {
            let path = file.path.as_str();
            (path.starts_with(&format!("{work_home}/"))
                || path.starts_with(&format!("{expression_home}/")))
            .then(|| file.after.clone().map(|raw| (path.to_owned(), raw)))
            .flatten()
        })
        .collect())
}
