//! Actual isolated Forms caller; protected config selects the fixed owner.
use super::{absolute, capped, digest, selected_schema, text};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{CreationFilesystem, forms_publication};
use serde_json::{Value, json};
use std::collections::BTreeSet;
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
    if invocation.get("owner_context") != Some(&Value::Null)
        || invocation.get("assessment_schema_worker") != Some(&Value::Null)
    {
        return Err(SourceCommandError::Denied(
            "Forms invocation unused protected selectors",
        ));
    }
    let budgets = &invocation["budgets"];
    let original_revision = SourceRevision(digest(text(invocation, "original_source_revision")?)?);
    let original = store
        .open_source_cut(
            original_revision,
            CutReadLimits {
                max_revisions: usize::try_from(capped(budgets, "max_revisions", 4)?)
                    .map_err(|_| SourceCommandError::Invalid("Forms original revisions budget"))?,
                max_members: capped(budgets, "max_members", 2048)?,
                max_total_bytes: capped(budgets, "max_total_bytes", 33_554_432)?,
                max_member_bytes: capped(budgets, "max_member_bytes", 8_388_608)?,
            },
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("Forms original selected cut"))?;
    let (filesystem, configuration_raw) = CreationFilesystem::select_protected_native_owner(
        &absolute(text(invocation, "owner_config")?)?,
        deadline,
        cancelled,
    )?;
    let configuration = cmd::parse(&configuration_raw)?;
    if !matches!(
        cmd::text(&configuration, "schema_version")?,
        "tos_local_source_command_owner_v1"
            | "tos_local_canonical_form_owner_v1"
            | "tos_local_claim_form_owner_v1"
            | "tos_local_claim_form_owner_v2"
    ) {
        return Err(SourceCommandError::Denied("Forms exact protected family"));
    }
    let pending = forms_publication::pending(&filesystem, deadline, cancelled)?;
    let selected = if pending { &original } else { current };
    let mut files = Vec::new();
    let mut paths = BTreeSet::new();
    let mut authored_total = 0u64;
    for member in selected.current().members() {
        if !member.path.as_str().starts_with("ToS/") {
            continue;
        }
        authored_total = authored_total
            .checked_add(member.size_bytes)
            .filter(|n| *n <= 33_554_432)
            .ok_or(SourceCommandError::Invalid("Forms authored byte budget"))?;
        if !paths.insert(member.path.clone()) {
            return Err(SourceCommandError::Invalid("Forms duplicate selected path"));
        }
        let raw = selected
            .read_member(
                selected.current().revision(),
                &member.path,
                8_388_608,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("Forms authored member read"))?
            .raw;
        files.push(cmd::SourceFile {
            path: member.path.clone(),
            raw,
        });
    }
    let mut software_total = 0u64;
    for member in components.members() {
        if member.path.as_str().starts_with("ToS/") || !paths.insert(member.path.clone()) {
            return Err(SourceCommandError::Denied(
                "Forms software and authored namespaces overlap",
            ));
        }
        if authored_total
            .checked_add(software_total)
            .and_then(|n| n.checked_add(member.size_bytes))
            .is_none_or(|n| n > 33_554_432)
        {
            return Err(SourceCommandError::Invalid(
                "Forms combined selected context budget",
            ));
        }
        software_total = software_total
            .checked_add(member.size_bytes)
            .filter(|n| *n <= 33_554_432)
            .ok_or(SourceCommandError::Invalid("Forms software byte budget"))?;
        let raw = software
            .read_selected_component(components, &member.path, 8_388_608, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("Forms software component read"))?;
        files.push(cmd::SourceFile {
            path: member.path.clone(),
            raw,
        });
    }
    let context = cmd::CommandContext {
        base_revision: selected.current().revision(),
        configuration_raw,
        request_raw: request_raw.to_vec(),
        recorded_at: crate::source_serialization::instant()?,
        effective_uid: u64::from(rustix::process::getuid().as_raw()),
        files,
    };
    let mut worker = selected_schema(invocation, selected, deadline, cancelled)?;
    let proposal = forms_publication::run(
        &filesystem,
        &context,
        selected,
        software,
        components,
        &mut worker,
        deadline,
        cancelled,
    )?;
    let result: Value = serde_json::from_slice(&cmd::canonical(&proposal.response)?)
        .map_err(|_| SourceCommandError::Invalid("Forms response JSON"))?;
    Ok(json!({"schema_version":"tos_local_native_source_result_v1",
        "authentication":"local-unix-account","result":result,"grants_admission":false}))
}
