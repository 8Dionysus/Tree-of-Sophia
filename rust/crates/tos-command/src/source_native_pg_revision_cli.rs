//! Native `record.revise` over a bounded PostgreSQL/STO read epoch.
//!
//! This route selects an existing domain, private store, owner configuration,
//! software capture and current job lease. It routes each admitted maintained
//! revision family through that family's existing source-context rules. It
//! neither discovers a corpus cut nor creates or advances the selected lease.

use super::{absolute, capped, digest, exact, number, text};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::CreationFilesystem;
use crate::{DurableError, DurablePgCoordinator};
use serde_json::{Value, json};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonLimits, RelativePath};
use tos_segment_store::{SegmentLimits, SegmentStore};
use tos_source_store::{ReadLimits, SoftwareCaptureReader, SoftwareCaptureSelectionV1};

const MAX_REQUEST: usize = 1_048_576;
const MAX_DOMAIN_BYTES: usize = 1_024;
const MAX_JOB_ID_BYTES: usize = 256;
const MAX_CONTEXT_MEMBERS: u64 = 512;
const MAX_CONTEXT_BYTES: u64 = 8_388_608;
const MAX_CONTEXT_MEMBER_BYTES: u64 = 8_388_608;

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
            "PostgreSQL source revision domain",
        ));
    }
    let job_id = text(value, "job_id")?;
    if job_id.is_empty() || job_id.len() > MAX_JOB_ID_BYTES || job_id.contains('\0') {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL source revision job identity",
        ));
    }
    let job_fence = number(value, "job_fence")?;
    if job_fence == 0 {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL source revision job fence",
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
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL source revision store limits"))?;
    let max_journal_bytes = usize::try_from(number(limits, "max_journal_bytes")?)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL source revision store limits"))?;
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
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL source revision context budget"))?;
    let max_total_bytes = capped(budgets, "max_total_bytes", MAX_CONTEXT_BYTES)?;
    let max_member_bytes = capped(budgets, "max_member_bytes", MAX_CONTEXT_MEMBER_BYTES)?;
    // Validate the worker's resource envelope before opening the selected PG
    // session. The schema caller applies the same values to its executor.
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
            "PostgreSQL source revision selected member budget",
        ));
    }
    let mut total = 0u64;
    for file in &context.files {
        let bytes = u64::try_from(file.raw.len())
            .map_err(|_| SourceCommandError::Invalid("source revision byte budget"))?;
        if bytes > max_member_bytes {
            return Err(SourceCommandError::Invalid(
                "PostgreSQL source revision member byte budget",
            ));
        }
        total = total
            .checked_add(bytes)
            .filter(|sum| *sum <= max_total_bytes)
            .ok_or(SourceCommandError::Invalid(
                "PostgreSQL source revision total byte budget",
            ))?;
    }
    Ok(())
}

fn software_selection(
    invocation: &Value,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(
    SoftwareCaptureReader,
    tos_source_store::SoftwareComponentSelectionV1,
)> {
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

fn source_error(error: DurableError) -> SourceCommandError {
    match error {
        DurableError::Source(error) => error,
        // Conflict reasons are static owner-authored labels. Preserve them so
        // a failed pre-seal guard can be diagnosed without returning request,
        // configuration, or database payloads.
        DurableError::Conflict(reason) => SourceCommandError::Conflict(reason),
        // Refusal reasons are static owner-authored labels, just like conflict
        // reasons above. Keeping them makes bounded guard failures diagnosable
        // without exposing request, configuration, or database payloads.
        DurableError::Refused(reason) => SourceCommandError::Denied(reason),
        DurableError::Invalid(_) | DurableError::Corrupt(_) => {
            SourceCommandError::Invalid("PostgreSQL source revision metadata")
        }
        DurableError::Database(_) | DurableError::Storage(_) | DurableError::Indeterminate(_) => {
            SourceCommandError::Conflict("PostgreSQL source revision unavailable")
        }
    }
}

fn value(value: &tos_foundation::JsonValue) -> SourceCommandResult<Value> {
    serde_json::from_slice(&cmd::canonical(value)?)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL revision response JSON"))
}

fn request_bytes(input: impl Read) -> SourceCommandResult<Vec<u8>> {
    let mut raw = Vec::new();
    input
        .take((MAX_REQUEST + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL revision request read"))?;
    if raw.len() > MAX_REQUEST {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL revision request budget",
        ));
    }
    Ok(raw)
}

fn provision_selected_revision_schema(
    operation: &str,
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    uid: u32,
    owner_path: &Path,
    owner_fs: &CreationFilesystem,
    owner_raw: &[u8],
    request_raw: &[u8],
    selection: &SourceSelection,
    store: &SegmentStore,
    db: &mut DurablePgCoordinator,
    runtime_observation: &crate::source_serialization::ExecutableObservation,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    let selected = db
        .open_source_revision_selection(
            store,
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
    let context = cmd::CommandContext {
        base_revision: selected.cohort.initial_revision(),
        configuration_raw: owner_raw.to_vec(),
        request_raw: request_raw.to_vec(),
        recorded_at: crate::source_serialization::instant()?,
        effective_uid: u64::from(uid),
        files: Vec::new(),
    };
    match operation {
        "work.revision-schema.provision" => {
            crate::source_revisions::require_work_revision_schema_provision(&context)?;
        }
        "record.revision-schema.provision" => {
            crate::source_revisions::require_selected_revision_schema_provision(&context)?;
        }
        _ => {
            return Err(SourceCommandError::Unsupported(
                "PostgreSQL selected revision schema operation",
            ));
        }
    }
    owner_fs.current_context(&context, deadline, cancelled)?;
    let expected_executable_digest = digest(text(invocation, "native_executable_sha256")?)?;
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
                "PostgreSQL revision invocation changed before provision",
            ));
        }
        let (current_fs, current_raw) = CreationFilesystem::select_protected_native_owner_at_root(
            owner_path,
            &selection.store_root,
            deadline,
            cancelled,
        )
        .map_err(DurableError::Source)?;
        if current_raw != owner_raw {
            return Err(DurableError::Conflict(
                "PostgreSQL revision owner configuration changed before provision",
            ));
        }
        current_fs
            .current_context(&context, deadline, cancelled)
            .map_err(DurableError::Source)?;
        if runtime_observation
            .current_digest(deadline, cancelled)
            .map_err(DurableError::Source)?
            != expected_executable_digest
        {
            return Err(DurableError::Conflict(
                "native command executable changed before provision",
            ));
        }
        Ok(())
    };
    let created = db
        .provision_work_revision_schema(deadline, cancelled, owner_check)
        .map_err(source_error)?;
    Ok(json!({
        "schema_version":if operation == "work.revision-schema.provision" {
            "tos_local_pg_work_revision_schema_provision_result_v1"
        } else {
            "tos_local_pg_selected_revision_schema_provision_result_v1"
        },
        "extension":"cmd2_work_revision_history_v1",
        "created":created,
        "verified_schema_v1":true,
        "grants_admission":false
    }))
}

fn prepare_id(domain: &str, command_id: &str) -> Vec<u8> {
    let mut seed = b"tos-pg-record-revision-v1\0".to_vec();
    seed.extend_from_slice(&(domain.len() as u64).to_be_bytes());
    seed.extend_from_slice(domain.as_bytes());
    seed.extend_from_slice(&(command_id.len() as u64).to_be_bytes());
    seed.extend_from_slice(command_id.as_bytes());
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

/// Complete one explicit PostgreSQL record revision invocation. The PG read
/// epoch selects only the source members needed by the current owner command.
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
    // The running image is verified below; retain the invocation's declared
    // executable locator as a well-formed absolute path as well.
    let _ = absolute(text(invocation, "native_executable")?)?;
    let runtime_observation = crate::source_serialization::observe_executable(deadline, cancelled)?;
    if runtime_observation.current_digest(deadline, cancelled)?
        != digest(text(invocation, "native_executable_sha256")?)?
    {
        return Err(SourceCommandError::Conflict(
            "native command executable identity",
        ));
    }
    // The protected owner root is explicitly tied to the selected existing
    // STO root; this route has no filesystem source mirror to select instead.
    let owner_path = absolute(text(invocation, "owner_config")?)?;
    let (owner_fs, owner_raw) = CreationFilesystem::select_protected_native_owner_at_root(
        &owner_path,
        &selection.store_root,
        deadline,
        cancelled,
    )?;
    let owner = cmd::parse(&owner_raw)?;
    let owner_schema = cmd::text(&owner, "schema_version")?;
    let revision_family = crate::source_revisions::RevisionFamily::parse(owner_schema)?;
    let flat_owner = revision_family == crate::source_revisions::RevisionFamily::CorpusFlat;
    let selected_corpus_owner = matches!(
        revision_family,
        crate::source_revisions::RevisionFamily::CorpusSelectedV2
            | crate::source_revisions::RevisionFamily::CorpusSelectedV3
    );
    let historical_owner = revision_family == crate::source_revisions::RevisionFamily::Historical;
    let native_metadata_owner =
        revision_family == crate::source_revisions::RevisionFamily::NativeSelected;
    let work_owner = revision_family == crate::source_revisions::RevisionFamily::CorpusSelectedV3
        && cmd::text(&owner, "record_type")? == "work";
    if !flat_owner
        && !selected_corpus_owner
        && !historical_owner
        && !native_metadata_owner
        && !work_owner
    {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL maintained metadata revision owner",
        ));
    }
    let request_raw = request_bytes(input)?;
    let request = cmd::parse(&request_raw)?;
    if cmd::text(&request, "schema_version")? != "tos_local_source_command_v1" {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL record revision request profile",
        ));
    }
    let operation = cmd::text(&request, "operation")?;
    if !matches!(
        operation,
        "describe"
            | "prepare-revise"
            | "record.revise"
            | "work.revision-schema.provision"
            | "record.revision-schema.provision"
    ) {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL record revision operation",
        ));
    }
    if matches!(
        operation,
        "work.revision-schema.provision" | "record.revision-schema.provision"
    ) {
        cmd::exact_keys(&request, &["schema_version", "operation"])?;
        if operation == "work.revision-schema.provision" && !work_owner {
            return Err(SourceCommandError::Denied(
                "protected Work revision schema provisioning grant",
            ));
        }
        if operation == "record.revision-schema.provision" && !revision_family.selected() {
            return Err(SourceCommandError::Denied(
                "protected selected revision schema provisioning grant",
            ));
        }
    }
    if work_owner
        && operation == "record.revise"
        && cmd::field(&request, "expected_publication")? != &tos_foundation::JsonValue::Null
    {
        return Err(SourceCommandError::Conflict(
            "PG Work route owns a null publication predecessor",
        ));
    }
    let selected_store_root = selection
        .store_root
        .to_str()
        .ok_or(SourceCommandError::Invalid(
            "PostgreSQL source store root UTF-8",
        ))?;
    if cmd::text(&owner, "source_root")? != selected_store_root {
        return Err(SourceCommandError::Denied(
            "PostgreSQL owner root differs from selected STO root",
        ));
    }
    let source_path = cmd::text(&owner, "source_path")?;
    RelativePath::parse(source_path)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL revision source path"))?;
    let store = SegmentStore::open_existing(&selection.store_root, selection.store_limits)
        .map_err(|_| SourceCommandError::Conflict("selected private source store"))?;
    let mut db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected PostgreSQL unavailable"))?,
    );
    if matches!(
        operation,
        "work.revision-schema.provision" | "record.revision-schema.provision"
    ) {
        let result = provision_selected_revision_schema(
            operation,
            invocation_path,
            invocation_raw,
            invocation,
            uid,
            &owner_path,
            &owner_fs,
            &owner_raw,
            &request_raw,
            &selection,
            &store,
            &mut db,
            &runtime_observation,
            deadline,
            cancelled,
        )?;
        drop(owner_fs);
        return Ok(json!({
            "schema_version":"tos_local_pg_record_revision_result_v1",
            "result":result,
            "grants_admission":false
        }));
    }
    let (software, components) = software_selection(invocation, deadline, cancelled)?;
    let (prepared, readset) = db
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
                let identity =
                    crate::source_revisions::PgRecordRevisionIdentity::from_selection(&selected);
                let selected_record_extension = crate::source_revisions::RevisionFamily::parse(
                    cmd::text(&owner, "schema_version").map_err(DurableError::Source)?,
                )
                .map_err(DurableError::Source)?
                .selected()
                    && operation == "record.revise";
                let configuration = if work_owner || selected_record_extension {
                    Some(cmd::parse(&owner_raw).map_err(DurableError::Source)?)
                } else {
                    None
                };
                let work_revision = if let Some(configuration) = configuration.as_ref() {
                    let record_id =
                        cmd::text(configuration, "record_id").map_err(DurableError::Source)?;
                    let command_id = if operation == "record.revise" {
                        cmd::text(&request, "command_id").map_err(DurableError::Source)?
                    } else {
                        ""
                    };
                    if work_owner {
                        Some(session.observe_pg_work_revision(
                            source_path,
                            record_id,
                            command_id,
                            deadline,
                            cancelled,
                        )?)
                    } else {
                        Some(session.observe_pg_selected_revision(
                            source_path,
                            record_id,
                            command_id,
                            deadline,
                            cancelled,
                        )?)
                    }
                } else {
                    None
                };
                let recorded_at = if let Some(configuration) = configuration.as_ref()
                    && operation == "record.revise"
                    && work_revision.is_some()
                {
                    let command_id =
                        cmd::text(&request, "command_id").map_err(DurableError::Source)?;
                    let binding = crate::source_cohort::WorkRevisionRecoveryBinding {
                        domain: selection.domain.clone(),
                        prepare_id: prepare_id(&selection.domain, command_id),
                        raw_request_digest: Digest256::of_bytes(&request_raw),
                        owner_configuration_raw_digest: Digest256::of_bytes(&owner_raw),
                        source_path: source_path.to_owned(),
                        record_id: cmd::text(configuration, "record_id")
                            .map_err(DurableError::Source)?
                            .to_owned(),
                        command_id: command_id.to_owned(),
                        transaction_id: crate::source_revisions::transaction_id(&request)
                            .map_err(DurableError::Source)?,
                        request_canonical_digest: cmd::record_digest(&request)
                            .map_err(DurableError::Source)?,
                        owner_configuration_canonical_digest: cmd::record_digest(configuration)
                            .map_err(DurableError::Source)?,
                        job_id: selection.job_id.clone(),
                        job_fence: selection.job_fence,
                        family_tag: crate::source_revisions::RevisionFamily::parse(
                            cmd::text(configuration, "schema_version")
                                .map_err(DurableError::Source)?,
                        )
                        .map_err(DurableError::Source)?
                        .pg_family_tag(configuration)
                        .map_err(DurableError::Source)?,
                    };
                    match session.ready_work_recorded_at(&binding, deadline, cancelled)? {
                        Some(recorded_at) => recorded_at,
                        None => {
                            crate::source_serialization::instant().map_err(DurableError::Source)?
                        }
                    }
                } else {
                    crate::source_serialization::instant().map_err(DurableError::Source)?
                };
                if operation == "record.revise"
                    && (native_metadata_owner || (selected_corpus_owner && !work_owner))
                {
                    super::pg_selected_revision::prepare(
                        invocation,
                        &owner_raw,
                        &request_raw,
                        &recorded_at,
                        source_path,
                        selected.cohort.initial_revision(),
                        session,
                        &software,
                        &components,
                        identity,
                        work_revision,
                        max_members,
                        max_total_bytes,
                        max_member_bytes,
                        deadline,
                        cancelled,
                    )
                    .map_err(DurableError::Source)
                } else {
                    let context = if selected_corpus_owner
                        || historical_owner
                        || native_metadata_owner
                        || work_owner
                    {
                        super::revisions::context_selected_record_from_transport_with_limits(
                            &owner_raw,
                            &request_raw,
                            &recorded_at,
                            source_path,
                            selected.cohort.initial_revision(),
                            session,
                            &software,
                            &components,
                            max_members,
                            max_total_bytes,
                            max_member_bytes,
                            deadline,
                            cancelled,
                        )
                        .map_err(DurableError::Source)?
                    } else {
                        super::revisions::context_flat_record_from_transport(
                            &owner_raw,
                            &request_raw,
                            &recorded_at,
                            source_path,
                            selected.cohort.initial_revision(),
                            session,
                            &software,
                            &components,
                            max_members,
                            max_total_bytes,
                            max_member_bytes,
                            deadline,
                            cancelled,
                        )
                        .map_err(DurableError::Source)?
                    };
                    check_context_budget(&context, max_members, max_total_bytes, max_member_bytes)
                        .map_err(DurableError::Source)?;
                    super::revisions::prepare_pg_record_revision_with_work(
                        invocation,
                        context,
                        identity,
                        work_revision,
                        deadline,
                        cancelled,
                    )
                    .map_err(DurableError::Source)
                }
            },
        )
        .map_err(source_error)?;
    owner_fs.current_context(prepared.context(), deadline, cancelled)?;

    let mut result = value(&prepared.proposal().response)?;
    if operation == "describe" && revision_family.selected() {
        let operations =
            result["command_operations"]
                .as_array_mut()
                .ok_or(SourceCommandError::Invalid(
                    "selected revision command operations",
                ))?;
        operations.push(json!("record.revision-schema.provision"));
        if work_owner {
            operations.push(json!("work.revision-schema.provision"));
        }
    }
    // Transport work is reported separately from physical I/O and software
    // capture reads. The counters charge repeated source reads as performed;
    // they are not the cardinality of the deduplicated admission readset.
    let source_work = readset.work;
    result["source_read_work"] = json!({
        "scope":"bounded PG/STO source selection; software capture and physical I/O excluded",
        "exact_source_members_read":source_work.exact_source_members_read,
        "exact_source_bytes_read":source_work.exact_source_bytes_read,
        "historical_source_members_read":source_work.historical_source_members_read,
        "historical_source_bytes_read":source_work.historical_source_bytes_read,
        "custody_metadata_bytes_read":source_work.custody_metadata_bytes_read,
        "segment_bytes_read":source_work.segment_bytes_read,
        "committed_member_rows_examined":source_work.committed_member_rows_examined,
        "metadata_member_lookups":source_work.metadata_member_lookups,
        "directory_index_rows_examined":source_work.directory_index_rows_examined,
        "unique_index_lookups":source_work.unique_index_lookups,
        "text_identity_lookups":source_work.text_identity_lookups,
    });
    if operation == "record.revise" {
        let command_id = cmd::text(&request, "command_id")?;
        let prepare_id = prepare_id(&selection.domain, command_id);
        let attempt = if prepared.proposal().replayed {
            db.recover_source_revision_attempt(
                &store,
                &selection.domain,
                &prepare_id,
                command_id,
                Digest256::of_bytes(&request_raw),
                prepared.proposal().configuration_raw_sha256,
                prepared.proposal().configuration_canonical_sha256,
                prepared.family_tag(),
                prepared.legacy_family_encoding(),
                &selection.job_id,
                selection.job_fence,
                None,
                deadline,
                cancelled,
            )
            .map_err(source_error)?
        } else {
            db.register_source_revision(
                &store,
                &prepared,
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
                    "PostgreSQL revision invocation changed before commit",
                ));
            }
            let (current_fs, current_raw) =
                CreationFilesystem::select_protected_native_owner_at_root(
                    &owner_path,
                    &selection.store_root,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
            if current_raw != owner_raw {
                return Err(DurableError::Conflict(
                    "PostgreSQL revision owner configuration changed before commit",
                ));
            }
            current_fs
                .current_context(prepared.context(), deadline, cancelled)
                .map_err(DurableError::Source)?;
            let expected_executable_digest =
                digest(text(invocation, "native_executable_sha256").map_err(DurableError::Source)?)
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
            .commit_source_revision(
                &store,
                &attempt,
                selection.contract,
                &owner_check,
                deadline,
                cancelled,
            )
            .map_err(source_error)?;
        result["replayed"] = json!(receipt.replayed);
        result["durable_commit"] = commit_result(&receipt);
    }
    // Drop the original selected owner FD only after a complete operation.
    drop(owner_fs);
    Ok(json!({
        "schema_version":"tos_local_pg_record_revision_result_v1",
        "result":result,
        "grants_admission":false
    }))
}
