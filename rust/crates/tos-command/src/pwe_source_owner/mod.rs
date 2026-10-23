//! Native, held-source PWE host. Module/export/call-site registration is owned
//! by the common integrator. These types are crate-private, never request data.
mod basis;
mod carrier;
mod frames;
mod owner;
mod registry;
mod producer;
mod reader;
mod roles;

pub(crate) use basis::NativeBasis;
pub(crate) use producer::NativePreservedCandidate;
pub(crate) use carrier::{PositiveCarrier, SelectedCarrier,SelectedGroup};
pub(crate) use frames::HeldFrames;
pub(crate) use owner::{NativeAccount,NativeConsumerOwner};
pub(crate) use registry::{BindingRegistry, DispatchSpec};
pub(crate) use reader::{ClaimDependencies,NativeExactSlot,NativeUnresolvedSlot,extract_claim_dependencies,check_description_revision,check_independent_correction};
pub(crate) use roles::{ExactDependencyReader, ResolvedDependency, RoleCache};

use crate::source_command::{SourceCommandError, SourceCommandResult};
use serde_json::Value;
use tos_compiler::pwe_native::Error;
use tos_foundation::{CanonicalProfile, Digest256, JsonLimits, JsonValue, canonical_bytes_v1};

const MAX_BYTES: usize = 1_048_576;
const MAX_RECORDS: usize = 4096;

fn refuse(e: SourceCommandError) -> Error {
    match e {
        SourceCommandError::Denied(_) | SourceCommandError::DeniedWithReason(_) => Error::Denied,
        SourceCommandError::Conflict(_) => Error::ExactMismatch,
        SourceCommandError::Unsupported(_) => Error::Upstream("native source/schema/resource operation incomplete"),
        _ => Error::Upstream("native owner source operation"),
    }
}
fn invalid(reason: &'static str) -> SourceCommandError { SourceCommandError::Invalid(reason) }
fn unsupported(reason: &'static str) -> SourceCommandError { SourceCommandError::Unsupported(reason) }
fn exact_match(value: &Value, actual: &JsonValue, id_field: &str, version_field: &str) -> SourceCommandResult<()> {
    let reference=value.as_object().filter(|m|m.len()==3).ok_or(invalid("PWE exact ref shape"))?;
    let id=reference.get("id").and_then(Value::as_str).filter(|id|!id.is_empty()&&!id.chars().any(char::is_control)).ok_or(invalid("PWE exact ID"))?;
    let version=reference.get("version").and_then(Value::as_u64).filter(|v|*v>0).ok_or(invalid("PWE exact version"))?;
    let digest=reference.get("digest").and_then(Value::as_str).and_then(|v|Digest256::from_prefixed(v).ok()).ok_or(invalid("PWE exact digest"))?;
    if actual.object_get(id_field).and_then(JsonValue::as_str)!=Some(id)
        || actual.object_get(version_field).and_then(JsonValue::as_u64)!=Some(version)
        || Digest256::of_bytes(&canonical(actual)?)!=digest {
        return Err(SourceCommandError::Conflict("PWE selected exact record differs"));
    } Ok(())
}
fn canonical(value: &JsonValue) -> SourceCommandResult<Vec<u8>> {
    canonical_bytes_v1(value,CanonicalProfile::SourceRecordDigestV1,JsonLimits::new(MAX_BYTES,96,1_000_000,4096))
        .map_err(|_|invalid("PWE canonical source Record"))
}
fn encoded_len(value:&Value)->SourceCommandResult<usize> {
    struct Counter(usize);
    impl std::io::Write for Counter {
        fn write(&mut self,bytes:&[u8])->std::io::Result<usize> {
            self.0=self.0.checked_add(bytes.len()).filter(|n|*n<=MAX_BYTES)
                .ok_or_else(||std::io::Error::other("PWE encoded byte cap"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self)->std::io::Result<()>{Ok(())}
    }
    let mut counter=Counter(0);
    serde_json::to_writer(&mut counter,value).map_err(|_|unsupported("PWE encoded value byte cap"))?;
    Ok(counter.0)
}
