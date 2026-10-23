//! Positive source-account carrier over explicit joint/alternative groups.
//! Scenario/carrier IDs come from actual native Records, never generated from
//! an unnamed group/index. This profile issues no historical or universal fact.
use super::owner::NativeAccount;
use super::{
    ExactDependencyReader, MAX_RECORDS, NativeConsumerOwner, invalid, refuse, unsupported,
};
use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_native_roles::NativeRole as Role;
use serde_json::Value;
use tos_compiler::pwe_native::{JointCarrier, JointCarrierOwner, JointWorld};
use tos_foundation::{CutPreparedSchemaExecutionBinding, SourceRevision};

const INTERPRETATION: &str = "positive-source-account-groups-v1";
pub(crate) struct SelectedGroup {
    reference: Value,
    positive: Vec<Value>,
}
impl SelectedGroup {
    pub(crate) fn world(&self) -> JointWorld<'_> {
        JointWorld {
            world_ref: &self.reference,
            positive: &self.positive,
            negative: &[],
        }
    }
}
pub(crate) struct SelectedCarrier {
    reference: Value,
    context: Value,
    scope: Value,
    groups: Vec<SelectedGroup>,
    revision: SourceRevision,
    worker: CutPreparedSchemaExecutionBinding,
}
impl SelectedCarrier {
    pub(crate) fn carrier(&self) -> JointCarrier<'_> {
        JointCarrier {
            reference: &self.reference,
            context: &self.context,
            scope: &self.scope,
        }
    }
    pub(crate) fn groups(&self) -> &[SelectedGroup] {
        &self.groups
    }
}
pub(crate) struct PositiveCarrier<'h, 'a, 'scope, R: ExactDependencyReader> {
    host: &'h mut NativeConsumerOwner<'a, 'scope, R>,
    selection: &'h SelectedCarrier,
}
impl<'h, 'a, 'scope, R: ExactDependencyReader> PositiveCarrier<'h, 'a, 'scope, R> {
    /// Select from the explicit source-owned carrier/scenario Records and an
    /// actual protected source Claim. Observed scenario list is not coverage.
    pub(crate) fn select(
        host: &mut NativeConsumerOwner<'a, 'scope, R>,
        carrier_ref: &Value,
        scenario_refs: &[Value],
        account: &NativeAccount,
    ) -> SourceCommandResult<SelectedCarrier> {
        host.admitted_account(account)?;
        let claim = account.claim();
        let object = account.object();
        if claim.get("polarity").and_then(Value::as_str) != Some("positive")
            || object.get("kind").and_then(Value::as_str) != Some("responsibility-account")
            || object.get("attribution_mode").and_then(Value::as_str) != Some("source-attribution")
            || object
                .pointer("/qualification/modality")
                .and_then(Value::as_str)
                != Some("source-reported")
        {
            return Err(unsupported(
                "PWE positive source-account carrier needs explicit source attribution/sign",
            ));
        }
        let carrier = host.role_value(carrier_ref, Role::JointCarrier)?;
        if carrier.get("schema_version").and_then(Value::as_str)
            != Some("tos_pwe_source_account_carrier_v1")
        {
            return Err(unsupported("PWE carrier actual profile differs"));
        }
        let body = carrier
            .get("body")
            .ok_or(invalid("PWE native carrier body"))?;
        if body.get("interpretation").and_then(Value::as_str) != Some(INTERPRETATION)
            || body.get("claim_ref") != Some(account.reference())
        {
            return Err(SourceCommandError::Conflict(
                "PWE carrier selected Claim/interpretation differs",
            ));
        }
        let context = object
            .pointer("/qualification/context_ref/value")
            .ok_or(unsupported("PWE source-account context unresolved"))?;
        let scope = object
            .pointer("/qualification/scope_ref/value")
            .ok_or(unsupported("PWE source-account scope unresolved"))?;
        if body.get("context_ref") != Some(context) || body.get("scope_ref") != Some(scope) {
            return Err(SourceCommandError::Conflict(
                "PWE carrier content scope/context differs",
            ));
        }
        let declared = body
            .get("scenario_refs")
            .and_then(Value::as_array)
            .ok_or(invalid("PWE actual carrier scenarios"))?;
        if scenario_refs.len() > MAX_RECORDS || declared.len() > MAX_RECORDS {
            return Err(unsupported("PWE carrier scenario count cap"));
        }
        let mut groups = Vec::new();
        host.frames.retain(
            scenario_refs
                .len()
                .checked_mul(std::mem::size_of::<SelectedGroup>())
                .ok_or(unsupported("PWE group reservation overflow"))?,
        )?;
        groups
            .try_reserve_exact(scenario_refs.len())
            .map_err(|_| unsupported("PWE group reservation"))?;
        for (i, reference) in scenario_refs.iter().enumerate() {
            if scenario_refs[..i].contains(reference) || !declared.contains(reference) {
                return Err(SourceCommandError::Conflict(
                    "PWE scenario not uniquely selected from actual carrier",
                ));
            }
            let scenario = host.role_value(reference, Role::Scenario)?;
            if scenario.get("schema_version").and_then(Value::as_str)
                != Some("tos_pwe_source_account_scenario_v1")
            {
                return Err(unsupported("PWE group scenario actual profile differs"));
            }
            let scenario = scenario
                .get("body")
                .ok_or(invalid("PWE native scenario body"))?;
            if scenario.get("interpretation").and_then(Value::as_str) != Some(INTERPRETATION)
                || scenario.get("carrier_ref") != Some(carrier_ref)
                || scenario.get("claim_ref") != Some(account.reference())
                || scenario.get("context_ref") != Some(context)
                || scenario.get("scope_ref") != Some(scope)
            {
                return Err(SourceCommandError::Conflict(
                    "PWE scenario source/carrier/scope/context differs",
                ));
            }
            let pointer = scenario
                .pointer("/group_selector/pointer")
                .and_then(Value::as_str)
                .ok_or(invalid("PWE explicit source group selector"))?;
            if scenario
                .pointer("/group_selector/syntax")
                .and_then(Value::as_str)
                != Some("json-pointer")
            {
                return Err(invalid("PWE source group selector syntax"));
            }
            let mode = object.pointer("/assignees/mode").and_then(Value::as_str);
            let permitted = if pointer == "/object/assignees/bearers" {
                mode == Some("joint")
            } else if let Some(index) = pointer.strip_prefix("/object/assignees/groups/") {
                mode == Some("alternatives")
                    && !index.is_empty()
                    && (index == "0" || !index.starts_with('0'))
                    && index.bytes().all(|b| b.is_ascii_digit())
                    && index.parse::<usize>().is_ok()
            } else {
                false
            };
            if !permitted {
                return Err(unsupported(
                    "PWE source contract does not declare this explicit joint group",
                ));
            }
            let positive = claim
                .pointer(pointer)
                .and_then(Value::as_array)
                .ok_or(invalid("PWE actual source group absent"))?;
            if positive.is_empty() || positive.len() > 128 {
                return Err(invalid("PWE source group cardinality"));
            }
            let group_value = claim.pointer(pointer).ok_or(invalid("PWE group absent"))?;
            let holder = super::encoded_len(group_value)?
                .checked_mul(64)
                .and_then(|n| n.checked_add(1024))
                .ok_or(unsupported("PWE group holder overflow"))?;
            host.frames.retain(holder)?;
            groups.push(SelectedGroup {
                reference: reference.clone(),
                positive: positive.clone(),
            });
        }
        host.frames.verify()?;
        Ok(SelectedCarrier {
            reference: carrier_ref.clone(),
            context: context.clone(),
            scope: scope.clone(),
            groups,
            revision: host.frames.revision(),
            worker: host.frames.worker.prepared_execution_binding(),
        })
    }
    pub(crate) fn held(
        host: &'h mut NativeConsumerOwner<'a, 'scope, R>,
        selection: &'h SelectedCarrier,
    ) -> SourceCommandResult<Self> {
        host.frames.verify()?;
        if host.frames.revision() != selection.revision
            || host.frames.worker.prepared_execution_binding() != selection.worker
        {
            return Err(SourceCommandError::Conflict(
                "PWE carrier source/worker differs",
            ));
        }
        Ok(Self { host, selection })
    }
}
impl<R: ExactDependencyReader> JointCarrierOwner for PositiveCarrier<'_, '_, '_, R> {
    fn current_fence(
        &mut self,
        carrier: &JointCarrier<'_>,
    ) -> tos_compiler::pwe_native::Result<()> {
        self.host.frames.verify().map_err(refuse)?;
        if carrier.reference != &self.selection.reference
            || carrier.context != &self.selection.context
            || carrier.scope != &self.selection.scope
            || self.host.frames.revision() != self.selection.revision
            || self.host.frames.worker.prepared_execution_binding() != self.selection.worker
        {
            return Err(tos_compiler::pwe_native::Error::ExactMismatch);
        }
        Ok(())
    }
    fn admissible_world(
        &mut self,
        carrier: &JointCarrier<'_>,
        world: &JointWorld<'_>,
    ) -> tos_compiler::pwe_native::Result<()> {
        self.current_fence(carrier)?;
        let group = self
            .selection
            .groups
            .iter()
            .find(|g| &g.reference == world.world_ref)
            .ok_or(tos_compiler::pwe_native::Error::NeedsBinding(
                "actual source-account scenario not selected",
            ))?;
        if !world.negative.is_empty() || world.positive != group.positive.as_slice() {
            return Err(tos_compiler::pwe_native::Error::QualificationMismatch(
                "explicit positive source group",
            ));
        }
        Ok(())
    }
}
