//! Maintained initial-package CLI bridge. All mutation and retained-byte
//! semantics remain in the existing creation and filesystem owners.
use super::{absolute, capped, digest, selected_schema, selected_schema_with_profile, text};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation::{self as owner, CreationFamily};
use crate::source_creation_store::CreationFilesystem;
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::SourceRevision;
use tos_source_store::{
    CorpusCutReader, CorpusReader, CutReadLimits, SoftwareCaptureReader,
    SoftwareComponentSelectionV1,
};

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
    if !invocation.get("owner_context").is_some_and(Value::is_null) {
        return Err(SourceCommandError::Denied(
            "initial creation has no owner-context grant",
        ));
    }
    let original_revision = SourceRevision(digest(text(invocation, "original_source_revision")?)?);
    let budgets = &invocation["budgets"];
    let original = store
        .open_source_cut(
            original_revision,
            CutReadLimits {
                max_revisions: usize::try_from(capped(budgets, "max_revisions", 4)?)
                    .map_err(|_| SourceCommandError::Invalid("creation original cut budget"))?,
                max_members: capped(budgets, "max_members", 2048)?,
                max_total_bytes: capped(budgets, "max_total_bytes", 33_554_432)?,
                max_member_bytes: capped(budgets, "max_member_bytes", 8_388_608)?,
            },
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("creation original cut"))?;
    let configuration_path = absolute(text(invocation, "owner_config")?)?;
    let (filesystem, configuration_raw) =
        CreationFilesystem::select_creation_owner(&configuration_path, deadline, cancelled)?;
    let config = cmd::parse(&configuration_raw)?;
    let family = CreationFamily::parse(cmd::text(&config, "schema_version")?)?;
    let request = cmd::parse(request_raw)?;
    let operation = cmd::text(&request, "operation")?;
    let mutation = match family {
        CreationFamily::HistoricalV1 | CreationFamily::HistoricalV2 => "historical.create",
        CreationFamily::Sign => "sign.promote",
        _ => "source.create",
    };
    if !matches!(operation, "describe" | "prepare" | "prepare-create") && operation != mutation {
        return Err(SourceCommandError::Unsupported(
            "initial creation information entry not selected",
        ));
    }
    components
        .members()
        .try_fold(0u64, |sum, member| sum.checked_add(member.size_bytes))
        .filter(|bytes| *bytes <= 33_554_432)
        .ok_or(SourceCommandError::Unsupported(
            "creation selected software byte budget",
        ))?;
    let mut files = Vec::new();
    for member in components.members() {
        let raw = software
            .read_selected_component(components, &member.path, 8_388_608, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("creation selected software read"))?;
        files.push(cmd::SourceFile {
            path: member.path.clone(),
            raw,
        });
    }
    let mut context = cmd::CommandContext {
        base_revision: original.current().revision(),
        configuration_raw,
        request_raw: request_raw.to_vec(),
        recorded_at: crate::source_serialization::instant()?,
        effective_uid: u64::from(rustix::process::getuid().as_raw()),
        files,
    };
    let mut authored =
        crate::source_claims::complete_authored_inputs(&context, &original, deadline, cancelled)?;
    authored.append(&mut context.files);
    context.files = authored;
    filesystem.current_context(&context, deadline, cancelled)?;
    let mut worker = selected_schema(invocation, &original, deadline, cancelled)?;
    let mut batch = tos_validation::executor::BatchBudget::laboratory();
    batch.total_execution_wall = deadline.saturating_duration_since(Instant::now());
    batch.cpu_seconds = capped(budgets, "worker_cpu_seconds", 3)?;
    batch.address_space_bytes = capped(budgets, "worker_address_space_bytes", 1_073_741_824)?;
    let limits = tos_validation::assessment::AssessmentLimits {
        max_input_bytes: 8_388_608,
        max_work: 4_194_304,
        batch,
        deadline,
    };
    if matches!(operation, "describe" | "prepare") {
        if original_revision != current.current().revision() {
            return Err(SourceCommandError::Conflict(
                "creation information requires current selected cut",
            ));
        }
        let exists = filesystem.creation_target_exists(&context, deadline, cancelled)?;
        let mut response = owner::initial_information_from_captures(
            &context,
            &original,
            software,
            components,
            &mut worker,
            exists,
            deadline,
            cancelled,
        )?;
        if family == CreationFamily::Sign && operation == "describe" {
            let mut assessment = selected_schema_with_profile(
                invocation,
                &original,
                "assessment_schema_worker",
                tos_validation::FormatProfile::AssertedSourceCandidateV1,
                deadline,
                cancelled,
            )?;
            let mut selected = crate::source_sign::SignPromotionRead::select(
                &configuration_path,
                &context,
                &original,
                deadline,
                cancelled,
            )?;
            let promotion = selected.describe_promotion(
                &context,
                &mut worker,
                &mut assessment,
                limits,
                cancelled,
            )?;
            cmd::set(&mut response, "promotion", promotion)?;
        }
        crate::source_creation_store::finish_creation_worker(&mut worker, deadline, cancelled)?;
        return envelope(response);
    }
    let prepared = if family == CreationFamily::Sign {
        let mut assessment = selected_schema_with_profile(
            invocation,
            &original,
            "assessment_schema_worker",
            tos_validation::FormatProfile::AssertedSourceCandidateV1,
            deadline,
            cancelled,
        )?;
        owner::prepare_sign_promotion_from_captures(
            &configuration_path,
            &context,
            &original,
            software,
            components,
            &mut worker,
            &mut assessment,
            limits,
            cancelled,
        )?
    } else {
        owner::prepare_source_creation_from_captures(
            &context,
            &original,
            software,
            components,
            &mut worker,
            deadline,
            cancelled,
        )?
    };
    let response = if operation == "prepare-create" {
        if original_revision != current.current().revision() {
            return Err(SourceCommandError::Conflict(
                "creation preview requires current original cut",
            ));
        }
        let response = prepared.preview()?;
        crate::source_creation_store::finish_creation_worker(&mut worker, deadline, cancelled)?;
        response
    } else {
        let retained = filesystem.read_creation_retained(&prepared, deadline, cancelled)?;
        let (serialized, replay) = if let Some(retained) = retained {
            (
                prepared.serialize_retained(
                    software,
                    components,
                    &mut worker,
                    &retained,
                    deadline,
                    cancelled,
                )?,
                true,
            )
        } else {
            if original_revision != current.current().revision() {
                return Err(SourceCommandError::Conflict(
                    "fresh creation requires current original cut",
                ));
            }
            (
                prepared.serialize(software, components, &mut worker, deadline, cancelled)?,
                false,
            )
        };
        crate::source_creation_store::finish_creation_worker(&mut worker, deadline, cancelled)?;
        let publication = if family == CreationFamily::Sign {
            let mut local = selected_schema(invocation, &original, deadline, cancelled)?;
            let mut assessment = selected_schema_with_profile(
                invocation,
                &original,
                "assessment_schema_worker",
                tos_validation::FormatProfile::AssertedSourceCandidateV1,
                deadline,
                cancelled,
            )?;
            if replay {
                filesystem.replay_sign_isolated(
                    &serialized,
                    &original,
                    software,
                    components,
                    &mut local,
                    &mut assessment,
                    limits,
                    cancelled,
                )?
            } else {
                filesystem.publish_sign_isolated(
                    &serialized,
                    &original,
                    software,
                    components,
                    &mut local,
                    &mut assessment,
                    limits,
                    cancelled,
                )?
            }
        } else if replay {
            filesystem.replay_isolated(
                &serialized,
                &original,
                software,
                components,
                deadline,
                cancelled,
            )?
        } else {
            filesystem.publish_isolated(
                &serialized,
                &original,
                software,
                components,
                deadline,
                cancelled,
            )?
        };
        serialized.published_result(publication.replayed)?
    };
    envelope(response)
}
fn envelope(response: tos_foundation::JsonValue) -> SourceCommandResult<Value> {
    let response: Value = serde_json::from_slice(&cmd::canonical(&response)?)
        .map_err(|_| SourceCommandError::Invalid("creation native response"))?;
    Ok(json!({"schema_version":"tos_local_native_source_result_v1",
        "authentication":"local-unix-account", "result":response, "grants_admission":false}))
}
