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
        let object = account.object(&mut host.frames)?;
        if !host.frames.text_is(claim, "polarity", "positive")?
            || !host
                .frames
                .text_is(object, "kind", "responsibility-account")?
            || !host
                .frames
                .text_is(object, "attribution_mode", "source-attribution")?
            || !host
                .frames
                .pointer_text_is(object, "/qualification/modality", "source-reported")?
        {
            return Err(unsupported(
                "PWE positive source-account carrier needs explicit source attribution/sign",
            ));
        }
        let carrier = host.role_value(carrier_ref, Role::JointCarrier)?;
        if !host.frames.text_is(
            &carrier,
            "schema_version",
            "tos_pwe_source_account_carrier_v1",
        )? {
            return Err(unsupported("PWE carrier actual profile differs"));
        }
        let body = host
            .frames
            .get(&carrier, "body")?
            .ok_or(invalid("PWE native carrier body"))?;
        if !host
            .frames
            .text_is(body, "interpretation", INTERPRETATION)?
            || !host
                .frames
                .field_equal(body, "claim_ref", account.reference())?
        {
            return Err(SourceCommandError::Conflict(
                "PWE carrier selected Claim/interpretation differs",
            ));
        }
        let context = host
            .frames
            .pointer(object, "/qualification/context_ref/value")?
            .ok_or(unsupported("PWE source-account context unresolved"))?;
        let scope = host
            .frames
            .pointer(object, "/qualification/scope_ref/value")?
            .ok_or(unsupported("PWE source-account scope unresolved"))?;
        if !host.frames.field_equal(body, "context_ref", context)?
            || !host.frames.field_equal(body, "scope_ref", scope)?
        {
            return Err(SourceCommandError::Conflict(
                "PWE carrier content scope/context differs",
            ));
        }
        let declared = host
            .frames
            .get(body, "scenario_refs")?
            .and_then(Value::as_array)
            .ok_or(invalid("PWE actual carrier scenarios"))?;
        if scenario_refs.len() > MAX_RECORDS || declared.len() > MAX_RECORDS {
            return Err(unsupported("PWE carrier scenario count cap"));
        }
        let mut groups = Vec::new();
        let slots = scenario_refs
            .len()
            .checked_mul(std::mem::size_of::<SelectedGroup>())
            .ok_or(unsupported("PWE group reservation overflow"))?;
        let available = host.frames.remaining_live()?;
        host.frames.retain(slots)?;
        groups
            .try_reserve_exact(scenario_refs.len())
            .map_err(|_| unsupported("PWE group reservation"))?;
        let actual = groups
            .capacity()
            .checked_mul(std::mem::size_of::<SelectedGroup>())
            .ok_or(unsupported("PWE group capacity overflow"))?;
        if actual > available {
            return Err(unsupported("PWE group live allowance"));
        }
        host.frames.retain(actual.saturating_sub(slots))?;
        for (i, reference) in scenario_refs.iter().enumerate() {
            if host.frames.contains(&scenario_refs[..i], reference)?
                || !host.frames.contains(declared, reference)?
            {
                return Err(SourceCommandError::Conflict(
                    "PWE scenario not uniquely selected from actual carrier",
                ));
            }
            let scenario = host.role_value(reference, Role::Scenario)?;
            if !host.frames.text_is(
                &scenario,
                "schema_version",
                "tos_pwe_source_account_scenario_v1",
            )? {
                return Err(unsupported("PWE group scenario actual profile differs"));
            }
            let scenario = host
                .frames
                .get(&scenario, "body")?
                .ok_or(invalid("PWE native scenario body"))?;
            if !host
                .frames
                .text_is(scenario, "interpretation", INTERPRETATION)?
                || !host
                    .frames
                    .field_equal(scenario, "carrier_ref", carrier_ref)?
                || !host
                    .frames
                    .field_equal(scenario, "claim_ref", account.reference())?
                || !host.frames.field_equal(scenario, "context_ref", context)?
                || !host.frames.field_equal(scenario, "scope_ref", scope)?
            {
                return Err(SourceCommandError::Conflict(
                    "PWE scenario source/carrier/scope/context differs",
                ));
            }
            let pointer = host
                .frames
                .pointer(scenario, "/group_selector/pointer")?
                .and_then(Value::as_str)
                .ok_or(invalid("PWE explicit source group selector"))?;
            if !host
                .frames
                .pointer_text_is(scenario, "/group_selector/syntax", "json-pointer")?
            {
                return Err(invalid("PWE source group selector syntax"));
            }
            let mode = host
                .frames
                .pointer(object, "/assignees/mode")?
                .and_then(Value::as_str);
            host.frames.inspect_access(
                pointer
                    .len()
                    .checked_mul(3)
                    .ok_or(unsupported("PWE group pointer work"))?,
                1,
            )?;
            let permitted = if host
                .frames
                .text_equal(pointer, "/object/assignees/bearers")?
            {
                match mode {
                    Some(mode) => host.frames.text_equal(mode, "joint")?,
                    None => false,
                }
            } else if let Some(index) = pointer.strip_prefix("/object/assignees/groups/") {
                matches_mode(&mut host.frames, mode, "alternatives")?
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
            let positive = host
                .frames
                .pointer(claim, pointer)?
                .and_then(Value::as_array)
                .ok_or(invalid("PWE actual source group absent"))?;
            if positive.is_empty() || positive.len() > 128 {
                return Err(invalid("PWE source group cardinality"));
            }
            let group_value = host
                .frames
                .pointer(claim, pointer)?
                .ok_or(invalid("PWE group absent"))?;
            host.frames.reserve_domain_copy(reference)?;
            host.frames.reserve_domain_copy(group_value)?;
            groups.push(SelectedGroup {
                reference: reference.clone(),
                positive: positive.clone(),
            });
        }
        host.frames.reserve_domain_copy(carrier_ref)?;
        host.frames.reserve_domain_copy(context)?;
        host.frames.reserve_domain_copy(scope)?;
        host.frames.retain(std::mem::size_of::<SelectedCarrier>())?;
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
        host.frames.budget.charge_work(1)?;
        host.frames.retain(std::mem::size_of::<Self>())?;
        Ok(Self { host, selection })
    }
}
impl<R: ExactDependencyReader> JointCarrierOwner for PositiveCarrier<'_, '_, '_, R> {
    fn current_fence(
        &mut self,
        carrier: &JointCarrier<'_>,
    ) -> tos_compiler::pwe_native::Result<()> {
        self.host.frames.verify().map_err(refuse)?;
        if !self
            .host
            .frames
            .equal(carrier.reference, &self.selection.reference)
            .map_err(refuse)?
            || !self
                .host
                .frames
                .equal(carrier.context, &self.selection.context)
                .map_err(refuse)?
            || !self
                .host
                .frames
                .equal(carrier.scope, &self.selection.scope)
                .map_err(refuse)?
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
        let mut selected = None;
        for group in &self.selection.groups {
            if self
                .host
                .frames
                .equal(&group.reference, world.world_ref)
                .map_err(refuse)?
            {
                selected = Some(group);
                break;
            }
        }
        let group = selected.ok_or(tos_compiler::pwe_native::Error::NeedsBinding(
            "actual source-account scenario not selected",
        ))?;
        let mut positives_match = world.positive.len() == group.positive.len();
        for (left, right) in world.positive.iter().zip(&group.positive) {
            if !self.host.frames.equal(left, right).map_err(refuse)? {
                positives_match = false;
                break;
            }
        }
        if !world.negative.is_empty() || !positives_match {
            return Err(tos_compiler::pwe_native::Error::QualificationMismatch(
                "explicit positive source group",
            ));
        }
        Ok(())
    }
}

fn matches_mode(
    frames: &mut super::HeldFrames<'_, '_>,
    actual: Option<&str>,
    expected: &str,
) -> SourceCommandResult<bool> {
    match actual {
        Some(actual) => frames.text_equal(actual, expected),
        None => Ok(false),
    }
}
