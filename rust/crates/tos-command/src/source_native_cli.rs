//! Explicit local invocation of the native Alignment owner route. The selected
//! cut, software capture and worker image are byte evidence, never a grant.

use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_serialization::executable;
use crate::source_text_alignment_entry::{
    NativeAlignmentRecovery, describe_owner_alignment_selected,
    execute_owner_alignment_from_captures, inspect_owner_alignment_recovery_selected,
    inspect_owner_alignment_selected, prepare_owner_alignment_from_captures,
    selected_owner_cli_profile,
};
use crate::source_text_owner::{normalized_absolute, read_absolute};
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tos_foundation::{Digest256, JsonLimits, RelativePath, SourceRevision};
use tos_source_store::{
    CorpusReader, CutReadLimits, ReadLimits, SoftwareCaptureReader, SoftwareCaptureSelectionV1,
};
use tos_validation::FormatProfile;
use tos_validation::executor::{ExactWorkerIdentity, ExecutorBudget};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerLimits, CutWorkerSchemaExecutor};

const MAX_INVOCATION: usize = 1_048_576;
const MAX_REQUEST: usize = 1_048_576;

fn text<'a>(value: &'a Value, key: &str) -> SourceCommandResult<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or(SourceCommandError::Invalid("native invocation field"))
}

fn number(value: &Value, key: &str) -> SourceCommandResult<u64> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .filter(|n| *n > 0)
        .ok_or(SourceCommandError::Invalid("native invocation budget"))
}

fn capped(value: &Value, key: &str, maximum: u64) -> SourceCommandResult<u64> {
    let selected = number(value, key)?;
    if selected > maximum {
        return Err(SourceCommandError::Invalid(
            "native invocation budget exceeds selected profile",
        ));
    }
    Ok(selected)
}

fn digest(value: &str) -> SourceCommandResult<Digest256> {
    Digest256::from_prefixed(value)
        .map_err(|_| SourceCommandError::Invalid("native invocation digest"))
}

fn absolute(value: &str) -> SourceCommandResult<std::path::PathBuf> {
    normalized_absolute(value)
}

fn exact(value: &Value, keys: &[&str]) -> SourceCommandResult<()> {
    let object = value
        .as_object()
        .ok_or(SourceCommandError::Invalid("native invocation object"))?;
    if object.len() != keys.len() || object.keys().any(|key| !keys.contains(&key.as_str())) {
        return Err(SourceCommandError::Invalid("native invocation fields"));
    }
    Ok(())
}

fn bounded_read(mut input: impl Read, max: usize) -> SourceCommandResult<Vec<u8>> {
    let mut raw = Vec::new();
    input
        .take((max + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| SourceCommandError::Invalid("native command input read"))?;
    if raw.len() > max {
        return Err(SourceCommandError::Invalid("native command input budget"));
    }
    Ok(raw)
}

/// Complete one explicitly selected local invocation. Both protected owner
/// files are independently reread by the Alignment entry at current use.
pub fn run(invocation_path: &Path, input: impl Read) -> SourceCommandResult<Value> {
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(60);
    let uid = rustix::process::getuid().as_raw();
    if rustix::process::geteuid().as_raw() != uid {
        return Err(SourceCommandError::Denied("native command setuid refused"));
    }
    let selected = normalized_absolute(
        invocation_path
            .to_str()
            .ok_or(SourceCommandError::Invalid("native invocation path UTF-8"))?,
    )?;
    let raw = read_absolute(&selected, uid, true, MAX_INVOCATION, deadline, &cancelled)?;
    let checked = cmd::parse(&raw)?;
    let invocation: Value = serde_json::from_slice(&cmd::canonical(&checked)?)
        .map_err(|_| SourceCommandError::Invalid("native invocation JSON"))?;
    let item_invocation =
        text(&invocation, "schema_version")? == "tos_local_native_item_invocation_v1";
    let mut keys = vec![
        "schema_version",
        "owner_config",
        "native_executable",
        "native_executable_sha256",
        "corpus_store",
        "source_revision",
        "software_capture",
        "software_restored_root",
        "software_selection",
        "software_components",
        "schema_worker",
        "budgets",
    ];
    keys.push(if item_invocation {
        "original_source_revision"
    } else {
        "owner_context"
    });
    exact(&invocation, &keys)?;
    if !item_invocation
        && text(&invocation, "schema_version")? != "tos_local_native_owner_invocation_v1"
    {
        return Err(SourceCommandError::Invalid("native invocation profile"));
    }
    if executable(deadline, &cancelled)? != digest(text(&invocation, "native_executable_sha256")?)?
    {
        return Err(SourceCommandError::Conflict(
            "native command executable identity",
        ));
    }
    let request_raw = bounded_read(input, MAX_REQUEST)?;
    let request = cmd::parse(&request_raw)?;
    let operation = cmd::text(&request, "operation")?;
    let implemented = if item_invocation {
        matches!(
            operation,
            "describe" | "prepare-create" | "item.adopt" | "item.adoption.recover"
        )
    } else {
        matches!(
            operation,
            "describe"
                | "prepare-create"
                | "prepare-revise"
                | "alignment.create"
                | "alignment.revise"
                | "inspect"
                | "inspect-version"
                | "inspect-recovery"
        )
    };
    if !implemented {
        return Err(SourceCommandError::Unsupported(
            "native owner command operation",
        ));
    }
    let budgets = &invocation["budgets"];
    exact(
        budgets,
        &[
            "max_revisions",
            "max_members",
            "max_total_bytes",
            "max_member_bytes",
            "max_schema_receipts",
            "max_schema_receipt_bytes",
            "worker_cpu_seconds",
            "worker_address_space_bytes",
        ],
    )?;
    let read_limits = ReadLimits {
        max_manifest_bytes: 4_194_304,
        max_manifest_entries: 2048,
        max_selected_object_bytes: 8_388_608,
        json: JsonLimits::default(),
    };
    let store =
        CorpusReader::open_existing(&absolute(text(&invocation, "corpus_store")?)?, read_limits)
            .map_err(|_| SourceCommandError::Conflict("selected corpus store"))?;
    let revision = SourceRevision(digest(text(&invocation, "source_revision")?)?);
    if store
        .select_current()
        .map_err(|_| SourceCommandError::Conflict("corpus current pointer"))?
        .is_some_and(|current| current != revision)
    {
        return Err(SourceCommandError::Conflict(
            "corpus current revision changed",
        ));
    }
    let cut = store
        .open_source_cut(
            revision,
            CutReadLimits {
                max_revisions: usize::try_from(capped(budgets, "max_revisions", 4)?)
                    .map_err(|_| SourceCommandError::Invalid("cut budget"))?,
                max_members: capped(budgets, "max_members", 2048)?,
                max_total_bytes: capped(budgets, "max_total_bytes", 33_554_432)?,
                max_member_bytes: capped(budgets, "max_member_bytes", 8_388_608)?,
            },
            deadline,
            &cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("selected corpus cut"))?;
    let selection = &invocation["software_selection"];
    exact(
        selection,
        &[
            "source_git_commit",
            "source_git_tree",
            "capture_manifest_sha256",
        ],
    )?;
    let software = SoftwareCaptureReader::open(
        &absolute(text(&invocation, "software_capture")?)?,
        &absolute(text(&invocation, "software_restored_root")?)?,
        SoftwareCaptureSelectionV1 {
            source_git_commit: text(selection, "source_git_commit")?.to_owned(),
            source_git_tree: text(selection, "source_git_tree")?.to_owned(),
            capture_manifest_sha256: digest(text(selection, "capture_manifest_sha256")?)?,
        },
        read_limits,
        deadline,
        &cancelled,
    )
    .map_err(|_| SourceCommandError::Conflict("selected software capture"))?;
    let component_values = invocation["software_components"]
        .as_array()
        .filter(|items| !items.is_empty() && items.len() <= 128)
        .ok_or(SourceCommandError::Invalid("selected software components"))?;
    let component_paths = component_values
        .iter()
        .map(|value| {
            RelativePath::parse(
                value
                    .as_str()
                    .ok_or(SourceCommandError::Invalid("selected component path"))?,
            )
            .map_err(|_| SourceCommandError::Invalid("selected component path"))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    let components = software
        .select_components(&component_paths)
        .map_err(|_| SourceCommandError::Conflict("selected software components"))?;
    let worker = &invocation["schema_worker"];
    exact(worker, &["absolute_path", "sha256"])?;
    let mut worker_budget = ExecutorBudget::laboratory();
    worker_budget.execution_wall = deadline.saturating_duration_since(Instant::now());
    worker_budget.cpu_seconds = capped(budgets, "worker_cpu_seconds", 3)?;
    worker_budget.address_space_bytes =
        capped(budgets, "worker_address_space_bytes", 1_073_741_824)?;
    let mut schema = CutWorkerSchemaExecutor::from_cut(
        &cut,
        FormatProfile::LegacyPythonObserved20260923,
        ExactWorkerIdentity {
            absolute_path: absolute(text(worker, "absolute_path")?)?,
            sha256: digest(text(worker, "sha256")?)?,
        },
        worker_budget,
        CutWorkerLimits {
            max_receipts: usize::try_from(capped(budgets, "max_schema_receipts", 128)?)
                .map_err(|_| SourceCommandError::Invalid("schema receipt budget"))?,
            max_receipt_bytes: usize::try_from(capped(
                budgets,
                "max_schema_receipt_bytes",
                262_144,
            )?)
            .map_err(|_| SourceCommandError::Invalid("schema receipt budget"))?,
        },
        deadline,
        &cancelled,
    )
    .map_err(|_| SourceCommandError::Denied("native schema worker"))?;
    if item_invocation {
        return run_item(
            &invocation,
            &request_raw,
            &cut,
            &store,
            &software,
            &components,
            &mut schema,
            deadline,
            &cancelled,
        );
    }
    let context = absolute(text(&invocation, "owner_context")?)?;
    let owner = absolute(text(&invocation, "owner_config")?)?;
    let result = match operation {
        "describe" | "inspect" | "inspect-version" | "inspect-recovery" => {
            let profile = selected_owner_cli_profile(
                &context,
                &owner,
                &cut,
                &mut schema,
                deadline,
                &cancelled,
            )?;
            let prepare = if profile.delegated_operation == "alignment.create" {
                "prepare-create"
            } else {
                "prepare-revise"
            };
            let mut result = json!({"schema_version":"tos_local_native_alignment_result_v1",
                "authentication":"local-unix-account",
                "owner_configuration":profile.owner_configuration,"target_exists":profile.target_exists,
                "expected_source":null,"expected_revision":null,
                "supported_operations":[profile.delegated_operation],
                "command_operations":["describe",prepare,profile.delegated_operation,"inspect","inspect-version","inspect-recovery"],
                "receipt_sha256":null,"replayed":false,"content_disclosure":"withheld",
                "grants_admission":false,"assessment_applied":false,"aligner_executed":false});
            match operation {
                "describe" => {
                    let described = describe_owner_alignment_selected(
                        profile.context,
                        profile.grant,
                        &mut schema,
                        deadline,
                        &cancelled,
                    )?;
                    if described.delegated_operation != profile.delegated_operation
                        || described.target_exists != profile.target_exists
                    {
                        return Err(SourceCommandError::Conflict(
                            "native Alignment description changed",
                        ));
                    }
                }
                "inspect" | "inspect-version" => {
                    let exact = if operation == "inspect-version" {
                        Some(cmd::field(&request, "source")?)
                    } else {
                        None
                    };
                    let inspected = inspect_owner_alignment_selected(
                        profile.context,
                        profile.grant,
                        exact,
                        &mut schema,
                        deadline,
                        &cancelled,
                    )?;
                    let summary: Value =
                        serde_json::from_slice(&cmd::canonical(&inspected.mapping_summary)?)
                            .map_err(|_| SourceCommandError::Invalid("native Alignment summary"))?;
                    let object = result
                        .as_object_mut()
                        .ok_or(SourceCommandError::Invalid("native result object"))?;
                    object.insert(
                        "inspected_source_sha256".into(),
                        json!(inspected.inspected_source_sha256),
                    );
                    object.insert("record_version".into(), json!(inspected.record_version));
                    object.insert("claim_version".into(), json!(inspected.claim_version));
                    object.insert("change_kind".into(), json!(inspected.change_kind));
                    object.insert("history_depth".into(), json!(inspected.history_depth));
                    object.insert(
                        "metadata_verified".into(),
                        json!(inspected.metadata_verified),
                    );
                    object.insert("content_verified".into(), json!(inspected.content_verified));
                    object.insert("mapping_summary".into(), summary);
                    object.insert(
                        "competition_forward_count".into(),
                        json!(inspected.competing_forward_count),
                    );
                    object.insert(
                        "reverse_competition_posture".into(),
                        json!("derived_from_explicit_external_record_refs_not_claimed_complete"),
                    );
                }
                "inspect-recovery" => {
                    let recovery = inspect_owner_alignment_recovery_selected(
                        profile.context,
                        profile.grant,
                        &profile.source_path,
                        cmd::text(&request, "command_id")?,
                        &mut schema,
                        deadline,
                        &cancelled,
                    )?;
                    let state = match recovery {
                        NativeAlignmentRecovery::Absent => "absent",
                        NativeAlignmentRecovery::RetainedExactPlan => "retained_exact_plan",
                        NativeAlignmentRecovery::Committed => "committed",
                    };
                    let object = result
                        .as_object_mut()
                        .ok_or(SourceCommandError::Invalid("native result object"))?;
                    object.insert("recovery_state".into(), json!(state));
                    if state != "absent" {
                        object.insert("recovery_action".into(), json!("retry_exact_original_command; torn_or_foreign_stage_requires_owner_review"));
                    }
                }
                _ => unreachable!(),
            }
            result
        }
        "prepare-create" | "prepare-revise" => {
            let preview = prepare_owner_alignment_from_captures(
                &context,
                &owner,
                &request,
                &cut,
                &software,
                &components,
                &mut schema,
                deadline,
                &cancelled,
            )?;
            json!({"schema_version":"tos_local_native_alignment_result_v1", "authentication":"local-unix-account",
                "owner_configuration":preview.owner_configuration,"target_exists":preview.target_exists,
                "expected_source":null,"expected_revision":null,
                "supported_operations":[if operation == "prepare-create" {"alignment.create"} else {"alignment.revise"}],
                "command_operations":["describe",operation,if operation == "prepare-create" {"alignment.create"} else {"alignment.revise"},"inspect","inspect-version","inspect-recovery"],
                "receipt_sha256":null,"replayed":false,"content_disclosure":"withheld",
                "grants_admission":false,"assessment_applied":false,"aligner_executed":false,
                "expected_dependencies":preview.expected_dependencies})
        }
        "alignment.create" | "alignment.revise" => {
            let done = execute_owner_alignment_from_captures(
                &context,
                &owner,
                &request,
                &cut,
                &software,
                &components,
                &mut schema,
                deadline,
                &cancelled,
            )?;
            let receipt_raw = cmd::canonical(&done.receipt)?;
            json!({"schema_version":"tos_local_native_alignment_result_v1", "authentication":"local-unix-account",
                "owner_configuration":cmd::text(&done.receipt,"owner_configuration")?,
                "target_exists":true,"expected_source":null,"expected_revision":null,
                "supported_operations":[operation],
                "command_operations":["describe",if operation == "alignment.create" {"prepare-create"} else {"prepare-revise"},operation,"inspect","inspect-version","inspect-recovery"],
                "receipt_sha256":Digest256::of_bytes(&receipt_raw).to_prefixed(),"replayed":done.replayed,
                "content_disclosure":"withheld","grants_admission":done.grants_admission,
                "assessment_applied":false,"aligner_executed":done.aligner_executed})
        }
        _ => {
            return Err(SourceCommandError::Unsupported(
                "native owner command operation",
            ));
        }
    };
    Ok(result)
}

fn run_item(
    invocation: &Value,
    request_raw: &[u8],
    cut: &tos_source_store::CorpusCutReader,
    store: &CorpusReader,
    software: &SoftwareCaptureReader,
    components: &tos_source_store::SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    use crate::source_creation_store::{self as owner, CreationFilesystem};
    let path = absolute(text(invocation, "owner_config")?)?;
    let fs = CreationFilesystem::select_item_owner(&path, deadline, cancelled)?;
    let configuration_raw = read_absolute(
        &path,
        rustix::process::getuid().as_raw(),
        true,
        MAX_INVOCATION,
        deadline,
        cancelled,
    )?;
    let config = cmd::parse(&configuration_raw)?;
    let request = cmd::parse(request_raw)?;
    let operation = cmd::text(&request, "operation")?;
    let mut files = Vec::with_capacity(components.members().count());
    for member in components.members() {
        let raw = software
            .read_selected_component(components, &member.path, 2_097_152, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("Item selected software component"))?;
        files.push(cmd::SourceFile {
            path: member.path.clone(),
            raw,
        });
    }
    let mut ctx = cmd::CommandContext {
        base_revision: cut.current().revision(),
        configuration_raw,
        request_raw: request_raw.to_vec(),
        recorded_at: crate::source_serialization::instant()?,
        effective_uid: u64::from(rustix::process::getuid().as_raw()),
        files,
    };
    let limits = tos_validation::item_rules::ItemLimits {
        max_member_bytes: 2_097_152,
        max_total_bytes: 134_217_728,
        max_state_bytes: 134_217_728,
        max_issues: 256,
        deadline,
    };
    let original = invocation
        .get("original_source_revision")
        .ok_or(SourceCommandError::Invalid("Item original cut selection"))?;
    let original_cut = if original.is_null() {
        None
    } else {
        let revision = SourceRevision(digest(
            original
                .as_str()
                .ok_or(SourceCommandError::Invalid("Item original revision"))?,
        )?);
        Some(
            store
                .open_source_cut(
                    revision,
                    CutReadLimits {
                        max_revisions: 4,
                        max_members: 2048,
                        max_total_bytes: 33_554_432,
                        max_member_bytes: 8_388_608,
                    },
                    deadline,
                    cancelled,
                )
                .map_err(|_| SourceCommandError::Conflict("Item original source cut"))?,
        )
    };
    let mut response = json!({"schema_version":"tos_edition_item_result_v1","authentication":"local-unix-account","owner_configuration":cmd::record_digest(&config)?.to_prefixed(),"operation":"item.adopt","command_operations":["describe","prepare-create","item.adopt","item.adoption.recover"],"allowed_operations":serde_json::from_slice::<Value>(&cmd::canonical(cmd::field(&config,"allowed_operations")?)?).map_err(|_|SourceCommandError::Invalid("Item result operations"))?,"edition_source_path":cmd::text(&config,"edition_source_path")?,"item_source_path":cmd::text(&config,"item_source_path")?,"receipt":null,"replayed":false,"grants_admission":false});
    if operation == "prepare-create" {
        if original_cut.is_some() {
            return Err(SourceCommandError::Invalid(
                "Item preview cannot select original replay cut",
            ));
        }
        let prepared = owner::prepare_isolated_item_adoption_from_proposal(
            &fs, &ctx, cut, software, components, worker, limits, cancelled,
        )?;
        let metadata = owner::item_result_fields(&fs, &ctx, cut, limits, cancelled)?;
        let metadata: Value = serde_json::from_slice(&cmd::canonical(&metadata)?)
            .map_err(|_| SourceCommandError::Invalid("Item metadata result"))?;
        for (key, value) in metadata.as_object().unwrap() {
            response
                .as_object_mut()
                .unwrap()
                .insert(key.clone(), value.clone());
        }
        let request: Value = serde_json::from_slice(&cmd::canonical(prepared.request())?)
            .map_err(|_| SourceCommandError::Invalid("Item preview result"))?;
        let object = response.as_object_mut().unwrap();
        for field in [
            "expected_source",
            "expected_revision",
            "expected_dependencies",
            "expected_publication",
            "inventory",
            "inventory_limitation",
            "fixity_verified_at",
        ] {
            object.insert(field.into(), request[field].clone());
        }
        object.insert("prepared_fields".into(), request["fields"].clone());
        let projection: Value =
            serde_json::from_slice(&cmd::canonical(prepared.projected_result())?)
                .map_err(|_| SourceCommandError::Invalid("Item projected result"))?;
        for (key, value) in projection.as_object().unwrap() {
            object.insert(key.clone(), value.clone());
        }
        object.insert("prepared_request".into(), request);
        object.insert("target_exists".into(), json!(false));
        return Ok(response);
    }
    if operation == "describe" {
        cmd::exact_keys(&request, &["schema_version", "operation"])?;
        if cmd::text(&request, "schema_version")? != "tos_local_item_adoption_command_v1" {
            return Err(SourceCommandError::Invalid("Item describe request"));
        }
        fs.current_context(&ctx, deadline, cancelled)?;
        cmd::validate_expiry(
            cmd::text(&config, "expires_at")?,
            &crate::source_serialization::instant()?,
        )?;
        worker.finish(deadline, cancelled).map_err(|reason| {
            SourceCommandError::SchemaExecution {
                path: "item.adopt".into(),
                root: "native Item description".into(),
                reason,
            }
        })?;
        let metadata = owner::item_result_fields(&fs, &ctx, cut, limits, cancelled)?;
        let metadata: Value = serde_json::from_slice(&cmd::canonical(&metadata)?)
            .map_err(|_| SourceCommandError::Invalid("Item description result"))?;
        for (key, value) in metadata.as_object().unwrap() {
            response
                .as_object_mut()
                .unwrap()
                .insert(key.clone(), value.clone());
        }
        return Ok(response);
    }
    let publication = if operation == "item.adoption.recover" {
        let original = original_cut.as_ref().unwrap_or(cut);
        ctx.base_revision = original.current().revision();
        owner::recover_isolated_item_adoption_from_captures(
            &fs, &ctx, original, software, components, worker, limits, cancelled,
        )?
    } else if let Some(original) = original_cut.as_ref() {
        ctx.base_revision = original.current().revision();
        owner::replay_isolated_item_adoption_from_captures(
            &fs, &ctx, original, cut, software, components, worker, limits, cancelled,
        )?
    } else {
        owner::execute_isolated_item_adoption_from_captures(
            &fs, &ctx, cut, software, components, worker, limits, cancelled,
        )?
    };
    let metadata = owner::item_result_fields(&fs, &ctx, cut, limits, cancelled)?;
    let metadata: Value = serde_json::from_slice(&cmd::canonical(&metadata)?)
        .map_err(|_| SourceCommandError::Invalid("Item current result"))?;
    for (key, value) in metadata.as_object().unwrap() {
        response
            .as_object_mut()
            .unwrap()
            .insert(key.clone(), value.clone());
    }
    let object = response.as_object_mut().unwrap();
    object.insert(
        "materializations".into(),
        publication
            .materializations()
            .map_or(Ok(Value::Null), |r| {
                serde_json::from_slice(&cmd::canonical(r)?)
                    .map_err(|_| SourceCommandError::Invalid("Item materialization result"))
            })?,
    );
    object.insert(
        "recovery".into(),
        if operation == "item.adoption.recover" {
            serde_json::from_slice(&cmd::canonical(publication.publication())?)
                .map_err(|_| SourceCommandError::Invalid("Item recovery result"))?
        } else {
            Value::Null
        },
    );
    object.insert("transaction_id".into(), json!(publication.transaction_id()));
    object.insert(
        "receipt".into(),
        publication.receipt().map_or(Ok(Value::Null), |r| {
            serde_json::from_slice(&cmd::canonical(r)?)
                .map_err(|_| SourceCommandError::Invalid("Item receipt result"))
        })?,
    );
    object.insert(
        "deposit".into(),
        serde_json::from_slice(&cmd::canonical(publication.deposit())?)
            .map_err(|_| SourceCommandError::Invalid("Item deposit result"))?,
    );
    object.insert("replayed".into(), json!(publication.replayed()));
    if !publication.metadata_committed() {
        object.insert("next_route".into(),json!("source inventory owner: add a bounded supported profile; explicit rollback retains these bytes"));
    }
    Ok(response)
}
