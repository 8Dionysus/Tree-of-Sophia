//! Selected-input adapters for maintained Growth owner packages.
//!
//! The native CLI front door owns invocation dispatch. This module contains
//! the bounded PG/STO owner preparation types used by that route.

#[path = "source_native_pg_growth_input.rs"]
mod input;
#[path = "source_native_pg_growth_owner_inputs.rs"]
mod owner_inputs;
#[path = "source_native_pg_growth_sign.rs"]
mod sign;

pub(crate) use input::PgGrowthCurrentInput;
pub(crate) use owner_inputs::context_with_pg_exact_reads;
pub(super) use owner_inputs::{
    PgCollectionMembershipInputs, PgExpressionEditionInputs, PgExpressionResponsibilityInputs,
    PgGrowthSelectedInputs, PgWorkExpressionInputs,
};

use crate::source_cohort::revision::{
    PgRecordRevisionIdentity, SourceRevisionReadSession, SourceRevisionReadSet,
};
use crate::source_cohort::{
    self, IndexRows, PreparedPgSourceRevision, SourceReads, SourceRevisionWrite,
};
use crate::source_command::{
    self as cmd, CommandContext, PreparedCommand, SourceCommandError, SourceCommandResult,
};
use crate::source_creation_store::CreationFilesystem;
use crate::source_creation_store::PgWorkExpressionOwnerPackage;
use crate::{DurableError, DurablePgCoordinator, DurableResult};
use postgres::Transaction;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::JsonValue;
use tos_foundation::{Digest256, JsonLimits, RelativePath};
use tos_segment_store::{SegmentLimits, SegmentStore};
use tos_source_store::{
    ReadLimits, SoftwareCaptureReader, SoftwareCaptureSelectionV1, SoftwareComponentSelectionV1,
};
use tos_validation::item_rules::ItemLimits;
use tos_validation::record_biblio_cut::SourceCutInputWithIdentity;

const MAX_REQUEST_BYTES: usize = 1_048_576;
const MAX_DOMAIN_BYTES: usize = 1_024;
const MAX_JOB_ID_BYTES: usize = 256;
const MAX_SELECTED_MEMBERS: u64 = 512;
const MAX_SELECTED_BYTES: u64 = 8_388_608;
const MAX_SELECTED_MEMBER_BYTES: u64 = 8_388_608;

struct GrowthSourceSelection {
    domain: String,
    contract: Digest256,
    store_root: PathBuf,
    store_limits: SegmentLimits,
    job_id: String,
    job_fence: u64,
}

fn growth_selection(invocation: &Value) -> SourceCommandResult<GrowthSourceSelection> {
    let value = &invocation["source_selection"];
    super::exact(
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
    let domain = super::text(value, "domain")?;
    if domain.is_empty() || domain.len() > MAX_DOMAIN_BYTES || domain.contains('\0') {
        return Err(SourceCommandError::Invalid("PostgreSQL Growth domain"));
    }
    let job_id = super::text(value, "job_id")?;
    if job_id.is_empty() || job_id.len() > MAX_JOB_ID_BYTES || job_id.contains('\0') {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL Growth job identity",
        ));
    }
    let job_fence = super::number(value, "job_fence")?;
    if job_fence == 0 {
        return Err(SourceCommandError::Invalid("PostgreSQL Growth job fence"));
    }
    let limits = &value["store_limits"];
    super::exact(
        limits,
        &[
            "max_segment_bytes",
            "max_frame_bytes",
            "max_frames",
            "max_journal_bytes",
        ],
    )?;
    let max_frames = u32::try_from(super::number(limits, "max_frames")?)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL Growth store limits"))?;
    let max_journal_bytes = usize::try_from(super::number(limits, "max_journal_bytes")?)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL Growth store limits"))?;
    Ok(GrowthSourceSelection {
        domain: domain.to_owned(),
        contract: super::digest(super::text(value, "source_contract_sha256")?)?,
        store_root: super::absolute(super::text(value, "store_root")?)?,
        store_limits: SegmentLimits {
            max_segment_bytes: super::number(limits, "max_segment_bytes")?,
            max_frame_bytes: super::number(limits, "max_frame_bytes")?,
            max_frames,
            max_journal_bytes,
        },
        job_id: job_id.to_owned(),
        job_fence,
    })
}

fn growth_invocation_budgets(invocation: &Value) -> SourceCommandResult<(usize, u64, u64)> {
    let budgets = &invocation["budgets"];
    super::exact(
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
    let members = usize::try_from(super::capped(budgets, "max_members", MAX_SELECTED_MEMBERS)?)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL Growth member budget"))?;
    let total = super::capped(budgets, "max_total_bytes", MAX_SELECTED_BYTES)?;
    let member = super::capped(budgets, "max_member_bytes", MAX_SELECTED_MEMBER_BYTES)?;
    let _ = super::capped(budgets, "max_schema_receipts", 128)?;
    let _ = super::capped(budgets, "max_schema_receipt_bytes", 262_144)?;
    let _ = super::capped(budgets, "worker_cpu_seconds", 3)?;
    let _ = super::capped(budgets, "worker_address_space_bytes", 1_073_741_824)?;
    Ok((members, total, member))
}

fn growth_software_selection(
    invocation: &Value,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(SoftwareCaptureReader, SoftwareComponentSelectionV1)> {
    super::exact(
        &invocation["software_selection"],
        &[
            "source_git_commit",
            "source_git_tree",
            "capture_manifest_sha256",
        ],
    )?;
    let software = SoftwareCaptureReader::open(
        &super::absolute(super::text(invocation, "software_capture")?)?,
        &super::absolute(super::text(invocation, "software_restored_root")?)?,
        SoftwareCaptureSelectionV1 {
            source_git_commit: super::text(&invocation["software_selection"], "source_git_commit")?
                .to_owned(),
            source_git_tree: super::text(&invocation["software_selection"], "source_git_tree")?
                .to_owned(),
            capture_manifest_sha256: super::digest(super::text(
                &invocation["software_selection"],
                "capture_manifest_sha256",
            )?)?,
        },
        ReadLimits {
            max_manifest_bytes: 1_048_576,
            max_manifest_entries: 512,
            max_selected_object_bytes: MAX_SELECTED_BYTES,
            json: JsonLimits::default(),
        },
        deadline,
        cancelled,
    )
    .map_err(|_| SourceCommandError::Conflict("selected Growth software capture"))?;
    let paths = invocation["software_components"]
        .as_array()
        .filter(|items| !items.is_empty() && items.len() <= 128)
        .ok_or(SourceCommandError::Invalid(
            "selected Growth software components",
        ))?
        .iter()
        .map(|value| {
            let value = value.as_str().ok_or(SourceCommandError::Invalid(
                "selected Growth software component path",
            ))?;
            RelativePath::parse(value)
                .map_err(|_| SourceCommandError::Invalid("selected Growth software component path"))
        })
        .collect::<SourceCommandResult<Vec<_>>>()?;
    if paths.iter().collect::<BTreeSet<_>>().len() != paths.len() {
        return Err(SourceCommandError::Invalid(
            "duplicate selected Growth software component",
        ));
    }
    let components = software
        .select_components(&paths)
        .map_err(|_| SourceCommandError::Conflict("selected Growth software components"))?;
    Ok((software, components))
}

fn read_request(input: impl Read) -> SourceCommandResult<Vec<u8>> {
    let mut raw = Vec::new();
    input
        .take((MAX_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL Growth request read"))?;
    if raw.len() > MAX_REQUEST_BYTES {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL Growth request budget",
        ));
    }
    Ok(raw)
}

fn growth_prepare_id(domain: &str, command_id: &str) -> Vec<u8> {
    let mut seed = b"tos-pg-maintained-growth-v1\0".to_vec();
    seed.extend_from_slice(&(domain.len() as u64).to_be_bytes());
    seed.extend_from_slice(domain.as_bytes());
    seed.extend_from_slice(&(command_id.len() as u64).to_be_bytes());
    seed.extend_from_slice(command_id.as_bytes());
    Digest256::of_bytes(&seed).as_bytes().to_vec()
}

fn growth_source_error(error: DurableError) -> SourceCommandError {
    match error {
        DurableError::Source(error) => error,
        DurableError::Conflict(reason) => SourceCommandError::Conflict(reason),
        DurableError::Refused(reason) => SourceCommandError::Denied(reason),
        DurableError::Invalid(_) | DurableError::Corrupt(_) => {
            SourceCommandError::Invalid("PostgreSQL Growth metadata")
        }
        DurableError::Database(_) | DurableError::Storage(_) | DurableError::Indeterminate(_) => {
            SourceCommandError::Conflict("PostgreSQL Growth route unavailable")
        }
    }
}

fn growth_value(value: &JsonValue) -> SourceCommandResult<Value> {
    serde_json::from_slice(&cmd::canonical(value)?)
        .map_err(|_| SourceCommandError::Invalid("PostgreSQL Growth result JSON"))
}

fn growth_commit_result(receipt: &crate::DurableCommitReceipt) -> Value {
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

fn check_growth_context_budget(
    context: &CommandContext,
    max_members: usize,
    max_total: u64,
    max_member: u64,
) -> SourceCommandResult<()> {
    if context.files.len() > max_members {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL Growth member budget",
        ));
    }
    let mut total = 0u64;
    for file in &context.files {
        let size = u64::try_from(file.raw.len())
            .map_err(|_| SourceCommandError::Invalid("PostgreSQL Growth byte budget"))?;
        if size > max_member {
            return Err(SourceCommandError::Invalid(
                "PostgreSQL Growth member byte budget",
            ));
        }
        total = total
            .checked_add(size)
            .filter(|bytes| *bytes <= max_total)
            .ok_or(SourceCommandError::Invalid(
                "PostgreSQL Growth total byte budget",
            ))?;
    }
    Ok(())
}

/// Execute the ordinary native owner command against one existing, fenced
/// PostgreSQL/STO source selection. Each maintained owner keeps its own
/// preparation contract while the families share durable recovery and commit.
pub(super) fn run(
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    input: impl Read,
    uid: u32,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    super::exact(
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
    let selection = growth_selection(invocation)?;
    let (max_members, max_total_bytes, max_member_bytes) = growth_invocation_budgets(invocation)?;
    let expected_executable = super::digest(super::text(invocation, "native_executable_sha256")?)?;
    let _ = super::absolute(super::text(invocation, "native_executable")?)?;
    let runtime = crate::source_serialization::observe_executable(deadline, cancelled)?;
    if runtime.current_digest(deadline, cancelled)? != expected_executable {
        return Err(SourceCommandError::Conflict(
            "native Growth executable identity",
        ));
    }

    let owner_path = super::absolute(super::text(invocation, "owner_config")?)?;
    let (owner_fs, owner_raw) = CreationFilesystem::select_protected_native_owner_at_root(
        &owner_path,
        &selection.store_root,
        deadline,
        cancelled,
    )?;
    let owner_config = cmd::parse(&owner_raw)?;
    let owner_schema = cmd::text(&owner_config, "schema_version")?;
    let request_raw = read_request(input)?;
    let request = cmd::parse(&request_raw)?;
    let requested_operation = cmd::text(&request, "operation")?;
    if owner_schema == "tos_local_sign_promote_owner_v2"
        || owner_schema == "tos_local_sign_promote_owner_v1"
    {
        return sign::run(
            invocation_path,
            invocation_raw,
            invocation,
            &request_raw,
            &request,
            uid,
            &selection,
            &owner_path,
            &owner_raw,
            &owner_config,
            expected_executable,
            max_members,
            max_total_bytes,
            max_member_bytes,
            deadline,
            cancelled,
        );
    }
    if owner_schema == "tos_local_work_expression_owner_v1"
        && matches!(requested_operation, "describe" | "prepare-create")
    {
        return run_pg_work_expression_preview(
            invocation_path,
            invocation_raw,
            invocation,
            &request_raw,
            &request,
            uid,
            &selection,
            &owner_path,
            &owner_fs,
            &owner_raw,
            &owner_config,
            expected_executable,
            max_members,
            max_total_bytes,
            max_member_bytes,
            deadline,
            cancelled,
        );
    }
    if owner_schema == "tos_local_object_link_create_owner_v1" {
        return run_pg_object_link(
            invocation_path,
            invocation_raw,
            invocation,
            &request_raw,
            &request,
            uid,
            &selection,
            &owner_path,
            &owner_fs,
            &owner_raw,
            &owner_config,
            expected_executable,
            deadline,
            cancelled,
        );
    }
    if owner_schema == "tos_local_expression_edition_owner_v1" {
        if matches!(requested_operation, "describe" | "prepare-create") {
            return run_pg_growth_family_preview(
                invocation_path,
                invocation_raw,
                invocation,
                &request_raw,
                &request,
                "tos_local_expression_edition_command_v1",
                "prepare-create",
                PgGrowthFamily::ExpressionEdition,
                requested_operation,
                uid,
                &selection,
                &owner_path,
                &owner_fs,
                &owner_raw,
                &owner_config,
                expected_executable,
                max_members,
                max_total_bytes,
                max_member_bytes,
                deadline,
                cancelled,
            );
        }
        return run_pg_growth_family(
            invocation_path,
            invocation_raw,
            invocation,
            &request_raw,
            &request,
            "tos_local_expression_edition_command_v1",
            PgGrowthFamily::ExpressionEdition,
            uid,
            &selection,
            &owner_path,
            &owner_fs,
            &owner_raw,
            &owner_config,
            expected_executable,
            max_members,
            max_total_bytes,
            max_member_bytes,
            deadline,
            cancelled,
            |fs, session, software, components, recorded_at| {
                prepare_pg_expression_edition_growth(
                    fs,
                    session,
                    invocation,
                    &owner_raw,
                    &owner_config,
                    &request_raw,
                    &request,
                    recorded_at,
                    uid,
                    software,
                    components,
                    growth_item_limits(max_total_bytes, max_member_bytes, deadline),
                    deadline,
                    cancelled,
                )
            },
        );
    }
    if owner_schema == "tos_local_expression_responsibility_owner_v1" {
        if matches!(requested_operation, "describe" | "prepare-attach") {
            return run_pg_growth_family_preview(
                invocation_path,
                invocation_raw,
                invocation,
                &request_raw,
                &request,
                "tos_local_expression_responsibility_command_v1",
                "prepare-attach",
                PgGrowthFamily::ExpressionResponsibility,
                requested_operation,
                uid,
                &selection,
                &owner_path,
                &owner_fs,
                &owner_raw,
                &owner_config,
                expected_executable,
                max_members,
                max_total_bytes,
                max_member_bytes,
                deadline,
                cancelled,
            );
        }
        return run_pg_growth_family(
            invocation_path,
            invocation_raw,
            invocation,
            &request_raw,
            &request,
            "tos_local_expression_responsibility_command_v1",
            PgGrowthFamily::ExpressionResponsibility,
            uid,
            &selection,
            &owner_path,
            &owner_fs,
            &owner_raw,
            &owner_config,
            expected_executable,
            max_members,
            max_total_bytes,
            max_member_bytes,
            deadline,
            cancelled,
            |fs, session, software, components, recorded_at| {
                prepare_pg_expression_responsibility_growth(
                    fs,
                    session,
                    invocation,
                    &owner_raw,
                    &owner_config,
                    &request_raw,
                    &request,
                    recorded_at,
                    uid,
                    software,
                    components,
                    growth_item_limits(max_total_bytes, max_member_bytes, deadline),
                    deadline,
                    cancelled,
                )
            },
        );
    }
    if owner_schema == "tos_local_collection_membership_owner_v1" {
        if matches!(requested_operation, "describe" | "prepare-attach") {
            return run_pg_growth_family_preview(
                invocation_path,
                invocation_raw,
                invocation,
                &request_raw,
                &request,
                "tos_local_collection_membership_command_v1",
                "prepare-attach",
                PgGrowthFamily::CollectionMembership,
                requested_operation,
                uid,
                &selection,
                &owner_path,
                &owner_fs,
                &owner_raw,
                &owner_config,
                expected_executable,
                max_members,
                max_total_bytes,
                max_member_bytes,
                deadline,
                cancelled,
            );
        }
        return run_pg_growth_family(
            invocation_path,
            invocation_raw,
            invocation,
            &request_raw,
            &request,
            "tos_local_collection_membership_command_v1",
            PgGrowthFamily::CollectionMembership,
            uid,
            &selection,
            &owner_path,
            &owner_fs,
            &owner_raw,
            &owner_config,
            expected_executable,
            max_members,
            max_total_bytes,
            max_member_bytes,
            deadline,
            cancelled,
            |fs, session, software, components, recorded_at| {
                prepare_pg_collection_membership_growth(
                    fs,
                    session,
                    invocation,
                    &owner_raw,
                    &owner_config,
                    &request_raw,
                    &request,
                    recorded_at,
                    uid,
                    software,
                    components,
                    growth_item_limits(max_total_bytes, max_member_bytes, deadline),
                    deadline,
                    cancelled,
                )
            },
        );
    }
    if owner_schema != "tos_local_work_expression_owner_v1" {
        return Err(SourceCommandError::Denied(
            "PostgreSQL Growth protected owner family",
        ));
    }
    if cmd::text(&request, "schema_version")? != "tos_local_work_expression_command_v1" {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL Growth request profile",
        ));
    }
    let operation = cmd::text(&request, "operation")?;
    if operation != "work.expression.create" {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL Work Expression mutation operation",
        ));
    }
    let command_id = cmd::text(&request, "command_id")?;
    if command_id.is_empty() || command_id.len() > MAX_JOB_ID_BYTES {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL Growth command identity",
        ));
    }

    let store = SegmentStore::open_existing(&selection.store_root, selection.store_limits)
        .map_err(|_| SourceCommandError::Conflict("selected Growth STO"))?;
    let (software, components) = growth_software_selection(invocation, deadline, cancelled)?;
    let mut db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected PostgreSQL unavailable"))?,
    );
    let prepare_id = growth_prepare_id(&selection.domain, command_id);

    // Resolve an already sealed command before fresh owner authorization.
    // This retains the attempt's original source, request and rights binding
    // while still requiring the current protected configuration and runtime.
    match db.recover_source_revision_attempt(
        &store,
        &selection.domain,
        &prepare_id,
        command_id,
        Digest256::of_bytes(&request_raw),
        Digest256::of_bytes(&owner_raw),
        cmd::record_digest(&owner_config)?,
        PgGrowthFamily::WorkExpression.family_tag(),
        false,
        &selection.job_id,
        selection.job_fence,
        None,
        deadline,
        cancelled,
    ) {
        Ok(attempt) => {
            if attempt.contract != selection.contract {
                return Err(SourceCommandError::Conflict(
                    "recovered Growth source contract changed",
                ));
            }
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
                        "PostgreSQL Growth invocation changed before recovery",
                    ));
                }
                let (_, current_raw) = CreationFilesystem::select_protected_native_owner_at_root(
                    &owner_path,
                    &selection.store_root,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                if current_raw != owner_raw {
                    return Err(DurableError::Conflict(
                        "PostgreSQL Growth owner configuration changed before recovery",
                    ));
                }
                if runtime
                    .current_digest(deadline, cancelled)
                    .map_err(DurableError::Source)?
                    != expected_executable
                {
                    return Err(DurableError::Conflict(
                        "native Growth executable changed before recovery",
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
                .map_err(growth_source_error)?;
            return Ok(json!({
                "schema_version":"tos_local_pg_growth_result_v1",
                "operation":operation,
                "replayed":receipt.replayed,
                "durable_commit":growth_commit_result(&receipt),
                "grants_admission":false
            }));
        }
        Err(DurableError::Refused("source revision attempt absent")) => {}
        Err(error) => return Err(growth_source_error(error)),
    }

    let recorded_at = crate::source_serialization::instant()?;
    let (prepared, readset) = db
        .with_source_revision_session(
            &store,
            &selection.domain,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                if session.selection().contract != selection.contract {
                    return Err(DurableError::Conflict(
                        "selected Growth source contract differs from current domain",
                    ));
                }
                let prepared = prepare_pg_work_expression_growth(
                    &owner_fs,
                    session,
                    invocation,
                    &owner_raw,
                    &owner_config,
                    &request_raw,
                    cmd::field(&request, "expected_publication")
                        .map_err(DurableError::Source)?
                        .as_str(),
                    recorded_at.clone(),
                    uid,
                    &software,
                    &components,
                    ItemLimits {
                        max_member_bytes: usize::try_from(max_member_bytes)
                            .map_err(|_| DurableError::Invalid("PG Growth owner member budget"))?,
                        max_total_bytes,
                        max_state_bytes: usize::try_from(max_total_bytes)
                            .map_err(|_| DurableError::Invalid("PG Growth owner state budget"))?,
                        max_issues: 256,
                        deadline,
                    },
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                check_growth_context_budget(
                    prepared.prepared.context(),
                    max_members,
                    max_total_bytes,
                    max_member_bytes,
                )
                .map_err(DurableError::Source)?;
                Ok(prepared)
            },
        )
        .map_err(growth_source_error)?;

    let mut result = growth_value(&prepared.prepared.proposal().response)?;
    result["source_read_work"] = growth_source_read_work(&readset.work);
    let attempt = db
        .register_source_revision(
            &store,
            &prepared.prepared,
            &readset,
            &prepare_id,
            deadline,
            cancelled,
        )
        .map_err(growth_source_error)?;
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
                "PostgreSQL Growth invocation changed before commit",
            ));
        }
        let (current_fs, current_raw) = CreationFilesystem::select_protected_native_owner_at_root(
            &owner_path,
            &selection.store_root,
            deadline,
            cancelled,
        )
        .map_err(DurableError::Source)?;
        if current_raw != owner_raw {
            return Err(DurableError::Conflict(
                "PostgreSQL Growth owner configuration changed before commit",
            ));
        }
        crate::source_creation_store::verify_pg_work_context(
            &current_fs,
            prepared.prepared.context(),
            &BTreeSet::new(),
            deadline,
            cancelled,
        )
        .map_err(DurableError::Source)?;
        if runtime
            .current_digest(deadline, cancelled)
            .map_err(DurableError::Source)?
            != expected_executable
        {
            return Err(DurableError::Conflict(
                "native Growth executable changed before commit",
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
        .map_err(growth_source_error)?;
    result["replayed"] = json!(receipt.replayed);
    result["durable_commit"] = growth_commit_result(&receipt);
    drop(owner_fs);
    Ok(json!({
        "schema_version":"tos_local_pg_growth_result_v1",
        "result":result,
        "grants_admission":false
    }))
}

fn run_pg_work_expression_preview(
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    request_raw: &[u8],
    request: &JsonValue,
    uid: u32,
    selection: &GrowthSourceSelection,
    owner_path: &Path,
    owner_fs: &CreationFilesystem,
    owner_raw: &[u8],
    owner_config: &JsonValue,
    expected_executable: Digest256,
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    if cmd::text(request, "schema_version")? != "tos_local_work_expression_command_v1" {
        return Err(SourceCommandError::Invalid(
            "PG Work preview request schema",
        ));
    }
    let operation = cmd::text(request, "operation")?;
    if operation == "describe" {
        cmd::exact_keys(request, &["schema_version", "operation"])?;
    } else if operation != "prepare-create" {
        return Err(SourceCommandError::Unsupported("PG Work preview operation"));
    }
    let runtime = crate::source_serialization::observe_executable(deadline, cancelled)?;
    if runtime.current_digest(deadline, cancelled)? != expected_executable {
        return Err(SourceCommandError::Conflict(
            "native Growth executable identity",
        ));
    }
    let store = SegmentStore::open_existing(&selection.store_root, selection.store_limits)
        .map_err(|_| SourceCommandError::Conflict("selected Growth STO"))?;
    let (software, components) = growth_software_selection(invocation, deadline, cancelled)?;
    let mut db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected PostgreSQL unavailable"))?,
    );
    let recorded_at = crate::source_serialization::instant()?;
    let (prepared, readset) = db
        .with_source_revision_session(
            &store,
            &selection.domain,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                if session.selection().contract != selection.contract {
                    return Err(DurableError::Conflict(
                        "selected Growth source contract differs from current domain",
                    ));
                }
                let identity = session.selection().identity();
                let input = PgGrowthCurrentInput::new(
                    session,
                    4096,
                    MAX_GROWTH_SOFTWARE_MEMBER_BYTES as usize,
                )
                .map_err(|_| DurableError::Invalid("PG Growth selected input budget"))?;
                let mut selected = if operation == "prepare-create" {
                    PgWorkExpressionInputs::select(&input, owner_config, None, deadline, cancelled)
                        .map_err(DurableError::Source)?
                        .selected
                } else {
                    let mut selected =
                        PgGrowthSelectedInputs::select(&input, None, deadline, cancelled)
                            .map_err(DurableError::Source)?;
                    selected
                        .record(
                            input.source_input(),
                            "work",
                            cmd::text(owner_config, "work_id").map_err(DurableError::Source)?,
                            cmd::text(owner_config, "work_source_path")
                                .map_err(DurableError::Source)?,
                            deadline,
                            cancelled,
                        )
                        .map_err(DurableError::Source)?;
                    selected
                };
                for path in WORK_EXPRESSION_CONTRACTS {
                    selected
                        .member(input.source_input(), path, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                }
                let software_files =
                    selected_software_files(&software, &components, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                let context = selected
                    .context(
                        owner_raw,
                        request_raw,
                        recorded_at.clone(),
                        uid,
                        &software_files,
                    )
                    .map_err(DurableError::Source)?;
                let mut worker = super::revisions::selected_candidate_schema_from_context(
                    invocation,
                    &context.files,
                    identity,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                let preview = crate::source_creation_store::preview_pg_work_expression(
                    owner_fs,
                    &context,
                    &input,
                    &software,
                    &components,
                    &mut worker,
                    operation,
                    growth_item_limits(max_total_bytes, max_member_bytes, deadline),
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                drop(input);
                selected
                    .bind_selected_readset(session, deadline, cancelled)
                    .map_err(|_| DurableError::Conflict("PG Work preview selected read epoch"))?;
                observe_growth_semantic_indexes(
                    session,
                    &preview.semantic_indexes,
                    deadline,
                    cancelled,
                )?;
                let (context, absences) =
                    context_with_pg_exact_reads(&context, session, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                check_growth_context_budget(
                    &context,
                    max_members,
                    max_total_bytes,
                    max_member_bytes,
                )
                .map_err(DurableError::Source)?;
                Ok((preview, context, absences))
            },
        )
        .map_err(growth_source_error)?;
    let mut result = growth_value(&prepared.0.response)?;
    result["source_read_work"] = growth_source_read_work(&readset.work);
    if super::read_absolute(
        invocation_path,
        uid,
        true,
        invocation_raw.len(),
        deadline,
        cancelled,
    )? != invocation_raw
    {
        return Err(SourceCommandError::Conflict(
            "PG Growth invocation changed before preview",
        ));
    }
    let (current_fs, current_raw) = CreationFilesystem::select_protected_native_owner_at_root(
        owner_path,
        &selection.store_root,
        deadline,
        cancelled,
    )?;
    if current_raw != owner_raw {
        return Err(SourceCommandError::Conflict(
            "PG Growth owner configuration changed before preview",
        ));
    }
    crate::source_creation_store::verify_pg_work_context(
        &current_fs,
        &prepared.1,
        &prepared.2,
        deadline,
        cancelled,
    )?;
    prepared.0.fence.verify(deadline, cancelled)?;
    if runtime.current_digest(deadline, cancelled)? != expected_executable {
        return Err(SourceCommandError::Conflict(
            "native Growth executable changed before preview",
        ));
    }
    Ok(json!({
        "schema_version":"tos_local_pg_growth_result_v1",
        "result":result,
        "grants_admission":false
    }))
}

const WORK_EXPRESSION_CONTRACTS: &[&str] = &[
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    "ToS/contracts/corpus-record.schema.json",
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/semantic-relation-type-registry.schema.json",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
    "ToS/contracts/provenance-event-v2.schema.json",
];

const OBJECT_LINK_CONTRACTS: &[&str] = &[
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    "ToS/contracts/corpus-record.schema.json",
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/semantic-relation-type-registry.schema.json",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
    "ToS/contracts/provenance-event-v2.schema.json",
    "ToS/contracts/source-link.schema.json",
    "ToS/contracts/object-link-claim-v2.schema.json",
    "ToS/contracts/claim-packet.schema.json",
    "ToS/contracts/source-claim-record.schema.json",
    "ToS/contracts/claim-display-fields.schema.json",
    "ToS/contracts/artifact-source-witness.schema.json",
    "ToS/contracts/artifact-source-witness-v2.schema.json",
];

const MAX_GROWTH_SOFTWARE_MEMBERS: usize = 128;
const MAX_GROWTH_SOFTWARE_MEMBER_BYTES: u64 = 8_388_608;
const MAX_GROWTH_SOFTWARE_TOTAL_BYTES: u64 = 8_388_608;

/// Exact request families admitted by the maintained Growth source writer.
/// Recovery retains the original operation's tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PgGrowthFamily {
    ExpressionEdition,
    ExpressionResponsibility,
    CollectionMembership,
    WorkExpression,
    ObjectLink,
    SignPromotion,
}

impl PgGrowthFamily {
    pub(crate) fn operation(self) -> &'static str {
        match self {
            Self::ExpressionEdition => "expression.edition.create",
            Self::ExpressionResponsibility => "expression.responsibility.attach",
            Self::CollectionMembership => "collection.work.attach",
            Self::WorkExpression => "work.expression.create",
            Self::ObjectLink => "object.link.create",
            Self::SignPromotion => "sign.promote",
        }
    }

    pub(crate) fn handler(self) -> &'static str {
        match self {
            Self::ExpressionEdition => "native-expression-edition",
            Self::ExpressionResponsibility => "native-expression-responsibility",
            Self::CollectionMembership => "native-collection-membership",
            Self::WorkExpression => "native-work-expression",
            Self::ObjectLink => "native-object-link-create",
            // The owner serializer uses the maintained source-create handler
            // id and the writer binds that exact value into the proposal.
            Self::SignPromotion => "sign-promotion",
        }
    }

    pub(crate) fn family_tag(self) -> &'static str {
        match self {
            Self::ExpressionEdition => "tos-pg-maintained-growth-v1:native-expression-edition",
            Self::ExpressionResponsibility => {
                "tos-pg-maintained-growth-v1:native-expression-responsibility"
            }
            Self::CollectionMembership => {
                "tos-pg-maintained-growth-v1:native-collection-membership"
            }
            Self::WorkExpression => "tos-pg-maintained-growth-v1:native-work-expression",
            Self::ObjectLink => "tos-pg-maintained-growth-v1:native-object-link-create",
            Self::SignPromotion => "tos-pg-maintained-growth-v1:sign-promotion",
        }
    }
}

/// Readset seal captured after the owner has read its exact source closure.
/// This is a PG selection identity plus its actual predicate/addressed reads,
/// never a synthesized source revision or whole-source membership report.
#[derive(Clone)]
pub(crate) struct PgGrowthReadBinding {
    identity: PgRecordRevisionIdentity,
    reads: SourceReads,
}

impl PgGrowthReadBinding {
    pub(crate) fn capture(session: &SourceRevisionReadSession<'_>) -> Self {
        Self {
            identity: session.selection().identity(),
            reads: session.reads_snapshot(),
        }
    }

    fn validate(&self, readset: &SourceRevisionReadSet) -> DurableResult<()> {
        let current = readset.reads();
        if self.identity != PgRecordRevisionIdentity::from_selection(readset.selection())
            || self.reads.reads != current.reads
            || self.reads.locators != current.locators
            || self.reads.managed_original != current.managed_original
            || self.reads.revision != current.revision
        {
            return Err(DurableError::Conflict(
                "PG Growth owner package read epoch differs",
            ));
        }
        Ok(())
    }
}

/// Owner-prepared bytes entering the common PG revision transaction. The
/// family-specific preparer supplies exact semantic index identities; this
/// wrapper binds those results to the selected epoch and proposal.
pub(crate) struct PreparedPgGrowthRevision {
    context: CommandContext,
    proposal: PreparedCommand,
    family: PgGrowthFamily,
    binding: PgGrowthReadBinding,
    semantic_indexes: IndexRows,
}

/// A fresh Work -> Expression owner revision and its protected physical
/// source fence. Keep this wrapper alive until the common PG registrar has
/// completed; the fence closes the gap between owner preparation and commit.
pub(crate) struct PreparedPgWorkExpressionGrowth<'a> {
    pub(crate) prepared: PreparedPgGrowthRevision,
    _owner: PgWorkExpressionOwnerPackage<'a>,
}

/// Prepare one fresh Work -> Expression command inside its held PG read epoch.
/// The dispatcher must resolve command-id replay before calling this function;
/// replayed attempts use the durable recovery route and must not be freshly
/// authorized against today's source state.
pub(crate) fn prepare_pg_work_expression_growth<'a>(
    fs: &'a CreationFilesystem,
    session: &mut SourceRevisionReadSession<'_>,
    invocation: &serde_json::Value,
    configuration_raw: &[u8],
    configuration: &JsonValue,
    request_raw: &[u8],
    publication_token: Option<&str>,
    recorded_at: String,
    effective_uid: u32,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    limits: ItemLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedPgWorkExpressionGrowth<'a>> {
    let identity = session.selection().identity();
    let input = PgGrowthCurrentInput::new(session, 4096, MAX_GROWTH_SOFTWARE_MEMBER_BYTES as usize)
        .map_err(|_| SourceCommandError::Invalid("PG Growth selected input budget"))?;
    let mut selected = PgWorkExpressionInputs::select(
        &input,
        configuration,
        publication_token,
        deadline,
        cancelled,
    )?;
    for path in WORK_EXPRESSION_CONTRACTS {
        selected
            .selected
            .member(input.source_input(), path, deadline, cancelled)?;
    }
    let software_files = selected_software_files(software, components, deadline, cancelled)?;
    let context = selected.selected.context(
        configuration_raw,
        request_raw,
        recorded_at,
        effective_uid,
        &software_files,
    )?;
    let mut worker = super::revisions::selected_candidate_schema_from_context(
        invocation,
        &context.files,
        identity,
        deadline,
        cancelled,
    )?;
    let owner = crate::source_creation_store::prepare_pg_work_expression(
        fs,
        &context,
        &input,
        software,
        components,
        &mut worker,
        limits,
        cancelled,
    )?;

    // Release the input's mutable borrow of the PG session before materializing
    // the owner-specific exact read closure and selected catalog into context.
    drop(input);
    selected
        .selected
        .bind_selected_readset(session, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("PG Growth selected catalog read epoch"))?;
    let (final_context, absences) =
        context_with_pg_exact_reads(&owner.context, session, deadline, cancelled)?;
    crate::source_creation_store::verify_pg_work_context(
        fs,
        &final_context,
        &absences,
        deadline,
        cancelled,
    )?;
    observe_growth_semantic_indexes(session, &owner.semantic_indexes, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("PG Growth semantic index observation"))?;
    let binding = PgGrowthReadBinding::capture(session);
    let proposal = final_context.plan(
        owner.proposal.handler_id.as_str(),
        owner.proposal.response.clone(),
        owner.proposal.changes.clone(),
        owner.proposal.replayed,
    )?;
    let prepared = PreparedPgGrowthRevision::new(
        final_context,
        proposal,
        PgGrowthFamily::WorkExpression,
        binding,
        owner.semantic_indexes.clone(),
    )
    .map_err(|_| SourceCommandError::Conflict("PG Work Expression prepared revision"))?;
    Ok(PreparedPgWorkExpressionGrowth {
        prepared,
        _owner: owner,
    })
}

fn selected_software_files(
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Vec<cmd::SourceFile>> {
    let member_count = components.members().count();
    if member_count == 0 || member_count > MAX_GROWTH_SOFTWARE_MEMBERS {
        return Err(SourceCommandError::Invalid(
            "PG Growth software member budget",
        ));
    }
    let mut total_bytes = 0u64;
    let mut files = Vec::with_capacity(member_count);
    for member in components.members() {
        if member.size_bytes > MAX_GROWTH_SOFTWARE_MEMBER_BYTES {
            return Err(SourceCommandError::Invalid(
                "PG Growth software member byte budget",
            ));
        }
        let raw = software
            .read_selected_component(
                components,
                &member.path,
                MAX_GROWTH_SOFTWARE_MEMBER_BYTES,
                deadline,
                cancelled,
            )
            .map_err(|_| SourceCommandError::Conflict("selected Growth software member"))?;
        total_bytes = total_bytes
            .checked_add(raw.len() as u64)
            .filter(|sum| *sum <= MAX_GROWTH_SOFTWARE_TOTAL_BYTES)
            .ok_or(SourceCommandError::Invalid(
                "PG Growth software total byte budget",
            ))?;
        files.push(cmd::SourceFile {
            path: member.path.clone(),
            raw,
        });
    }
    Ok(files)
}

impl PreparedPgGrowthRevision {
    pub(crate) fn new(
        context: CommandContext,
        proposal: PreparedCommand,
        family: PgGrowthFamily,
        binding: PgGrowthReadBinding,
        semantic_indexes: IndexRows,
    ) -> DurableResult<Self> {
        let identity = binding.identity;
        if context.base_revision != identity.bootstrap_revision
            || proposal.base_revision != identity.bootstrap_revision
            || proposal.handler_id != family.handler()
            || proposal.operation != family.operation()
            || proposal.changes.is_empty()
        {
            return Err(DurableError::Conflict(
                "PG Growth prepared owner family, operation or base differs",
            ));
        }
        let changed_paths = proposal
            .changes
            .iter()
            .map(|change| change.path.as_str().to_owned())
            .collect::<BTreeSet<_>>();
        if proposal.changes.iter().any(|change| change.after.is_none())
            || semantic_indexes.iter().any(|(kind, token, path)| {
                ![
                    "metadata",
                    "claim",
                    "event",
                    "anchor",
                    "form",
                    "sign-candidate",
                ]
                .contains(&kind.as_str())
                    || token.is_empty()
                    || !changed_paths.contains(path)
            })
            || semantic_indexes
                .iter()
                .map(|(_, token, _)| token)
                .collect::<BTreeSet<_>>()
                .len()
                != semantic_indexes.len()
        {
            return Err(DurableError::Conflict(
                "PG Growth semantic index package differs from owner writes",
            ));
        }
        Ok(Self {
            context,
            proposal,
            family,
            binding,
            semantic_indexes,
        })
    }

    pub(crate) fn context(&self) -> &CommandContext {
        &self.context
    }

    pub(crate) fn proposal(&self) -> &PreparedCommand {
        &self.proposal
    }


}

/// Owner-package contract for validating its exact context against the
/// protected physical source while its retained corpus fence is held.
trait GrowthOwnerFence {
    fn verify_local(
        &self,
        fs: &CreationFilesystem,
        context: &CommandContext,
        absences: &BTreeSet<String>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()>;
}

fn verify_work_absences(
    fs: &CreationFilesystem,
    absences: &BTreeSet<String>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    for path in absences {
        let (parent, leaf) = path
            .rsplit_once('/')
            .ok_or(SourceCommandError::Invalid("PG Growth absence path"))?;
        let Some(directory) =
            crate::source_creation_store::work_transaction::read_existing_parent(fs, parent)?
        else {
            continue;
        };
        if crate::source_creation_store::work_transaction::read_at(
            &directory,
            leaf,
            fs.protected_uid(),
            MAX_SELECTED_MEMBER_BYTES as usize,
            deadline,
            cancelled,
        )?
        .is_some()
        {
            return Err(SourceCommandError::Conflict(
                "PG Growth exact path absence differs from protected corpus",
            ));
        }
    }
    Ok(())
}

impl GrowthOwnerFence for PgWorkExpressionOwnerPackage<'_> {
    fn verify_local(
        &self,
        fs: &CreationFilesystem,
        context: &CommandContext,
        absences: &BTreeSet<String>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        crate::source_creation_store::verify_pg_work_context(
            fs, context, absences, deadline, cancelled,
        )?;
        verify_work_absences(fs, absences, deadline, cancelled)
    }
}

impl GrowthOwnerFence for crate::source_creation_store::PgExpressionEditionOwnerPackage<'_> {
    fn verify_local(
        &self,
        fs: &CreationFilesystem,
        context: &CommandContext,
        absences: &BTreeSet<String>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        crate::source_creation_store::verify_pg_expression_edition_context(
            fs, context, absences, deadline, cancelled,
        )
    }
}

impl GrowthOwnerFence for crate::source_creation_store::PgExpressionResponsibilityOwnerPackage<'_> {
    fn verify_local(
        &self,
        fs: &CreationFilesystem,
        context: &CommandContext,
        absences: &BTreeSet<String>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        crate::source_creation_store::verify_pg_expression_responsibility_context(
            fs, context, absences, deadline, cancelled,
        )
    }
}

impl GrowthOwnerFence for crate::source_creation_store::PgCollectionMembershipOwnerPackage<'_> {
    fn verify_local(
        &self,
        fs: &CreationFilesystem,
        context: &CommandContext,
        absences: &BTreeSet<String>,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        crate::source_creation_store::verify_pg_collection_context(
            fs, context, absences, deadline, cancelled,
        )
    }
}

pub(crate) struct PreparedPgGrowthFamilyPackage<'a> {
    pub(crate) prepared: PreparedPgGrowthRevision,
    _owner: Box<dyn GrowthOwnerFence + 'a>,
    absences: BTreeSet<String>,
}

impl PreparedPgGrowthFamilyPackage<'_> {
    fn verify_local(
        &self,
        fs: &CreationFilesystem,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> SourceCommandResult<()> {
        self._owner.verify_local(
            fs,
            self.prepared.context(),
            &self.absences,
            deadline,
            cancelled,
        )
    }
}

fn finish_selected_growth_package<'a, P: GrowthOwnerFence + 'a>(
    fs: &CreationFilesystem,
    session: &mut SourceRevisionReadSession<'_>,
    selected: &PgGrowthSelectedInputs,
    family: PgGrowthFamily,
    owner_context: &CommandContext,
    owner_proposal: &PreparedCommand,
    semantic_indexes: &IndexRows,
    owner: P,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedPgGrowthFamilyPackage<'a>> {
    selected
        .bind_selected_readset(session, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("PG Growth selected owner read epoch"))?;
    let (context, absences) =
        context_with_pg_exact_reads(owner_context, session, deadline, cancelled)?;
    owner.verify_local(fs, &context, &absences, deadline, cancelled)?;
    observe_growth_semantic_indexes(session, semantic_indexes, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("PG Growth semantic index observation"))?;
    let binding = PgGrowthReadBinding::capture(session);
    let proposal = context.plan(
        owner_proposal.handler_id.as_str(),
        owner_proposal.response.clone(),
        owner_proposal.changes.clone(),
        owner_proposal.replayed,
    )?;
    let prepared =
        PreparedPgGrowthRevision::new(context, proposal, family, binding, semantic_indexes.clone())
            .map_err(|_| SourceCommandError::Conflict("PG Growth prepared owner revision"))?;
    Ok(PreparedPgGrowthFamilyPackage {
        prepared,
        _owner: Box::new(owner),
        absences,
    })
}

fn growth_item_limits(
    max_total_bytes: u64,
    max_member_bytes: u64,
    deadline: Instant,
) -> ItemLimits {
    let max_member_bytes = usize::try_from(max_member_bytes).unwrap_or(usize::MAX);
    let max_state_bytes = usize::try_from(max_total_bytes).unwrap_or(usize::MAX);
    ItemLimits {
        max_member_bytes,
        max_total_bytes,
        max_state_bytes,
        max_issues: 256,
        deadline,
    }
}

fn run_pg_growth_family_preview(
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    request_raw: &[u8],
    request: &JsonValue,
    request_schema: &str,
    prepare_operation: &str,
    family: PgGrowthFamily,
    operation: &str,
    uid: u32,
    selection: &GrowthSourceSelection,
    owner_path: &Path,
    owner_fs: &CreationFilesystem,
    owner_raw: &[u8],
    owner_config: &JsonValue,
    expected_executable: Digest256,
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    if cmd::text(request, "schema_version")? != request_schema {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL Growth preview request profile",
        ));
    }
    if operation == "describe" {
        cmd::exact_keys(request, &["schema_version", "operation"])?;
    } else if operation != prepare_operation {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL Growth preview operation",
        ));
    }
    let runtime = crate::source_serialization::observe_executable(deadline, cancelled)?;
    if runtime.current_digest(deadline, cancelled)? != expected_executable {
        return Err(SourceCommandError::Conflict(
            "native Growth executable identity",
        ));
    }
    let store = SegmentStore::open_existing(&selection.store_root, selection.store_limits)
        .map_err(|_| SourceCommandError::Conflict("selected Growth STO"))?;
    let (software, components) = growth_software_selection(invocation, deadline, cancelled)?;
    let mut db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected PostgreSQL unavailable"))?,
    );
    let recorded_at = crate::source_serialization::instant()?;
    let ((response, context, absences, binding), readset) = db
        .with_source_revision_session(
            &store,
            &selection.domain,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                if session.selection().contract != selection.contract {
                    return Err(DurableError::Conflict(
                        "selected Growth source contract differs from current domain",
                    ));
                }
                let (response, context, absences) = prepare_pg_growth_family_preview(
                    family,
                    operation,
                    owner_fs,
                    session,
                    invocation,
                    owner_raw,
                    owner_config,
                    request_raw,
                    request,
                    recorded_at.clone(),
                    uid,
                    &software,
                    &components,
                    growth_item_limits(max_total_bytes, max_member_bytes, deadline),
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                let binding = PgGrowthReadBinding::capture(session);
                check_growth_context_budget(
                    &context,
                    max_members,
                    max_total_bytes,
                    max_member_bytes,
                )
                .map_err(DurableError::Source)?;
                Ok((response, context, absences, binding))
            },
        )
        .map_err(growth_source_error)?;
    binding
        .validate(&readset)
        .map_err(|_| SourceCommandError::Conflict("PG Growth preview readset changed"))?;
    let mut result = growth_value(&response)?;
    result["source_read_work"] = growth_source_read_work(&readset.work);
    if super::read_absolute(
        invocation_path,
        uid,
        true,
        invocation_raw.len(),
        deadline,
        cancelled,
    )? != invocation_raw
    {
        return Err(SourceCommandError::Conflict(
            "PG Growth invocation changed before preview",
        ));
    }
    let (current_fs, current_raw) = CreationFilesystem::select_protected_native_owner_at_root(
        owner_path,
        &selection.store_root,
        deadline,
        cancelled,
    )?;
    if current_raw != owner_raw {
        return Err(SourceCommandError::Conflict(
            "PG Growth owner configuration changed before preview",
        ));
    }
    verify_pg_growth_family_context(
        family,
        &current_fs,
        &context,
        &absences,
        deadline,
        cancelled,
    )?;
    if runtime.current_digest(deadline, cancelled)? != expected_executable {
        return Err(SourceCommandError::Conflict(
            "native Growth executable changed before preview",
        ));
    }
    Ok(json!({
        "schema_version":"tos_local_pg_growth_result_v1",
        "result":result,
        "grants_admission":false
    }))
}

#[allow(clippy::too_many_arguments)]
fn prepare_pg_growth_family_preview(
    family: PgGrowthFamily,
    operation: &str,
    fs: &CreationFilesystem,
    session: &mut SourceRevisionReadSession<'_>,
    invocation: &Value,
    configuration_raw: &[u8],
    configuration: &JsonValue,
    request_raw: &[u8],
    _request: &JsonValue,
    recorded_at: String,
    effective_uid: u32,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    limits: ItemLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<(JsonValue, CommandContext, BTreeSet<String>)> {
    let identity = session.selection().identity();
    let input = PgGrowthCurrentInput::new(session, 4096, MAX_GROWTH_SOFTWARE_MEMBER_BYTES as usize)
        .map_err(|_| SourceCommandError::Invalid("PG Growth selected input budget"))?;
    let publication =
        crate::source_creation_store::work_transaction::PublicationSnapshot::select_from_input(
            input.source_input(),
            deadline,
            cancelled,
        )?;
    let mut selected =
        PgGrowthSelectedInputs::select(&input, publication.token.as_deref(), deadline, cancelled)?;
    for path in EXPRESSION_GROWTH_CONTRACTS {
        selected.member(input.source_input(), path, deadline, cancelled)?;
    }
    let software_files = selected_software_files(software, components, deadline, cancelled)?;
    let context = selected.context(
        configuration_raw,
        request_raw,
        recorded_at,
        effective_uid,
        &software_files,
    )?;
    let mut worker = super::revisions::selected_candidate_schema_from_context(
        invocation,
        &context.files,
        identity,
        deadline,
        cancelled,
    )?;
    let mut projected_outputs = None;
    let response = match family {
        PgGrowthFamily::ExpressionEdition if operation == "describe" => {
            crate::source_creation_store::pg_current_result_fields(
                fs,
                &context,
                &input,
                &selected.catalog,
                software,
                components,
                &mut worker,
                limits,
                cancelled,
            )?
        }
        PgGrowthFamily::ExpressionEdition => {
            let prepared = crate::source_creation_store::prepare_pg_expression_edition_preview(
                fs,
                &context,
                &input,
                &selected.catalog,
                software,
                components,
                &mut worker,
                limits,
                cancelled,
            )?;
            projected_outputs = Some(prepared.projected_outputs().clone());
            prepared.result_fields(configuration)?
        }
        PgGrowthFamily::ExpressionResponsibility if operation == "describe" => {
            crate::source_creation_store::pg_expression_responsibility_current_result_fields(
                fs,
                &context,
                &input,
                &selected.catalog,
                software,
                components,
                &mut worker,
                limits,
                cancelled,
            )?
        }
        PgGrowthFamily::ExpressionResponsibility => {
            let prepared =
                crate::source_creation_store::prepare_pg_expression_responsibility_preview(
                    fs,
                    &context,
                    &input,
                    &selected.catalog,
                    software,
                    components,
                    &mut worker,
                    limits,
                    cancelled,
                )?;
            projected_outputs = Some(prepared.projected_outputs().clone());
            prepared.result_fields(configuration)?
        }
        PgGrowthFamily::CollectionMembership if operation == "describe" => {
            crate::source_creation_store::pg_collection_current_result_fields(
                fs,
                &context,
                &input,
                &selected.catalog,
                software,
                components,
                &mut worker,
                limits,
                cancelled,
            )?
        }
        PgGrowthFamily::CollectionMembership => {
            let prepared = crate::source_creation_store::prepare_pg_collection_membership_preview(
                fs,
                &context,
                &input,
                &selected.catalog,
                software,
                components,
                &mut worker,
                limits,
                cancelled,
            )?;
            projected_outputs = Some(prepared.projected_outputs().clone());
            prepared.result_fields(configuration)?
        }
        _ => {
            return Err(SourceCommandError::Unsupported(
                "PostgreSQL Growth preview family",
            ));
        }
    };
    drop(input);
    selected
        .bind_selected_readset(session, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("PG Growth preview selected read epoch"))?;
    let (context, absences) = context_with_pg_exact_reads(&context, session, deadline, cancelled)?;
    if let Some(outputs) = projected_outputs {
        let writes = projected_preview_writes(&context, &outputs)?;
        let indexes =
            expected_growth_revision_indexes(family, &writes).map_err(growth_source_error)?;
        let semantic_indexes = indexes
            .into_iter()
            .filter(|(kind, _, _)| kind != "path")
            .collect::<crate::source_cohort::IndexRows>();
        observe_growth_semantic_indexes(session, &semantic_indexes, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("PG Growth preview identity observation"))?;
    }
    Ok((response, context, absences))
}

fn projected_preview_writes(
    context: &CommandContext,
    outputs: &BTreeMap<String, Vec<u8>>,
) -> SourceCommandResult<Vec<SourceRevisionWrite>> {
    outputs
        .iter()
        .map(|(path, exact_bytes)| {
            let relative = RelativePath::parse(path)
                .map_err(|_| SourceCommandError::Invalid("PG Growth preview output path"))?;
            if relative.as_str() != path {
                return Err(SourceCommandError::Invalid(
                    "PG Growth preview noncanonical output path",
                ));
            }
            let predecessor = context
                .files
                .iter()
                .find(|file| file.path.as_str() == path)
                .map(|file| (1, Digest256::of_bytes(&file.raw)));
            Ok(SourceRevisionWrite {
                subject: path.clone(),
                expected_predecessor: predecessor,
                proposed_revision: 1,
                exact_bytes: exact_bytes.clone(),
            })
        })
        .collect()
}

fn verify_pg_growth_family_context(
    family: PgGrowthFamily,
    fs: &CreationFilesystem,
    context: &CommandContext,
    absences: &BTreeSet<String>,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
    match family {
        PgGrowthFamily::ExpressionEdition => {
            crate::source_creation_store::verify_pg_expression_edition_context(
                fs, context, absences, deadline, cancelled,
            )
        }
        PgGrowthFamily::ExpressionResponsibility => {
            crate::source_creation_store::verify_pg_expression_responsibility_context(
                fs, context, absences, deadline, cancelled,
            )
        }
        PgGrowthFamily::CollectionMembership => {
            crate::source_creation_store::verify_pg_collection_context(
                fs, context, absences, deadline, cancelled,
            )
        }
        _ => Err(SourceCommandError::Unsupported(
            "PostgreSQL Growth preview fence family",
        )),
    }
}

fn growth_source_read_work(work: &crate::source_cohort::revision::SourceRevisionReadWork) -> Value {
    json!({
        "scope":"bounded PG/STO source selection; software capture and physical I/O excluded",
        "exact_source_members_read":work.exact_source_members_read,
        "exact_source_bytes_read":work.exact_source_bytes_read,
        "historical_source_members_read":work.historical_source_members_read,
        "historical_source_bytes_read":work.historical_source_bytes_read,
        "custody_metadata_bytes_read":work.custody_metadata_bytes_read,
        "segment_bytes_read":work.segment_bytes_read,
        "committed_member_rows_examined":work.committed_member_rows_examined,
        "metadata_member_lookups":work.metadata_member_lookups,
        "directory_index_rows_examined":work.directory_index_rows_examined,
        "unique_index_lookups":work.unique_index_lookups,
        "text_identity_lookups":work.text_identity_lookups,
        "claim_dependency_lookups":work.claim_dependency_lookups,
        "claim_navigation_lookups":work.claim_navigation_lookups,
        "item_file_lookups":work.item_file_lookups,
        "item_rights_lookups":work.item_rights_lookups,
        "item_edition_lookups":work.item_edition_lookups,
        "sign_candidate_lookups":work.sign_candidate_lookups,
    })
}

fn run_pg_growth_family<'owner, Prepare>(
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    request_raw: &[u8],
    request: &JsonValue,
    request_schema: &str,
    family: PgGrowthFamily,
    uid: u32,
    selection: &GrowthSourceSelection,
    owner_path: &Path,
    owner_fs: &'owner CreationFilesystem,
    owner_raw: &[u8],
    owner_config: &JsonValue,
    expected_executable: Digest256,
    max_members: usize,
    max_total_bytes: u64,
    max_member_bytes: u64,
    deadline: Instant,
    cancelled: &AtomicBool,
    prepare: Prepare,
) -> SourceCommandResult<Value>
where
    Prepare: for<'session> FnOnce(
        &'owner CreationFilesystem,
        &mut SourceRevisionReadSession<'session>,
        &SoftwareCaptureReader,
        &SoftwareComponentSelectionV1,
        String,
    )
        -> SourceCommandResult<PreparedPgGrowthFamilyPackage<'owner>>,
{
    if cmd::text(request, "schema_version")? != request_schema
        || cmd::text(request, "operation")? != family.operation()
    {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL maintained Growth operation",
        ));
    }
    let command_id = cmd::text(request, "command_id")?;
    if command_id.is_empty() || command_id.len() > MAX_JOB_ID_BYTES {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL Growth command identity",
        ));
    }
    let runtime = crate::source_serialization::observe_executable(deadline, cancelled)?;
    if runtime.current_digest(deadline, cancelled)? != expected_executable {
        return Err(SourceCommandError::Conflict(
            "native Growth executable identity",
        ));
    }
    let store = SegmentStore::open_existing(&selection.store_root, selection.store_limits)
        .map_err(|_| SourceCommandError::Conflict("selected Growth STO"))?;
    let (software, components) = growth_software_selection(invocation, deadline, cancelled)?;
    let mut db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected PostgreSQL unavailable"))?,
    );
    let prepare_id = growth_prepare_id(&selection.domain, command_id);
    match db.recover_source_revision_attempt(
        &store,
        &selection.domain,
        &prepare_id,
        command_id,
        Digest256::of_bytes(request_raw),
        Digest256::of_bytes(owner_raw),
        cmd::record_digest(owner_config)?,
        family.family_tag(),
        false,
        &selection.job_id,
        selection.job_fence,
        None,
        deadline,
        cancelled,
    ) {
        Ok(attempt) => {
            if attempt.contract != selection.contract {
                return Err(SourceCommandError::Conflict(
                    "recovered Growth source contract changed",
                ));
            }
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
                        "PostgreSQL Growth invocation changed before recovery",
                    ));
                }
                let (_, current_raw) = CreationFilesystem::select_protected_native_owner_at_root(
                    owner_path,
                    &selection.store_root,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                if current_raw != owner_raw {
                    return Err(DurableError::Conflict(
                        "PostgreSQL Growth owner configuration changed before recovery",
                    ));
                }
                if runtime
                    .current_digest(deadline, cancelled)
                    .map_err(DurableError::Source)?
                    != expected_executable
                {
                    return Err(DurableError::Conflict(
                        "native Growth executable changed before recovery",
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
                .map_err(growth_source_error)?;
            return Ok(json!({
                "schema_version":"tos_local_pg_growth_result_v1",
                "operation":family.operation(),
                "replayed":receipt.replayed,
                "durable_commit":growth_commit_result(&receipt),
                "grants_admission":false
            }));
        }
        Err(DurableError::Refused("source revision attempt absent")) => {}
        Err(error) => return Err(growth_source_error(error)),
    }

    let recorded_at = crate::source_serialization::instant()?;
    let (prepared, readset) = db
        .with_source_revision_session(
            &store,
            &selection.domain,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                if session.selection().contract != selection.contract {
                    return Err(DurableError::Conflict(
                        "selected Growth source contract differs from current domain",
                    ));
                }
                let prepared = prepare(
                    owner_fs,
                    session,
                    &software,
                    &components,
                    recorded_at.clone(),
                )
                .map_err(DurableError::Source)?;
                check_growth_context_budget(
                    prepared.prepared.context(),
                    max_members,
                    max_total_bytes,
                    max_member_bytes,
                )
                .map_err(DurableError::Source)?;
                Ok(prepared)
            },
        )
        .map_err(growth_source_error)?;
    let mut result = growth_value(&prepared.prepared.proposal().response)?;
    result["source_read_work"] = growth_source_read_work(&readset.work);
    let attempt = db
        .register_source_revision(
            &store,
            &prepared.prepared,
            &readset,
            &prepare_id,
            deadline,
            cancelled,
        )
        .map_err(growth_source_error)?;
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
                "PostgreSQL Growth invocation changed before commit",
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
                "PostgreSQL Growth owner configuration changed before commit",
            ));
        }
        prepared
            .verify_local(&current_fs, deadline, cancelled)
            .map_err(DurableError::Source)?;
        if runtime
            .current_digest(deadline, cancelled)
            .map_err(DurableError::Source)?
            != expected_executable
        {
            return Err(DurableError::Conflict(
                "native Growth executable changed before commit",
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
        .map_err(growth_source_error)?;
    result["replayed"] = json!(receipt.replayed);
    result["durable_commit"] = growth_commit_result(&receipt);
    Ok(json!({
        "schema_version":"tos_local_pg_growth_result_v1",
        "result":result,
        "grants_admission":false
    }))
}

const EXPRESSION_GROWTH_CONTRACTS: &[&str] = &[
    "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "ToS/doctrine/semantic-interchange/relation-types.v1.json",
    "ToS/contracts/corpus-record.schema.json",
    "ToS/contracts/semantic-entity-type-registry.schema.json",
    "ToS/contracts/semantic-relation-type-registry.schema.json",
    "ToS/contracts/human-form.schema.json",
    "ToS/contracts/human-form-set.schema.json",
    "ToS/contracts/human-form-template.schema.json",
    "ToS/contracts/provenance-event-v2.schema.json",
];

pub(crate) fn prepare_pg_expression_edition_growth<'a>(
    fs: &'a CreationFilesystem,
    session: &mut SourceRevisionReadSession<'_>,
    invocation: &Value,
    configuration_raw: &[u8],
    configuration: &JsonValue,
    request_raw: &[u8],
    request: &JsonValue,
    recorded_at: String,
    effective_uid: u32,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    limits: ItemLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedPgGrowthFamilyPackage<'a>> {
    let identity = session.selection().identity();
    let input = PgGrowthCurrentInput::new(session, 4096, MAX_GROWTH_SOFTWARE_MEMBER_BYTES as usize)
        .map_err(|_| SourceCommandError::Invalid("PG Growth selected input budget"))?;
    let mut selected = PgExpressionEditionInputs::select(
        &input,
        fs,
        configuration,
        cmd::field(request, "expected_publication")?.as_str(),
        deadline,
        cancelled,
    )?;
    for path in EXPRESSION_GROWTH_CONTRACTS {
        selected
            .selected
            .member(input.source_input(), path, deadline, cancelled)?;
    }
    let software_files = selected_software_files(software, components, deadline, cancelled)?;
    let context = selected.selected.context(
        configuration_raw,
        request_raw,
        recorded_at,
        effective_uid,
        &software_files,
    )?;
    let mut worker = super::revisions::selected_candidate_schema_from_context(
        invocation,
        &context.files,
        identity,
        deadline,
        cancelled,
    )?;
    let owner = crate::source_creation_store::prepare_pg_expression_edition(
        fs,
        &context,
        &input,
        &selected.selected.catalog,
        software,
        components,
        &mut worker,
        limits,
        cancelled,
    )?;
    drop(input);
    let owner_context = owner.context.clone();
    let owner_proposal = owner.proposal.clone();
    let semantic_indexes = owner.semantic_indexes.clone();
    finish_selected_growth_package(
        fs,
        session,
        &selected.selected,
        PgGrowthFamily::ExpressionEdition,
        &owner_context,
        &owner_proposal,
        &semantic_indexes,
        owner,
        deadline,
        cancelled,
    )
}

pub(crate) fn prepare_pg_expression_responsibility_growth<'a>(
    fs: &'a CreationFilesystem,
    session: &mut SourceRevisionReadSession<'_>,
    invocation: &Value,
    configuration_raw: &[u8],
    configuration: &JsonValue,
    request_raw: &[u8],
    request: &JsonValue,
    recorded_at: String,
    effective_uid: u32,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    limits: ItemLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedPgGrowthFamilyPackage<'a>> {
    let identity = session.selection().identity();
    let input = PgGrowthCurrentInput::new(session, 4096, MAX_GROWTH_SOFTWARE_MEMBER_BYTES as usize)
        .map_err(|_| SourceCommandError::Invalid("PG Growth selected input budget"))?;
    let mut selected = PgExpressionResponsibilityInputs::select(
        &input,
        fs,
        configuration,
        cmd::field(request, "expected_publication")?.as_str(),
        deadline,
        cancelled,
    )?;
    for path in EXPRESSION_GROWTH_CONTRACTS {
        selected
            .selected
            .member(input.source_input(), path, deadline, cancelled)?;
    }
    let software_files = selected_software_files(software, components, deadline, cancelled)?;
    let context = selected.selected.context(
        configuration_raw,
        request_raw,
        recorded_at,
        effective_uid,
        &software_files,
    )?;
    let mut worker = super::revisions::selected_candidate_schema_from_context(
        invocation,
        &context.files,
        identity,
        deadline,
        cancelled,
    )?;
    let owner = crate::source_creation_store::prepare_pg_expression_responsibility(
        fs,
        &context,
        &input,
        &selected.selected.catalog,
        software,
        components,
        &mut worker,
        limits,
        cancelled,
    )?;
    drop(input);
    let owner_context = owner.context.clone();
    let owner_proposal = owner.proposal.clone();
    let semantic_indexes = owner.semantic_indexes.clone();
    finish_selected_growth_package(
        fs,
        session,
        &selected.selected,
        PgGrowthFamily::ExpressionResponsibility,
        &owner_context,
        &owner_proposal,
        &semantic_indexes,
        owner,
        deadline,
        cancelled,
    )
}

pub(crate) fn prepare_pg_collection_membership_growth<'a>(
    fs: &'a CreationFilesystem,
    session: &mut SourceRevisionReadSession<'_>,
    invocation: &Value,
    configuration_raw: &[u8],
    configuration: &JsonValue,
    request_raw: &[u8],
    request: &JsonValue,
    recorded_at: String,
    effective_uid: u32,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    limits: ItemLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<PreparedPgGrowthFamilyPackage<'a>> {
    let identity = session.selection().identity();
    let input = PgGrowthCurrentInput::new(session, 4096, MAX_GROWTH_SOFTWARE_MEMBER_BYTES as usize)
        .map_err(|_| SourceCommandError::Invalid("PG Growth selected input budget"))?;
    let mut selected = PgCollectionMembershipInputs::select(
        &input,
        fs,
        configuration,
        cmd::field(request, "expected_publication")?.as_str(),
        deadline,
        cancelled,
    )?;
    for path in EXPRESSION_GROWTH_CONTRACTS {
        selected
            .selected
            .member(input.source_input(), path, deadline, cancelled)?;
    }
    let software_files = selected_software_files(software, components, deadline, cancelled)?;
    let context = selected.selected.context(
        configuration_raw,
        request_raw,
        recorded_at,
        effective_uid,
        &software_files,
    )?;
    let mut worker = super::revisions::selected_candidate_schema_from_context(
        invocation,
        &context.files,
        identity,
        deadline,
        cancelled,
    )?;
    let owner = crate::source_creation_store::prepare_pg_collection_membership(
        fs,
        &context,
        &input,
        &selected.selected.catalog,
        software,
        components,
        &mut worker,
        limits,
        cancelled,
    )?;
    drop(input);
    let owner_context = owner.context.clone();
    let owner_proposal = owner.proposal.clone();
    let semantic_indexes = owner.semantic_indexes.clone();
    finish_selected_growth_package(
        fs,
        session,
        &selected.selected,
        PgGrowthFamily::CollectionMembership,
        &owner_context,
        &owner_proposal,
        &semantic_indexes,
        owner,
        deadline,
        cancelled,
    )
}

impl PreparedPgSourceRevision for PreparedPgGrowthRevision {
    fn context(&self) -> &CommandContext {
        &self.context
    }

    fn proposal(&self) -> &PreparedCommand {
        &self.proposal
    }

    fn identity(&self) -> PgRecordRevisionIdentity {
        self.binding.identity
    }

    fn family_tag(&self) -> &str {
        self.family.family_tag()
    }

    fn legacy_family_encoding(&self) -> bool {
        false
    }

    fn validate_readset(&self, readset: &SourceRevisionReadSet) -> DurableResult<()> {
        self.binding.validate(readset)
    }

    fn work_revision_evidence(
        &self,
        _writes: &[SourceRevisionWrite],
    ) -> DurableResult<Option<crate::source_pg_work_revision::CommitEvidence>> {
        Ok(None)
    }

    fn check_indexes(
        &self,
        tx: &mut Transaction<'_>,
        domain: &str,
        writes: &[SourceRevisionWrite],
    ) -> DurableResult<IndexRows> {
        let mut indexes = source_cohort::check_revision_path_indexes(tx, domain, writes)?;
        let globally_unique = vec![
            "metadata".to_owned(),
            "claim".to_owned(),
            "event".to_owned(),
            "anchor".to_owned(),
            "form".to_owned(),
        ];
        for (kind, token, path) in &self.semantic_indexes {
            let occupied = if kind == "sign-candidate" {
                tx.query_opt(
                    "SELECT 1 FROM cmd2_source_index WHERE domain=$1 AND kind='sign-candidate' AND token=$2 LIMIT 1",
                    &[&domain, token],
                )?
            } else {
                tx.query_opt(
                    "SELECT 1 FROM cmd2_source_index WHERE domain=$1 AND kind=ANY($2) AND token=$3 LIMIT 1",
                    &[&domain, &globally_unique, token],
                )?
            };
            if !writes.iter().any(|write| write.subject == *path) || occupied.is_some() {
                return Err(DurableError::Conflict(
                    "PG Growth semantic identity is occupied or unbound",
                ));
            }
            indexes.insert((kind.clone(), token.clone(), path.clone()));
        }
        if indexes != expected_growth_revision_indexes(self.family, writes)? {
            return Err(DurableError::Corrupt(
                "PG Growth index rows differ from sealed owner package",
            ));
        }
        Ok(indexes)
    }

    fn inventory_owner_contributions(
        &self,
        writes: &[SourceRevisionWrite],
    ) -> DurableResult<BTreeMap<String, JsonValue>> {
        match self.family {
            PgGrowthFamily::WorkExpression => {
                crate::source_creation_store::expected_work_expression_inventory_contributions(
                    writes,
                    &self.context,
                    &self.proposal.response,
                )
            }
            PgGrowthFamily::ObjectLink => {
                crate::source_creation_store::expected_object_link_inventory_contributions(
                    writes,
                    &self.context,
                    &self.proposal.response,
                )
            }
            PgGrowthFamily::CollectionMembership => {
                crate::source_creation_store::expected_collection_inventory_contributions(
                    writes,
                    &self.context,
                    &self.proposal.response,
                )
            }
            PgGrowthFamily::ExpressionEdition => {
                crate::source_creation_store::expected_expression_edition_inventory_contributions(
                    writes,
                    &self.context,
                    &self.proposal.response,
                )
            }
            PgGrowthFamily::ExpressionResponsibility => {
                crate::source_creation_store::expected_expression_responsibility_inventory_contributions(
                    writes,
                    &self.context,
                    &self.proposal.response,
                )
            }
            PgGrowthFamily::SignPromotion => {
                crate::source_sign::pg::expected_inventory_contributions_from_writes(
                    &self.context,
                    &self.proposal,
                    writes,
                )
                .map_err(DurableError::Source)
            }
        }
    }
}

/// Re-derive each maintained family's path and semantic indexes from the
/// exact immutable package bytes. Registration and recovery both use this
/// route so the serialized attempt indexes cannot become owner authority.
pub(crate) fn expected_growth_revision_indexes(
    family: PgGrowthFamily,
    writes: &[SourceRevisionWrite],
) -> DurableResult<IndexRows> {
    match family {
        PgGrowthFamily::WorkExpression => {
            crate::source_creation_store::expected_work_expression_revision_indexes(writes)
        }
        PgGrowthFamily::ObjectLink => {
            crate::source_creation_store::expected_object_link_revision_indexes(writes)
        }
        PgGrowthFamily::CollectionMembership => {
            crate::source_creation_store::expected_collection_membership_revision_indexes(writes)
        }
        PgGrowthFamily::ExpressionEdition => {
            crate::source_creation_store::expected_expression_edition_revision_indexes(writes)
        }
        PgGrowthFamily::ExpressionResponsibility => {
            crate::source_creation_store::expected_expression_responsibility_revision_indexes(
                writes,
            )
        }
        PgGrowthFamily::SignPromotion => {
            crate::source_sign::pg::expected_semantic_indexes_from_writes(writes)
                .map_err(DurableError::Source)
        }
    }
}

/// Compare a captured selected-input binding to the fenced PG readset after
/// owner preparation. Kept crate-local so sibling owner readers share one law.
pub(crate) fn validate_growth_readset(
    binding: &PgGrowthReadBinding,
    readset: &SourceRevisionReadSet,
) -> DurableResult<()> {
    binding.validate(readset)
}

/// Bind each newly proposed semantic identity to the five fixed unique
/// namespaces before the owner package captures its read epoch. The session
/// observation records every namespace generation even when the token is
/// initially absent.
pub(crate) fn observe_growth_semantic_indexes(
    session: &mut SourceRevisionReadSession<'_>,
    semantic_indexes: &IndexRows,
    deadline: std::time::Instant,
    cancelled: &std::sync::atomic::AtomicBool,
) -> DurableResult<()> {
    if semantic_indexes.len() > 512
        || semantic_indexes.iter().any(|(kind, token, path)| {
            ![
                "metadata",
                "claim",
                "event",
                "anchor",
                "form",
                "sign-candidate",
            ]
            .contains(&kind.as_str())
                || token.is_empty()
                || path.is_empty()
        })
    {
        return Err(DurableError::Invalid(
            "PG Growth semantic index observation bound",
        ));
    }
    for (kind, token, _) in semantic_indexes {
        if kind == "sign-candidate"
            && session
                .lookup_sign_promotion_candidate(token, deadline, cancelled)?
                .is_some()
        {
            return Err(DurableError::Conflict(
                "PG Sign promotion candidate is already indexed",
            ));
        }
    }
    let tokens = semantic_indexes
        .iter()
        .filter(|(kind, _, _)| kind != "sign-candidate")
        .map(|(_, token, _)| token.as_str())
        .collect::<BTreeSet<_>>();
    for token in tokens {
        for kind in crate::source_cohort::revision::PG_AGENT_INDEX_KINDS {
            session.observe_unique_index(kind, token, deadline, cancelled)?;
        }
    }
    Ok(())
}

fn prepare_pg_object_link_growth<'a>(
    fs: &'a CreationFilesystem,
    session: &mut SourceRevisionReadSession<'_>,
    invocation: &Value,
    configuration_raw: &[u8],
    configuration: &JsonValue,
    request_raw: &[u8],
    publication_token: Option<&str>,
    recorded_at: String,
    effective_uid: u32,
    software: &SoftwareCaptureReader,
    components: &SoftwareComponentSelectionV1,
    limits: ItemLimits,
    deadline: Instant,
    cancelled: &'a AtomicBool,
) -> SourceCommandResult<crate::source_creation_store::PreparedPgObjectLink<'a>> {
    let identity = session.selection().identity();
    let input = PgGrowthCurrentInput::new(session, 4096, MAX_GROWTH_SOFTWARE_MEMBER_BYTES as usize)
        .map_err(|_| SourceCommandError::Invalid("PG ObjectLink selected input budget"))?;
    let mut selected =
        PgGrowthSelectedInputs::select(&input, publication_token, deadline, cancelled)?;
    for path in OBJECT_LINK_CONTRACTS {
        selected.member(input.source_input(), path, deadline, cancelled)?;
    }
    let software_files = selected_software_files(software, components, deadline, cancelled)?;
    let context = selected.context(
        configuration_raw,
        request_raw,
        recorded_at,
        effective_uid,
        &software_files,
    )?;
    let mut worker = super::revisions::selected_candidate_schema_from_context(
        invocation,
        &context.files,
        identity,
        deadline,
        cancelled,
    )?;
    let requirements =
        crate::source_creation_store::pg_object_link_requirements(fs, &context, limits, cancelled)?;
    for home in requirements.homes() {
        input
            .observe_home_absence(home, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("ObjectLink PG home absence"))?;
    }
    for (kind, token) in requirements.unique_indexes() {
        input
            .observe_unique_index(kind, token, deadline, cancelled)
            .map_err(|_| SourceCommandError::Conflict("ObjectLink PG identity absence"))?;
    }
    let mut prepared = crate::source_creation_store::prepare_pg_object_link(
        fs,
        &context,
        &input,
        &mut worker,
        software,
        components,
        limits,
        cancelled,
    )?;
    drop(input);
    selected
        .bind_selected_readset(session, deadline, cancelled)
        .map_err(|_| SourceCommandError::Conflict("ObjectLink PG selected read epoch"))?;
    prepared
        .bind_read_snapshot(session.reads_snapshot())
        .map_err(|_| SourceCommandError::Conflict("ObjectLink PG prepared readset"))?;
    Ok(prepared)
}

fn run_pg_object_link(
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    request_raw: &[u8],
    request: &JsonValue,
    uid: u32,
    selection: &GrowthSourceSelection,
    owner_path: &Path,
    owner_fs: &CreationFilesystem,
    owner_raw: &[u8],
    owner_config: &JsonValue,
    expected_executable: Digest256,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    if cmd::text(request, "schema_version")? != "tos_local_object_link_command_v1" {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL ObjectLink operation",
        ));
    }
    if matches!(
        cmd::text(request, "operation")?,
        "describe" | "prepare-create"
    ) {
        return run_pg_object_link_preview(
            invocation_path,
            invocation_raw,
            invocation,
            request_raw,
            request,
            uid,
            selection,
            owner_path,
            owner_fs,
            owner_raw,
            owner_config,
            expected_executable,
            deadline,
            cancelled,
        );
    }
    if cmd::text(request, "operation")? != "object.link.create" {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL ObjectLink operation",
        ));
    }
    let command_id = cmd::text(request, "command_id")?;
    if command_id.is_empty() || command_id.len() > MAX_JOB_ID_BYTES {
        return Err(SourceCommandError::Invalid(
            "PostgreSQL ObjectLink command identity",
        ));
    }
    let runtime = crate::source_serialization::observe_executable(deadline, cancelled)?;
    if runtime.current_digest(deadline, cancelled)? != expected_executable {
        return Err(SourceCommandError::Conflict(
            "native ObjectLink executable identity",
        ));
    }
    let store = SegmentStore::open_existing(&selection.store_root, selection.store_limits)
        .map_err(|_| SourceCommandError::Conflict("selected ObjectLink STO"))?;
    let (software, components) = growth_software_selection(invocation, deadline, cancelled)?;
    let mut db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected PostgreSQL unavailable"))?,
    );
    let prepare_id = growth_prepare_id(&selection.domain, command_id);
    match db.recover_source_revision_attempt(
        &store,
        &selection.domain,
        &prepare_id,
        command_id,
        Digest256::of_bytes(request_raw),
        Digest256::of_bytes(owner_raw),
        cmd::record_digest(owner_config)?,
        PgGrowthFamily::ObjectLink.family_tag(),
        false,
        &selection.job_id,
        selection.job_fence,
        None,
        deadline,
        cancelled,
    ) {
        Ok(attempt) => {
            if attempt.contract != selection.contract {
                return Err(SourceCommandError::Conflict(
                    "recovered ObjectLink source contract changed",
                ));
            }
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
                        "PostgreSQL ObjectLink invocation changed before recovery",
                    ));
                }
                let (_, current_raw) = CreationFilesystem::select_protected_native_owner_at_root(
                    owner_path,
                    &selection.store_root,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                if current_raw != owner_raw {
                    return Err(DurableError::Conflict(
                        "PostgreSQL ObjectLink owner configuration changed before recovery",
                    ));
                }
                if runtime
                    .current_digest(deadline, cancelled)
                    .map_err(DurableError::Source)?
                    != expected_executable
                {
                    return Err(DurableError::Conflict(
                        "native ObjectLink executable changed before recovery",
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
                .map_err(growth_source_error)?;
            return Ok(json!({
                "schema_version":"tos_local_pg_growth_result_v1",
                "operation":"object.link.create",
                "replayed":receipt.replayed,
                "durable_commit":growth_commit_result(&receipt),
                "grants_admission":false
            }));
        }
        Err(DurableError::Refused("source revision attempt absent")) => {}
        Err(error) => return Err(growth_source_error(error)),
    }

    let recorded_at = crate::source_serialization::instant()?;
    let (prepared, readset) = db
        .with_source_revision_session(
            &store,
            &selection.domain,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                if session.selection().contract != selection.contract {
                    return Err(DurableError::Conflict(
                        "selected ObjectLink source contract differs from current domain",
                    ));
                }
                let prepared = prepare_pg_object_link_growth(
                    owner_fs,
                    session,
                    invocation,
                    owner_raw,
                    owner_config,
                    request_raw,
                    cmd::field(request, "expected_publication")
                        .map_err(DurableError::Source)?
                        .as_str(),
                    recorded_at.clone(),
                    uid,
                    &software,
                    &components,
                    ItemLimits {
                        max_member_bytes: 2_097_152,
                        max_total_bytes: 33_554_432,
                        max_state_bytes: 33_554_432,
                        max_issues: 256,
                        deadline,
                    },
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                check_growth_context_budget(
                    PreparedPgSourceRevision::context(&prepared),
                    MAX_SELECTED_MEMBERS as usize,
                    MAX_SELECTED_BYTES,
                    MAX_SELECTED_MEMBER_BYTES,
                )
                .map_err(DurableError::Source)?;
                Ok(prepared)
            },
        )
        .map_err(growth_source_error)?;
    let mut result = growth_value(&PreparedPgSourceRevision::proposal(&prepared).response)?;
    result["source_read_work"] = growth_source_read_work(&readset.work);
    let attempt = db
        .register_source_revision(
            &store,
            &prepared,
            &readset,
            &prepare_id,
            deadline,
            cancelled,
        )
        .map_err(growth_source_error)?;
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
                "PostgreSQL ObjectLink invocation changed before commit",
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
                "PostgreSQL ObjectLink owner configuration changed before commit",
            ));
        }
        current_fs
            .current_context(
                PreparedPgSourceRevision::context(&prepared),
                deadline,
                cancelled,
            )
            .map_err(DurableError::Source)?;
        if runtime
            .current_digest(deadline, cancelled)
            .map_err(DurableError::Source)?
            != expected_executable
        {
            return Err(DurableError::Conflict(
                "native ObjectLink executable changed before commit",
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
        .map_err(growth_source_error)?;
    result["replayed"] = json!(receipt.replayed);
    result["durable_commit"] = growth_commit_result(&receipt);
    Ok(json!({
        "schema_version":"tos_local_pg_growth_result_v1",
        "result":result,
        "grants_admission":false
    }))
}

fn run_pg_object_link_preview(
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    request_raw: &[u8],
    request: &JsonValue,
    uid: u32,
    selection: &GrowthSourceSelection,
    owner_path: &Path,
    owner_fs: &CreationFilesystem,
    owner_raw: &[u8],
    _owner_config: &JsonValue,
    expected_executable: Digest256,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<Value> {
    let operation = cmd::text(request, "operation")?;
    if operation == "describe" {
        cmd::exact_keys(request, &["schema_version", "operation"])?;
    } else if operation != "prepare-create" {
        return Err(SourceCommandError::Unsupported(
            "PostgreSQL ObjectLink preview operation",
        ));
    }
    let runtime = crate::source_serialization::observe_executable(deadline, cancelled)?;
    if runtime.current_digest(deadline, cancelled)? != expected_executable {
        return Err(SourceCommandError::Conflict(
            "native ObjectLink executable identity",
        ));
    }
    let store = SegmentStore::open_existing(&selection.store_root, selection.store_limits)
        .map_err(|_| SourceCommandError::Conflict("selected ObjectLink STO"))?;
    let (software, components) = growth_software_selection(invocation, deadline, cancelled)?;
    let mut db = DurablePgCoordinator::from_configured_client(
        crate::source_projection_cli::connect_selected_source_pg16()
            .map_err(|_| SourceCommandError::Conflict("selected PostgreSQL unavailable"))?,
    );
    let recorded_at = crate::source_serialization::instant()?;
    let ((preview, context, absences), readset) = db
        .with_source_revision_session(
            &store,
            &selection.domain,
            &selection.job_id,
            selection.job_fence,
            deadline,
            cancelled,
            |session| {
                if session.selection().contract != selection.contract {
                    return Err(DurableError::Conflict(
                        "selected ObjectLink source contract differs from current domain",
                    ));
                }
                let identity = session.selection().identity();
                let input = PgGrowthCurrentInput::new(
                    session,
                    4096,
                    MAX_GROWTH_SOFTWARE_MEMBER_BYTES as usize,
                )
                .map_err(|_| DurableError::Invalid("PG ObjectLink selected input budget"))?;
                let publication = crate::source_creation_store::work_transaction::PublicationSnapshot::select_from_input(
                    input.source_input(),
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                let mut selected = PgGrowthSelectedInputs::select(
                    &input,
                    publication.token.as_deref(),
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                if operation == "prepare-create" {
                    for path in OBJECT_LINK_CONTRACTS {
                        selected
                            .member(input.source_input(), path, deadline, cancelled)
                            .map_err(DurableError::Source)?;
                    }
                }
                let software_files =
                    selected_software_files(&software, &components, deadline, cancelled)
                        .map_err(DurableError::Source)?;
                let context = selected
                    .context(
                        owner_raw,
                        request_raw,
                        recorded_at.clone(),
                        uid,
                        &software_files,
                    )
                    .map_err(DurableError::Source)?;
                let mut preview = if operation == "describe" {
                    crate::source_creation_store::describe_pg_object_link(
                        owner_fs,
                        &context,
                        &input,
                        publication.token.as_deref(),
                        growth_item_limits(MAX_SELECTED_BYTES, MAX_SELECTED_MEMBER_BYTES, deadline),
                        cancelled,
                    )
                    .map_err(DurableError::Source)?
                } else {
                    let mut worker = super::revisions::selected_candidate_schema_from_context(
                        invocation,
                        &context.files,
                        identity,
                        deadline,
                        cancelled,
                    )
                    .map_err(DurableError::Source)?;
                    crate::source_creation_store::prepare_pg_object_link_preview(
                        owner_fs,
                        &context,
                        &input,
                        &mut worker,
                        &software,
                        &components,
                        publication.token.as_deref(),
                        growth_item_limits(
                            MAX_SELECTED_BYTES,
                            MAX_SELECTED_MEMBER_BYTES,
                            deadline,
                        ),
                        cancelled,
                    )
                    .map_err(DurableError::Source)?
                };
                drop(input);
                selected
                    .bind_selected_readset(session, deadline, cancelled)
                    .map_err(|_| {
                        DurableError::Conflict("ObjectLink preview selected read epoch")
                    })?;
                if operation == "prepare-create" {
                    preview
                        .observe_requirements(session, deadline, cancelled)
                        .map_err(|_| {
                            DurableError::Conflict("ObjectLink preview absence observations")
                        })?;
                }
                let (context, absences) = context_with_pg_exact_reads(
                    &context,
                    session,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)?;
                check_growth_context_budget(
                    &context,
                    MAX_SELECTED_MEMBERS as usize,
                    MAX_SELECTED_BYTES,
                    MAX_SELECTED_MEMBER_BYTES,
                )
                .map_err(DurableError::Source)?;
                preview
                    .bind_read_snapshot(session.reads_snapshot())
                    .map_err(|_| DurableError::Conflict("ObjectLink preview read snapshot"))?;
                Ok((preview, context, absences))
            },
        )
        .map_err(growth_source_error)?;
    preview
        .validate_readset(&readset)
        .map_err(|_| SourceCommandError::Conflict("ObjectLink preview readset changed"))?;
    let mut result = growth_value(preview.response())?;
    result["source_read_work"] = growth_source_read_work(&readset.work);
    if super::read_absolute(
        invocation_path,
        uid,
        true,
        invocation_raw.len(),
        deadline,
        cancelled,
    )? != invocation_raw
    {
        return Err(SourceCommandError::Conflict(
            "PG ObjectLink invocation changed before preview",
        ));
    }
    let (current_fs, current_raw) = CreationFilesystem::select_protected_native_owner_at_root(
        owner_path,
        &selection.store_root,
        deadline,
        cancelled,
    )?;
    if current_raw != owner_raw {
        return Err(SourceCommandError::Conflict(
            "PG ObjectLink owner configuration changed before preview",
        ));
    }
    current_fs.current_context(&context, deadline, cancelled)?;
    verify_work_absences(&current_fs, &absences, deadline, cancelled)?;
    if runtime.current_digest(deadline, cancelled)? != expected_executable {
        return Err(SourceCommandError::Conflict(
            "native ObjectLink executable changed before preview",
        ));
    }
    Ok(json!({
        "schema_version":"tos_local_pg_growth_result_v1",
        "result":result,
        "grants_admission":false
    }))
}
