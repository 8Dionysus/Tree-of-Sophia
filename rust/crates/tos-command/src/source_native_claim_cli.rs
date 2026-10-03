//! Typed Claim dispatch through existing isolated publication and retry entries.
use super::{absolute, capped, digest, selected_schema, text};
use crate::source_claims as owner;
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
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
    let original_revision = SourceRevision(digest(text(invocation, "original_source_revision")?)?);
    let budgets = &invocation["budgets"];
    let original = store
        .open_source_cut(
            original_revision,
            CutReadLimits {
                max_revisions: usize::try_from(capped(budgets, "max_revisions", 4)?)
                    .map_err(|_| SourceCommandError::Invalid("Claim original cut budget"))?,
                max_members: capped(budgets, "max_members", 2048)?,
                max_total_bytes: capped(budgets, "max_total_bytes", 33_554_432)?,
                max_member_bytes: capped(budgets, "max_member_bytes", 8_388_608)?,
            },
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("Claim original source cut"))?;
    let (filesystem, configuration_raw) = CreationFilesystem::select_claim_owner(
        &absolute(text(invocation, "owner_config")?)?,
        deadline,
        cancelled,
    )?;
    let config = cmd::parse(&configuration_raw)?;
    let (_, creation, _) = owner::family(cmd::text(&config, "schema_version")?)?;
    let request = cmd::parse(request_raw)?;
    let operation = cmd::text(&request, "operation")?;
    if (creation && matches!(operation, "prepare-revise" | "claim.revise"))
        || (!creation && matches!(operation, "prepare-create" | "claims.create"))
    {
        return Err(SourceCommandError::Denied(
            "Claim invocation owner operation",
        ));
    }
    // Select software separately; the existing creation executor also needs
    // authenticated authored profile bytes before its internal package loader.
    components
        .members()
        .try_fold(0u64, |sum, member| sum.checked_add(member.size_bytes))
        .filter(|n| *n <= 33_554_432)
        .ok_or(SourceCommandError::Unsupported(
            "Claim selected software budget",
        ))?;
    let mut files = Vec::new();
    for member in components.members() {
        let raw = software
            .read_selected_component(components, &member.path, 8_388_608, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("Claim selected software read"))?;
        files.push(cmd::SourceFile {
            path: member.path.clone(),
            raw,
        });
    }
    let selected = if matches!(operation, "prepare-create" | "claims.create") {
        &original
    } else {
        current
    };
    let mut context = cmd::CommandContext {
        base_revision: selected.current().revision(),
        configuration_raw,
        request_raw: request_raw.to_vec(),
        recorded_at: crate::source_serialization::instant()?,
        effective_uid: u64::from(rustix::process::getuid().as_raw()),
        files,
    };
    let mut authored = owner::complete_authored_inputs(&context, selected, deadline, cancelled)?;
    authored.append(&mut context.files);
    context.files = authored;
    filesystem.current_context(&context, deadline, cancelled)?;
    let mut worker = selected_schema(invocation, selected, deadline, cancelled)?;
    let response = match operation {
        "prepare-create" => {
            if original_revision != current.current().revision() {
                return Err(SourceCommandError::Conflict(
                    "Claim creation preview requires current original cut",
                ));
            }
            owner::prepare_isolated_claim_creation_from_captures(
                &filesystem,
                &context,
                &original,
                software,
                components,
                &mut worker,
                deadline,
                cancelled,
            )?
            .response
        }
        "claims.create" => {
            let mut current_worker = selected_schema(invocation, current, deadline, cancelled)?;
            let (_, _, response) = owner::execute_isolated_claim_creation_from_captures(
                &filesystem,
                &context,
                &original,
                current,
                software,
                components,
                &mut worker,
                Some(&mut current_worker),
                deadline,
                cancelled,
            )?;
            response
        }
        "prepare-revise" => {
            owner::prepare_isolated_claim_revision_from_captures(
                &filesystem,
                &context,
                &original,
                current,
                software,
                components,
                &mut worker,
                deadline,
                cancelled,
            )?
            .response
        }
        "claim.revise" => {
            owner::execute_isolated_claim_revision_from_captures(
                &filesystem,
                &context,
                &original,
                current,
                software,
                components,
                &mut worker,
                deadline,
                cancelled,
            )?
            .0
            .response
        }
        "describe" | "inspect-version" => {
            owner::run_claim_command_from_captures(
                &context,
                current,
                software,
                components,
                &mut worker,
                deadline,
                cancelled,
            )?
            .response
        }
        _ => return Err(SourceCommandError::Unsupported("Claim native operation")),
    };
    // The response and receipt remain owned by the existing Claim executor.
    let response: Value = serde_json::from_slice(&cmd::canonical(&response)?)
        .map_err(|_| SourceCommandError::Invalid("Claim native response"))?;
    Ok(json!({"schema_version":"tos_local_native_claim_result_v1",
        "authentication":"local-unix-account", "result":response, "grants_admission":false}))
}
