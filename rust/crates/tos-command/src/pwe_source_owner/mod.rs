//! Native, held-source PWE host. Module/export/call-site registration is owned
//! by the common integrator. These types are crate-private, never request data.
mod access;
mod basis;
mod budget;
mod carrier;
mod frames;
mod owner;
mod producer;
mod reader;
mod registry;
mod roles;
mod source_budget;

pub(crate) use basis::NativeBasis;
pub(crate) use carrier::{PositiveCarrier, SelectedCarrier, SelectedGroup};
pub(crate) use frames::{HeldFrames, HeldPweEvaluation, HeldEvaluationPosition};
pub(crate) use owner::{NativeAccount, NativeConsumerOwner, NativeAssessmentHost, ApplicationCheckedTicket, CheckedPweParent};
pub(crate) use producer::NativePreservedCandidate;
pub(crate) use reader::{
    ClaimDependencies, NativeExactSlot, NativeUnresolvedSlot, check_description_revision,
    check_description_revision_source, check_independent_correction,
    check_independent_correction_source, extract_claim_dependencies,
    extract_claim_dependencies_source,
};
pub(crate) use registry::{BindingRegistry, DispatchSpec};
pub(crate) use roles::{ExactDependencyReader, ResolvedDependency, RoleCache};
pub(crate) use source_budget::{SourcePayer, source_canonical, assessment_canonical, assessment_domain_value};

use crate::source_command::{SourceCommandError, SourceCommandResult};
use access::BorrowedAccess;
use budget::CanonicalLedger;
use serde_json::Value;
use tos_compiler::pwe_native::Error;
use tos_foundation::JsonValue;

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
    let reference = ledger.exact(value)?;
    let actual_id = ledger
        .native_get(actual, id_field)?
        .and_then(JsonValue::as_str);
    let names = match actual_id {
        Some(id) => ledger.text_equal(id, reference.id.as_str())?,
        None => false,
    };
    let version = ledger
        .native_get(actual, version_field)?
        .and_then(JsonValue::as_u64);
    if !names || version != Some(reference.version.get()) || actual_digest != reference.digest {
        return Err(SourceCommandError::Conflict(
            "PWE selected exact record differs",
        ));
    }
    Ok(())
}
