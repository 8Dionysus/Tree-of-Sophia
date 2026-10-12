//! Native Historical and generic public Profile creation over a bounded PG
//! source read epoch. The semantic owner prepares and seals the package; this
//! adapter only selects exact current inputs and invokes the shared writer.

use super::*;
use crate::source_cohort::creation::{COMMITMENT_VERSION, PgCreationAttempt};
use crate::source_cohort::revision::{SourceRevisionIdentityClosure, SourceRevisionReadSession};
use crate::source_creation::pg_family::PgFamilyCreationObservations;
use crate::source_revisions::ReadonlyRecordFiles;
use crate::{DurableError, DurablePgCoordinator};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, RelativePath};
use tos_segment_store::SegmentStore;

const INVOCATION_SCHEMA: &str = "tos_local_pg_historical_profile_creation_invocation_v1";

fn prepare_id(domain: &str, command_id: &str, family_tag: &str, record_type: &str) -> Vec<u8> {
    let mut seed = b"tos-pg-historical-profile-creation-v1\0".to_vec();
    for value in [domain, command_id, family_tag, record_type] {
        seed.extend_from_slice(&(value.len() as u64).to_be_bytes());
        seed.extend_from_slice(value.as_bytes());
    }
    Digest256::of_bytes(&seed).as_bytes().to_vec()
}

fn replay_binding(
    invocation_raw: &[u8],
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
) -> Digest256 {
    fn append(seed: &mut Vec<u8>, bytes: &[u8]) {
        seed.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
        seed.extend_from_slice(bytes);
    }

    let capture = software.selection();
    let mut seed = b"tos-pg-historical-profile-replay-binding-v1\0".to_vec();
    append(&mut seed, Digest256::of_bytes(invocation_raw).as_bytes());
    append(&mut seed, software.profile().as_bytes());
    append(&mut seed, capture.source_git_commit.as_bytes());
    append(&mut seed, capture.source_git_tree.as_bytes());
    append(&mut seed, capture.capture_manifest_sha256.as_bytes());
    append(
        &mut seed,
        &(components.members().count() as u64).to_be_bytes(),
    );
    for member in components.members() {
        append(&mut seed, member.path.as_str().as_bytes());
        append(&mut seed, &member.size_bytes.to_be_bytes());
        append(&mut seed, member.sha256.as_bytes());
        append(&mut seed, &member.mode.to_be_bytes());
    }
    Digest256::of_bytes(&seed)
}

fn verify_selected_software_components(
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    for member in components.members() {
        let raw = software
            .read_selected_component(
                components,
                &member.path,
                max_member_bytes,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("selected software component changed"))?;
        if raw.len() as u64 != member.size_bytes || Digest256::of_bytes(&raw) != member.sha256 {
            return Err(SourceCommandError::Conflict(
                "selected software component fixity differs",
            ));
        }
    }
    Ok(())
}

fn creation_envelope(response: Value, historical: bool) -> Value {
    json!({
        "schema_version": if historical {
            "tos_local_pg_historical_creation_result_v1"
        } else {
            "tos_local_pg_public_profile_creation_result_v1"
        },
        "authentication":"local-unix-account",
        "result":response,
        "grants_admission":false
    })
}

fn response(
    request: &tos_foundation::JsonValue,
    owner_config: &tos_foundation::JsonValue,
    attempt: &PgCreationAttempt,
    receipt: &crate::DurableCommitReceipt,
    read_work: crate::source_cohort::revision::SourceRevisionReadWork,
    replay_verification: bool,
) -> SourceCommandResult<Value> {
    let operation = cmd::text(request, "operation")?;
    let record = cmd::field(request, "record")?;
    let record_id = cmd::text(record, "record_id")?;
    let record_type = cmd::text(record, "record_type")?;
    let configured_type = cmd::text(owner_config, "record_type")?;
    let source_path = cmd::text(owner_config, "source_path")?;
    if !["historical.create", "source.create"].contains(&operation)
        || record_id != cmd::text(owner_config, "record_id")?
        || record_type != configured_type
        || record_type != attempt.record_type
        || !attempt
            .writes
            .iter()
            .any(|write| write.subject == source_path)
    {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL Historical/Profile response identity differs",
        ));
    }
    let created_source_paths = attempt
        .writes
        .iter()
        .map(|write| write.subject.clone())
        .collect::<Vec<_>>();
    Ok(json!({
        "operation":operation,
        "record_id":record_id,
        "record_type":record_type,
        "source_path":source_path,
        "replayed":receipt.replayed,
        "created_source_paths":created_source_paths,
        "durable_commit":super::commit_result(receipt),
        "source_read_work": if replay_verification {
            super::source_read_work(
                crate::source_cohort::revision::SourceRevisionReadWork::default(),
                "durable replay verification; current source read predicates are checked by the writer",
            )
        } else {
            super::source_read_work(
                read_work,
                "bounded PG/STO Historical/Profile source selection; software capture and physical I/O excluded",
            )
        },
    }))
}

fn is_private_path(path: &RelativePath) -> bool {
    path.as_str()
        .split('/')
        .any(|part| ["owner-local", "payload", "local-content", "catalog"].contains(&part))
}

fn add_owner_paths(
    closure: &SourceRevisionIdentityClosure,
    pending: &mut BTreeSet<RelativePath>,
    required: &mut BTreeSet<RelativePath>,
) -> SourceCommandResult<()> {
    for owner in closure.owners().values().flatten() {
        let path = owner.1.clone();
        if is_private_path(&path) {
            return Err(SourceCommandError::Denied(
                "Historical Claim reference resolves to private source content",
            ));
        }
        required.insert(path.clone());
        pending.insert(path);
    }
    Ok(())
}

fn select_context_from_pg(
    session: &mut SourceRevisionReadSession<'_>,
    mut context: cmd::CommandContext,
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(cmd::CommandContext, PgFamilyCreationObservations), DurableError> {
    let requirements =
        crate::source_creation::pg_family::requirements(&context).map_err(DurableError::Source)?;
    let home = session.observe_home_absence(&requirements.home, deadline, cancelled)?;
    let unique = requirements
        .unique_tokens
        .iter()
        .map(|item| session.observe_unique_index(&item.kind, &item.token, deadline, cancelled))
        .collect::<Result<Vec<_>, _>>()?;
    let identity_closure =
        session.lookup_source_identities(&requirements.reference_tokens, deadline, cancelled)?;
    let dependency_generation =
        session.observe_dependency_commitment(COMMITMENT_VERSION, deadline, cancelled)?;

    let mut selected = BTreeMap::<RelativePath, Vec<u8>>::new();
    let mut visited = BTreeSet::<RelativePath>::new();
    let mut required = BTreeSet::<RelativePath>::new();
    let mut dependencies = BTreeSet::<RelativePath>::new();
    let mut total = context
        .files
        .iter()
        .try_fold(0u64, |sum, file| {
            sum.checked_add(file.raw.len() as u64)
                .filter(|value| *value <= max_total_bytes)
        })
        .ok_or(DurableError::Refused(
            "PG Historical/Profile selected context byte budget",
        ))?;

    loop {
        let current = crate::source_creation::pg_family::requirements(&context)
            .map_err(DurableError::Source)?;
        let mut pending = BTreeSet::new();
        for path in &current.required_source_inputs {
            required.insert(path.clone());
            pending.insert(path.clone());
        }
        for path in &current.source_refs {
            pending.insert(path.clone());
        }
        add_owner_paths(&identity_closure, &mut pending, &mut required)
            .map_err(DurableError::Source)?;
        let missing_required = required.iter().any(|path| {
            !selected.contains_key(path)
                && !context.files.iter().any(|file| file.path == *path)
                && visited.contains(path)
        });
        if missing_required {
            return Err(DurableError::Conflict(
                "PG Historical/Profile required selected source member absent",
            ));
        }
        pending.retain(|path| {
            !visited.contains(path)
                && !selected.contains_key(path)
                && !context.files.iter().any(|file| file.path == *path)
        });
        if pending.is_empty() {
            break;
        }
        while let Some(path) = pending.pop_first() {
            if !visited.insert(path.clone()) {
                continue;
            }
            if is_private_path(&path) {
                return Err(DurableError::Refused(
                    "PG Historical/Profile dependency addresses private source content",
                ));
            }
            if context.files.iter().any(|file| file.path == path) {
                continue;
            }
            if selected.len() + context.files.len() >= max_members {
                return Err(DurableError::Refused(
                    "PG Historical/Profile selected context member budget",
                ));
            }
            let present =
                ReadonlyRecordFiles::has_file(session, path.as_str(), deadline, cancelled)
                    .map_err(DurableError::Source)?;
            if !present {
                if required.contains(&path) || dependencies.contains(&path) {
                    return Err(DurableError::Conflict(
                        "PG Historical/Profile selected dependency absent",
                    ));
                }
                continue;
            }
            let member_cap = max_member_bytes.min(max_total_bytes.saturating_sub(total));
            if member_cap == 0 {
                return Err(DurableError::Refused(
                    "PG Historical/Profile selected context byte budget",
                ));
            }
            let member = session.read_current_member(&path, member_cap, deadline, cancelled)?;
            total = total
                .checked_add(member.raw.len() as u64)
                .filter(|value| *value <= max_total_bytes)
                .ok_or(DurableError::Refused(
                    "PG Historical/Profile selected context byte budget",
                ))?;
            for dependency in member.dependency_claims.as_deref().unwrap_or(&[]) {
                if dependency.as_str().starts_with("ToS/") {
                    dependencies.insert(dependency.clone());
                    pending.insert(dependency.clone());
                }
            }
            selected.insert(path, member.raw);
        }
    }
    for (path, raw) in selected {
        context.files.push(cmd::SourceFile { path, raw });
    }
    context
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    super::check_context_budget(&context, max_members, max_total_bytes, max_member_bytes)
        .map_err(DurableError::Source)?;
    context.check().map_err(DurableError::Source)?;
    Ok((
        context,
        PgFamilyCreationObservations {
            family: requirements.family,
            record_type: requirements.record_type,
            home,
            unique,
            identity_closure,
            dependency_generation,
        },
    ))
}

/// Execute one exact HistoricalV1/V2 or generic PublicProfile creation route.
pub(crate) fn run(
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    input: impl Read,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    if text(invocation, "schema_version")? != INVOCATION_SCHEMA {
        return Err(SourceCommandError::Unsupported(
            "PG Historical/Profile invocation profile",
        ));
    }
    exact(
        invocation,
        &[
            "schema_version",
            "owner_config",
            "native_executable",
            "native_executable_sha256",
            "source_selection",
            "software_capture",
            "software_restored_root",
            "software_selection",
            "software_components",
            "schema_worker",
            "budgets",
        ],
    )?;
    let selection = super::selected_source(invocation)?;
    let (max_members, max_total_bytes, max_member_bytes) = super::invocation_budgets(invocation)?;
    let _ = super::absolute(text(invocation, "native_executable")?)?;
    let runtime_observation = crate::source_serialization::observe_executable(deadline, cancelled)?;
    let expected_executable_digest = super::digest(text(invocation, "native_executable_sha256")?)?;
    if runtime_observation.current_digest(deadline, cancelled)? != expected_executable_digest {
        return Err(SourceCommandError::Conflict(
            "native Historical/Profile command executable identity",
        ));
    }

    let owner_path = super::absolute(text(invocation, "owner_config")?)?;
    let (owner_fs, owner_raw) =
        CreationFilesystem::select_creation_owner(&owner_path, deadline, cancelled)?;
    let owner_config = cmd::parse(&owner_raw)?;
    let family = CreationFamily::parse(cmd::text(&owner_config, "schema_version")?)?;
    if !matches!(
        family,
        CreationFamily::HistoricalV1 | CreationFamily::HistoricalV2 | CreationFamily::PublicProfile
    ) {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL Historical/Profile owner family",
        ));
    }
    let request_raw = super::request_bytes(input)?;
    let request = cmd::parse(&request_raw)?;
    if cmd::text(&request, "schema_version")? != "tos_local_source_command_v1" {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL Historical/Profile source request profile",
        ));
    }
    let operation = cmd::text(&request, "operation")?;
    if !["prepare-create", family.operation()].contains(&operation) {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL Historical/Profile operation",
        ));
    }
    let record = cmd::field(&request, "record")?;
    let record_type = cmd::text(record, "record_type")?.to_owned();
    if operation == family.operation() {
        let command_id = cmd::text(&request, "command_id")?;
        if command_id.is_empty() || command_id.chars().count() > MAX_JOB_ID_BYTES {
            return Err(SourceCommandError::Invalid(
                "PostgreSQL Historical/Profile command identity",
            ));
        }
    }

    let store = SegmentStore::open_existing(&selection.store_root, selection.store_limits)
        .map_err(|_| SourceCommandError::Conflict("selected private source store"))?;
    let (software, components) = super::software_selection(invocation, deadline, cancelled)?;
    if components
        .members()
        .any(|member| member.path.as_str().starts_with("ToS/"))
    {
        return Err(SourceCommandError::Denied(
            "Historical/Profile PG software selection cannot replace ToS source members",
        ));
    }
    let current_replay_binding = replay_binding(invocation_raw, &software, &components);
    let mut db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected PostgreSQL unavailable"))?,
    );
    let family_tag = crate::source_creation::pg_family::family_tag(family)?;
    let prepare_id = if operation == family.operation() {
        Some(prepare_id(
            &selection.domain,
            cmd::text(&request, "command_id")?,
            family_tag,
            &record_type,
        ))
    } else {
        None
    };
    let recovered = if let Some(prepare_id) = prepare_id.as_deref() {
        let configuration_canonical_digest = Digest256::of_bytes(&cmd::canonical(&owner_config)?);
        match db.recover_pg_creation_attempt(
            &store,
            &selection.domain,
            prepare_id,
            cmd::text(&request, "command_id")?,
            Digest256::of_bytes(&request_raw),
            &record_type,
            Digest256::of_bytes(&owner_raw),
            configuration_canonical_digest,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
        ) {
            Ok(attempt) => {
                if attempt.family_tag.as_deref() != Some(family_tag) {
                    return Err(SourceCommandError::Conflict(
                        "PG Historical/Profile ready family differs",
                    ));
                }
                if attempt.replay_binding != Some(current_replay_binding) {
                    return Err(SourceCommandError::Conflict(
                        "PG Historical/Profile invocation or software selection differs from ready attempt",
                    ));
                }
                Some(attempt)
            }
            Err(DurableError::Refused("bounded PG creation attempt absent")) => None,
            Err(error) => return Err(super::source_error(error)),
        }
    } else {
        None
    };

    if let Some(attempt) = recovered {
        verify_selected_software_components(
            &software,
            &components,
            max_member_bytes,
            deadline,
            cancelled,
        )?;
        let selected = db
            .open_source_revision_selection(
                &store,
                &selection.domain,
                &selection.job_id,
                selection.job_fence,
                deadline,
                cancelled,
            )
            .map_err(super::source_error)?;
        if selected.contract != selection.contract {
            return Err(SourceCommandError::Conflict(
                "selected source contract differs from current domain",
            ));
        }
        let replay_context = super::selected_context(
            &owner_raw,
            &request_raw,
            selected.cohort.initial_revision(),
            &software,
            &components,
            uid,
            max_members,
            max_total_bytes,
            max_member_bytes,
            deadline,
            cancelled,
        )?;
        owner_fs.current_context(&replay_context, deadline, cancelled)?;
        let owner_check = || {
            if super::read_absolute(
                invocation_path,
                uid,
                true,
                invocation_raw.len(),
                deadline,
                cancelled,
            )
            .map_err(DurableError::Source)?
                != invocation_raw
            {
                return Err(DurableError::Conflict(
                    "Historical/Profile PG invocation changed before replay",
                ));
            }
            let (current_owner, current_raw) =
                CreationFilesystem::select_protected_native_owner(&owner_path, deadline, cancelled)
                    .map_err(DurableError::Source)?;
            if current_raw != owner_raw {
                return Err(DurableError::Conflict(
                    "Historical/Profile PG owner changed before replay",
                ));
            }
            current_owner
                .current_context(&replay_context, deadline, cancelled)
                .map_err(DurableError::Source)?;
            if runtime_observation
                .current_digest(deadline, cancelled)
                .map_err(DurableError::Source)?
                != expected_executable_digest
            {
                return Err(DurableError::Conflict(
                    "native command executable changed before replay",
                ));
            }
            Ok(())
        };
        let (receipt, _) = db
            .commit_pg_creation(
                &store,
                &attempt,
                selection.contract,
                &owner_check,
                deadline,
                cancelled,
            )
            .map_err(super::source_error)?;
        let result = response(
            &request,
            &owner_config,
            &attempt,
            &receipt,
            crate::source_cohort::revision::SourceRevisionReadWork::default(),
            true,
        )?;
        return Ok(creation_envelope(result, family.historical()));
    }

    let ((context, identity, observations), readset) = db
        .with_source_revision_session(
            &store,
            &selection.domain,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                let selected = session.selection().clone();
                if selected.contract != selection.contract {
                    return Err(DurableError::Conflict(
                        "selected source contract differs from current domain",
                    ));
                }
                let identity = selected.identity();
                let context = super::selected_context(
                    &owner_raw,
                    &request_raw,
                    selected.cohort.initial_revision(),
                    &software,
                    &components,
                    uid,
                    max_members,
                    max_total_bytes,
                    max_member_bytes,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                let (context, observations) = select_context_from_pg(
                    session,
                    context,
                    max_members,
                    max_total_bytes,
                    max_member_bytes,
                    deadline,
                    cancelled,
                )?;
                owner_fs
                    .current_context(&context, deadline, cancelled)
                    .map_err(DurableError::Source)?;
                Ok((context, identity, observations))
            },
        )
        .map_err(super::source_error)?;
    observations
        .identity_closure
        .verify_readset(&readset)
        .map_err(super::source_error)?;
    let mut worker = super::revisions::selected_candidate_schema_from_context(
        invocation,
        &context.files,
        identity,
        deadline,
        cancelled,
    )?;
    let prepared = crate::source_creation::pg_family::prepare_pg_family_creation(
        context,
        identity,
        observations,
        &software,
        &components,
        current_replay_binding,
        &mut worker,
        deadline,
        cancelled,
    )?;
    if operation == "prepare-create" {
        let response = super::response_value(&prepared.preview()?)?;
        worker
            .finish(deadline, cancelled)
            .map_err(|_| SourceCommandError::Denied("native candidate schema worker finish"))?;
        return Ok(creation_envelope(response, family.historical()));
    }
    let serialized = prepared.serialize(&software, &mut worker, deadline, cancelled)?;
    worker
        .finish(deadline, cancelled)
        .map_err(|_| SourceCommandError::Denied("native candidate schema worker finish"))?;
    owner_fs.current_context(serialized.context(), deadline, cancelled)?;
    let prepare_id = prepare_id.ok_or(SourceCommandError::Invalid(
        "PG Historical/Profile prepare identity",
    ))?;
    let attempt = db
        .register_pg_creation(
            &store,
            &serialized,
            &readset,
            &prepare_id,
            deadline,
            cancelled,
        )
        .map_err(super::source_error)?;
    let owner_check = || {
        if super::read_absolute(
            invocation_path,
            uid,
            true,
            invocation_raw.len(),
            deadline,
            cancelled,
        )
        .map_err(DurableError::Source)?
            != invocation_raw
        {
            return Err(DurableError::Conflict(
                "Historical/Profile PG invocation changed before commit",
            ));
        }
        let (current_owner, current_raw) =
            CreationFilesystem::select_protected_native_owner(&owner_path, deadline, cancelled)
                .map_err(DurableError::Source)?;
        if current_raw != owner_raw {
            return Err(DurableError::Conflict(
                "Historical/Profile PG owner changed before commit",
            ));
        }
        current_owner
            .current_context(serialized.context(), deadline, cancelled)
            .map_err(DurableError::Source)?;
        if runtime_observation
            .current_digest(deadline, cancelled)
            .map_err(DurableError::Source)?
            != expected_executable_digest
        {
            return Err(DurableError::Conflict(
                "native command executable changed before commit",
            ));
        }
        Ok(())
    };
    let (receipt, _) = db
        .commit_pg_creation(
            &store,
            &attempt,
            selection.contract,
            &owner_check,
            deadline,
            cancelled,
        )
        .map_err(super::source_error)?;
    let result = response(
        &request,
        &owner_config,
        &attempt,
        &receipt,
        readset.work,
        false,
    )?;
    Ok(creation_envelope(result, family.historical()))
}
