//! Explicit Collection owner dispatch under the common protected invocation.
use super::{absolute, capped, digest, selected_schema, text};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{self as owner, CollectionRecoveryDecision, CreationFilesystem};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{JsonValue, SourceRevision};
use tos_source_store::{
    CorpusCutReader, CorpusReader, CutReadLimits, SoftwareCaptureReader,
    SoftwareComponentSelectionV1,
};
use tos_validation::item_rules::ItemLimits;
use tos_validation::source_cut::CutSchemaExecutor;

fn value(body: &JsonValue) -> SourceCommandResult<Value> {
    serde_json::from_slice(&cmd::canonical(body)?)
        .map_err(|_| SourceCommandError::Invalid("Collection CLI result conversion"))
}
fn extend(response: &mut Value, fields: JsonValue) -> SourceCommandResult<()> {
    let fields = value(&fields)?;
    let object = fields
        .as_object()
        .ok_or(SourceCommandError::Invalid("Collection result fields"))?;
    response
        .as_object_mut()
        .ok_or(SourceCommandError::Invalid("Collection result object"))?
        .extend(
            object
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );
    Ok(())
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
    let (filesystem, configuration_raw) = CreationFilesystem::select_collection_owner(
        &absolute(text(invocation, "owner_config")?)?,
        deadline,
        cancelled,
    )?;
    let configuration = cmd::parse(&configuration_raw)?;
    let request = cmd::parse(request_raw)?;
    let operation = cmd::text(&request, "operation")?;
    let original_revision =
        invocation
            .get("original_source_revision")
            .ok_or(SourceCommandError::Invalid(
                "Collection original cut selection",
            ))?;
    let original = if original_revision.is_null() {
        None
    } else {
        let revision = SourceRevision(digest(
            original_revision
                .as_str()
                .ok_or(SourceCommandError::Invalid("Collection original revision"))?,
        )?);
        let budgets = &invocation["budgets"];
        Some(
            store
                .open_source_cut(
                    revision,
                    CutReadLimits {
                        max_revisions: usize::try_from(capped(budgets, "max_revisions", 4)?)
                            .map_err(|_| {
                                SourceCommandError::Invalid("Collection original revision count")
                            })?,
                        max_members: capped(budgets, "max_members", 2048)?,
                        max_total_bytes: capped(budgets, "max_total_bytes", 33_554_432)?,
                        max_member_bytes: capped(budgets, "max_member_bytes", 8_388_608)?,
                    },
                    deadline,
                    cancelled,
                )
                .map_err(|_| SourceCommandError::Conflict("Collection original source cut"))?,
        )
    };
    let pending = if matches!(
        operation,
        "collection.work.attach" | "collection.work.recover"
    ) {
        owner::work_transaction::read_pending(&filesystem, deadline, cancelled)?.is_some()
    } else {
        false
    };
    let selected = if operation == "collection.work.recover" || pending {
        original.as_ref().unwrap_or(current)
    } else {
        current
    };
    let mut worker = selected_schema(invocation, selected, deadline, cancelled)?;
    components
        .members()
        .try_fold(0u64, |sum, member| sum.checked_add(member.size_bytes))
        .filter(|sum| *sum <= 33_554_432)
        .ok_or(SourceCommandError::Invalid(
            "Collection software subset budget",
        ))?;
    let mut files = Vec::with_capacity(components.members().count());
    for member in components.members() {
        let raw = software
            .read_selected_component(components, &member.path, 2_097_152, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("Collection selected software component"))?;
        files.push(cmd::SourceFile {
            path: member.path.clone(),
            raw,
        });
    }
    let mut context = cmd::CommandContext {
        base_revision: selected.current().revision(),
        configuration_raw,
        request_raw: request_raw.to_vec(),
        recorded_at: crate::source_serialization::instant()?,
        effective_uid: u64::from(rustix::process::getuid().as_raw()),
        files,
    };
    let limits = ItemLimits {
        max_member_bytes: 2_097_152,
        max_total_bytes: 134_217_728,
        max_state_bytes: 134_217_728,
        max_issues: 256,
        deadline,
    };
    let mut response = json!({"schema_version":"tos_collection_membership_result_v1",
        "authentication":"local-unix-account","owner_configuration":cmd::record_digest(&configuration)?.to_prefixed(),
        "operation":"collection.work.attach","command_operations":["describe","prepare-attach","collection.work.attach","collection.work.recover"],
        "allowed_operations":value(cmd::field(&configuration,"allowed_operations")?)?,
        "collection_source_path":cmd::text(&configuration,"collection_source_path")?,
        "claim_source_path":cmd::text(&configuration,"claim_source_path")?,
        "receipt":null,"replayed":false,"recovery":null,"materializations":null,"grants_admission":false});
    if operation == "describe" {
        cmd::exact_keys(&request, &["schema_version", "operation"])?;
        if cmd::text(&request, "schema_version")? != "tos_local_collection_membership_command_v1" {
            return Err(SourceCommandError::Invalid(
                "Collection describe request schema",
            ));
        }
        context.check_from_selected_captures(current, software, components, deadline, cancelled)?;
        worker
            .finish(deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("Collection describe worker finish"))?;
        extend(
            &mut response,
            owner::collection_result_fields(
                &filesystem,
                &context,
                current,
                limits,
                cancelled,
                false,
            )?,
        )?;
        return Ok(response);
    }
    if operation == "prepare-attach" {
        if original.is_some() {
            return Err(SourceCommandError::Invalid(
                "Collection preview cannot select replay cut",
            ));
        }
        let prepared = owner::prepare_isolated_collection_membership_from_proposal(
            &filesystem,
            &context,
            current,
            software,
            components,
            &mut worker,
            limits,
            cancelled,
        )?;
        extend(
            &mut response,
            owner::collection_result_fields(
                &filesystem,
                &context,
                current,
                limits,
                cancelled,
                false,
            )?,
        )?;
        extend(&mut response, prepared.result_fields(&configuration)?)?;
        return Ok(response);
    }
    let publication = if operation == "collection.work.recover" {
        let decision = match cmd::text(&request, "decision")? {
            "resume" => CollectionRecoveryDecision::Resume,
            "rollback" => CollectionRecoveryDecision::Rollback,
            _ => return Err(SourceCommandError::Invalid("Collection recovery decision")),
        };
        owner::recover_isolated_collection_membership_from_captures(
            &filesystem,
            &context,
            selected,
            software,
            components,
            &mut worker,
            decision,
            limits,
            cancelled,
        )?
    } else if operation == "collection.work.attach" {
        if let Some(original) = original.as_ref().filter(|_| !pending) {
            context.base_revision = original.current().revision();
            owner::replay_isolated_collection_membership_from_captures(
                &filesystem,
                &context,
                original,
                current,
                software,
                components,
                &mut worker,
                limits,
                cancelled,
            )?
        } else {
            owner::execute_isolated_collection_membership_from_captures(
                &filesystem,
                &context,
                selected,
                software,
                components,
                &mut worker,
                limits,
                cancelled,
            )?
        }
    } else {
        return Err(SourceCommandError::Unsupported(
            "Collection native operation",
        ));
    };
    extend(
        &mut response,
        owner::collection_result_fields(
            &filesystem,
            &context,
            current,
            limits,
            cancelled,
            !publication.replayed() && publication.receipt() != &JsonValue::Null,
        )?,
    )?;
    let object = response
        .as_object_mut()
        .ok_or(SourceCommandError::Invalid("Collection result object"))?;
    object.insert("receipt".into(), value(publication.receipt())?);
    object.insert("replayed".into(), json!(publication.replayed()));
    object.insert("transaction_id".into(), json!(publication.transaction_id()));
    if operation == "collection.work.recover" {
        object.insert(
            "recovery".into(),
            json!({
                "transaction_id": publication.transaction_id(),
                "manifest_sha256": publication.manifest_sha256(),
                "publication": value(publication.publication())?,
                "committed": cmd::text(&request, "decision")? == "resume",
            }),
        );
    }
    Ok(response)
}
