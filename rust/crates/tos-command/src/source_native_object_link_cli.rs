//! Maintained ObjectLink cross-process entry. The owner engine builds actual
//! journal plans; the invocation only selects custody and protected authority.
use super::{absolute, capped, digest, selected_schema, text};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{self as owner, CreationFilesystem};
use serde_json::{json, Value};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::SourceRevision;
use tos_source_store::{
    CorpusCutReader, CorpusReader, CutReadLimits, SoftwareCaptureReader,
    SoftwareComponentSelectionV1,
};
fn json(v: &tos_foundation::JsonValue) -> SourceCommandResult<Value> {
    serde_json::from_slice(&cmd::canonical(v)?)
        .map_err(|_| SourceCommandError::Invalid("ObjectLink result JSON"))
}
fn envelope(result: Value) -> SourceCommandResult<Value> {
    Ok(
        json!({"schema_version":"tos_local_native_source_result_v1","authentication":"local-unix-account","result":result,"grants_admission":false}),
    )
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
    if !invocation.get("owner_context").is_some_and(Value::is_null)
        || !invocation
            .get("assessment_schema_worker")
            .is_some_and(Value::is_null)
    {
        return Err(SourceCommandError::Denied(
            "ObjectLink unused owner context/assessment worker must be null",
        ));
    }
    let budgets = &invocation["budgets"];
    let original_revision = SourceRevision(digest(text(invocation, "original_source_revision")?)?);
    let original = store
        .open_source_cut(
            original_revision,
            CutReadLimits {
                max_revisions: usize::try_from(capped(budgets, "max_revisions", 4)?)
                    .map_err(|_| SourceCommandError::Invalid("ObjectLink revision budget"))?,
                max_members: capped(budgets, "max_members", 2048)?,
                max_total_bytes: capped(budgets, "max_total_bytes", 33_554_432)?,
                max_member_bytes: capped(budgets, "max_member_bytes", 8_388_608)?,
            },
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("ObjectLink original cut unavailable"))?;
    let path = absolute(text(invocation, "owner_config")?)?;
    let (fs, configuration_raw) =
        CreationFilesystem::select_protected_native_owner(&path, deadline, cancelled)?;
    let config = cmd::parse(&configuration_raw)?;
    if cmd::text(&config, "schema_version")? != "tos_local_object_link_create_owner_v1" {
        return Err(SourceCommandError::Denied(
            "ObjectLink protected owner schema",
        ));
    }
    let request = cmd::parse(request_raw)?;
    if cmd::text(&request, "schema_version")? != "tos_local_object_link_command_v1" {
        return Err(SourceCommandError::Invalid("ObjectLink command schema"));
    }
    let operation = cmd::text(&request, "operation")?;
    if !matches!(
        operation,
        "describe" | "prepare-create" | "object.link.create" | "object.link.recover"
    ) {
        return Err(SourceCommandError::Unsupported("ObjectLink operation"));
    }
    if matches!(operation, "describe" | "prepare-create")
        && original_revision != current.current().revision()
    {
        return Err(SourceCommandError::Conflict(
            "ObjectLink information requires current source cut",
        ));
    }
    components
        .members()
        .try_fold(0u64, |n, m| n.checked_add(m.size_bytes))
        .filter(|n| *n <= 33_554_432)
        .ok_or(SourceCommandError::Invalid(
            "ObjectLink selected software budget",
        ))?;
    let mut files = Vec::new();
    for member in components.members() {
        let raw = software
            .read_selected_component(components, &member.path, 2_097_152, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("ObjectLink selected software component"))?;
        files.push(cmd::SourceFile {
            path: member.path.clone(),
            raw,
        });
    }
    let mut ctx = cmd::CommandContext {
        base_revision: original.current().revision(),
        configuration_raw,
        request_raw: request_raw.to_vec(),
        recorded_at: crate::source_serialization::instant()?,
        effective_uid: u64::from(rustix::process::getuid().as_raw()),
        files,
    };
    let mut authored =
        crate::source_claims::complete_authored_inputs(&ctx, &original, deadline, cancelled)?;
    authored.append(&mut ctx.files);
    ctx.files = authored;
    let limits = tos_validation::item_rules::ItemLimits {
        max_member_bytes: 2_097_152,
        max_total_bytes: 134_217_728,
        max_state_bytes: 134_217_728,
        max_issues: 256,
        deadline,
    };
    if operation == "describe" {
        cmd::exact_keys(&request, &["schema_version", "operation"])?;
        ctx.check_from_selected_captures(&original, software, components, deadline, cancelled)?;
        return envelope(json(&owner::object_link_result_fields(
            &fs, &ctx, current, limits, cancelled,
        )?)?);
    }
    let claim_path = cmd::text(&config, "claim_source_path")?;
    let receipt_path = format!(
        "{}/object-link-creation-receipt.json",
        claim_path
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("ObjectLink receipt parent"))?
            .0
    );
    let replay = operation == "object.link.create"
        && current
            .current()
            .member(
                &tos_foundation::RelativePath::parse(&receipt_path)
                    .map_err(|_| SourceCommandError::Invalid("ObjectLink receipt locator"))?,
            )
            .is_some();
    let selected = if replay { current } else { &original };
    let mut worker = selected_schema(invocation, selected, deadline, cancelled)?;
    if operation == "prepare-create" {
        let prepared = owner::prepare_isolated_object_link_from_proposal(
            &fs,
            &ctx,
            &original,
            software,
            components,
            &mut worker,
            limits,
            cancelled,
        )?;
        let mut result = json(&owner::object_link_result_fields(
            &fs, &ctx, current, limits, cancelled,
        )?)?;
        let receipt = json(prepared.receipt())?;
        let request = json(prepared.request())?;
        result["prepared_link"] = receipt["link"].clone();
        result["prepared_claim"] = receipt["claim"].clone();
        result["prepared_forms"] = receipt["forms"].clone();
        result["prepared_materializations"] = json(prepared.materializations())?;
        result["expected_publication"] = request["expected_publication"].clone();
        result["expected_dependencies"] = request["expected_dependencies"].clone();
        return envelope(result);
    }
    let pending = owner::work_transaction::read_pending(&fs, deadline, cancelled)?.is_some();
    let outcome = if operation == "object.link.recover" || pending {
        let decision = if operation == "object.link.recover" {
            match cmd::text(&request, "decision")? {
                "resume" => owner::ObjectLinkRecoveryDecision::Resume,
                "rollback" => owner::ObjectLinkRecoveryDecision::Rollback,
                _ => return Err(SourceCommandError::Invalid("ObjectLink recovery decision")),
            }
        } else {
            owner::ObjectLinkRecoveryDecision::Resume
        };
        owner::recover_isolated_object_link_from_captures(
            &fs,
            &ctx,
            &original,
            software,
            components,
            &mut worker,
            decision,
            limits,
            cancelled,
        )?
    } else if replay {
        owner::replay_isolated_object_link_from_captures(
            &fs,
            &ctx,
            &original,
            current,
            software,
            components,
            &mut worker,
            limits,
            cancelled,
        )?
    } else {
        owner::execute_isolated_object_link_from_captures(
            &fs,
            &ctx,
            &original,
            software,
            components,
            &mut worker,
            limits,
            cancelled,
        )?
    };
    // Fresh selection reads only the current profile/control files; it never
    // mistakes the prepublication cut for the newly committed source membership.
    let profiles = owner::object_link_result_fields(&fs, &ctx, current, limits, cancelled)?;
    let mut result = json(&profiles)?;
    result["receipt"] = outcome
        .receipt()
        .map(json)
        .transpose()?
        .unwrap_or(Value::Null);
    result["replayed"] = Value::Bool(outcome.replayed());
    result["materializations"] = outcome
        .materializations()
        .map(json)
        .transpose()?
        .unwrap_or(Value::Null);
    if operation == "object.link.recover" || pending {
        result["recovery"] = json(outcome.publication())?;
    }
    envelope(result)
}
