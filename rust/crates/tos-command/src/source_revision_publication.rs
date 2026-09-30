//! Fixed record-revision writer for protected isolated owners. Domain plans
//! remain proposals; this caller retains the existing three-file CAS journal
//! and historical-create lock without opening production admission.
use super::work_expression as physical;
use super::work_transaction::{self as tx, PublicationSnapshot, SelectedFile, WorkPlan};
use super::{CreationFilesystem, active};
use crate::source_command::{
    self as cmd, CommandContext, PreparedCommand, SourceCommandError, SourceCommandResult,
    SourceFile,
};
use crate::source_revisions::{self as revision, RevisionFamily};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue, RelativePath};
use tos_source_store::{CorpusCutReader, SoftwareCaptureReader, SoftwareComponentSelectionV1};
use tos_validation::source_cut::{CutSchemaExecutor, CutWorkerSchemaExecutor};
const CONTROL: &str = "ToS/source-witnesses/.metadata-publication.json";

fn read(
    fs: &CreationFilesystem,
    path: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Option<Vec<u8>>> {
    let (parent, leaf) = path
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("revision member parent"))?;
    let Some(parent) = tx::read_existing_parent(fs, parent, deadline, cancelled)? else {
        return Ok(None);
    };
    tx::read_at(&parent, leaf, fs.uid, 8_388_608, deadline, cancelled)
}
fn package(
    ctx: &CommandContext,
    config: &JsonValue,
) -> SourceCommandResult<BTreeMap<String, Vec<u8>>> {
    let (_, family) = revision::configuration(ctx)?;
    revision::package(
        ctx,
        cmd::text(config, "source_path")?,
        family.selected(),
        None,
    )
}
fn authorization(
    ctx: &CommandContext,
    config: &JsonValue,
    family: RevisionFamily,
    record: &JsonValue,
) -> SourceCommandResult<JsonValue> {
    Ok(cmd::object(vec![
        (
            "schema_version",
            cmd::string(if family.selected() {
                "tos_selected_metadata_revision_authorization_v1"
            } else {
                "tos_isolated_metadata_revision_authorization_v1"
            }),
        ),
        ("principal_id", cmd::field(config, "principal_id")?.clone()),
        (
            "authority_ref",
            cmd::field(config, "authority_ref")?.clone(),
        ),
        ("source_path", cmd::field(config, "source_path")?.clone()),
        ("record_id", cmd::field(config, "record_id")?.clone()),
        (
            "record_type",
            if family.selected() {
                cmd::field(config, "record_type")?.clone()
            } else {
                cmd::field(record, "record_type")
                    .cloned()
                    .unwrap_or_else(|_| cmd::string(family.handler_id()))
            },
        ),
        ("request", cmd::parse(&ctx.request_raw)?),
    ]))
}
fn plan(
    ctx: &CommandContext,
    config: &JsonValue,
    family: RevisionFamily,
    record: &JsonValue,
    proposal: &PreparedCommand,
) -> SourceCommandResult<WorkPlan> {
    let source = cmd::text(config, "source_path")?;
    let parent = source
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("revision source parent"))?
        .0;
    let mut files = Vec::new();
    for name in revision::names(source)? {
        let path = RelativePath::parse(&format!("{parent}/{name}"))
            .map_err(|_| SourceCommandError::Invalid("revision selected path"))?;
        let change = proposal
            .changes
            .iter()
            .find(|change| change.path == path)
            .ok_or(SourceCommandError::Conflict(
                "revision proposal missing selected member",
            ))?;
        let before = ctx.file(&path)?.map(<[u8]>::to_vec);
        if before.as_ref().map(|raw| Digest256::of_bytes(raw)) != change.before {
            return Err(SourceCommandError::Conflict(
                "revision proposal before side differs",
            ));
        }
        files.push(SelectedFile {
            path,
            before,
            after: change.after.clone(),
        });
    }
    let request = cmd::parse(&ctx.request_raw)?;
    Ok(WorkPlan {
        transaction_id: revision::transaction_id(&request)?,
        authorization: authorization(ctx, config, family, record)?,
        item_path_profile: None,
        files,
        new_directories: Vec::new(),
    })
}
fn empty_bindings() -> JsonValue {
    cmd::object(vec![
        ("catalog_and_sources", cmd::object(vec![])),
        ("contracts", cmd::object(vec![])),
        ("implementation", cmd::object(vec![])),
        ("retained_transactions", cmd::object(vec![])),
    ])
}
fn dependencies_current(
    fs: &CreationFilesystem,
    proposal: &PreparedCommand,
    selected: &WorkPlan,
    archive: &tx::WorkArchive,
    extent: &tx::WorkGuard<'_>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    let archive_members = archive.member_paths().collect::<BTreeSet<_>>();
    for dependency in &proposal.reads {
        active(deadline, cancelled)?;
        let path = dependency.path.as_str();
        if !path.starts_with("ToS/")
            || path == CONTROL
            || archive_members.contains(path)
            || extent.journal_members.contains(path)
        {
            continue;
        }
        let raw = read(fs, path, deadline, cancelled)?;
        if let Some(file) = selected
            .files
            .iter()
            .find(|file| file.path == dependency.path)
        {
            if raw != file.before && raw != file.after {
                return Err(SourceCommandError::Conflict(
                    "revision selected dependency third state",
                ));
            }
        } else if raw.as_ref().map(|raw| Digest256::of_bytes(raw)) != Some(dependency.raw_sha256) {
            return Err(SourceCommandError::Conflict("revision dependency changed"));
        }
    }
    Ok(())
}
fn current_observation(
    fs: &CreationFilesystem,
    original: &CommandContext,
    pending: &tx::PendingWork,
    archive: &tx::WorkArchive,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<CommandContext> {
    let mut context = original.clone();
    let mut paths = tx::committed_member_paths(&pending.plan)?;
    paths.extend(archive.member_paths());
    paths.insert(CONTROL.into());
    for path in paths {
        let reference = RelativePath::parse(&path)
            .map_err(|_| SourceCommandError::Invalid("revision retained observation path"))?;
        context.files.retain(|file| file.path != reference);
        if let Some(raw) = read(fs, &path, deadline, cancelled)? {
            context.files.push(SourceFile {
                path: reference,
                raw,
            });
        }
    }
    // These additional bytes are live FD observations under the retained mover
    // fence. They are never claimed to belong to the original semantic cut.
    context.check()?;
    Ok(context)
}
fn retained_time(pending: &tx::PendingWork, config: &JsonValue) -> SourceCommandResult<String> {
    let source = cmd::text(config, "source_path")?;
    let parent = source
        .rsplit_once('/')
        .ok_or(SourceCommandError::Invalid("revision history parent"))?
        .0;
    let history = format!("{parent}/source-revision-history.json");
    let raw = pending
        .plan
        .files
        .iter()
        .find(|file| file.path.as_str() == history)
        .and_then(|file| file.after.as_deref())
        .ok_or(SourceCommandError::Conflict(
            "revision pending history absent",
        ))?;
    let value = cmd::parse(raw)?;
    let receipt = cmd::array(&value, "receipts")?
        .last()
        .ok_or(SourceCommandError::Conflict(
            "revision pending receipt absent",
        ))?;
    let time = cmd::text(receipt, "recorded_at")?.to_owned();
    cmd::validate_expiry("9999-12-31T23:59:59+00:00", &time)?;
    Ok(time)
}

pub(crate) fn pending(
    fs: &CreationFilesystem,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<bool> {
    Ok(tx::read_pending(fs, deadline, cancelled)?.is_some())
}

/// The supplied context/cut owns semantics. Pending adds only exact retained
/// control/journal/archive observations; selected physical sides remain guarded
/// separately on every mover edge.
pub(crate) fn run(
    fs: &CreationFilesystem,
    ctx: &CommandContext,
    cut: &CorpusCutReader,
    original: Option<&CorpusCutReader>,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    worker: &mut CutWorkerSchemaExecutor,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedCommand> {
    ctx.check_from_selected_captures(cut, software, components, deadline, cancelled)?;
    fs.current_context(ctx, deadline, cancelled)?;
    let (config, family) = revision::configuration(ctx)?;
    let request = cmd::parse(&ctx.request_raw)?;
    let operation = cmd::text(&request, "operation")?;
    let fence = tx::WorkCorpusFence::hold(fs, deadline, cancelled)?;
    let pending = tx::read_pending(fs, deadline, cancelled)?;
    if let Some(pending) = &pending {
        if !matches!(operation, "record.revise" | "record.recover")
            || pending.plan.item_path_profile.is_some()
            || !pending.plan.new_directories.is_empty()
            || pending.plan.files.len() != 3
        {
            return Err(SourceCommandError::Denied(
                "revision exact pending owner plan",
            ));
        }
        let original_cut = original.ok_or(SourceCommandError::Denied(
            "revision pending original cut required",
        ))?;
        if original_cut.current().revision() != cut.current().revision() {
            return Err(SourceCommandError::Conflict(
                "revision pending semantic cut is not original",
            ));
        }
        let original_request = cmd::field(&pending.plan.authorization, "request")?;
        let mut original_ctx = ctx.clone();
        original_ctx.request_raw = cmd::published(original_request)?;
        original_ctx.recorded_at = retained_time(pending, &config)?;
        let before = package(&original_ctx, &config)?;
        let names = revision::names(cmd::text(&config, "source_path")?)?;
        let record = cmd::parse(before.get(&names[0]).ok_or(SourceCommandError::Conflict(
            "revision original record absent",
        ))?)?;
        if !cmd::same(
            &authorization(&original_ctx, &config, family, &record)?,
            &pending.plan.authorization,
        )? {
            return Err(SourceCommandError::Denied(
                "revision pending current owner scope differs",
            ));
        }
        let archive = tx::record_revision_archive(
            fs,
            ctx,
            &record,
            &before,
            &revision::revision(&before)?,
            deadline,
            cancelled,
            false,
        )?;
        let observed =
            current_observation(fs, &original_ctx, pending, &archive, deadline, cancelled)?;
        let mut command_observation = observed.clone();
        command_observation.request_raw = ctx.request_raw.clone();
        if family.selected() {
            command_observation.recorded_at = ctx.recorded_at.clone();
        }
        let ids = [pending.plan.transaction_id.as_str()];
        let publication = family
            .selected()
            .then(|| revision::read_record_revision_publication(&command_observation, &ids))
            .transpose()?;
        // Fixed isolated owner supplies authenticated live retained carriers to
        // the existing domain reconstruction. Fresh capture entry is unchanged.
        let mut proposal = revision::prepare_record_revision_inner(
            &command_observation,
            publication.as_ref(),
            Some(cut),
            worker,
            deadline,
            cancelled,
        )?;
        worker
            .finish(deadline, cancelled)
            .map_err(|_| SourceCommandError::Denied("revision worker FINAL"))?;
        let rollback =
            operation == "record.recover" && cmd::text(&request, "decision")? == "rollback";
        if operation == "record.recover" && !family.selected() {
            return Err(SourceCommandError::Denied(
                "legacy revision has no recovery operation",
            ));
        }
        if !family.selected() {
            if !cmd::same(&request, original_request)? {
                return Err(SourceCommandError::Conflict(
                    "legacy revision pending requires exact original request",
                ));
            }
            let expected = plan(&original_ctx, &config, family, &record, &proposal)?;
            if !physical::same_selected_plan(&expected, &pending.plan)? {
                return Err(SourceCommandError::Conflict(
                    "legacy revision reconstructed pending plan differs",
                ));
            }
        } else {
            for file in &pending.plan.files {
                let change = proposal
                    .changes
                    .iter()
                    .find(|change| change.path == file.path)
                    .ok_or(SourceCommandError::Conflict(
                        "revision pending reconstruction missing member",
                    ))?;
                if change.after
                    != if rollback {
                        file.before.clone()
                    } else {
                        file.after.clone()
                    }
                {
                    return Err(SourceCommandError::Conflict(
                        "revision reconstructed pending target differs",
                    ));
                }
            }
        }
        let selected = physical::selected_sides(&pending.plan)?;
        let prior_token = cmd::field(&pending.base_publication, "token")?.as_str();
        let prior = physical::original_prior_publication(cut, prior_token, deadline, cancelled)?;
        let bindings = empty_bindings();
        let renewal = if operation == "record.recover" {
            Some(cmd::object(vec![
                (
                    "schema_version",
                    cmd::string("tos_selected_metadata_recovery_authorization_v1"),
                ),
                ("principal_id", cmd::field(&config, "principal_id")?.clone()),
                (
                    "authority_ref",
                    cmd::field(&config, "authority_ref")?.clone(),
                ),
                (
                    "owner_configuration",
                    cmd::string(&cmd::record_digest(&config)?.to_prefixed()),
                ),
                ("transaction_id", cmd::string(&pending.plan.transaction_id)),
                (
                    "decision",
                    cmd::string(if rollback { "rollback" } else { "resume" }),
                ),
            ]))
        } else {
            None
        };
        let result = fence.recover(
            pending,
            rollback,
            renewal,
            |_, extent| {
                physical::physical_current(
                    fs,
                    ctx,
                    cut,
                    &selected,
                    &bindings,
                    None,
                    &archive,
                    prior.as_deref().zip(prior_token),
                    &extent,
                    deadline,
                    cancelled,
                )?;
                physical::software_current(fs, ctx, deadline, cancelled)?;
                dependencies_current(
                    fs,
                    &proposal,
                    &pending.plan,
                    &archive,
                    &extent,
                    deadline,
                    cancelled,
                )
            },
            deadline,
            cancelled,
        )?;
        let snapshot = PublicationSnapshot::select(fs, deadline, cancelled)?;
        physical::terminal_cut_current(
            fs,
            cut,
            &snapshot,
            &pending.plan,
            &pending.base_publication,
            &archive,
            rollback,
            deadline,
            cancelled,
        )?;
        physical::software_current(fs, ctx, deadline, cancelled)?;
        if family.selected() {
            cmd::set(
                &mut proposal.response,
                "publication_snapshot",
                cmd::field(&result.publication, "token")?.clone(),
            )?;
            cmd::set(
                &mut proposal.response,
                "recovery",
                cmd::object(vec![
                    ("transaction_id", cmd::string(&result.transaction_id)),
                    (
                        "status",
                        cmd::string(if result.committed {
                            "committed"
                        } else {
                            "rolled-back"
                        }),
                    ),
                    ("publication", result.publication.clone()),
                    (
                        "manifest_ref",
                        cmd::string(&format!(
                            "ToS/source-witnesses/.metadata-transactions/{}/manifest.json",
                            &result.transaction_id[7..]
                        )),
                    ),
                    ("manifest_sha256", cmd::string(&result.manifest_sha256)),
                    ("replayed", JsonValue::Bool(false)),
                    ("is_current_publication", JsonValue::Bool(true)),
                    ("current_selected_bytes_verified", JsonValue::Bool(true)),
                    ("grants_admission", JsonValue::Bool(false)),
                ]),
            )?;
        }
        fence.verify(deadline, cancelled)?;
        fs.current_context(ctx, deadline, cancelled)?;
        return Ok(proposal);
    }
    let mut ids = Vec::new();
    if operation == "record.revise" {
        let id = revision::transaction_id(&request)?;
        let manifest = RelativePath::parse(&format!(
            "ToS/source-witnesses/.metadata-transactions/{}/manifest.json",
            &id[7..]
        ))
        .map_err(|_| SourceCommandError::Invalid("revision retained manifest selector"))?;
        if ctx.file(&manifest)?.is_some() {
            ids.push(id);
        }
    }
    let id_refs = ids.iter().map(String::as_str).collect::<Vec<_>>();
    let publication = family
        .selected()
        .then(|| revision::read_record_revision_publication(ctx, &id_refs))
        .transpose()?;
    let mut proposal = revision::prepare_record_revision_from_captures(
        ctx,
        publication.as_ref(),
        cut,
        software,
        components,
        worker,
        deadline,
        cancelled,
    )?;
    worker
        .finish(deadline, cancelled)
        .map_err(|_| SourceCommandError::Denied("revision worker FINAL"))?;
    if proposal.changes.is_empty() {
        if proposal.replayed {
            let predecessor = original.ok_or(SourceCommandError::Denied(
                "revision retained replay requires original cut",
            ))?;
            let receipt = cmd::field(&proposal.response, "receipt")?;
            let (archived, _) = revision::read_archive(ctx, &config, receipt)?;
            let source = cmd::text(&config, "source_path")?;
            let parent = source
                .rsplit_once('/')
                .ok_or(SourceCommandError::Invalid(
                    "revision original package parent",
                ))?
                .0;
            let prefix = format!("{parent}/");
            for (name, expected) in &archived {
                let path = RelativePath::parse(&format!("{prefix}{name}"))
                    .map_err(|_| SourceCommandError::Invalid("revision original package member"))?;
                let observed = predecessor
                    .read_member(
                        predecessor.current().revision(),
                        &path,
                        2_097_152,
                        deadline,
                        cancelled,
                    )
                    .map_err(|_| {
                        SourceCommandError::Conflict("revision original archive member custody")
                    })?;
                if observed.raw != *expected {
                    return Err(SourceCommandError::Conflict(
                        "revision original archive package bytes differ",
                    ));
                }
            }
            if !family.selected()
                && predecessor.current().members().any(|member| {
                    member
                        .path
                        .as_str()
                        .strip_prefix(&prefix)
                        .is_some_and(|name| name.contains('/') || !archived.contains_key(name))
                })
            {
                return Err(SourceCommandError::Conflict(
                    "revision original flat package membership differs",
                ));
            }
            let transaction = revision::transaction_id(&request)?;
            let (_, retained, _, _) = tx::inspect_committed(fs, &transaction, deadline, cancelled)?;
            if retained.item_path_profile.is_some()
                || !retained.new_directories.is_empty()
                || retained.files.len() != 3
            {
                return Err(SourceCommandError::Denied(
                    "revision replay retained closure",
                ));
            }
            for file in &retained.files {
                match (predecessor.current().member(&file.path), &file.before) {
                    (Some(member), Some(raw))
                        if member.sha256 == Digest256::of_bytes(raw)
                            && member.size_bytes == raw.len() as u64 =>
                    {
                        ()
                    }
                    (None, None) => (),
                    _ => {
                        return Err(SourceCommandError::Conflict(
                            "revision original replay cut differs from retained predecessor",
                        ));
                    }
                }
            }
        }
        physical::complete_current_cut(
            fs,
            cut,
            &PublicationSnapshot::select(fs, deadline, cancelled)?,
            deadline,
            cancelled,
        )?;
        physical::software_current(fs, ctx, deadline, cancelled)?;
        fs.current_context(ctx, deadline, cancelled)?;
        fence.verify(deadline, cancelled)?;
        return Ok(proposal);
    }
    if original.is_some_and(|original| original.current().revision() != cut.current().revision()) {
        return Err(SourceCommandError::Denied(
            "fresh revision cannot select unrelated original cut",
        ));
    }
    if operation != "record.revise" {
        return Err(SourceCommandError::Denied(
            "revision writes require exact revise operation",
        ));
    }
    let before = package(ctx, &config)?;
    let names = revision::names(cmd::text(&config, "source_path")?)?;
    let record = cmd::parse(before.get(&names[0]).ok_or(SourceCommandError::Conflict(
        "revision original record absent",
    ))?)?;
    let expected = plan(ctx, &config, family, &record, &proposal)?;
    let selected = physical::selected_sides(&expected)?;
    physical::after_cut_budget(cut, &selected)?;
    let snapshot = PublicationSnapshot::select(fs, deadline, cancelled)?;
    physical::complete_current_cut(fs, cut, &snapshot, deadline, cancelled)?;
    physical::software_current(fs, ctx, deadline, cancelled)?;
    fs.current_context(ctx, deadline, cancelled)?;
    let archive_path = revision::archive_path(&config, &revision::revision(&before)?)?;
    let mut permitted = expected
        .files
        .iter()
        .map(|file| file.path.as_str().to_owned())
        .collect::<BTreeSet<_>>();
    permitted.insert(format!("{archive_path}/manifest.json"));
    for raw in before.values() {
        permitted.insert(format!(
            "{archive_path}/{}.blob",
            Digest256::of_bytes(raw).to_hex()
        ));
    }
    if proposal
        .changes
        .iter()
        .any(|change| !permitted.contains(change.path.as_str()))
    {
        return Err(SourceCommandError::Denied(
            "revision proposal outside exact selected/archive closure",
        ));
    }
    let archive = tx::record_revision_archive(
        fs,
        ctx,
        &record,
        &before,
        &revision::revision(&before)?,
        deadline,
        cancelled,
        true,
    )?;
    let prior =
        physical::original_prior_publication(cut, snapshot.token.as_deref(), deadline, cancelled)?;
    let bindings = empty_bindings();
    let base_publication = cmd::object(vec![
        (
            "token",
            snapshot
                .token
                .as_deref()
                .map(cmd::string)
                .unwrap_or(JsonValue::Null),
        ),
        ("generation", cmd::number(snapshot.generation)),
    ]);
    let guard_plan = expected.clone();
    let result = fence.apply(
        expected,
        &snapshot,
        |_, extent| {
            physical::physical_current(
                fs,
                ctx,
                cut,
                &selected,
                &bindings,
                Some(&snapshot),
                &archive,
                prior.as_deref().zip(snapshot.token.as_deref()),
                &extent,
                deadline,
                cancelled,
            )?;
            physical::software_current(fs, ctx, deadline, cancelled)?;
            dependencies_current(
                fs,
                &proposal,
                &guard_plan,
                &archive,
                &extent,
                deadline,
                cancelled,
            )
        },
        deadline,
        cancelled,
    )?;
    if !result.committed {
        return Err(SourceCommandError::Conflict(
            "revision fresh publication rolled back",
        ));
    }
    let terminal = PublicationSnapshot::select(fs, deadline, cancelled)?;
    physical::terminal_cut_current(
        fs,
        cut,
        &terminal,
        &guard_plan,
        &base_publication,
        &archive,
        false,
        deadline,
        cancelled,
    )?;
    if family.selected() {
        cmd::set(
            &mut proposal.response,
            "publication_snapshot",
            cmd::field(&result.publication, "token")?.clone(),
        )?;
    }
    physical::software_current(fs, ctx, deadline, cancelled)?;
    fs.current_context(ctx, deadline, cancelled)?;
    fence.verify(deadline, cancelled)?;
    Ok(proposal)
}
