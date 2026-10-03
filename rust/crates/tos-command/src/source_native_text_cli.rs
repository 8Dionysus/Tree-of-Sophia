//! Explicit protected TextUnit/TextLayer callers of the existing whole engines.
//! Original cut identity remains explicit evidence; current validation and the
//! engines' retained-package replay govern every publication and retry.
use super::{absolute, capped, digest, selected_schema, text};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_text_layer_derived_entry as derived;
use crate::source_text_layer_entry as initial;
use crate::source_text_owner::{
    OwnerTextContext, OwnerTextDerivedSelection, OwnerTextInitialLayerSelection,
    OwnerTextUnitSelection, read_absolute,
};
use crate::source_text_unit_entry as unit;
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath, SourceRevision};
use tos_source_store::{
    CorpusCutReader, CorpusReader, CutReadLimits, SoftwareCaptureReader,
    SoftwareComponentSelectionV1,
};
use tos_validation::source_cut::CutSchemaExecutor;

fn value(raw: &JsonValue) -> SourceCommandResult<Value> {
    serde_json::from_slice(&cmd::canonical(raw)?)
        .map_err(|_| SourceCommandError::Invalid("native Text response JSON"))
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
    if invocation.get("assessment_schema_worker") != Some(&Value::Null) {
        return Err(SourceCommandError::Denied(
            "Text invocation assessment worker",
        ));
    }
    let original_revision = SourceRevision(digest(text(invocation, "original_source_revision")?)?);
    let budgets = &invocation["budgets"];
    let _original = store
        .open_source_cut(
            original_revision,
            CutReadLimits {
                max_revisions: usize::try_from(capped(budgets, "max_revisions", 4)?)
                    .map_err(|_| SourceCommandError::Invalid("Text original revision budget"))?,
                max_members: capped(budgets, "max_members", 2048)?,
                max_total_bytes: capped(budgets, "max_total_bytes", 33_554_432)?,
                max_member_bytes: capped(budgets, "max_member_bytes", 8_388_608)?,
            },
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("Text original source cut"))?;
    let context_path = absolute(text(invocation, "owner_context")?)?;
    let grant_path = absolute(text(invocation, "owner_config")?)?;
    // This read chooses only a fixed owner family. Its full protected selection
    // below independently rereads and validates the grant; these bytes grant nothing.
    let raw = read_absolute(
        &grant_path,
        rustix::process::getuid().as_raw(),
        true,
        1_048_576,
        deadline,
        cancelled,
    )?;
    let config_hint = cmd::parse(&raw)?;
    let family = cmd::text(&config_hint, "schema_version")?;
    let is_unit = matches!(
        family,
        "tos_local_text_unit_create_owner_v1" | "tos_local_text_unit_create_owner_v2"
    );
    let is_initial = family == "tos_local_text_layer_create_owner_v1";
    if !is_unit
        && !is_initial
        && !matches!(
            family,
            "tos_local_text_layer_derive_owner_v1"
                | "tos_local_text_layer_record_owner_ocr_v1"
                | "tos_local_text_layer_record_owner_page_ocr_v1"
        )
    {
        return Err(SourceCommandError::Denied("native Text owner family"));
    }
    let request = cmd::parse(request_raw)?;
    let operation = cmd::text(&request, "operation")?;
    let mut worker = selected_schema(invocation, current, deadline, cancelled)?;
    let schema_path = RelativePath::parse("ToS/contracts/owner-local-source-context.schema.json")
        .map_err(|_| SourceCommandError::Invalid("Text context schema path"))?;
    let schema = current
        .read_member(
            current.current().revision(),
            &schema_path,
            1_048_576,
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("Text context schema selection"))?;
    let (context, _) =
        OwnerTextContext::select(&context_path, &schema.raw, &mut worker, deadline, cancelled)?;
    let (config, owner_configuration, delegated_operation) = if is_unit {
        let grant = OwnerTextUnitSelection::select(&context, &grant_path, deadline, cancelled)?;
        let contracts = unit::selected_contracts(
            &context,
            &worker,
            cmd::text(&grant.config, "schema_version")? == "tos_local_text_unit_create_owner_v1",
            deadline,
            cancelled,
        )?;
        let selected = unit::configuration(&context, &grant, &contracts, deadline, cancelled)?;
        (grant.config, selected, "text-unit.create".to_owned())
    } else if is_initial {
        let grant =
            OwnerTextInitialLayerSelection::select(&context, &grant_path, deadline, cancelled)?;
        let contracts = initial::selected_contracts(&context, &worker, deadline, cancelled)?;
        let selected =
            initial::selected_configuration(&context, &grant, &contracts, deadline, cancelled)?;
        (grant.config, selected, "text-layer.create".to_owned())
    } else {
        let grant = OwnerTextDerivedSelection::select(&context, &grant_path, deadline, cancelled)?;
        let contracts = derived::contracts(&context, &worker, deadline, cancelled)?;
        let selected =
            derived::selected_configuration(&context, &grant, &contracts, deadline, cancelled)?;
        (grant.config, selected, grant.operation)
    };
    if cmd::text(&config, "schema_version")? != family {
        return Err(SourceCommandError::Conflict("Text owner family changed"));
    }
    if operation != "describe" && operation != "prepare-create" && operation != delegated_operation
    {
        return Err(SourceCommandError::Denied(
            "Text independently selected operation",
        ));
    }
    let path = cmd::text(&config, "source_path")?;
    let target = context.private_new_package_target(path)?;
    let target_exists = match std::fs::symlink_metadata(&target) {
        Ok(_) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => return Err(SourceCommandError::Denied("Text target observation")),
    };
    let result_profile = if is_unit {
        if family.ends_with("v2") {
            "tos_local_text_unit_create_result_v2"
        } else {
            "tos_local_text_unit_create_result_v1"
        }
    } else if is_initial {
        "tos_local_text_layer_create_result_v1"
    } else {
        "tos_local_text_layer_derive_result_v1"
    };
    let mut response = json!({"schema_version":result_profile,
        "authentication":"local-unix-account", "owner_configuration":owner_configuration,
        "target_exists":target_exists,"supported_operations":[delegated_operation],
        "command_operations":["describe","prepare-create",delegated_operation],
        "expected_source":null,"expected_revision":null,"receipt_sha256":null,
        "replayed":false,"grants_admission":false,"content_disclosure":"withheld"});
    if is_unit && family.ends_with("v1") {
        let config_value = value(&config)?;
        for field in [
            "source_path",
            "packet_id",
            "allowed_operations",
            "allowed_text_scope",
            "unit_slots",
            "gap_anchor_refs",
        ] {
            response[field] = config_value[field].clone();
        }
        response["proposal_fields"] = json!({"spans":["unit_id","start","end","certainty","status_reason"],
            "excluded_gaps":["anchor_ref","start","end"]});
        response["position_unit"] = json!("unicode_code_point");
        response["interval"] = json!("half_open");
        response["record_schema_ref"] =
            json!("ToS/contracts/source-text-unit-packet-v1.schema.json");
        response["receipt"] = Value::Null;
        response["content_disclosure"] = json!("owner_local_only");
        response["replay_input_posture"] = Value::Null;
        response
            .as_object_mut()
            .expect("object response")
            .remove("receipt_sha256");
    }
    if is_unit && family.ends_with("v2") {
        response["input_mode"] = json!("exact_layer_first_segmentation");
    }
    if operation == "describe" {
        cmd::exact_keys(&request, &["operation"])?;
        worker
            .finish(deadline, cancelled)
            .map_err(|_| SourceCommandError::Denied("Text describe worker FINAL"))?;
        // FINAL does not freeze the protected delegation. Reselect it at the
        // disclosure boundary and recompute the same owner configuration with
        // the existing contract/current-context helpers, without private bytes.
        let current_configuration = if is_unit {
            let grant = OwnerTextUnitSelection::select(&context, &grant_path, deadline, cancelled)?;
            let contracts = unit::selected_contracts(
                &context,
                &worker,
                cmd::text(&grant.config, "schema_version")?
                    == "tos_local_text_unit_create_owner_v1",
                deadline,
                cancelled,
            )?;
            unit::configuration(&context, &grant, &contracts, deadline, cancelled)?
        } else if is_initial {
            let grant =
                OwnerTextInitialLayerSelection::select(&context, &grant_path, deadline, cancelled)?;
            let contracts = initial::selected_contracts(&context, &worker, deadline, cancelled)?;
            initial::selected_configuration(&context, &grant, &contracts, deadline, cancelled)?
        } else {
            let grant =
                OwnerTextDerivedSelection::select(&context, &grant_path, deadline, cancelled)?;
            let contracts = derived::contracts(&context, &worker, deadline, cancelled)?;
            derived::selected_configuration(&context, &grant, &contracts, deadline, cancelled)?
        };
        if current_configuration != owner_configuration {
            return Err(SourceCommandError::Conflict(
                "Text describe owner changed before disclosure",
            ));
        }
    } else if operation == "prepare-create" {
        let (configuration, dependencies) = if is_unit {
            let prepared = unit::prepare_first_text_unit_from_captures(
                &context_path,
                &grant_path,
                &request,
                current,
                software,
                components,
                &mut worker,
                deadline,
                cancelled,
            )?;
            if family.ends_with("v1") {
                response["prepared_source"] = json!(prepared.source_path);
                response["prepared_files"] = value(&prepared.prepared_files)?;
            }
            response["capture_at_apply"] = json!([
                "source-create-request.json",
                "source-create-environment.json",
                "source-create-provenance.jsonl"
            ]);
            (prepared.owner_configuration, prepared.expected_dependencies)
        } else if is_initial {
            cmd::exact_keys(&request, &["operation"])?;
            let prepared = initial::prepare_initial_text_layer_from_captures(
                &context_path,
                &grant_path,
                current,
                software,
                components,
                &mut worker,
                deadline,
                cancelled,
            )?;
            (prepared.owner_configuration, prepared.expected_dependencies)
        } else {
            cmd::exact_keys(&request, &["operation"])?;
            let prepared = derived::prepare_derived_text_layer_from_captures(
                &context_path,
                &grant_path,
                current,
                software,
                components,
                &mut worker,
                deadline,
                cancelled,
            )?;
            (prepared.owner_configuration, prepared.expected_dependencies)
        };
        response["owner_configuration"] = json!(configuration);
        response["expected_dependencies"] = json!(dependencies);
    } else {
        let (receipt, replayed) = if is_unit {
            let result = unit::execute_first_text_unit_from_captures(
                &context_path,
                &grant_path,
                &request,
                current,
                software,
                components,
                &mut worker,
                deadline,
                cancelled,
            )?;
            (result.receipt, result.replayed)
        } else if is_initial {
            let result = initial::execute_initial_text_layer_from_captures(
                &context_path,
                &grant_path,
                &request,
                current,
                software,
                components,
                &mut worker,
                deadline,
                cancelled,
            )?;
            (result.receipt, result.replayed)
        } else {
            let result = derived::execute_derived_text_layer_from_captures(
                &context_path,
                &grant_path,
                &request,
                current,
                software,
                components,
                &mut worker,
                deadline,
                cancelled,
            )?;
            (result.receipt, result.replayed)
        };
        response["target_exists"] = json!(true);
        response["replayed"] = json!(replayed);
        response["receipt_sha256"] =
            json!(Digest256::of_bytes(&cmd::canonical(&receipt)?).to_prefixed());
        if is_unit && family.ends_with("v1") {
            response["receipt"] = value(&receipt)?;
            response["replay_input_posture"] = if replayed {
                json!("historical_request_current_validation")
            } else {
                Value::Null
            };
            response
                .as_object_mut()
                .expect("object response")
                .remove("receipt_sha256");
        }
    }
    Ok(json!({"schema_version":"tos_local_native_source_result_v1",
        "authentication":"local-unix-account","result":response,"grants_admission":false}))
}
