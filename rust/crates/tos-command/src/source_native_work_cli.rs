//! Fixed Work→Expression native caller over the existing isolated owner.
use super::{absolute, capped, digest, selected_schema, text};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{self as owner, CreationFilesystem};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{JsonValue, SourceRevision};
use tos_source_store::{
    CorpusCutReader, CorpusReader, CutReadLimits, SoftwareCaptureReader,
    SoftwareComponentSelectionV1,
};
use tos_validation::item_rules::ItemLimits;
use tos_validation::source_cut::CutSchemaExecutor;

fn context(
    configuration_raw: &[u8],
    request_raw: &[u8],
    recorded_at: &str,
    cut: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<cmd::CommandContext> {
    let mut files = Vec::new();
    let mut paths = BTreeSet::new();
    let mut total = 0u64;
    for member in cut.current().members() {
        if !member.path.as_str().starts_with("ToS/") {
            return Err(SourceCommandError::Denied("Work source cut namespace"));
        }
        total = total
            .checked_add(member.size_bytes)
            .filter(|n| *n <= 33_554_432)
            .ok_or(SourceCommandError::Invalid(
                "Work selected context byte budget",
            ))?;
        if !paths.insert(member.path.clone()) {
            return Err(SourceCommandError::Invalid("Work duplicate selected path"));
        }
        let raw = cut
            .read_member(
                cut.current().revision(),
                &member.path,
                8_388_608,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("Work source member read"))?
            .raw;
        files.push(cmd::SourceFile {
            path: member.path.clone(),
            raw,
        });
    }
    for member in components.members() {
        if member.path.as_str().starts_with("ToS/") || !paths.insert(member.path.clone()) {
            return Err(SourceCommandError::Denied(
                "Work source/software namespaces overlap",
            ));
        }
        total = total
            .checked_add(member.size_bytes)
            .filter(|n| *n <= 33_554_432)
            .ok_or(SourceCommandError::Invalid(
                "Work combined selected context byte budget",
            ))?;
        let raw = software
            .read_selected_component(components, &member.path, 8_388_608, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("Work software component read"))?;
        files.push(cmd::SourceFile {
            path: member.path.clone(),
            raw,
        });
    }
    Ok(cmd::CommandContext {
        base_revision: cut.current().revision(),
        configuration_raw: configuration_raw.to_vec(),
        request_raw: request_raw.to_vec(),
        recorded_at: recorded_at.to_owned(),
        effective_uid: u64::from(rustix::process::getuid().as_raw()),
        files,
    })
}
fn value(value: &JsonValue) -> SourceCommandResult<Value> {
    serde_json::from_slice(&cmd::canonical(value)?)
        .map_err(|_| SourceCommandError::Invalid("Work response JSON"))
}

pub(super) fn run(
    invocation: &Value,
    request_raw: &[u8],
    store: &CorpusReader,
    current: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    if invocation.get("owner_context") != Some(&Value::Null)
        || invocation.get("assessment_schema_worker") != Some(&Value::Null)
    {
        return Err(SourceCommandError::Denied(
            "Work invocation unused protected selectors",
        ));
    }
    let (filesystem, configuration_raw) = CreationFilesystem::select_protected_native_owner(
        &absolute(text(invocation, "owner_config")?)?,
        deadline,
        cancelled,
    )?;
    let configuration = cmd::parse(&configuration_raw)?;
    if cmd::text(&configuration, "schema_version")? != "tos_local_work_expression_owner_v1" {
        return Err(SourceCommandError::Denied("Work exact protected family"));
    }
    let request = cmd::parse(request_raw)?;
    let operation = cmd::text(&request, "operation")?;
    if !matches!(
        operation,
        "describe" | "prepare-create" | "work.expression.create" | "work.expression.recover"
    ) {
        return Err(SourceCommandError::Unsupported("Work command operation"));
    }
    if operation == "describe" {
        cmd::exact_keys(&request, &["schema_version", "operation"])?;
        if cmd::text(&request, "schema_version")? != "tos_local_work_expression_command_v1" {
            return Err(SourceCommandError::Invalid("Work describe request"));
        }
    }
    let now = crate::source_serialization::instant()?;
    let current_ctx = context(
        &configuration_raw,
        request_raw,
        &now,
        current,
        software,
        components,
        deadline,
        cancelled,
    )?;
    let retained =
        owner::retained_work_request(&filesystem, &current_ctx, current, deadline, cancelled)?;
    let original = match invocation.get("original_source_revision") {
        Some(Value::Null) => None,
        Some(Value::String(revision)) => {
            let budgets = &invocation["budgets"];
            Some(
                store
                    .open_source_cut(
                        SourceRevision(digest(revision)?),
                        CutReadLimits {
                            max_revisions: usize::try_from(capped(budgets, "max_revisions", 4)?)
                                .map_err(|_| {
                                    SourceCommandError::Invalid("Work original revision budget")
                                })?,
                            max_members: capped(budgets, "max_members", 2048)?,
                            max_total_bytes: capped(budgets, "max_total_bytes", 33_554_432)?,
                            max_member_bytes: capped(budgets, "max_member_bytes", 8_388_608)?,
                        },
                        deadline,
                        cancelled,
                    )
                    .map_err(|_| SourceCommandError::Conflict("Work original selected cut"))?,
            )
        }
        _ => {
            return Err(SourceCommandError::Invalid(
                "Work original source revision selector",
            ));
        }
    };
    if retained.is_some() && original.is_none()
        || retained.is_none()
            && original
                .as_ref()
                .is_some_and(|cut| cut.current().revision() != current.current().revision())
    {
        return Err(SourceCommandError::Denied(
            "Work original cut required only for retained replay/recovery",
        ));
    }
    let original_ctx = if let (Some(retained), Some(original)) = (&retained, &original) {
        Some(context(
            &configuration_raw,
            &retained.request_raw,
            &retained.recorded_at,
            original,
            software,
            components,
            deadline,
            cancelled,
        )?)
    } else {
        None
    };
    let selected = if retained.is_some() {
        original.as_ref().ok_or(SourceCommandError::Denied(
            "Work retained original cut absent",
        ))?
    } else {
        current
    };
    let ctx = original_ctx.as_ref().unwrap_or(&current_ctx);
    let limits = ItemLimits {
        max_member_bytes: 2_097_152,
        max_total_bytes: 33_554_432,
        max_state_bytes: 33_554_432,
        max_issues: 256,
        deadline,
    };
    let mut worker = selected_schema(invocation, selected, deadline, cancelled)?;
    let result = (|| {
        ctx.check_from_selected_captures(selected, software, components, deadline, cancelled)?;
        let profiles = crate::source_claims::work_expression_source_descriptors(
            ctx,
            &mut worker,
            limits,
            cancelled,
        )?;
        let mut response;
        if operation == "describe" {
            worker.finish(deadline, cancelled).map_err(|reason| {
                SourceCommandError::SchemaExecution {
                    path: "work.expression.describe".into(),
                    root: "native Work description".into(),
                    reason,
                }
            })?;
            response = value(&owner::work_expression_owner_result(
                &filesystem,
                ctx,
                current,
                software,
                components,
                profiles,
                None,
                None,
                false,
                None,
                None,
                deadline,
                cancelled,
            )?)?;
        } else if operation == "prepare-create" {
            let prepared = owner::prepare_isolated_work_expression_from_proposal(
                &filesystem,
                ctx,
                current,
                software,
                components,
                &mut worker,
                limits,
                cancelled,
            )?;
            response = value(&owner::work_expression_owner_result(
                &filesystem,
                ctx,
                current,
                software,
                components,
                profiles,
                None,
                None,
                false,
                None,
                None,
                deadline,
                cancelled,
            )?)?;
            let prepared_request = value(prepared.request())?;
            let outputs = prepared.projected_outputs();
            let expression_home = cmd::text(&configuration, "expression_source_path")?
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid("Work preview child parent"))?
                .0;
            let receipt = cmd::parse(
                outputs
                    .get(&format!("{expression_home}/work-expression-receipt.json"))
                    .ok_or(SourceCommandError::Conflict("Work preview receipt absent"))?,
            )?;
            let object = response
                .as_object_mut()
                .ok_or(SourceCommandError::Invalid("Work result object"))?;
            for (key, field) in [
                ("prepared_fields", "fields"),
                ("expected_dependencies", "expected_dependencies"),
                ("expected_publication", "expected_publication"),
            ] {
                object.insert(key.into(), prepared_request[field].clone());
            }
            for (key, field) in [
                ("prepared_work", "parent_after"),
                ("prepared_expression", "expression"),
                ("prepared_claim", "claim"),
                ("prepared_forms", "forms"),
            ] {
                object.insert(key.into(), value(cmd::field(&receipt, field)?)?);
            }
            object.insert(
                "prepared_materializations".into(),
                value(&owner::work_expression_materializations(
                    &configuration,
                    outputs,
                )?)?,
            );
        } else {
            let pending = retained.as_ref().is_some_and(|retained| retained.pending);
            let publication = if pending {
                let decision = if operation == "work.expression.recover"
                    && cmd::text(&request, "decision")? == "rollback"
                {
                    owner::WorkRecoveryDecision::Rollback
                } else {
                    owner::WorkRecoveryDecision::Resume
                };
                if operation == "work.expression.recover" {
                    owner::recover_isolated_work_expression_from_captures(
                        &filesystem,
                        ctx,
                        selected,
                        software,
                        components,
                        &mut worker,
                        decision,
                        limits,
                        cancelled,
                    )?
                } else {
                    owner::resume_isolated_work_expression_from_captures(
                        &filesystem,
                        ctx,
                        selected,
                        software,
                        components,
                        &mut worker,
                        limits,
                        cancelled,
                    )?
                }
            } else if retained.is_some() {
                owner::replay_isolated_work_expression_from_captures(
                    &filesystem,
                    ctx,
                    selected,
                    current,
                    software,
                    components,
                    &mut worker,
                    limits,
                    cancelled,
                )?
            } else {
                owner::execute_isolated_work_expression_from_captures(
                    &filesystem,
                    ctx,
                    current,
                    software,
                    components,
                    &mut worker,
                    limits,
                    cancelled,
                )?
            };
            let views = owner::published_work_materializations(
                &filesystem,
                ctx,
                &publication,
                deadline,
                cancelled,
            )?;
            let response_ctx = if publication.replayed() {
                &current_ctx
            } else {
                ctx
            };
            let response_cut = if publication.replayed() {
                current
            } else {
                selected
            };
            response = value(&owner::work_expression_owner_result(
                &filesystem,
                response_ctx,
                response_cut,
                software,
                components,
                profiles,
                Some(&publication),
                Some(publication.receipt().clone()),
                publication.replayed(),
                pending.then(|| publication.publication().clone()),
                Some(views),
                deadline,
                cancelled,
            )?)?;
        }
        Ok(
            json!({"schema_version":"tos_local_native_source_result_v1","authentication":"local-unix-account","result":response,"grants_admission":false}),
        )
    })();
    if result.is_err() {
        let _ = worker.finish(deadline, cancelled);
    }
    result
}
