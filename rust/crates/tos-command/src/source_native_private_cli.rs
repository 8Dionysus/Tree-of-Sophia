//! Explicit private metadata invocation. The public cut is evidence only;
//! the selected OwnerTextContext independently owns private byte routing.
use super::{absolute, capped, digest, selected_schema, text};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_text_owner::{OwnerTextContext, read_absolute};
use crate::source_text_private_store::PrivateOwnerStore;
use serde_json::Value;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{JsonValue, RelativePath, SourceRevision};
use tos_source_store::{
    CorpusCutReader, CorpusReader, CutReadLimits, SoftwareCaptureReader,
    SoftwareComponentSelectionV1,
};
use tos_validation::source_cut::CutSchemaExecutor;
use tos_validation::source_cut::CutWorkerSchemaExecutor;

// Python's implementation map hashes these bytes from source.ROOT. They are
// software rules, distinct from owner-context grammar at the same source refs.
const RULE_SCHEMA_COMPONENTS: &[&str] = &[
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
    "ToS/contracts/provenance-event-v2.schema.json",
];

fn verify_selected_software(
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let mut total = 0u64;
    for member in components.members() {
        total = total
            .checked_add(member.size_bytes)
            .ok_or(SourceCommandError::Invalid("private software budget"))?;
        if total > 33_554_432 || member.size_bytes > 8_388_608 {
            return Err(SourceCommandError::Invalid("private software budget"));
        }
        software
            .read_selected_component(components, &member.path, 8_388_608, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("private selected software changed"))?;
    }
    Ok(())
}

#[path = "source_private_profile.rs"]
pub(crate) mod profile;

#[path = "source_private_claim.rs"]
mod claim;

struct SelectedPrivateInvocation {
    owner: OwnerTextContext,
    context_config: JsonValue,
    context: cmd::CommandContext,
    worker: CutWorkerSchemaExecutor,
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
    let mut selected = select(
        invocation,
        request_raw,
        store,
        current,
        software,
        components,
        deadline,
        cancelled,
    )?;
    let config = cmd::parse(&selected.context.configuration_raw)?;
    let request = cmd::parse(request_raw)?;
    let operation = cmd::text(&request, "operation")?;
    let is_claim = matches!(
        cmd::text(&config, "schema_version")?,
        claim::CONFIG_V1 | claim::CONFIG_V2
    );
    let mut owner_store =
        PrivateOwnerStore::select(&selected.owner, cmd::text(&config, "source_path")?)?;
    let before = owner_store.read_package(deadline, cancelled)?;
    let basenames = if is_claim {
        claim::identity_basenames(
            &selected.context,
            current,
            &mut selected.worker,
            deadline,
            cancelled,
        )?
    } else {
        profile::identity_basenames(
            &selected.context,
            current,
            &mut selected.worker,
            deadline,
            cancelled,
        )?
    };
    let creating = matches!(
        operation,
        "source.create" | "claims.create" | "prepare-create"
    );
    let inputs = if is_claim {
        owner_store.read_claim_identity_inputs(
            &basenames,
            Some(cmd::text(&config, "source_path")?),
            creating,
            current,
            deadline,
            cancelled,
        )?
    } else {
        owner_store.read_identity_inputs(
            &basenames,
            Some(cmd::text(&config, "source_path")?),
            creating,
            deadline,
            cancelled,
        )?
    };
    let (before_write, after, response, archive, reads) = if is_claim {
        let plan = claim::prepare(
            &selected.context,
            current,
            software,
            components,
            &selected.owner,
            &selected.context_config,
            before.as_ref(),
            &inputs,
            &mut owner_store,
            &mut selected.worker,
            deadline,
            cancelled,
        )?;
        (
            plan.before,
            plan.files,
            plan.response,
            plan.archive,
            plan.reads,
        )
    } else {
        let plan = profile::prepare(
            &selected.context,
            current,
            software,
            components,
            &selected.owner,
            &selected.context_config,
            before.as_ref(),
            &inputs,
            &mut owner_store,
            &mut selected.worker,
            deadline,
            cancelled,
        )?;
        (
            plan.before,
            plan.after,
            plan.response,
            plan.archive,
            plan.reads,
        )
    };
    selected
        .worker
        .finish(deadline, cancelled)
        .map_err(|_| SourceCommandError::Invalid("private metadata worker finish"))?;
    let grant_path = absolute(text(invocation, "owner_config")?)?;
    let guard = || {
        if read_absolute(
            &grant_path,
            selected.owner.account_uid(),
            true,
            1_048_576,
            deadline,
            cancelled,
        )? != selected.context.configuration_raw
        {
            return Err(SourceCommandError::Conflict(
                "private metadata grant changed",
            ));
        }
        cmd::validate_expiry(
            cmd::text(&config, "expires_at")?,
            &crate::source_serialization::instant()?,
        )?;
        selected.owner.snapshot(deadline, cancelled)?;
        owner_store.verify_observed_archives(deadline, cancelled)?;
        crate::source_creation_store::verify_owner_metadata_current_cut(
            selected.owner.public_root(),
            selected.owner.account_uid(),
            current,
            deadline,
            cancelled,
        )?;
        let fresh = if is_claim {
            owner_store.read_claim_identity_inputs(
                &basenames,
                Some(cmd::text(&config, "source_path")?),
                creating,
                current,
                deadline,
                cancelled,
            )?
        } else {
            owner_store.read_identity_inputs(
                &basenames,
                Some(cmd::text(&config, "source_path")?),
                creating,
                deadline,
                cancelled,
            )?
        };
        if fresh.files != inputs.files {
            return Err(SourceCommandError::Conflict(
                "private identity inputs changed",
            ));
        }
        for input in &reads {
            if selected
                .owner
                .read(input.path.as_str(), input.raw.len(), deadline, cancelled)?
                != input.raw
            {
                return Err(SourceCommandError::Conflict(
                    "private selected dependency changed",
                ));
            }
        }
        verify_selected_software(software, components, deadline, cancelled)?;
        selected
            .context
            .check_from_selected_captures(current, software, components, deadline, cancelled)
    };
    if let Some(files) = after {
        if before_write != before {
            return Err(SourceCommandError::Conflict(
                "private engine predecessor differs from selected package",
            ));
        }
        if let Some(previous) = before_write {
            owner_store.publish_successor(
                &request,
                &previous,
                &files,
                archive
                    .as_ref()
                    .map(|(reference, files)| (reference.as_str(), files)),
                guard,
                guard,
                deadline,
                cancelled,
            )?;
        } else {
            if archive.is_some() {
                return Err(SourceCommandError::Invalid(
                    "private creation predecessor archive",
                ));
            }
            owner_store.publish_new(&request, &files, guard, guard, deadline, cancelled)?;
        }
    } else {
        guard()?;
        if owner_store.read_package(deadline, cancelled)? != before {
            return Err(SourceCommandError::Conflict(
                "private readonly package changed",
            ));
        }
    }
    let response: Value = serde_json::from_slice(&cmd::canonical(&response)?)
        .map_err(|_| SourceCommandError::Invalid("private metadata response"))?;
    Ok(
        serde_json::json!({"schema_version":"tos_local_native_source_result_v1",
        "authentication":"local-unix-account","result":response,"grants_admission":false}),
    )
}

fn select(
    invocation: &Value,
    request_raw: &[u8],
    store: &CorpusReader,
    current: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<SelectedPrivateInvocation> {
    if invocation.get("assessment_schema_worker") != Some(&Value::Null) {
        return Err(SourceCommandError::Denied(
            "private metadata assessment worker",
        ));
    }
    // Retain the explicit original identity as separate evidence. Current
    // selected state, retained packages and cold-chain checks govern replay.
    let original = SourceRevision(digest(text(invocation, "original_source_revision")?)?);
    let budgets = &invocation["budgets"];
    store
        .open_source_cut(
            original,
            CutReadLimits {
                max_revisions: usize::try_from(capped(budgets, "max_revisions", 4)?)
                    .map_err(|_| SourceCommandError::Invalid("private original revisions"))?,
                max_members: capped(budgets, "max_members", 2048)?,
                max_total_bytes: capped(budgets, "max_total_bytes", 33_554_432)?,
                max_member_bytes: capped(budgets, "max_member_bytes", 8_388_608)?,
            },
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("private original source cut"))?;
    let context_path = absolute(text(invocation, "owner_context")?)?;
    let grant_path = absolute(text(invocation, "owner_config")?)?;
    let uid = rustix::process::getuid().as_raw();
    let configuration_raw = read_absolute(&grant_path, uid, true, 1_048_576, deadline, cancelled)?;
    let config = cmd::parse(&configuration_raw)?;
    if !matches!(
        cmd::text(&config, "schema_version")?,
        profile::CONFIG | claim::CONFIG_V1 | claim::CONFIG_V2
    ) {
        return Err(SourceCommandError::Denied(
            "private metadata exact owner family",
        ));
    }
    if cmd::integer(&config, "uid")? != u64::from(uid) {
        return Err(SourceCommandError::Denied("private grant account differs"));
    }
    cmd::validate_expiry(
        cmd::text(&config, "expires_at")?,
        &crate::source_serialization::instant()?,
    )?;
    if cmd::text(&config, "source_context_ref")?
        != context_path
            .to_str()
            .ok_or(SourceCommandError::Invalid("private context path"))?
    {
        return Err(SourceCommandError::Denied("private grant context differs"));
    }
    let mut worker = selected_schema(invocation, current, deadline, cancelled)?;
    let path = RelativePath::parse("ToS/contracts/owner-local-source-context.schema.json")
        .map_err(|_| SourceCommandError::Invalid("private context schema"))?;
    let schema = current
        .read_member(
            current.current().revision(),
            &path,
            1_048_576,
            deadline,
            cancelled,
        )
        .map_err(|_| SourceCommandError::Conflict("private context schema cut"))?;
    let (owner, context_config) =
        OwnerTextContext::select(&context_path, &schema.raw, &mut worker, deadline, cancelled)?;
    let mut context = cmd::CommandContext {
        base_revision: current.current().revision(),
        configuration_raw,
        request_raw: request_raw.to_vec(),
        recorded_at: crate::source_serialization::instant()?,
        effective_uid: u64::from(uid),
        files: Vec::new(),
    };
    context.files =
        crate::source_claims::complete_authored_inputs(&context, current, deadline, cancelled)?;
    let mut total = 0u64;
    for member in components.members() {
        total = total
            .checked_add(member.size_bytes)
            .ok_or(SourceCommandError::Invalid("private software size"))?;
        if total > 33_554_432 {
            return Err(SourceCommandError::Invalid("private software budget"));
        }
        let raw = software
            .read_selected_component(components, &member.path, 8_388_608, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("private selected software"))?;
        if member.path.as_str().starts_with("ToS/") {
            if !RULE_SCHEMA_COMPONENTS.contains(&member.path.as_str()) {
                return Err(SourceCommandError::Denied(
                    "private rule software schema scope",
                ));
            }
            // Keep the typed software carrier separate. CommandContext ToS
            // inputs always remain the independently selected current cut.
            continue;
        }
        context.files.push(cmd::SourceFile {
            path: member.path.clone(),
            raw,
        });
    }
    context.check_from_selected_captures(current, software, components, deadline, cancelled)?;
    crate::source_creation_store::verify_owner_metadata_current_cut(
        owner.public_root(),
        owner.account_uid(),
        current,
        deadline,
        cancelled,
    )?;
    cmd::validate_expiry(cmd::text(&config, "expires_at")?, &context.recorded_at)?;
    if cmd::integer(&config, "uid")? != u64::from(uid) {
        return Err(SourceCommandError::Denied("private grant account differs"));
    }
    Ok(SelectedPrivateInvocation {
        owner,
        context_config,
        context,
        worker,
    })
}
