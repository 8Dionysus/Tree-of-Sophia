//! Native initial Corpus creation over a bounded PostgreSQL/STO read epoch.
//!
//! This entry selects the protected creation owner, current PG job lease,
//! software capture and bounded bootstrap context. It does not select a full
//! corpus cut: the PG creation adapter supplies only the source members named
//! by the request and schema closure.

use super::{absolute, capped, digest, exact, number, text};
use crate::source_cohort::creation::{
    COMMITMENT_VERSION, PgCorpusCreationAttempt, PgCorpusCreationInput,
};
use crate::source_cohort::revision::SourceRevisionReadSession;
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation::{self as owner, CreationFamily};
use crate::source_creation_store::CreationFilesystem;
use crate::source_revisions::ReadonlyRecordFiles;
use crate::{DurableError, DurablePgCoordinator};
use serde_json::{Value, json};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonLimits, RelativePath, SourceRevision};
use tos_segment_store::{SegmentLimits, SegmentStore};
use tos_source_store::{
    ReadLimits, SoftwareCaptureReader, SoftwareCaptureSelectionV1, SoftwareComponentSelectionV1,
};
use tos_validation::source_cut::CutSchemaExecutor;

const MAX_REQUEST: usize = 1_048_576;
const MAX_DOMAIN_BYTES: usize = 1_024;
const MAX_JOB_ID_BYTES: usize = 256;
const MAX_CONTEXT_MEMBERS: u64 = 512;
const MAX_CONTEXT_BYTES: u64 = 33_554_432;
const MAX_CONTEXT_MEMBER_BYTES: u64 = 8_388_608;

#[path = "source_native_pg_family_creation_cli.rs"]
pub(crate) mod family;

struct SourceSelection {
    domain: String,
    contract: Digest256,
    store_root: PathBuf,
    store_limits: SegmentLimits,
    job_id: String,
    job_fence: u64,
}

fn selected_source(invocation: &Value) -> SourceCommandResult<SourceSelection> {
    let value = &invocation["source_selection"];
    exact(
        value,
        &[
            "domain",
            "source_contract_sha256",
            "store_root",
            "store_limits",
            "job_id",
            "job_fence",
        ],
    )?;
    let domain = text(value, "domain")?;
    if domain.is_empty() || domain.len() > MAX_DOMAIN_BYTES || domain.contains('\0') {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL Corpus creation domain",
        ));
    }
    let job_id = text(value, "job_id")?;
    if job_id.is_empty() || job_id.len() > MAX_JOB_ID_BYTES || job_id.contains('\0') {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL Corpus creation job identity",
        ));
    }
    let job_fence = number(value, "job_fence")?;
    if job_fence == 0 {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL Corpus creation job fence",
        ));
    }
    let limits = &value["store_limits"];
    exact(
        limits,
        &[
            "max_segment_bytes",
            "max_frame_bytes",
            "max_frames",
            "max_journal_bytes",
        ],
    )?;
    let max_frames = u32::try_from(number(limits, "max_frames")?)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL Corpus creation store limits"))?;
    let max_journal_bytes = usize::try_from(number(limits, "max_journal_bytes")?)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL Corpus creation store limits"))?;
    Ok(SourceSelection {
        domain: domain.to_owned(),
        contract: digest(text(value, "source_contract_sha256")?)?,
        store_root: absolute(text(value, "store_root")?)?,
        store_limits: SegmentLimits {
            max_segment_bytes: number(limits, "max_segment_bytes")?,
            max_frame_bytes: number(limits, "max_frame_bytes")?,
            max_frames,
            max_journal_bytes,
        },
        job_id: job_id.to_owned(),
        job_fence,
    })
}

fn invocation_budgets(invocation: &Value) -> SourceCommandResult<(usize, u64, u64)> {
    let budgets = &invocation["budgets"];
    exact(
        budgets,
        &[
            "max_members",
            "max_total_bytes",
            "max_member_bytes",
            "max_schema_receipts",
            "max_schema_receipt_bytes",
            "worker_cpu_seconds",
            "worker_address_space_bytes",
        ],
    )?;
    let max_members = usize::try_from(capped(budgets, "max_members", MAX_CONTEXT_MEMBERS)?)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL Corpus creation context budget"))?;
    let max_total_bytes = capped(budgets, "max_total_bytes", MAX_CONTEXT_BYTES)?;
    let max_member_bytes = capped(budgets, "max_member_bytes", MAX_CONTEXT_MEMBER_BYTES)?;
    // Apply the same finite worker limits when constructing the selected schema
    // executor after the adapter has loaded its bounded source context.
    let _ = capped(budgets, "max_schema_receipts", 128)?;
    let _ = capped(budgets, "max_schema_receipt_bytes", 262_144)?;
    let _ = capped(budgets, "worker_cpu_seconds", 3)?;
    let _ = capped(budgets, "worker_address_space_bytes", 1_073_741_824)?;
    Ok((max_members, max_total_bytes, max_member_bytes))
}

fn check_context_budget(
    context: &cmd::CommandContext,
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
) -> SourceCommandResult<()> {
    if context.files.len() > max_members {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL Corpus creation selected member budget",
        ));
    }
    let mut total = 0u64;
    for file in &context.files {
        let bytes = u64::try_from(file.raw.len())
            .map_err(|_| SourceCommandError::Invalid("Corpus creation byte budget"))?;
        if bytes > max_member_bytes {
            return Err(SourceCommandError::Invalid(
                "PostgreSQL Corpus creation member byte budget",
            ));
        }
        total = total
            .checked_add(bytes)
            .filter(|sum| *sum <= max_total_bytes)
            .ok_or(SourceCommandError::Invalid(
                "PostgreSQL Corpus creation total byte budget",
            ))?;
    }
    Ok(())
}

fn software_selection(
    invocation: &Value,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(SoftwareCaptureReader, SoftwareComponentSelectionV1)> {
    let read_limits = ReadLimits {
        max_manifest_bytes: 1_048_576,
        max_manifest_entries: 512,
        max_selected_object_bytes: MAX_CONTEXT_BYTES,
        json: JsonLimits::default(),
    };
    let selected = &invocation["software_selection"];
    exact(
        selected,
        &[
            "source_git_commit",
            "source_git_tree",
            "capture_manifest_sha256",
        ],
    )?;
    let software = SoftwareCaptureReader::open(
        &absolute(text(invocation, "software_capture")?)?,
        &absolute(text(invocation, "software_restored_root")?)?,
        SoftwareCaptureSelectionV1 {
            source_git_commit: text(selected, "source_git_commit")?.to_owned(),
            source_git_tree: text(selected, "source_git_tree")?.to_owned(),
            capture_manifest_sha256: digest(text(selected, "capture_manifest_sha256")?)?,
        },
        read_limits,
        deadline,
        cancelled,
    )
    .map_err(|_| SourceCommandError::Conflict("selected software capture"))?;
    let component_values = invocation["software_components"]
        .as_array()
        .filter(|items| !items.is_empty() && items.len() <= 128)
        .ok_or(SourceCommandError::Invalid("selected software components"))?;
    let component_paths = component_values
        .iter()
        .map(|value| {
            let value = value.as_str().ok_or(SourceCommandError::Invalid(
                "selected software component path",
            ))?;
            RelativePath::parse(value)
                .map_err(|_| SourceCommandError::Invalid("selected software component path"))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    if component_paths
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != component_paths.len()
    {
        return Err(SourceCommandError::Invalid(
            "duplicate selected software component",
        ));
    }
    let components = software
        .select_components(&component_paths)
        .map_err(|_| SourceCommandError::Conflict("selected software components"))?;
    Ok((software, components))
}

fn selected_context(
    owner_raw: &[u8],
    request_raw: &[u8],
    base_revision: SourceRevision,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    effective_uid: u32,
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<cmd::CommandContext> {
    let mut files = Vec::new();
    let mut total = 0u64;
    for member in components.members() {
        if files.len() >= max_members || member.size_bytes > max_member_bytes {
            return Err(SourceCommandError::Invalid(
                "selected software component context budget",
            ));
        }
        total = total
            .checked_add(member.size_bytes)
            .filter(|sum| *sum <= max_total_bytes)
            .ok_or(SourceCommandError::Invalid(
                "selected software component total byte budget",
            ))?;
        let raw = software
            .read_selected_component(
                components,
                &member.path,
                max_member_bytes,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("selected software component read"))?;
        if raw.len() as u64 != member.size_bytes {
            return Err(SourceCommandError::Conflict(
                "selected software component size changed",
            ));
        }
        files.push(cmd::SourceFile {
            path: member.path.clone(),
            raw,
        });
    }
    let context = cmd::CommandContext {
        // This is the actual source bootstrap revision, not the current PG
        // generation identity. The adapter carries both identities explicitly.
        base_revision,
        configuration_raw: owner_raw.to_vec(),
        request_raw: request_raw.to_vec(),
        recorded_at: crate::source_serialization::instant()?,
        effective_uid: u64::from(effective_uid),
        files,
    };
    check_context_budget(&context, max_members, max_total_bytes, max_member_bytes)?;
    context.check()?;
    Ok(context)
}

fn select_context_from_pg(
    session: &mut SourceRevisionReadSession<'_>,
    mut context: cmd::CommandContext,
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(cmd::CommandContext, owner::pg::PgCorpusCreationObservations), DurableError> {
    let requirements = owner::pg::requirements(&context).map_err(DurableError::Source)?;
    let home_observation = session.observe_home_absence(&requirements.home, deadline, cancelled)?;
    let unique_observations = requirements
        .unique_tokens
        .iter()
        .map(|requirement| {
            session.observe_unique_index(&requirement.kind, &requirement.token, deadline, cancelled)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let dependency_generation =
        session.observe_dependency_commitment(COMMITMENT_VERSION, deadline, cancelled)?;

    let required_inputs = requirements
        .required_source_inputs
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let mut pending = required_inputs.clone();
    for path in &requirements.source_refs {
        if path.as_str().starts_with("ToS/") {
            pending.insert(path.clone());
        }
    }
    let mut selected = std::collections::BTreeMap::<RelativePath, Vec<u8>>::new();
    let mut total = context
        .files
        .iter()
        .try_fold(0u64, |sum, file| {
            sum.checked_add(file.raw.len() as u64)
                .filter(|value| *value <= max_total_bytes)
        })
        .ok_or(DurableError::Refused(
            "PG Corpus selected context byte budget",
        ))?;
    while let Some(path) = pending.pop_first() {
        if selected.contains_key(&path) {
            continue;
        }
        if context.files.iter().any(|file| file.path == path) {
            return Err(DurableError::Invalid("duplicate PG Corpus context path"));
        }
        let path_text = path.as_str();
        let present = ReadonlyRecordFiles::has_file(session, path_text, deadline, cancelled)
            .map_err(DurableError::Source)?;
        if !present {
            if required_inputs.contains(&path) {
                return Err(DurableError::Conflict(
                    "PG Corpus selected schema resource absent",
                ));
            }
            continue;
        }
        if selected.len() + context.files.len() >= max_members {
            return Err(DurableError::Refused(
                "PG Corpus selected context member budget",
            ));
        }
        if path_text
            .split('/')
            .any(|part| ["owner-local", "payload", "local-content"].contains(&part))
        {
            return Err(DurableError::Refused(
                "PG Corpus addressed input requires private/native reader",
            ));
        }
        let member_cap = max_member_bytes.min(max_total_bytes.saturating_sub(total));
        if member_cap == 0 {
            return Err(DurableError::Refused(
                "PG Corpus selected context byte budget",
            ));
        }
        let member = session.read_current_member(&path, member_cap, deadline, cancelled)?;
        total = total
            .checked_add(member.raw.len() as u64)
            .filter(|value| *value <= max_total_bytes)
            .ok_or(DurableError::Refused(
                "PG Corpus selected context byte budget",
            ))?;
        for dependency in member.dependency_claims.as_deref().unwrap_or(&[]) {
            if dependency.as_str().starts_with("ToS/") {
                pending.insert(dependency.clone());
            }
        }
        selected.insert(path, member.raw);
    }
    for (path, raw) in selected {
        context.files.push(cmd::SourceFile { path, raw });
    }
    context
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    check_context_budget(&context, max_members, max_total_bytes, max_member_bytes)
        .map_err(DurableError::Source)?;
    context.check().map_err(DurableError::Source)?;
    Ok((
        context,
        owner::pg::PgCorpusCreationObservations {
            record_type: requirements.record_type,
            home: home_observation,
            unique: unique_observations,
            dependency_generation,
        },
    ))
}

fn creation_envelope(response: Value, record_type: &str) -> Value {
    let schema_version = if record_type == "agent" {
        "tos_local_pg_agent_creation_result_v1"
    } else {
        "tos_local_pg_corpus_creation_result_v1"
    };
    json!({
        "schema_version":schema_version,
        "authentication":"local-unix-account",
        "result":response,
        "grants_admission":false
    })
}

fn source_read_work(
    work: crate::source_cohort::revision::SourceRevisionReadWork,
    scope: &str,
) -> Value {
    json!({
        "scope":scope,
        "exact_source_members_read":work.exact_source_members_read,
        "exact_source_bytes_read":work.exact_source_bytes_read,
        "custody_metadata_bytes_read":work.custody_metadata_bytes_read,
        "segment_bytes_read":work.segment_bytes_read,
        "committed_member_rows_examined":work.committed_member_rows_examined,
        "metadata_member_lookups":work.metadata_member_lookups,
        "directory_index_rows_examined":work.directory_index_rows_examined,
        "unique_index_lookups":work.unique_index_lookups,
    })
}

fn durable_creation_response(
    request: &tos_foundation::JsonValue,
    owner_config: &tos_foundation::JsonValue,
    attempt: &PgCorpusCreationAttempt,
    receipt: &crate::DurableCommitReceipt,
    read_work: crate::source_cohort::revision::SourceRevisionReadWork,
    replay_verification: bool,
) -> SourceCommandResult<Value> {
    let operation = cmd::text(request, "operation")?;
    let record_id = cmd::text(cmd::field(request, "record")?, "record_id")?;
    let record_type = cmd::text(cmd::field(request, "record")?, "record_type")?;
    let configured_record_id = cmd::text(owner_config, "record_id")?;
    let source_path = cmd::text(owner_config, "source_path")?;
    if operation != "source.create"
        || record_id != configured_record_id
        || record_type != cmd::text(owner_config, "record_type")?
        || record_type != attempt.record_type
        || !attempt
            .writes
            .iter()
            .any(|write| write.subject == source_path)
    {
        return Err(SourceCommandError::Conflict(
            "PostgreSQL Corpus creation response identity differs",
        ));
    }
    let created_source_paths = attempt
        .writes
        .iter()
        .map(|write| write.subject.clone())
        .collect::<Vec<_>>();
    let read_work = if replay_verification {
        source_read_work(
            crate::source_cohort::revision::SourceRevisionReadWork::default(),
            "durable replay verification; excluded from ordinary source selection",
        )
    } else {
        source_read_work(
            read_work,
            "bounded PG/STO source selection; software capture and physical I/O excluded",
        )
    };
    let mut response = json!({
        "operation":operation,
        "record_id":record_id,
        "source_path":source_path,
        "replayed":receipt.replayed,
        "created_source_paths":created_source_paths,
        "durable_commit":commit_result(receipt),
        "source_read_work":read_work,
    });
    if record_type != "agent" {
        response["record_type"] = json!(record_type);
    }
    Ok(response)
}

/// Execute one explicit PostgreSQL initial-Corpus creation invocation. The PG
/// session reads only required schema resources, addressed references and
/// their declared dependency closure; no full source cut is selected.
pub(super) fn run(
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    input: impl Read,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
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
    let selection = selected_source(invocation)?;
    let (max_members, max_total_bytes, max_member_bytes) = invocation_budgets(invocation)?;
    let _ = absolute(text(invocation, "native_executable")?)?;
    let runtime_observation = crate::source_serialization::observe_executable(deadline, cancelled)?;
    let expected_executable_digest = digest(text(invocation, "native_executable_sha256")?)?;
    if runtime_observation.current_digest(deadline, cancelled)? != expected_executable_digest {
        return Err(SourceCommandError::Conflict(
            "native command executable identity",
        ));
    }

    let owner_path = absolute(text(invocation, "owner_config")?)?;
    let (owner_fs, owner_raw) =
        CreationFilesystem::select_creation_owner(&owner_path, deadline, cancelled)?;
    let owner_config = cmd::parse(&owner_raw)?;
    let family = CreationFamily::parse(cmd::text(&owner_config, "schema_version")?)?;
    if !matches!(family, CreationFamily::CorpusV1 | CreationFamily::CorpusV2) {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL initial Corpus owner",
        ));
    }
    let request_raw = request_bytes(input)?;
    let request = cmd::parse(&request_raw)?;
    if cmd::text(&request, "schema_version")? != "tos_local_source_command_v1" {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL initial Corpus request profile",
        ));
    }
    let operation = cmd::text(&request, "operation")?;
    let record_type = cmd::text(cmd::field(&request, "record")?, "record_type")?.to_owned();
    if !matches!(operation, "prepare-create" | "source.create") {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL initial Corpus operation",
        ));
    }
    if operation == "source.create" {
        let command_id = cmd::text(&request, "command_id")?;
        if command_id.is_empty() || command_id.chars().count() > MAX_JOB_ID_BYTES {
            return Err(SourceCommandError::Invalid(
                "PostgreSQL initial Corpus command identity",
            ));
        }
    }

    let store = SegmentStore::open_existing(&selection.store_root, selection.store_limits)
        .map_err(|_| SourceCommandError::Conflict("selected private source store"))?;
    let (software, components) = software_selection(invocation, deadline, cancelled)?;
    let mut db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected PostgreSQL unavailable"))?,
    );
    if operation == "source.create" {
        let command_id = cmd::text(&request, "command_id")?;
        let prepare_id = prepare_id(&selection.domain, command_id, &record_type);
        let configuration_canonical_digest = Digest256::of_bytes(&cmd::canonical(&owner_config)?);
        match db.recover_pg_corpus_creation_attempt(
            &store,
            &selection.domain,
            &prepare_id,
            command_id,
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
                let selected = db
                    .open_source_revision_selection(
                        &store,
                        &selection.domain,
                        &selection.job_id,
                        selection.job_fence,
                        deadline,
                        cancelled,
                    )
                    .map_err(source_error)?;
                if selected.contract != selection.contract {
                    return Err(SourceCommandError::Conflict(
                        "selected source contract differs from current domain",
                    ));
                }
                let replay_context = selected_context(
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
                            "PostgreSQL Corpus creation invocation changed before replay",
                        ));
                    }
                    let (current_owner, current_raw) =
                        CreationFilesystem::select_protected_native_owner(
                            &owner_path,
                            deadline,
                            cancelled,
                        )
                        .map_err(DurableError::Source)?;
                    if current_raw != owner_raw {
                        return Err(DurableError::Conflict(
                            "PostgreSQL Corpus creation owner changed before replay",
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
                    .commit_pg_corpus_creation(
                        &store,
                        &attempt,
                        selection.contract,
                        &owner_check,
                        deadline,
                        cancelled,
                    )
                    .map_err(source_error)?;
                let response = durable_creation_response(
                    &request,
                    &owner_config,
                    &attempt,
                    &receipt,
                    crate::source_cohort::revision::SourceRevisionReadWork::default(),
                    true,
                )?;
                return Ok(creation_envelope(response, &record_type));
            }
            Err(DurableError::Refused("bounded PG creation attempt absent")) => {}
            Err(error) => return Err(source_error(error)),
        }
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
                let context = selected_context(
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
        .map_err(source_error)?;
    let mut worker = super::revisions::selected_candidate_schema_from_context(
        invocation,
        &context.files,
        identity,
        deadline,
        cancelled,
    )?;
    let prepared = owner::pg::prepare_pg_corpus_creation(
        context,
        identity,
        observations,
        &software,
        &components,
        &mut worker,
        deadline,
        cancelled,
    )?;
    if operation == "prepare-create" {
        let response = prepared.preview()?;
        worker
            .finish(deadline, cancelled)
            .map_err(|_| SourceCommandError::Denied("native candidate schema worker finish"))?;
        return Ok(creation_envelope(response_value(&response)?, &record_type));
    }

    let serialized = prepared.serialize(&software, &mut worker, deadline, cancelled)?;
    worker
        .finish(deadline, cancelled)
        .map_err(|_| SourceCommandError::Denied("native candidate schema worker finish"))?;
    owner_fs.current_context(serialized.context(), deadline, cancelled)?;
    let command_id = cmd::text(&request, "command_id")?;
    let prepare_id = prepare_id(&selection.domain, command_id, &record_type);
    let attempt = if serialized.plan().replayed {
        db.recover_pg_corpus_creation_attempt(
            &store,
            &selection.domain,
            &prepare_id,
            command_id,
            Digest256::of_bytes(&request_raw),
            &record_type,
            serialized.plan().configuration_raw_sha256,
            serialized.plan().configuration_canonical_sha256,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
        )
        .map_err(source_error)?
    } else {
        db.register_pg_corpus_creation(
            &store,
            &serialized,
            &readset,
            &prepare_id,
            deadline,
            cancelled,
        )
        .map_err(source_error)?
    };
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
                "PostgreSQL Corpus creation invocation changed before commit",
            ));
        }
        let (current_owner, current_raw) =
            CreationFilesystem::select_protected_native_owner(&owner_path, deadline, cancelled)
                .map_err(DurableError::Source)?;
        if current_raw != owner_raw {
            return Err(DurableError::Conflict(
                "PostgreSQL Corpus creation owner changed before commit",
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
        .commit_pg_corpus_creation(
            &store,
            &attempt,
            selection.contract,
            &owner_check,
            deadline,
            cancelled,
        )
        .map_err(source_error)?;
    let result = durable_creation_response(
        &request,
        &owner_config,
        &attempt,
        &receipt,
        readset.work,
        false,
    )?;
    drop(owner_fs);
    Ok(creation_envelope(result, &record_type))
}

fn source_error(error: DurableError) -> SourceCommandError {
    match error {
        DurableError::Source(error) => error,
        DurableError::Conflict(_) => {
            SourceCommandError::Conflict("PostgreSQL Corpus creation selection changed")
        }
        DurableError::Refused(_) => {
            SourceCommandError::Denied("PostgreSQL Corpus creation selection refused")
        }
        DurableError::Invalid(_) | DurableError::Corrupt(_) => {
            SourceCommandError::Invalid("PostgreSQL Corpus creation metadata")
        }
        DurableError::Database(_) | DurableError::Storage(_) | DurableError::Indeterminate(_) => {
            SourceCommandError::Conflict("PostgreSQL Corpus creation unavailable")
        }
    }
}

fn response_value(value: &tos_foundation::JsonValue) -> SourceCommandResult<Value> {
    serde_json::from_slice(&cmd::canonical(value)?)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL Corpus creation response JSON"))
}

fn request_bytes(input: impl Read) -> SourceCommandResult<Vec<u8>> {
    let mut raw = Vec::new();
    input
        .take((MAX_REQUEST + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL Corpus creation request read"))?;
    if raw.len() > MAX_REQUEST {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL Corpus creation request budget",
        ));
    }
    Ok(raw)
}

fn prepare_id(domain: &str, command_id: &str, record_type: &str) -> Vec<u8> {
    let mut seed = if record_type == "agent" {
        b"tos-pg-agent-creation-v1\0".to_vec()
    } else {
        b"tos-pg-corpus-creation-v1\0".to_vec()
    };
    seed.extend_from_slice(&(domain.len() as u64).to_be_bytes());
    seed.extend_from_slice(domain.as_bytes());
    seed.extend_from_slice(&(command_id.len() as u64).to_be_bytes());
    seed.extend_from_slice(command_id.as_bytes());
    if record_type != "agent" {
        seed.extend_from_slice(&(record_type.len() as u64).to_be_bytes());
        seed.extend_from_slice(record_type.as_bytes());
    }
    Digest256::of_bytes(&seed).as_bytes().to_vec()
}

fn commit_result(receipt: &crate::DurableCommitReceipt) -> Value {
    json!({
        "domain": receipt.domain,
        "command_id": receipt.command_id,
        "prepare_id_sha256": Digest256::of_bytes(&receipt.prepare_id).to_prefixed(),
        "commit_seq": receipt.commit_seq,
        "request_sha256": receipt.raw_request_digest.to_prefixed(),
        "delta_sha256": receipt.delta_digest.to_prefixed(),
        "member_root_sha256": receipt.member_root.to_prefixed(),
        "replayed": receipt.replayed,
    })
}
