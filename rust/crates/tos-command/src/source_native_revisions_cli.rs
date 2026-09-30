//! Fixed native caller for the seven maintained record-revision owner schemas.
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
            return Err(SourceCommandError::Denied(
                "Record revision source cut namespace",
            ));
        }
        total = total
            .checked_add(member.size_bytes)
            .filter(|n| *n <= 33_554_432)
            .ok_or(SourceCommandError::Invalid(
                "Record revision selected context byte budget",
            ))?;
        if !paths.insert(member.path.clone()) {
            return Err(SourceCommandError::Invalid(
                "Record revision duplicate selected path",
            ));
        }
        let raw = cut
            .read_member(
                cut.current().revision(),
                &member.path,
                8_388_608,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("Record revision source member read"))?
            .raw;
        files.push(cmd::SourceFile {
            path: member.path.clone(),
            raw,
        });
    }
    for member in components.members() {
        if member.path.as_str().starts_with("ToS/") || !paths.insert(member.path.clone()) {
            return Err(SourceCommandError::Denied(
                "Record revision source/software namespaces overlap",
            ));
        }
        total = total
            .checked_add(member.size_bytes)
            .filter(|n| *n <= 33_554_432)
            .ok_or(SourceCommandError::Invalid(
                "Record revision combined selected context byte budget",
            ))?;
        let raw = software
            .read_selected_component(components, &member.path, 8_388_608, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("Record revision software component read"))?;
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
        .map_err(|_| SourceCommandError::Invalid("Record revision response JSON"))
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
            "record revision unused protected selectors",
        ));
    }
    let (filesystem, configuration_raw) = CreationFilesystem::select_protected_native_owner(
        &absolute(text(invocation, "owner_config")?)?,
        deadline,
        cancelled,
    )?;
    let configuration = cmd::parse(&configuration_raw)?;
    if !matches!(
        cmd::text(&configuration, "schema_version")?,
        "tos_local_source_revision_owner_v1"
            | "tos_local_profile_revision_owner_v1"
            | "tos_local_profile_revision_owner_v2"
            | "tos_local_corpus_revision_owner_v1"
            | "tos_local_corpus_revision_owner_v2"
            | "tos_local_corpus_revision_owner_v3"
            | "tos_local_native_metadata_revision_owner_v1"
    ) {
        return Err(SourceCommandError::Denied(
            "record revision exact protected family",
        ));
    }
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
                                    SourceCommandError::Invalid("record original revision budget")
                                })?,
                            max_members: capped(budgets, "max_members", 2048)?,
                            max_total_bytes: capped(budgets, "max_total_bytes", 33_554_432)?,
                            max_member_bytes: capped(budgets, "max_member_bytes", 8_388_608)?,
                        },
                        deadline,
                        cancelled,
                    )
                    .map_err(|_| SourceCommandError::Conflict("record original selected cut"))?,
            )
        }
        _ => {
            return Err(SourceCommandError::Invalid(
                "record original source selector",
            ));
        }
    };
    let pending = owner::revision_publication::pending(&filesystem, deadline, cancelled)?;
    let selected = if pending {
        original.as_ref().ok_or(SourceCommandError::Denied(
            "record pending original semantic cut required",
        ))?
    } else {
        current
    };
    let now = crate::source_serialization::instant()?;
    let ctx = context(
        &configuration_raw,
        request_raw,
        &now,
        selected,
        software,
        components,
        deadline,
        cancelled,
    )?;
    let mut worker = selected_schema(invocation, selected, deadline, cancelled)?;
    let result = owner::revision_publication::run(
        &filesystem,
        &ctx,
        selected,
        original.as_ref(),
        software,
        components,
        &mut worker,
        deadline,
        cancelled,
    );
    match result {
        Ok(proposal) => Ok(json!({"schema_version":"tos_local_native_source_result_v1",
            "authentication":"local-unix-account", "result":value(&proposal.response)?, "grants_admission":false})),
        Err(error) => {
            let _ = worker.finish(deadline, cancelled);
            Err(error)
        }
    }
}
