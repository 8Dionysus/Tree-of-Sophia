//! Maintained historical Claim revision/forms with an exact public owner.
use super::{absolute, capped, digest, selected_schema, text};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{CreationFilesystem, LegacyOwnerStore};
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::SourceRevision;
use tos_source_store::{
    CorpusCutReader, CorpusReader, CutReadLimits, SoftwareCaptureReader,
    SoftwareComponentSelectionV1,
};
use tos_validation::source_cut::CutSchemaExecutor;

#[path = "source_legacy_historical_claim.rs"]
mod engine;

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
            "historical Claim invocation context",
        ));
    }
    let budgets = &invocation["budgets"];
    let original = SourceRevision(digest(text(invocation, "original_source_revision")?)?);
    store
        .open_source_cut(
            original,
            CutReadLimits {
                max_revisions: usize::try_from(capped(budgets, "max_revisions", 4)?)
                    .map_err(|_| SourceCommandError::Invalid("historical original revisions"))?,
                max_members: capped(budgets, "max_members", 2048)?,
                max_total_bytes: capped(budgets, "max_total_bytes", 33_554_432)?,
                max_member_bytes: capped(budgets, "max_member_bytes", 8_388_608)?,
            },
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("historical original source cut"))?;
    let path = absolute(text(invocation, "owner_config")?)?;
    let (filesystem, configuration_raw) =
        CreationFilesystem::select_protected_native_owner(&path, deadline, cancelled)?;
    let config = cmd::parse(&configuration_raw)?;
    if !matches!(
        cmd::text(&config, "schema_version")?,
        "tos_local_historical_claim_revision_owner_v1" | "tos_local_historical_claim_form_owner_v1"
    ) {
        return Err(SourceCommandError::Denied("historical Claim typed family"));
    }
    let mut context = cmd::CommandContext {
        base_revision: current.current().revision(),
        configuration_raw,
        request_raw: request_raw.to_vec(),
        recorded_at: crate::source_serialization::instant()?,
        effective_uid: u64::from(rustix::process::getuid().as_raw()),
        files: Vec::new(),
    };
    context.files =
        crate::source_claims::complete_authored_inputs(&context, current, deadline, cancelled)?;
    let mut total = 0u64;
    for member in components.members() {
        total = total
            .checked_add(member.size_bytes)
            .ok_or(SourceCommandError::Invalid("historical software budget"))?;
        if total > 33_554_432 {
            return Err(SourceCommandError::Invalid("historical software budget"));
        }
        let raw = software
            .read_selected_component(components, &member.path, 8_388_608, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("historical selected software"))?;
        context.files.push(cmd::SourceFile {
            path: member.path.clone(),
            raw,
        });
    }
    context.check_from_selected_captures(current, software, components, deadline, cancelled)?;
    filesystem.current_context(&context, deadline, cancelled)?;
    let mut selected = LegacyOwnerStore::select(&filesystem, &context)?;
    let before = selected.read_package(deadline, cancelled)?;
    let mut worker = selected_schema(invocation, current, deadline, cancelled)?;
    let plan = engine::prepare(
        &context,
        current,
        &before,
        &mut selected,
        &mut worker,
        deadline,
        cancelled,
    )?;
    if plan.reads.iter().any(|input| {
        !context
            .files
            .iter()
            .any(|selected| selected.path == input.path && selected.raw == input.raw)
    }) {
        return Err(SourceCommandError::Denied(
            "historical Claim dependency outside selected closure",
        ));
    }
    worker
        .finish(deadline, cancelled)
        .map_err(|_| SourceCommandError::Invalid("historical Claim worker finish"))?;
    if let Some(after) = plan.files {
        let guard = || {
            selected.verify_current_cut(current, deadline, cancelled)?;
            selected.verify_observed_archives(deadline, cancelled)?;
            context.check_from_selected_captures(current, software, components, deadline, cancelled)
        };
        let request = cmd::parse(request_raw)?;
        selected.publish_successor(
            &request,
            &before,
            &after,
            plan.archive
                .as_ref()
                .map(|(reference, files)| (reference.as_str(), files)),
            guard,
            guard,
            deadline,
            cancelled,
        )?;
    } else {
        selected.verify_current_cut(current, deadline, cancelled)?;
        selected.verify_observed_archives(deadline, cancelled)?;
        context.check_from_selected_captures(current, software, components, deadline, cancelled)?;
        if selected.read_package(deadline, cancelled)? != before {
            return Err(SourceCommandError::Conflict(
                "historical readonly package changed",
            ));
        }
    }
    let response: Value = serde_json::from_slice(&cmd::canonical(&plan.response)?)
        .map_err(|_| SourceCommandError::Invalid("historical Claim response"))?;
    Ok(
        serde_json::json!({"schema_version":"tos_local_native_source_result_v1",
        "authentication":"local-unix-account","result":response,"grants_admission":false}),
    )
}
