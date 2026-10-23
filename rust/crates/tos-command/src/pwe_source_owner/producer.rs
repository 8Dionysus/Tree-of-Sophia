//! Native output-profile adapter. Whole Record -> /body container -> each item
//! are separate checks. Completed production never executes semantic resolution.
use super::{HeldFrames, ResolvedDependency, invalid, unsupported};
use crate::source_command::SourceCommandResult;
use serde_json::Value;
use tos_compiler::pwe_native::{PreparedAccount, Session};
use tos_foundation::{
    ExecutionStatus, JsonValue, OperationBindingLimits, ValueDisposition, check_operation_binding,
};

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
    pub(crate) fn body<'v>(
        &'v self,
        frames: &mut HeldFrames<'_, '_>,
    ) -> SourceCommandResult<&'v Value> {
        frames.field(&self.record, "body")
    }
    pub(crate) fn semantic_resolution_executed(&self) -> bool {
        false
    }
    pub(crate) fn values<'v>(
        &'v self,
        frames: &mut HeldFrames<'_, '_>,
    ) -> SourceCommandResult<&'v [Value]> {
        let body = self.body(frames)?;
        frames
            .field(body, "values")?
            .as_array()
            .map(Vec::as_slice)
            .ok_or(invalid("PWE projected items"))
    }
    pub(crate) fn item<'v>(
        &'v self,
        index: usize,
        session: &mut Session<'_>,
        frames: &mut HeldFrames<'_, '_>,
    ) -> tos_compiler::pwe_native::Result<PreparedAccount<'v>> {
        let values = self.values(frames).map_err(super::refuse)?;
        frames.inspect_access(0, 1).map_err(super::refuse)?;
        session.prepare(
            values
                .get(index)
                .ok_or(tos_compiler::pwe_native::Error::Malformed(
                    "PWE output item index",
                ))?,
        )
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
        if !frames.text_equal(profile.identity_field, "record_id")?
            || !frames.text_equal(profile.version_field, "record_version")?
            || !frames.text_equal(output.identity_field, "record_id")?
            || !frames.text_equal(output.version_field, "record_version")?
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
        let body = frames.native_field(&output.document, "body")?;
        frames.check_schema(&output.source_path, body, SCHEMAS[1])?;
        let profile_body = frames.native_field(&profile.document, "body")?;
        let bindings = frames.native_array(profile_body, "schema_bindings")?;
        if bindings.len() != 3 {
            return Err(invalid("PWE producer wrapper/container/item binding count"));
        }
        for (binding, path) in bindings.iter().zip(SCHEMAS) {
            let selected = frames
                .contract_digest(path)?
                .ok_or(unsupported("PWE output schema not selected"))?;
            let actual_path = frames
                .native_get(binding, "path")?
                .and_then(JsonValue::as_str);
            let actual_digest = frames
                .native_get(binding, "raw_sha256")?
                .and_then(JsonValue::as_str);
            let digest = selected.to_prefixed();
            if !match_text(frames, actual_path, path)?
                || !match_text(frames, actual_digest, &digest)?
            {
                return Err(crate::source_command::SourceCommandError::Conflict(
                    "PWE output profile schema bytes differ from held worker",
                ));
            }
        }
        let values = frames.native_array(body, "values")?;
        for item in values {
            frames.check_schema(&output.source_path, item, SCHEMAS[2])?;
        }
        let record = frames.domain_value(&output.document)?;
        let projected = frames.field(&record, "body")?;
        let projected = frames
            .field(projected, "values")?
            .as_array()
            .ok_or(invalid("PWE projected items"))?;
        for item in projected {
            session
                .prepare(item)
                .map_err(|_| invalid("PWE individual preserved value"))?;
        }
        frames.verify()?;
        Ok(Self { record })
    }
}

fn match_text(
    frames: &mut HeldFrames<'_, '_>,
    actual: Option<&str>,
    expected: &str,
) -> SourceCommandResult<bool> {
    match actual {
        Some(actual) => frames.text_equal(actual, expected),
        None => Ok(false),
    }
}
