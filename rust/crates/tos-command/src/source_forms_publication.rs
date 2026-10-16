//! Isolated filesystem caller for maintained Forms proposals. The production
//! PreparedCommand admission gate remains closed; this path moves only the
//! fixed protected owner's adjacent form set through the existing CAS journal.
use super::work_transaction::{self as tx, PublicationSnapshot, SelectedFile, WorkPlan};
use super::{CreationFilesystem, active, inode, owned, walk};
use crate::source_command::{
    self as cmd, CommandContext, PreparedCommand, SourceCommandError, SourceCommandResult,
};
use rustix::fs::{FlockOperation, Mode, OFlags};
use rustix::io::Errno;
use std::fs::File;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};

struct FormLock {
    directory: File,
    descriptor: File,
    name: String,
    uid: u32,
}
impl FormLock {
    fn hold(
        fs: &CreationFilesystem,
        parent: &str,
        name: String,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<Self> {
        let directory = walk(&fs.root, parent, fs.uid)?;
        let descriptor: File = rustix::fs::openat(
            &directory,
            name.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map(File::from)
        .map_err(|_| SourceCommandError::Denied("Forms exact cooperating lock"))?;
        owned(&descriptor, fs.uid, false)?;
        loop {
            active(deadline, cancelled)?;
            match rustix::fs::flock(&descriptor, FlockOperation::NonBlockingLockExclusive) {
                Ok(()) => break,
                Err(Errno::AGAIN) => std::thread::sleep(std::time::Duration::from_millis(10)),
                Err(_) => {
                    return Err(SourceCommandError::Denied(
                        "Forms cooperating lock acquisition",
                    ));
                }
            }
        }
        let result = Self {
            directory,
            descriptor,
            name,
            uid: fs.uid,
        };
        result.verify()?;
        Ok(result)
    }
    fn verify(&self) -> SourceCommandResult<()> {
        let current = tos_fd_open::open_regular_at(&self.directory, Path::new(&self.name))
            .map_err(|_| SourceCommandError::Conflict("Forms cooperating lock detached"))?;
        if inode(&owned(&current, self.uid, false)?)
            != inode(&owned(&self.descriptor, self.uid, false)?)
        {
            return Err(SourceCommandError::Conflict(
                "Forms cooperating lock replaced",
            ));
        }
        Ok(())
    }
}

fn transaction_id(ctx: &CommandContext) -> SourceCommandResult<String> {
    transaction_identity(&ctx.configuration_raw, &ctx.request_raw)
}

fn transaction_identity(
    configuration_raw: &[u8],
    request_raw: &[u8],
) -> SourceCommandResult<String> {
    let config = cmd::parse(configuration_raw)?;
    let request = cmd::parse(request_raw)?;
    let identity = cmd::object(vec![
        ("domain", cmd::string("tos-isolated-forms-v1")),
        (
            "source_path",
            cmd::string(cmd::text(&config, "source_path")?),
        ),
        (
            "request",
            cmd::string(&Digest256::of_bytes(&cmd::canonical(&request)?).to_prefixed()),
        ),
    ]);
    Ok(Digest256::of_bytes(&cmd::canonical(&identity)?).to_prefixed())
}

fn authorization(ctx: &CommandContext) -> SourceCommandResult<JsonValue> {
    let config = cmd::parse(&ctx.configuration_raw)?;
    let canonical = cmd::text(&config, "schema_version")? == "tos_local_canonical_form_owner_v1";
    Ok(cmd::object(vec![
        (
            "schema_version",
            cmd::string(if canonical {
                "tos_canonical_forms_authorization_v1"
            } else {
                "tos_public_forms_authorization_v1"
            }),
        ),
        (
            "source_path",
            cmd::string(cmd::text(&config, "source_path")?),
        ),
        (
            "configuration_raw_sha256",
            cmd::string(&Digest256::of_bytes(&ctx.configuration_raw).to_prefixed()),
        ),
        (
            "request_sha256",
            cmd::string(
                &Digest256::of_bytes(&cmd::canonical(&cmd::parse(&ctx.request_raw)?)?)
                    .to_prefixed(),
            ),
        ),
        ("recorded_at", cmd::string(&ctx.recorded_at)),
    ]))
}

fn plan(ctx: &CommandContext, proposal: &PreparedCommand) -> SourceCommandResult<WorkPlan> {
    if proposal.operation != "apply" || proposal.replayed || proposal.changes.len() != 1 {
        return Err(SourceCommandError::Invalid(
            "Forms publication requires one actual proposal",
        ));
    }
    let config = cmd::parse(&ctx.configuration_raw)?;
    let source = cmd::text(&config, "source_path")?;
    let expected = if matches!(
        cmd::text(&config, "schema_version")?,
        "tos_local_claim_form_owner_v1" | "tos_local_claim_form_owner_v2"
    ) {
        let stem = source
            .strip_suffix(".jsonl")
            .ok_or(SourceCommandError::Invalid("Forms Claim source suffix"))?;
        let suffix = Digest256::of_bytes(cmd::text(&config, "claim_id")?.as_bytes()).to_hex();
        format!("{stem}.{suffix}.human-forms.json")
    } else {
        let stem = source
            .strip_suffix(".json")
            .ok_or(SourceCommandError::Invalid("Forms source suffix"))?;
        format!("{stem}.human-forms.json")
    };
    let change = &proposal.changes[0];
    if change.path.as_str() != expected || change.after.is_none() {
        return Err(SourceCommandError::Denied(
            "Forms proposal outside adjacent protected target",
        ));
    }
    let before = ctx
        .files
        .iter()
        .find(|file| file.path == change.path)
        .map(|file| file.raw.clone());
    if before.as_ref().map(|raw| Digest256::of_bytes(raw)) != change.before {
        return Err(SourceCommandError::Conflict(
            "Forms before bytes differ from selected proposal",
        ));
    }
    Ok(WorkPlan {
        transaction_id: transaction_id(ctx)?,
        authorization: authorization(ctx)?,
        item_path_profile: None,
        files: vec![SelectedFile {
            path: change.path.clone(),
            before,
            after: change.after.clone(),
        }],
        new_directories: vec![],
        source_readset: None,
        source_successor: None,
    })
}

fn read(
    fs: &CreationFilesystem,
    path: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<Vec<u8>>> {
    let (parent, leaf) = path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Forms dependency parent"))?;
    active(deadline, cancelled)?;
    let directory = tx::read_existing_parent(fs, parent)?;
    active(deadline, cancelled)?;
    let Some(directory) = directory else {
        return Ok(None);
    };
    tx::read_at(&directory, leaf, fs.uid, 8_388_608, deadline, cancelled)
}

fn check_guard(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    proposal: &PreparedCommand,
    selected: &WorkPlan,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    active(deadline, cancelled)?;
    fs.current_context(ctx, deadline, cancelled)?;
    for dependency in &proposal.reads {
        if !dependency.path.as_str().starts_with("ToS/") {
            continue;
        }
        let raw = read(fs, dependency.path.as_str(), deadline, cancelled)?;
        if let Some(file) = selected
            .files
            .iter()
            .find(|file| file.path == dependency.path)
        {
            if raw != file.before && raw != file.after {
                return Err(SourceCommandError::Conflict(
                    "Forms selected target outside retained sides",
                ));
            }
        } else if raw.as_ref().map(|raw| Digest256::of_bytes(raw)) != Some(dependency.raw_sha256) {
            return Err(SourceCommandError::Conflict(
                "Forms source or schema changed before publication",
            ));
        }
    }
    // An absent predecessor has no read-dependency row. It still has an exact
    // before/after obligation at every stage and final journal guard.
    for file in &selected.files {
        let raw = read(fs, file.path.as_str(), deadline, cancelled)?;
        if raw != file.before && raw != file.after {
            return Err(SourceCommandError::Conflict("Forms target CAS changed"));
        }
    }
    Ok(())
}

pub(crate) fn pending(
    fs: &CreationFilesystem,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<bool> {
    Ok(tx::read_pending(fs, deadline, cancelled)?.is_some())
}

pub(crate) fn committed(
    fs: &CreationFilesystem,
    configuration_raw: &[u8],
    request_raw: &[u8],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<bool> {
    let request = cmd::parse(request_raw)?;
    if cmd::text(&request, "operation")? != "apply" {
        return Ok(false);
    }
    Ok(tx::inspect_committed_if_present(
        fs,
        &transaction_identity(configuration_raw, request_raw)?,
        deadline,
        cancelled,
    )?
    .is_some())
}

/// The selected cut is current for normal describe/prepare/apply and original
/// for an exact pending or committed replay. Recovery never reparses caller output as
/// authority: the maintained engine reconstructs and checks all proposed bytes.
pub(crate) fn run(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    selected: &CorpusCutReader,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedCommand> {
    fs.current_context(ctx, deadline, cancelled)?;
    let config = cmd::parse(&ctx.configuration_raw)?;
    if !matches!(
        cmd::text(&config, "schema_version")?,
        "tos_local_source_command_owner_v1"
            | "tos_local_canonical_form_owner_v1"
            | "tos_local_claim_form_owner_v1"
            | "tos_local_claim_form_owner_v2"
    ) {
        return Err(SourceCommandError::Denied(
            "isolated Forms exact supported owner",
        ));
    }
    let request = cmd::parse(&ctx.request_raw)?;
    let operation = cmd::text(&request, "operation")?;
    let mut context = ctx.clone();
    let pending = tx::read_pending(fs, deadline, cancelled)?;
    if let Some(retained) = &pending {
        if operation != "apply" || retained.plan.transaction_id != transaction_id(ctx)? {
            return Err(SourceCommandError::Conflict(
                "Forms pending transaction requires exact apply",
            ));
        }
        context.recorded_at = cmd::text(&retained.plan.authorization, "recorded_at")?.to_owned();
        if !cmd::same(&authorization(&context)?, &retained.plan.authorization)? {
            return Err(SourceCommandError::Conflict(
                "Forms pending owner or request changed",
            ));
        }
    }
    let committed = if pending.is_none() && operation == "apply" {
        tx::inspect_committed_if_present(fs, &transaction_id(ctx)?, deadline, cancelled)?
    } else {
        None
    };
    if let Some(retained) = &committed {
        context.recorded_at = cmd::text(&retained.authorization, "recorded_at")?.to_owned();
        if !cmd::same(&authorization(&context)?, &retained.authorization)? {
            return Err(SourceCommandError::Conflict(
                "Forms committed owner or request changed",
            ));
        }
    }
    let mut proposal = crate::source_forms::run_form_command_from_captures(
        &context, selected, software, components, worker, deadline, cancelled,
    )?;
    worker
        .finish(deadline, cancelled)
        .map_err(|_| SourceCommandError::Denied("Forms worker FINAL"))?;
    if operation != "apply" || proposal.replayed {
        if pending.is_some() {
            return Err(SourceCommandError::Conflict(
                "Forms pending proposal unexpectedly replayed",
            ));
        }
        fs.current_context(&context, deadline, cancelled)?;
        if proposal.replayed {
            let (_, retained, _, _) =
                tx::inspect_committed(fs, &transaction_id(&context)?, deadline, cancelled)?;
            if cmd::text(&retained.authorization, "source_path")?
                != cmd::text(&config, "source_path")?
                || cmd::text(&retained.authorization, "request_sha256")?
                    != proposal.request_canonical_sha256.to_prefixed()
                || retained.files.len() != 1
            {
                return Err(SourceCommandError::Conflict(
                    "Forms replay retained request differs",
                ));
            }
            let set = cmd::parse(retained.files[0].after.as_deref().ok_or(
                SourceCommandError::Conflict("Forms retained successor absent"),
            )?)?;
            let command_id = cmd::text(&request, "command_id")?;
            let receipt = cmd::array(&set, "growth_history")?
                .iter()
                .find(|receipt| {
                    receipt.object_get("command_id").and_then(JsonValue::as_str) == Some(command_id)
                })
                .ok_or(SourceCommandError::Conflict(
                    "Forms retained command receipt absent",
                ))?;
            if !cmd::same(receipt, cmd::field(&proposal.response, "receipt")?)? {
                return Err(SourceCommandError::Conflict(
                    "Forms replay receipt differs from journal",
                ));
            }
        }
        for dependency in &proposal.reads {
            if dependency.path.as_str().starts_with("ToS/")
                && read(fs, dependency.path.as_str(), deadline, cancelled)?
                    .as_ref()
                    .map(|raw| Digest256::of_bytes(raw))
                    != Some(dependency.raw_sha256)
            {
                return Err(SourceCommandError::Conflict(
                    "Forms read changed before result",
                ));
            }
        }
        return Ok(proposal);
    }
    let expected = plan(&context, &proposal)?;
    let guard_plan = expected.clone();
    let canonical_lock =
        if cmd::text(&config, "schema_version")? == "tos_local_canonical_form_owner_v1" {
            Some(FormLock::hold(
                fs,
                "ToS/canon",
                "..canonical-human-forms-lock.writer.lock".to_owned(),
                deadline,
                cancelled,
            )?)
        } else {
            None
        };
    let fence = tx::WorkCorpusFence::hold(fs, deadline, cancelled)?;
    let target = guard_plan.files[0].path.as_str();
    let (parent, name) = target
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("Forms target lock parent"))?;
    let target_lock = FormLock::hold(
        fs,
        parent,
        format!(".{name}.writer.lock"),
        deadline,
        cancelled,
    )?;
    let verify_locks = || -> SourceCommandResult<()> {
        if let Some(lock) = &canonical_lock {
            lock.verify()?;
        }
        target_lock.verify()
    };
    if let Some(retained) = committed {
        // Reconstruct from the original exact cut and the journal's original
        // clock, then verify current authorization and all source dependencies.
        // A lost response must never cause a second write or a fresh receipt.
        if !super::work_expression::same_selected_plan(&expected, &retained)? {
            return Err(SourceCommandError::Conflict(
                "Forms reconstructed committed plan differs",
            ));
        }
        let (_, current_retained, _, _) =
            tx::inspect_committed(fs, &expected.transaction_id, deadline, cancelled)?;
        if !super::work_expression::same_selected_plan(&retained, &current_retained)? {
            return Err(SourceCommandError::Conflict(
                "Forms committed journal changed",
            ));
        }
        verify_locks()?;
        check_guard(fs, &context, &proposal, &guard_plan, deadline, cancelled)?;
        for file in &guard_plan.files {
            if read(fs, file.path.as_str(), deadline, cancelled)? != file.after {
                return Err(SourceCommandError::Conflict(
                    "Forms committed replay target changed",
                ));
            }
        }
        fence.verify(deadline, cancelled)?;
        proposal.replayed = true;
        proposal.changes.clear();
        cmd::set(&mut proposal.response, "replayed", JsonValue::Bool(true))?;
        return Ok(proposal);
    }
    let result = if let Some(retained) = pending {
        if !super::work_expression::same_selected_plan(&expected, &retained.plan)? {
            return Err(SourceCommandError::Conflict(
                "Forms reconstructed pending plan differs",
            ));
        }
        fence.recover(
            &retained,
            false,
            None,
            |_, _| {
                verify_locks()?;
                check_guard(fs, &context, &proposal, &guard_plan, deadline, cancelled)
            },
            deadline,
            cancelled,
        )?
    } else {
        let snapshot = PublicationSnapshot::select(fs, deadline, cancelled)?;
        fence.apply(
            expected,
            &snapshot,
            |_, _| {
                verify_locks()?;
                check_guard(fs, &context, &proposal, &guard_plan, deadline, cancelled)
            },
            deadline,
            cancelled,
        )?
    };
    if !result.committed {
        return Err(SourceCommandError::Conflict(
            "Forms publication rolled back",
        ));
    }
    verify_locks()?;
    check_guard(fs, &context, &proposal, &guard_plan, deadline, cancelled)?;
    for file in &guard_plan.files {
        if read(fs, file.path.as_str(), deadline, cancelled)? != file.after {
            return Err(SourceCommandError::Conflict(
                "Forms committed target differs",
            ));
        }
    }
    Ok(proposal)
}
