//! Protected native Install/Revoke route for source-navigation metadata policy.
//!
//! This is a local owner-process operation over an explicit final receipt in
//! the protected navigation owner configuration. Preparing a policy template
//! or a model receipt does not install current-use authority.

use super::{absolute, capped, digest, exact, text};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_creation_store::{CreationFilesystem, CreationOwnerFence};
use crate::source_navigation_cli::{
    NavigationPolicyOwnerViewV1, SelectedSourceNavigation, SourceNavigationSelectionError,
    with_selected_navigation_policy_owner,
};
use crate::source_navigation_policy::{
    SourceNavigationPolicyMutationV1, SourceNavigationPolicyOperationV1,
};
use crate::{DurableError, DurablePgCoordinator};
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::{Digest256, JsonValue};

const MAX_REQUEST: usize = 1_048_576;
const MAX_COMMAND_ID_BYTES: usize = 256;
const MAX_LOCK_TIMEOUT_MS: u64 = 5_000;

fn source_error(error: DurableError) -> SourceCommandError {
    match error {
        DurableError::Source(error) => error,
        DurableError::Conflict(reason) => SourceCommandError::Conflict(reason),
        DurableError::Refused(reason) => SourceCommandError::Denied(reason),
        DurableError::Invalid(_) | DurableError::Corrupt(_) => {
            SourceCommandError::Invalid("source-navigation policy state")
        }
        DurableError::Database(error)
            if error.code() == Some(&postgres::error::SqlState::LOCK_NOT_AVAILABLE) =>
        {
            SourceCommandError::Conflict("source-navigation policy state is locked")
        }
        DurableError::Database(_) | DurableError::Storage(_) | DurableError::Indeterminate(_) => {
            SourceCommandError::Conflict("selected PostgreSQL unavailable")
        }
    }
}

fn selection_error(error: SourceNavigationSelectionError) -> SourceCommandError {
    match error {
        SourceNavigationSelectionError::Invalid(reason) => SourceCommandError::Invalid(reason),
        SourceNavigationSelectionError::Managed(
            crate::source_managed_selection::ManagedSelectionError::Source(error),
        ) => error,
        SourceNavigationSelectionError::Managed(
            crate::source_managed_selection::ManagedSelectionError::Durable(error),
        ) => source_error(error),
        SourceNavigationSelectionError::Managed(_) => {
            SourceCommandError::Conflict("source-navigation policy owner unavailable")
        }
    }
}

fn expected_policy_version(request: &JsonValue) -> SourceCommandResult<Option<u64>> {
    match cmd::field(request, "expected_policy_version")? {
        JsonValue::Null => Ok(None),
        value => value
            .as_u64()
            .filter(|version| *version > 0 && *version <= i64::MAX as u64)
            .map(Some)
            .ok_or(SourceCommandError::Invalid(
                "source-navigation expected policy version",
            )),
    }
}

fn verify_current(
    invocation_path: &Path,
    invocation_raw: &[u8],
    invocation: &Value,
    filesystem: &CreationFilesystem,
    owner_fence: &CreationOwnerFence<'_>,
    view: &NavigationPolicyOwnerViewV1<'_>,
    private_root: &Path,
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
            "source-navigation policy invocation changed",
        ));
    }
    filesystem.verify_protected_configuration(deadline, cancelled)?;
    let owner = cmd::parse(filesystem.protected_configuration_raw())?;
    let current_root = absolute(cmd::text(&owner, "source_root")?)?;
    if current_root.as_path() != private_root || current_root.as_path() != view.source_root {
        return Err(SourceCommandError::Conflict(
            "source-navigation policy owner root changed",
        ));
    }
    cmd::validate_expiry(
        cmd::text(&owner, "expires_at")?,
        &crate::source_serialization::instant()?,
    )?;
    if cmd::record_digest(&owner)? != view.owner_configuration_sha256 {
        return Err(SourceCommandError::Conflict(
            "source-navigation policy owner configuration changed",
        ));
    }
    owner_fence.verify_current(deadline, cancelled)?;
    let selected_executable = digest(text(invocation, "native_executable_sha256")?)?;
    if runtime_observation.current_digest(deadline, cancelled)? != selected_executable {
        return Err(SourceCommandError::Conflict(
            "native command executable identity",
        ));
    }
    Ok(())
}

/// Execute one explicit policy Install or Revoke under the protected native
/// owner-process path. No model, producer, or PG read lease is opened here;
/// the policy coordinator takes its own single transaction and currentness
/// locks after the protected owner fence has been acquired.
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
    if text(invocation, "schema_version")? != "tos_local_pg_source_navigation_policy_invocation_v1"
    {
        return Err(SourceCommandError::Unsupported(
            "source-navigation policy invocation profile",
        ));
    }
    let owner_path = absolute(text(invocation, "owner_config")?)?;
    let private_root = absolute(text(invocation, "private_source_root")?)?;
    let _native_executable = absolute(text(invocation, "native_executable")?)?;
    let budgets = &invocation["budgets"];
    exact(budgets, &["lock_timeout_ms"])?;
    let lock_timeout_ms = capped(budgets, "lock_timeout_ms", MAX_LOCK_TIMEOUT_MS)?;

    let runtime_observation = crate::source_serialization::observe_executable(deadline, cancelled)?;
    let selected_executable = digest(text(invocation, "native_executable_sha256")?)?;
    if runtime_observation.current_digest(deadline, cancelled)? != selected_executable {
        return Err(SourceCommandError::Conflict(
            "native command executable identity",
        ));
    }

    let request_raw = super::bounded_read(input, MAX_REQUEST)?;
    let request = cmd::parse(&request_raw)?;
    cmd::exact_keys(
        &request,
        &[
            "schema_version",
            "operation",
            "command_id",
            "expected_configuration",
            "expected_policy_version",
        ],
    )?;
    if cmd::text(&request, "schema_version")? != "tos_local_source_navigation_policy_command_v1" {
        return Err(SourceCommandError::Unsupported(
            "source-navigation policy command profile",
        ));
    }
    let operation = match cmd::text(&request, "operation")? {
        "install" => SourceNavigationPolicyOperationV1::Install,
        "revoke" => SourceNavigationPolicyOperationV1::Revoke,
        _ => {
            return Err(SourceCommandError::Unsupported(
                "source-navigation policy operation",
            ));
        }
    };
    let command_id = cmd::text(&request, "command_id")?;
    if command_id.is_empty() || command_id.len() > MAX_COMMAND_ID_BYTES || command_id.contains('\0')
    {
        return Err(SourceCommandError::Invalid(
            "source-navigation policy command identity",
        ));
    }
    let expected_configuration =
        Digest256::from_prefixed(cmd::text(&request, "expected_configuration")?)
            .map_err(|_| SourceCommandError::Invalid("source-navigation owner digest"))?;
    let expected_policy_version = expected_policy_version(&request)?;
    match (operation, expected_policy_version) {
        (SourceNavigationPolicyOperationV1::Install, None)
        | (SourceNavigationPolicyOperationV1::Install, Some(_))
        | (SourceNavigationPolicyOperationV1::Revoke, Some(_)) => {}
        (SourceNavigationPolicyOperationV1::Revoke, None) => {
            return Err(SourceCommandError::Invalid(
                "source-navigation revoke requires an expected version",
            ));
        }
    }
    let request_sha256 = cmd::record_digest(&request)?;
    let selected = SelectedSourceNavigation::new(&owner_path)
        .map_err(|_| SourceCommandError::Invalid("source-navigation owner path"))?;

    with_selected_navigation_policy_owner(
        &selected,
        deadline,
        cancelled,
        |filesystem, owner_fence, view| {
            if view.source_root != private_root.as_path() {
                return Err(DurableError::Refused(
                    "source-navigation invocation root differs from protected owner",
                ));
            }
            if view.owner_configuration_sha256 != expected_configuration {
                return Err(DurableError::Conflict(
                    "source-navigation expected owner configuration changed",
                ));
            }
            if view.policy_receipt.selection_profile != view.profile_id {
                return Err(DurableError::Refused(
                    "source-navigation policy receipt profile differs",
                ));
            }
            verify_current(
                invocation_path,
                invocation_raw,
                invocation,
                filesystem,
                owner_fence,
                view,
                &private_root,
                uid,
                &runtime_observation,
                deadline,
                cancelled,
            )
            .map_err(DurableError::Source)?;

            let client = crate::source_projection_cli::connect_selected_source_pg16()
                .map_err(|_| DurableError::Conflict("selected PostgreSQL unavailable"))?;
            let mut client = client;
            client
                .batch_execute(&format!("SET lock_timeout='{lock_timeout_ms}ms'"))
                .map_err(|_| DurableError::Conflict("PostgreSQL lock budget refused"))?;
            let mut db = DurablePgCoordinator::from_configured_client(client);
            let mutation = SourceNavigationPolicyMutationV1 {
                domain: view.domain,
                consumer_id: view.consumer_id,
                selected_profile: view.profile_id,
                command_id,
                operation,
                expected_policy_version,
                source_contract_sha256: view.source_contract,
                expected_rule_version: view.rule_version,
                expected_rights_version: view.rights_version,
                job_id: view.job_id,
                job_fence: view.job_fence,
                bootstrap_source_revision: view.bootstrap_source_revision,
                source_epoch: view.source_epoch,
                source_definition_digest: view.source_definition_digest,
                bootstrap_membership_digest: view.bootstrap_membership_digest,
                bootstrap_membership_count: view.bootstrap_membership_count,
                domain_digest: view.domain_digest,
                database_oid: view.database_oid,
                anchor_through_sequence: view.anchor_through_sequence,
                request_sha256,
                owner_configuration_sha256: view.owner_configuration_sha256,
                receipt_sha256: view.policy_receipt_sha256,
                receipt: &view.policy_receipt,
            };
            let owner_check = || {
                verify_current(
                    invocation_path,
                    invocation_raw,
                    invocation,
                    filesystem,
                    owner_fence,
                    view,
                    &private_root,
                    uid,
                    &runtime_observation,
                    deadline,
                    cancelled,
                )
                .map_err(DurableError::Source)
            };
            let receipt = db.change_source_navigation_policy_v1(&mutation, owner_check)?;
            Ok(json!({
                "schema_version":"tos_local_pg_source_navigation_policy_result_v1",
                "operation":operation.as_str(),
                "owner_configuration_sha256":view.owner_configuration_sha256.to_prefixed(),
                "policy_receipt_sha256":view.policy_receipt_sha256.to_prefixed(),
                "request_sha256":request_sha256.to_prefixed(),
                "receipt":{
                    "command_id":command_id,
                    "policy_version":receipt.policy_version,
                    "withdrawal_generation":receipt.withdrawal_generation,
                    "allowed":receipt.allowed,
                    "replayed":receipt.replayed
                },
            "current_policy":{
                "policy_version":receipt.current_policy_version,
                "withdrawal_generation":receipt.current_withdrawal_generation,
                "allowed":receipt.current_allowed,
                "receipt_sha256":receipt.current_receipt_sha256.to_prefixed(),
                "owner_configuration_sha256":receipt.current_owner_configuration_sha256.to_prefixed(),
                "matches_selected_receipt_and_owner":receipt.current_receipt_sha256 == view.policy_receipt_sha256
                    && receipt.current_owner_configuration_sha256 == view.owner_configuration_sha256
            },
                "scope":"public metadata source-navigation current use only",
                "cmd_rights_changed":false,
                "source_admission":false
            }))
        },
    )
    .map_err(selection_error)
}
