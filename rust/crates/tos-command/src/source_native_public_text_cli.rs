//! Explicit public project Text invocation. Authenticated cut/producer bytes
//! select schemas and implementation; only the protected public grant routes
//! construction. No owner-local context or assessment worker is manufactured.
use super::{absolute, capped, digest, selected_schema, text};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_public_text_owner::PublicNativeTextSelection;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{RelativePath, SourceRevision};
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
    if !invocation["owner_context"].is_null() || !invocation["assessment_schema_worker"].is_null() {
        return Err(SourceCommandError::Denied(
            "public Text requires independent public grant profile",
        ));
    }
    let original_revision = SourceRevision(digest(text(invocation, "original_source_revision")?)?);
    let budgets = &invocation["budgets"];
    let original = store
        .open_source_cut(
            original_revision,
            CutReadLimits {
                max_revisions: usize::try_from(capped(budgets, "max_revisions", 4)?).map_err(
                    |_| SourceCommandError::Invalid("public Text original revision budget"),
                )?,
                max_members: capped(budgets, "max_members", 2048)?,
                max_total_bytes: capped(budgets, "max_total_bytes", 33_554_432)?,
                max_member_bytes: capped(budgets, "max_member_bytes", 8_388_608)?,
            },
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("public Text original source cut"))?;
    let request = cmd::parse(request_raw)?;
    let operation = cmd::text(&request, "operation")?;
    let keys = match operation {
        "describe" | "prepare-create" => vec!["schema_version", "operation"],
        "inspect-recovery" => vec!["schema_version", "operation", "command_id"],
        "native-text.create" => vec![
            "schema_version",
            "operation",
            "command_id",
            "expected_configuration",
            "expected_dependencies",
            "expected_source",
            "expected_revision",
        ],
        _ => {
            return Err(SourceCommandError::Unsupported(
                "public Text native operation",
            ));
        }
    };
    cmd::exact_keys(&request, &keys)?;
    if cmd::text(&request, "schema_version")? != "tos_local_source_command_v1" {
        return Err(SourceCommandError::Invalid("public Text request schema"));
    }
    if operation == "prepare-create" && original_revision != current.current().revision() {
        return Err(SourceCommandError::Conflict(
            "public Text preview requires current original cut",
        ));
    }
    let mut worker = selected_schema(invocation, &original, deadline, cancelled)?;
    let mut schemas = BTreeMap::new();
    for name in [
        "ToS/contracts/public-native-text-create-owner.schema.json",
        "ToS/contracts/public-native-text-authority.schema.json",
        "ToS/contracts/source-text-unit-packet-v1.schema.json",
    ] {
        let path = RelativePath::parse(name)
            .map_err(|_| SourceCommandError::Invalid("public Text schema path"))?;
        let raw = original
            .read_member(
                original.current().revision(),
                &path,
                1_048_576,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("public Text selected original schema"))?
            .raw;
        schemas.insert(name.to_owned(), raw);
    }
    let selection = PublicNativeTextSelection::select(
        &absolute(text(invocation, "owner_config")?)?,
        &schemas,
        &mut worker,
        deadline,
        cancelled,
    )?;
    let response = crate::source_public_text_entry::run(
        &selection,
        &request,
        software,
        components,
        &mut worker,
        deadline,
        cancelled,
    )?;
    let response: Value = serde_json::from_slice(&cmd::canonical(&response)?)
        .map_err(|_| SourceCommandError::Invalid("public Text native response"))?;
    Ok(
        json!({"schema_version":"tos_local_native_source_result_v1","authentication":"local-unix-account","result":response,"grants_admission":false}),
    )
}
