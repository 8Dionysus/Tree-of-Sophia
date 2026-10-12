//! Protected owner command for CMD-local PostgreSQL rights revocation.
//!
//! This route changes only the selected private CMD domain's current-use bit.
//! It does not grant or assess ToS rights, publish source, or admit a source
//! change. The existing PG projection readers continue to enforce the current
//! rights version and refuse stale or revoked projections.

use super::{absolute, capped, digest, exact, number, text};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::CreationFilesystem;
use crate::{DurableError, DurablePgCoordinator};
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::Digest256;

const MAX_REQUEST: usize = 1_048_576;
const MAX_DOMAIN_BYTES: usize = 1_024;
const MAX_PRINCIPAL_BYTES: usize = 256;
const MAX_COMMAND_ID_BYTES: usize = 256;
const MAX_REASON_BYTES: usize = 512;
const MAX_LOCK_TIMEOUT_MS: u64 = 5_000;

struct SelectedOwner {
    domain: String,
    principal_id: String,
    configuration_digest: Digest256,
}

fn selected_owner(
    private_root: &Path,
    owner: &tos_foundation::JsonValue,
    uid: u32,
) -> SourceCommandResult<SelectedOwner> {
    cmd::exact_keys(
        owner,
        &[
            "schema_version",
            "uid",
            "source_root",
            "domain",
            "principal_id",
            "allowed_operations",
            "expires_at",
        ],
    )?;
    if cmd::text(owner, "schema_version")? != "tos_local_pg_rights_revoke_owner_v1"
        || cmd::integer(owner, "uid")? != u64::from(uid)
        || cmd::text(owner, "source_root")?
            != private_root
                .to_str()
                .ok_or(SourceCommandError::Invalid("CMD rights owner root UTF-8"))?
    {
        return Err(SourceCommandError::Denied("CMD local rights owner scope"));
    }
    let domain = cmd::text(owner, "domain")?;
    if domain.is_empty() || domain.len() > MAX_DOMAIN_BYTES || domain.contains('\0') {
        return Err(SourceCommandError::Invalid("CMD local rights domain"));
    }
    let principal_id = cmd::text(owner, "principal_id")?;
    if principal_id.is_empty()
        || principal_id.len() > MAX_PRINCIPAL_BYTES
        || principal_id.contains('\0')
    {
        return Err(SourceCommandError::Invalid("CMD local rights principal"));
    }
    let allowed = cmd::array(owner, "allowed_operations")?;
    if allowed.len() != 1 || allowed[0].as_str() != Some("rights.revoke") {
        return Err(SourceCommandError::Denied(
            "CMD local rights owner permits only revocation",
        ));
    }
    Ok(SelectedOwner {
        domain: domain.to_owned(),
        principal_id: principal_id.to_owned(),
        configuration_digest: cmd::record_digest(owner)?,
    })
}

fn request_bytes(input: impl Read) -> SourceCommandResult<Vec<u8>> {
    let mut raw = Vec::new();
    input
        .take((MAX_REQUEST + 1) as u64)
        .read_to_end(&mut raw)
        .map_err(|_| SourceCommandError::Invalid("CMD local rights request read"))?;
    if raw.len() > MAX_REQUEST {
        return Err(SourceCommandError::Invalid(
            "CMD local rights request budget",
        ));
    }
    Ok(raw)
}

fn source_error(error: DurableError) -> SourceCommandError {
    match error {
        DurableError::Source(error) => error,
        DurableError::Conflict(reason) => SourceCommandError::Conflict(reason),
        DurableError::Refused(reason) => SourceCommandError::Denied(reason),
        DurableError::Invalid(_) | DurableError::Corrupt(_) => {
            SourceCommandError::Invalid("CMD local rights metadata")
        }
        DurableError::Database(error)
            if error.code() == Some(&postgres::error::SqlState::LOCK_NOT_AVAILABLE) =>
        {
            SourceCommandError::Conflict("CMD local rights state is locked")
        }
        DurableError::Database(_) | DurableError::Storage(_) | DurableError::Indeterminate(_) => {
            SourceCommandError::Conflict("selected CMD PostgreSQL unavailable")
        }
    }
}

fn verify_current(
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    owner_fs: &CreationFilesystem,
    owner: &SelectedOwner,
    uid: u32,
    runtime_observation: &crate::source_serialization::ExecutableObservation,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> SourceCommandResult<()> {
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
            "CMD local rights invocation changed",
        ));
    }
    owner_fs.verify_protected_configuration(deadline, cancelled)?;
    let current_owner = cmd::parse(owner_fs.protected_configuration_raw())?;
    cmd::validate_expiry(
        cmd::text(&current_owner, "expires_at")?,
        &crate::source_serialization::instant()?,
    )?;
    let current_digest = runtime_observation.current_digest(deadline, cancelled)?;
    if current_digest != digest(text(invocation, "native_executable_sha256")?)? {
        return Err(SourceCommandError::Conflict(
            "native command executable identity",
        ));
    }
    if cmd::record_digest(&current_owner)? != owner.configuration_digest {
        return Err(SourceCommandError::Conflict(
            "CMD local rights owner configuration changed",
        ));
    }
    Ok(())
}

/// Execute one protected native CMD-local rights operation.
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
            "private_source_root",
            "native_executable",
            "native_executable_sha256",
            "budgets",
        ],
    )?;
    if text(invocation, "schema_version")? != "tos_local_pg_rights_revoke_invocation_v1" {
        return Err(SourceCommandError::Unsupported(
            "CMD local rights invocation profile",
        ));
    }
    let owner_path = absolute(text(invocation, "owner_config")?)?;
    let private_root = absolute(text(invocation, "private_source_root")?)?;
    let _ = absolute(text(invocation, "native_executable")?)?;
    let budgets = &invocation["budgets"];
    exact(budgets, &["lock_timeout_ms"])?;
    let lock_timeout_ms = capped(budgets, "lock_timeout_ms", MAX_LOCK_TIMEOUT_MS)?;

    let runtime_observation = crate::source_serialization::observe_executable(deadline, cancelled)?;
    if runtime_observation.current_digest(deadline, cancelled)?
        != digest(text(invocation, "native_executable_sha256")?)?
    {
        return Err(SourceCommandError::Conflict(
            "native command executable identity",
        ));
    }
    let (owner_fs, owner_raw) = CreationFilesystem::select_protected_native_owner_at_root(
        &owner_path,
        &private_root,
        deadline,
        cancelled,
    )?;
    let owner_config = cmd::parse(&owner_raw)?;
    let owner = selected_owner(&private_root, &owner_config, uid)?;
    let request_raw = request_bytes(input)?;
    let request = cmd::parse(&request_raw)?;
    if cmd::text(&request, "schema_version")? != "tos_local_source_command_v1" {
        return Err(SourceCommandError::Unsupported(
            "CMD local rights request profile",
        ));
    }
    let operation = cmd::text(&request, "operation")?;
    let (required, is_revoke) = match operation {
        "describe" => (
            &["schema_version", "operation", "expected_configuration"][..],
            false,
        ),
        "rights.revoke" => (
            &[
                "schema_version",
                "operation",
                "command_id",
                "expected_configuration",
                "expected_rights_version",
                "reason",
            ][..],
            true,
        ),
        _ => {
            return Err(SourceCommandError::Unsupported(
                "CMD local rights operation",
            ));
        }
    };
    cmd::exact_keys(&request, required)?;
    let expected_configuration =
        Digest256::from_prefixed(cmd::text(&request, "expected_configuration")?)
            .map_err(|_| SourceCommandError::Invalid("CMD local rights configuration digest"))?;
    if expected_configuration != owner.configuration_digest {
        return Err(SourceCommandError::Conflict(
            "CMD local rights owner configuration changed",
        ));
    }

    let mut client = crate::source_projection_cli::connect_selected_source_pg16()
        .map_err(|_| SourceCommandError::Conflict("selected PostgreSQL unavailable"))?;
    client
        .batch_execute(&format!("SET lock_timeout='{lock_timeout_ms}ms'"))
        .map_err(|_| SourceCommandError::Conflict("PostgreSQL lock budget refused"))?;
    let mut db = DurablePgCoordinator::from_configured_client(client);
    if !is_revoke {
        verify_current(
            invocation_path,
            invocation_raw,
            invocation,
            &owner_fs,
            &owner,
            uid,
            &runtime_observation,
            deadline,
            cancelled,
        )?;
        let snapshot = db
            .local_rights_snapshot(&owner.domain)
            .map_err(source_error)?;
        verify_current(
            invocation_path,
            invocation_raw,
            invocation,
            &owner_fs,
            &owner,
            uid,
            &runtime_observation,
            deadline,
            cancelled,
        )?;
        return Ok(json!({
            "schema_version":"tos_local_pg_rights_revoke_result_v1",
            "operation":"describe",
            "owner_configuration":owner.configuration_digest.to_prefixed(),
            "principal_id":owner.principal_id,
            "scope":"private CMD domain local current-use rights",
            "rights":{
                "head_seq":snapshot.head_seq,
                "rights_version":snapshot.rights_version,
                "rights_allowed":snapshot.rights_allowed
            },
            "grants_admission":false
        }));
    }

    let command_id = cmd::text(&request, "command_id")?;
    if command_id.is_empty() || command_id.len() > MAX_COMMAND_ID_BYTES || command_id.contains('\0')
    {
        return Err(SourceCommandError::Invalid(
            "CMD local rights command identity",
        ));
    }
    let reason = cmd::text(&request, "reason")?;
    if reason.trim().is_empty() || reason.len() > MAX_REASON_BYTES || reason.contains('\0') {
        return Err(SourceCommandError::Invalid("CMD local rights reason"));
    }
    let expected_rights_version = cmd::integer(&request, "expected_rights_version")?;
    if expected_rights_version > i64::MAX as u64 {
        return Err(SourceCommandError::Invalid("CMD local rights version"));
    }
    verify_current(
        invocation_path,
        invocation_raw,
        invocation,
        &owner_fs,
        &owner,
        uid,
        &runtime_observation,
        deadline,
        cancelled,
    )?;
    let request_digest = cmd::record_digest(&request)?;
    let owner_check = || {
        verify_current(
            invocation_path,
            invocation_raw,
            invocation,
            &owner_fs,
            &owner,
            uid,
            &runtime_observation,
            deadline,
            cancelled,
        )
        .map_err(DurableError::Source)
    };
    let receipt = db
        .revoke_local_command(
            &owner.domain,
            command_id,
            expected_rights_version,
            request_digest,
            owner_check,
        )
        .map_err(source_error)?;
    let snapshot = db
        .local_rights_snapshot(&owner.domain)
        .map_err(source_error)?;
    Ok(json!({
        "schema_version":"tos_local_pg_rights_revoke_result_v1",
        "operation":"rights.revoke",
        "owner_configuration":owner.configuration_digest.to_prefixed(),
        "principal_id":owner.principal_id,
        "receipt":{
            "command_id":command_id,
            "commit_seq":receipt.commit_seq,
            "rights_version":receipt.rights_version,
            "replayed":receipt.replayed,
            "request_sha256":request_digest.to_prefixed()
        },
        "current_rights":{
            "head_seq":snapshot.head_seq,
            "rights_version":snapshot.rights_version,
            "rights_allowed":snapshot.rights_allowed
        },
        "scope":"private CMD domain local current-use rights",
        "grants_admission":false
    }))
}
