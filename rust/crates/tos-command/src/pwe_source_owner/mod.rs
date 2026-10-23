//! Native, held-source PWE host. Module/export/call-site registration is owned
//! by the common integrator. These types are crate-private, never request data.
mod basis;
mod budget;
mod carrier;
mod frames;
mod owner;
mod producer;
mod reader;
mod registry;
mod roles;

pub(crate) use basis::NativeBasis;
pub(crate) use carrier::{PositiveCarrier, SelectedCarrier, SelectedGroup};
pub(crate) use frames::HeldFrames;
pub(crate) use owner::{NativeAccount, NativeConsumerOwner};
pub(crate) use producer::NativePreservedCandidate;
pub(crate) use reader::{
    ClaimDependencies, NativeExactSlot, NativeUnresolvedSlot, check_description_revision,
    check_independent_correction, extract_claim_dependencies,
};
pub(crate) use registry::{BindingRegistry, DispatchSpec};
pub(crate) use roles::{ExactDependencyReader, ResolvedDependency, RoleCache};

use crate::source_command::{SourceCommandError, SourceCommandResult};
use budget::CanonicalLedger;
use serde_json::Value;
use tos_compiler::pwe_native::Error;
use tos_foundation::{Digest256, JsonValue};

const MAX_BYTES: usize = 1_048_576;
const MAX_RECORDS: usize = 4096;

fn refuse(e: SourceCommandError) -> Error {
    match e {
        SourceCommandError::Denied(_) | SourceCommandError::DeniedWithReason(_) => Error::Denied,
        SourceCommandError::Conflict(_) => Error::ExactMismatch,
        SourceCommandError::Unsupported(_) => {
            Error::Upstream("native source/schema/resource operation incomplete")
        }
        _ => Error::Upstream("native owner source operation"),
    }
}
fn invalid(reason: &'static str) -> SourceCommandError {
    SourceCommandError::Invalid(reason)
}
fn unsupported(reason: &'static str) -> SourceCommandError {
    SourceCommandError::Unsupported(reason)
}
fn exact_match(
    value: &Value,
    actual: &JsonValue,
    id_field: &str,
    version_field: &str,
    ledger: &mut CanonicalLedger<'_>,
) -> SourceCommandResult<()> {
    let actual_digest = ledger.digest(actual)?;
    let reference = value
        .as_object()
        .filter(|m| m.len() == 3)
        .ok_or(invalid("PWE exact ref shape"))?;
    let id = reference
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty() && !id.chars().any(char::is_control))
        .ok_or(invalid("PWE exact ID"))?;
    let version = reference
        .get("version")
        .and_then(Value::as_u64)
        .filter(|v| *v > 0)
        .ok_or(invalid("PWE exact version"))?;
    let digest = reference
        .get("digest")
        .and_then(Value::as_str)
        .and_then(|v| Digest256::from_prefixed(v).ok())
        .ok_or(invalid("PWE exact digest"))?;
    if actual.object_get(id_field).and_then(JsonValue::as_str) != Some(id)
        || actual.object_get(version_field).and_then(JsonValue::as_u64) != Some(version)
        || actual_digest != digest
    {
        return Err(SourceCommandError::Conflict(
            "PWE selected exact record differs",
        ));
    }
    Ok(())
}
