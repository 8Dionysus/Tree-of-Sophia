//! Native Global Claims create/revise commands over one bounded PG/STO read
//! epoch. Claims semantics stay in `source_claims`; this module selects the
//! exact transport, invocation and durable commit boundary.

use super::{absolute, capped, digest, exact, number, text};
use crate::source_cohort::PreparedPgSourceRevision;
use crate::source_cohort::claims::{PgCompleteClaimCut, PreparedPgClaimCommand};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::CreationFilesystem;
use crate::{DurableError, DurablePgCoordinator, source_claims};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonLimits, RelativePath};
use tos_segment_store::{SegmentLimits, SegmentStore};
use tos_source_store::{
    ReadLimits, SoftwareCaptureReader, SoftwareCaptureSelectionV1, SoftwareComponentSelectionV1,
};

const MAX_REQUEST: usize = 1_048_576;
const MAX_DOMAIN_BYTES: usize = 1_024;
const MAX_JOB_ID_BYTES: usize = 256;
const MAX_CONTEXT_MEMBERS: usize = 4_096;
const MAX_CONTEXT_BYTES: u64 = 33_554_432;
const MAX_CONTEXT_MEMBER_BYTES: u64 = 8_388_608;

pub(super) struct SourceSelection {
    pub(super) domain: String,
    pub(super) contract: Digest256,
    pub(super) store_root: PathBuf,
    pub(super) store_limits: SegmentLimits,
    pub(super) job_id: String,
    pub(super) job_fence: u64,
}

pub(super) fn selected_source(invocation: &Value) -> SourceCommandResult<SourceSelection> {
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
        return Err(SourceCommandError::Invalid("PostgreSQL Claims domain"));
    }
    let job_id = text(value, "job_id")?;
    if job_id.is_empty() || job_id.len() > MAX_JOB_ID_BYTES || job_id.contains('\0') {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL Claims job identity",
        ));
    }
    let job_fence = number(value, "job_fence")?;
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
    Ok(SourceSelection {
        domain: domain.to_owned(),
        contract: digest(text(value, "source_contract_sha256")?)?,
        store_root: absolute(text(value, "store_root")?)?,
        store_limits: SegmentLimits {
            max_segment_bytes: number(limits, "max_segment_bytes")?,
            max_frame_bytes: number(limits, "max_frame_bytes")?,
            max_frames: u32::try_from(number(limits, "max_frames")?)
                .map_err(|_| SourceCommandError::Invalid("PostgreSQL Claims store limits"))?,
            max_journal_bytes: usize::try_from(number(limits, "max_journal_bytes")?)
                .map_err(|_| SourceCommandError::Invalid("PostgreSQL Claims store limits"))?,
        },
        job_id: job_id.to_owned(),
        job_fence,
    })
}

pub(super) fn invocation_budgets(invocation: &Value) -> SourceCommandResult<(usize, u64, u64)> {
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
    let max_members = usize::try_from(capped(budgets, "max_members", MAX_CONTEXT_MEMBERS as u64)?)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL Claims member budget"))?;
    let max_total_bytes = capped(budgets, "max_total_bytes", MAX_CONTEXT_BYTES)?;
    let max_member_bytes = capped(budgets, "max_member_bytes", MAX_CONTEXT_MEMBER_BYTES)?;
    let _ = capped(budgets, "max_schema_receipts", 128)?;
    let _ = capped(budgets, "max_schema_receipt_bytes", 262_144)?;
    let _ = capped(budgets, "worker_cpu_seconds", 3)?;
    let _ = capped(budgets, "worker_address_space_bytes", 1_073_741_824)?;
    Ok((max_members, max_total_bytes, max_member_bytes))
}

pub(super) fn software_selection(
    invocation: &Value,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(SoftwareCaptureReader, SoftwareComponentSelectionV1)> {
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
        ReadLimits {
            max_manifest_bytes: 1_048_576,
            max_manifest_entries: 512,
            max_selected_object_bytes: MAX_CONTEXT_BYTES,
            json: JsonLimits::default(),
        },
        deadline,
        cancelled,
    )
    .map_err(|_| SourceCommandError::Conflict("selected Claims software capture"))?;
    let paths = invocation["software_components"]
        .as_array()
        .filter(|items| !items.is_empty() && items.len() <= 128)
        .ok_or(SourceCommandError::Invalid(
            "selected Claims software components",
        ))?
        .iter()
        .map(|value| {
            RelativePath::parse(value.as_str().ok_or(SourceCommandError::Invalid(
                "selected Claims software component path",
            ))?)
            .map_err(|_| SourceCommandError::Invalid("selected Claims software component path"))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    if paths.iter().collect::<BTreeSet<_>>().len() != paths.len() {
        return Err(SourceCommandError::Invalid(
            "duplicate selected Claims software component",
        ));
    }
    let components = software
        .select_components(&paths)
        .map_err(|_| SourceCommandError::Conflict("selected Claims software components"))?;
    Ok((software, components))
}

pub(super) fn software_files(
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(Vec<cmd::SourceFile>, usize, u64)> {
    let mut files = Vec::new();
    let mut bytes = 0u64;
    for member in components.members() {
        if member.size_bytes > max_member_bytes {
            return Err(SourceCommandError::Invalid(
                "selected Claims software member byte budget",
            ));
        }
        let raw = software
            .read_selected_component(
                components,
                &member.path,
                max_member_bytes,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("selected Claims software member"))?;
        bytes = bytes
            .checked_add(raw.len() as u64)
            .filter(|sum| *sum <= MAX_CONTEXT_BYTES)
            .ok_or(SourceCommandError::Invalid(
                "selected Claims software byte budget",
            ))?;
        files.push(cmd::SourceFile {
            path: member.path.clone(),
            raw,
        });
    }
    Ok((files, components.members().count(), bytes))
}

pub(super) fn read_request(input: impl Read) -> SourceCommandResult<Vec<u8>> {
    let mut raw = Vec::new();
    input
        .take((MAX_REQUEST + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL Claims request read"))?;
    if raw.len() > MAX_REQUEST {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL Claims request budget",
        ));
    }
    Ok(raw)
}

pub(super) fn source_error(error: DurableError) -> SourceCommandError {
    match error {
        DurableError::Source(error) => error,
        DurableError::Conflict(reason) => SourceCommandError::Conflict(reason),
        DurableError::Refused(reason) => SourceCommandError::Denied(reason),
        DurableError::Invalid(_) | DurableError::Corrupt(_) => {
            SourceCommandError::Invalid("PostgreSQL Claims metadata")
        }
        DurableError::Database(_) | DurableError::Storage(_) | DurableError::Indeterminate(_) => {
            SourceCommandError::Conflict("PostgreSQL Claims unavailable")
        }
    }
}

pub(super) fn value(value: &tos_foundation::JsonValue) -> SourceCommandResult<Value> {
    serde_json::from_slice(&cmd::canonical(value)?)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL Claims response JSON"))
}

pub(super) fn prepare_id(domain: &str, command_id: &str) -> Vec<u8> {
    let mut seed = b"tos-pg-global-claims-v1\0".to_vec();
    seed.extend_from_slice(&(domain.len() as u64).to_be_bytes());
    seed.extend_from_slice(domain.as_bytes());
    seed.extend_from_slice(&(command_id.len() as u64).to_be_bytes());
    seed.extend_from_slice(command_id.as_bytes());
    Digest256::of_bytes(&seed).as_bytes().to_vec()
}

pub(super) fn commit_result(receipt: &crate::DurableCommitReceipt) -> Value {
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

pub(super) fn build_context(
    cut: &PgCompleteClaimCut,
    configuration_raw: &[u8],
    request_raw: &[u8],
    recorded_at: String,
    uid: u32,
    mut software_files: Vec<cmd::SourceFile>,
) -> cmd::CommandContext {
    let mut files = cut.members().to_vec();
    files.append(&mut software_files);
    cmd::CommandContext {
        base_revision: cut.base_revision(),
        configuration_raw: configuration_raw.to_vec(),
        request_raw: request_raw.to_vec(),
        recorded_at,
        effective_uid: u64::from(uid),
        files,
    }
}

pub(super) fn check_context_budget(
    context: &cmd::CommandContext,
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
) -> SourceCommandResult<()> {
    if context.files.len() > max_members {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL Claims member budget",
        ));
    }
    let mut total = 0u64;
    for file in &context.files {
        let bytes = file.raw.len() as u64;
        if bytes > max_member_bytes {
            return Err(SourceCommandError::Invalid(
                "PostgreSQL Claims member bytes",
            ));
        }
        total = total
            .checked_add(bytes)
            .filter(|sum| *sum <= max_total_bytes)
            .ok_or(SourceCommandError::Invalid("PostgreSQL Claims total bytes"))?;
    }
    Ok(())
}

/// Complete one explicit Global Claims PG invocation. Whole `ToS/` membership
/// is read recursively and fenced before owner semantics or durable writes.
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
    let expected_executable = digest(text(invocation, "native_executable_sha256")?)?;
    if runtime_observation.current_digest(deadline, cancelled)? != expected_executable {
        return Err(SourceCommandError::Conflict(
            "native Claims executable identity",
        ));
    }
    let owner_path = absolute(text(invocation, "owner_config")?)?;
    let (owner_fs, owner_raw) = CreationFilesystem::select_protected_native_owner_at_root(
        &owner_path,
        &selection.store_root,
        deadline,
        cancelled,
    )?;
    let owner_config = cmd::parse(&owner_raw)?;
    let owner_schema = cmd::text(&owner_config, "schema_version")?;
    let (_, creation_owner, _) = source_claims::family(owner_schema)?;
    let request_raw = read_request(input)?;
    let request = cmd::parse(&request_raw)?;
    if cmd::text(&request, "schema_version")? != "tos_local_source_command_v1" {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL Claims request profile",
        ));
    }
    let operation = cmd::text(&request, "operation")?;
    let expected_operation = if creation_owner {
        "claims.create"
    } else {
        "claim.revise"
    };
    if ![expected_operation, "describe", "inspect-version"].contains(&operation) {
        return Err(SourceCommandError::Denied(
            "PostgreSQL Claims owner operation",
        ));
    }
    let source_root = cmd::text(&owner_config, "source_root")?;
    if source_root != selection.store_root.to_string_lossy() {
        return Err(SourceCommandError::Denied(
            "PostgreSQL Claims owner root differs",
        ));
    }
    let source_path = cmd::text(&owner_config, "source_path")?;
    RelativePath::parse(source_path)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL Claims source path"))?;
    if max_members == 0 || max_total_bytes == 0 || max_member_bytes == 0 {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL Claims empty budget",
        ));
    }
    let (software, components) = software_selection(invocation, deadline, cancelled)?;
    let (software_files, software_members, software_bytes) = software_files(
        &software,
        &components,
        max_member_bytes,
        deadline,
        cancelled,
    )?;
    if software_members > max_members || software_bytes > max_total_bytes {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL Claims software budget",
        ));
    }
    let reserved_members = software_members
        .checked_add(MAX_CONTEXT_MEMBERS - max_members)
        .ok_or(SourceCommandError::Invalid(
            "PostgreSQL Claims member budget",
        ))?;
    let reserved_bytes = software_bytes
        .checked_add(MAX_CONTEXT_BYTES - max_total_bytes)
        .ok_or(SourceCommandError::Invalid(
            "PostgreSQL Claims total byte budget",
        ))?;
    let store = SegmentStore::open_existing(&selection.store_root, selection.store_limits)
        .map_err(|_| SourceCommandError::Conflict("selected Claims source store"))?;
    let mut db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected Claims PostgreSQL"))?,
    );
    let ((prepared, mut result, context, catalog_capture), readset) = db
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
                        "selected Claims source contract differs",
                    ));
                }
                let cut = crate::source_cohort::claims::read_pg_complete_claim_cut(
                    session,
                    reserved_members,
                    reserved_bytes,
                    max_member_bytes,
                    deadline,
                    cancelled,
                )?;
                let context = build_context(
                    &cut,
                    &owner_raw,
                    &request_raw,
                    crate::source_serialization::instant().map_err(DurableError::Source)?,
                    uid,
                    software_files,
                );
                check_context_budget(&context, max_members, max_total_bytes, max_member_bytes)
                    .map_err(DurableError::Source)?;
                let worker = super::revisions::selected_schema_from_context(
                    invocation,
                    &context.files,
                    context.base_revision,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                let mut worker = worker;
                let catalog_capture = if source_claims::pg_requires_catalog(&context)
                    .map_err(DurableError::Source)?
                {
                    Some(Rc::new(RefCell::new(
                        owner_fs
                            .select_claim_catalog(deadline, cancelled)
                            .map_err(DurableError::Source)?,
                    )))
                } else {
                    None
                };
                let whole_call = source_claims::claim_call_for_selected_context(&context)
                    .map_err(DurableError::Source)?;
                let prepared = match operation {
                    "claims.create" => {
                        let serialized = source_claims::claim_creation_from_pg_complete_cut(
                            &context,
                            &cut,
                            &software,
                            &components,
                            &mut worker,
                            catalog_capture.clone(),
                            Some(whole_call.clone()),
                            deadline,
                            cancelled,
                        )
                        .map_err(DurableError::Source)?;
                        if !serialized.command().replayed {
                            let config = cmd::parse(&context.configuration_raw)
                                .map_err(DurableError::Source)?;
                            let source_path =
                                cmd::text(&config, "source_path").map_err(DurableError::Source)?;
                            let source_path = RelativePath::parse(&source_path)
                                .map_err(|_| DurableError::Invalid("PG Claim source path"))?;
                            for (_, token, _) in source_claims::pg_claim_creation_index_rows(
                                &source_path,
                                serialized.files(),
                            )
                            .map_err(DurableError::Source)?
                            {
                                for unique_kind in ["metadata", "claim", "event", "anchor", "form"]
                                {
                                    session.observe_unique_index(
                                        unique_kind,
                                        &token,
                                        deadline,
                                        cancelled,
                                    )?;
                                }
                            }
                        }
                        let result =
                            value(&serialized.command().response).map_err(DurableError::Source)?;
                        let owner_contributions =
                            source_claims::pg_inventory::revision_owner_contributions(
                                serialized.context(),
                                serialized.command(),
                                &mut worker,
                                deadline,
                                cancelled,
                            )
                            .map_err(DurableError::Source)?;
                        let prepared = PreparedPgClaimCommand::from_creation(
                            &serialized,
                            cut,
                            owner_contributions,
                        )?;
                        (Some(prepared), result, context, catalog_capture)
                    }
                    "claim.revise" => {
                        let proposal = source_claims::run_claim_command_from_pg_complete_cut(
                            &context,
                            &cut,
                            catalog_capture.clone(),
                            Some(whole_call),
                            &mut worker,
                            deadline,
                            cancelled,
                        )
                        .map_err(DurableError::Source)?;
                        let result = value(&proposal.response).map_err(DurableError::Source)?;
                        let owner_contributions =
                            source_claims::pg_inventory::revision_owner_contributions(
                                &context,
                                &proposal,
                                &mut worker,
                                deadline,
                                cancelled,
                            )
                            .map_err(DurableError::Source)?;
                        let prepared = PreparedPgClaimCommand::from_revision(
                            context.clone(),
                            proposal,
                            cut,
                            owner_contributions,
                        )?;
                        (Some(prepared), result, context, catalog_capture)
                    }
                    "describe" | "inspect-version" => {
                        let proposal = source_claims::run_claim_command_from_pg_complete_cut(
                            &context,
                            &cut,
                            catalog_capture.clone(),
                            Some(whole_call),
                            &mut worker,
                            deadline,
                            cancelled,
                        )
                        .map_err(DurableError::Source)?;
                        let result = value(&proposal.response).map_err(DurableError::Source)?;
                        (None, result, context, catalog_capture)
                    }
                    _ => return Err(DurableError::Refused("unsupported PG Claims operation")),
                };
                crate::source_creation_store::finish_creation_worker(
                    &mut worker,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                Ok(prepared)
            },
        )
        .map_err(source_error)?;

    owner_fs.current_context(&context, deadline, cancelled)?;
    if let Some(catalog) = &catalog_capture {
        let selected_bytes = catalog.borrow().selected_bytes()?;
        catalog
            .borrow()
            .verify_current(&owner_fs, deadline, cancelled)?;
        if selected_bytes > MAX_CONTEXT_BYTES as usize {
            return Err(SourceCommandError::Invalid(
                "selected Claims catalog budget",
            ));
        }
    }
    let source_work = readset.work;
    result["source_read_work"] = json!({
        "scope":"complete bounded PG/STO ToS cut; software capture and physical I/O excluded",
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
    if let Some(prepared) = prepared {
        let proposal = prepared.proposal();
        let request = cmd::parse(&request_raw)?;
        let command_id = cmd::text(&request, "command_id")?.to_owned();
        let prepare_id = prepare_id(&selection.domain, &command_id);
        let attempt = if proposal.replayed {
            db.recover_source_revision_attempt(
                &store,
                &selection.domain,
                &prepare_id,
                &command_id,
                Digest256::of_bytes(&request_raw),
                proposal.configuration_raw_sha256,
                proposal.configuration_canonical_sha256,
                prepared.family_tag(),
                false,
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
                    "PostgreSQL Claims invocation changed",
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
                    "PostgreSQL Claims owner configuration changed",
                ));
            }
            current_fs
                .current_context(prepared.context(), deadline, cancelled)
                .map_err(DurableError::Source)?;
            if let Some(catalog) = &catalog_capture {
                catalog
                    .borrow()
                    .verify_current(&current_fs, deadline, cancelled)
                    .map_err(DurableError::Source)?;
            }
            if runtime_observation
                .current_digest(deadline, cancelled)
                .map_err(DurableError::Source)?
                != expected_executable
            {
                return Err(DurableError::Conflict("native Claims executable changed"));
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
    drop(owner_fs);
    Ok(json!({
        "schema_version":"tos_local_pg_global_claims_result_v1",
        "result":result,
        "grants_admission":false
    }))
}
