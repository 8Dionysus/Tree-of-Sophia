//! Native output-profile adapter. Whole Record -> /body container -> each item
//! are separate checks. Completed production never executes semantic resolution.
use super::{HeldFrames, ResolvedDependency, invalid, unsupported};
use crate::source_command::SourceCommandResult;
use serde_json::Value;
use tos_compiler::pwe_native::{PreparedAccount, Session};
use tos_foundation::{
    ExecutionStatus, JsonValue, OperationBindingLimits, ValueDisposition, check_operation_binding,
};
use tos_validation::source_cut::CutSchemaExecutor;

const PROFILE: &str = "ToS/contracts/pwe-prepared-values-output-profile.schema.json";
const SCHEMAS: [&str; 3] = [
    "ToS/contracts/pwe-prepared-values-record.schema.json",
    "ToS/contracts/pwe-preserved-candidate.schema.json",
    "ToS/contracts/pwe-values.schema.json",
];
pub(crate) struct NativePreservedCandidate {
    record: Value,
}
impl NativePreservedCandidate {
    pub(crate) fn record(&self) -> &Value {
        &self.record
    }
    pub(crate) fn body(&self) -> &Value {
        &self.record["body"]
    }
    pub(crate) fn semantic_resolution_executed(&self) -> bool {
        false
    }
    pub(crate) fn values(&self) -> &[Value] {
        self.record["body"]["values"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
    pub(crate) fn item<'v>(
        &'v self,
        index: usize,
        session: &mut Session<'_>,
    ) -> tos_compiler::pwe_native::Result<PreparedAccount<'v>> {
        session.prepare(self.values().get(index).ok_or(
            tos_compiler::pwe_native::Error::Malformed("PWE output item index"),
        )?)
    }
    /// Inputs are exact records returned by the maintained owner. Their source
    /// closure/schema/currentness is rechecked here; transport is not authority.
    pub(crate) fn check(
        frames: &mut HeldFrames<'_, '_>,
        session: &mut Session<'_>,
        envelope: &JsonValue,
        limits: OperationBindingLimits,
        profile: &ResolvedDependency,
        output: &ResolvedDependency,
    ) -> SourceCommandResult<Self> {
        frames.verify()?;
        if profile.identity_field != "record_id"
            || profile.version_field != "record_version"
            || output.identity_field != "record_id"
            || output.version_field != "record_version"
        {
            return Err(invalid(
                "PWE native producer requires whole Record identity",
            ));
        }
        // K0 has no exported usage counters. Prepay its entire allowance on the
        // existing ledgers, rather than introducing a reset callback budget.
        frames.budget.charge_work(limits.max_nodes)?;
        frames.budget.charge_bytes(limits.max_envelope_bytes)?;
        frames
            .whole
            .try_borrow()
            .map_err(|_| invalid("PWE whole-call budget in use"))?
            .check_live(limits.max_envelope_bytes)?;
        let checked = check_operation_binding(envelope, limits)
            .map_err(|_| invalid("PWE K0 producer envelope"))?;
        if checked.execution_status() != ExecutionStatus::Completed
            || checked.value_disposition() != ValueDisposition::Produced
        {
            return Err(unsupported(
                "PWE preservation producer is not Completed/Produced",
            ));
        }
        let pr = frames.domain_value(
            checked
                .output_profile_ref()
                .ok_or(invalid("PWE output profile ref absent"))?,
        )?;
        let or = frames.domain_value(
            checked
                .output_ref()
                .ok_or(invalid("PWE output ref absent"))?,
        )?;
        profile.verify(&pr, frames)?;
        output.verify(&or, frames)?;
        frames.exact_match(&pr, &profile.document, "record_id", "record_version")?;
        frames.exact_match(&or, &output.document, "record_id", "record_version")?;
        frames.check_schema(&profile.source_path, &profile.document, PROFILE)?;
        frames.check_schema(&output.source_path, &output.document, SCHEMAS[0])?;
        let body = output
            .document
            .object_get("body")
            .ok_or(invalid("PWE native container absent"))?;
        frames.check_schema(&output.source_path, body, SCHEMAS[1])?;
        let profile_body = profile
            .document
            .object_get("body")
            .ok_or(invalid("PWE native profile body absent"))?;
        let bindings = profile_body
            .object_get("schema_bindings")
            .and_then(JsonValue::as_array)
            .ok_or(invalid("PWE producer schema bindings"))?;
        if bindings.len() != 3 {
            return Err(invalid("PWE producer wrapper/container/item binding count"));
        }
        for (binding, path) in bindings.iter().zip(SCHEMAS) {
            let selected = frames
                .worker
                .selected_contract_digest(path)
                .ok_or(unsupported("PWE output schema not selected"))?;
            if binding.object_get("path").and_then(JsonValue::as_str) != Some(path)
                || binding.object_get("raw_sha256").and_then(JsonValue::as_str)
                    != Some(selected.to_prefixed().as_str())
            {
                return Err(crate::source_command::SourceCommandError::Conflict(
                    "PWE output profile schema bytes differ from held worker",
                ));
            }
        }
        let values = body
            .object_get("values")
            .and_then(JsonValue::as_array)
            .ok_or(invalid("PWE native container values"))?;
        for item in values {
            frames.check_schema(&output.source_path, item, SCHEMAS[2])?;
        }
        let record = frames.domain_value(&output.document)?;
        for item in record["body"]["values"]
            .as_array()
            .ok_or(invalid("PWE projected items"))?
        {
            session
                .prepare(item)
                .map_err(|_| invalid("PWE individual preserved value"))?;
        }
        frames.verify()?;
        Ok(Self { record })
    }
}
