use super::{
    BindingRegistry, ExactDependencyReader, HeldFrames, MAX_RECORDS, NativeBasis, RoleCache,
    canonical, exact_match, invalid, refuse, unsupported,
};
use crate::source_command::{self as cmd, SourceCommandError, SourceCommandResult};
use crate::source_native_cli::private_assessment::AssessmentEvaluationPlan;
use crate::source_native_roles::{NativeRole as Role, RoleRegistrySelection};
use serde_json::Value;
use tos_compiler::pwe_native::{
    CheckedMetadata, ConsumerOwner, DispatchRow, Kind, QualifiedAccount, ResolvedBasis, Session,
    ViewBasis,
};
use tos_foundation::ExactRecordRef;
use tos_validation::assessment::{AssessmentMechanicsReport, AssessmentSchemaBindingV1};
use tos_validation::source_cut::CutSchemaExecutor;

/// Only constructed from the actual protected plan and native evaluator.
/// It grants no new operation and is not an AssessmentMechanicsReport factory.
pub(crate) struct NativeAccount {
    claim: Value,
    reference: Value,
    report: AssessmentMechanicsReport,
    basis: NativeBasis,
}
impl NativeAccount {
    pub(crate) fn claim(&self) -> &Value {
        &self.claim
    }
    pub(crate) fn reference(&self) -> &Value {
        &self.reference
    }
    pub(crate) fn report(&self) -> &AssessmentMechanicsReport {
        &self.report
    }
    pub(crate) fn object(&self) -> &Value {
        &self.claim["object"]
    }
    pub(crate) fn basis(&self) -> &NativeBasis {
        &self.basis
    }
}
struct ApprovedDispatch {
    claim: Value,
    reference: Value,
    kind: Kind,
    signature: Value,
    predicate: Value,
    profile: Value,
    arguments: Vec<String>,
    assessment_scope: Value,
    assessment_policy: Value,
    basis_pins: Vec<Value>,
}
pub(crate) struct NativeConsumerOwner<'a, 'scope, R: ExactDependencyReader> {
    pub(super) frames: HeldFrames<'a, 'scope>,
    dependencies: &'a mut R,
    roles: RoleCache,
    registry: BindingRegistry,
    approved: Vec<ApprovedDispatch>,
}
impl<'a, 'scope, R: ExactDependencyReader> NativeConsumerOwner<'a, 'scope, R> {
    pub(crate) fn new(
        mut frames: HeldFrames<'a, 'scope>,
        dependencies: &'a mut R,
        role_registry: RoleRegistrySelection<'_>,
        kind_binding: &ExactRecordRef,
    ) -> SourceCommandResult<Self> {
        let registry = BindingRegistry::load(&mut frames)?;
        let roles = RoleCache::new(role_registry, kind_binding)?;
        Ok(Self {
            frames,
            dependencies,
            roles,
            registry,
            approved: Vec::new(),
        })
    }
    pub(super) fn role_value(
        &mut self,
        reference: &Value,
        role: Role,
    ) -> SourceCommandResult<Value> {
        self.roles.value(&mut self.frames, reference, role)
    }
    pub(super) fn admitted_account(&mut self, account: &NativeAccount) -> SourceCommandResult<()> {
        self.frames.verify()?;
        let selected = self
            .approved
            .iter()
            .find(|a| a.reference == account.reference && a.claim == account.claim)
            .ok_or(SourceCommandError::Denied(
                "PWE account not selected by this held native host",
            ))?;
        let report = &account.report;
        let binding = AssessmentSchemaBindingV1::SourceCut {
            source_revision: self.frames.revision(),
            worker: self.frames.worker.prepared_execution_binding(),
        };
        if report.source_revision() != Some(self.frames.revision())
            || report.source_input_identity() != self.frames.source_identity()
            || report.schema_binding() != binding
            || report.observed_now() != self.frames.context.recorded_at
            || report.scope() != &selected.assessment_scope
        {
            return Err(SourceCommandError::Conflict(
                "PWE source-account current native report differs",
            ));
        }
        let admission = report.current_admission();
        if admission.get("subject") != Some(&account.reference)
            || admission.get("policy") != Some(&selected.assessment_policy)
            || admission.get("can_use").and_then(Value::as_bool) != Some(true)
            || admission.get("status").and_then(Value::as_str) != Some("admitted")
            || report.source_read_required() && !report.source_read_ready()
        {
            return Err(unsupported(
                "PWE actual source-account admission/limits/read gate not satisfied",
            ));
        }
        Ok(())
    }
    /// Actual plan/config/source selection precedes this operation; no path or
    /// record body is taken from an unauthenticated query to construct an owner.
    pub(crate) fn prepare_account(
        &mut self,
        plan: &AssessmentEvaluationPlan<'_>,
        metadata: &[CheckedMetadata<'_>],
        session: &mut Session<'_>,
    ) -> SourceCommandResult<NativeAccount> {
        self.frames.verify()?;
        if self.approved.len() >= MAX_RECORDS {
            return Err(unsupported("PWE selected account count cap"));
        }
        let id = cmd::text(&plan.current, "id")?;
        let index = *plan.resolved.source_index.get(id).ok_or(unsupported(
            "PWE target is not an actual selected source Claim",
        ))?;
        let source = plan
            .resolved
            .source_records
            .get(index)
            .ok_or(invalid("PWE selected source index"))?;
        if !cmd::same(source, &plan.current)? {
            return Err(SourceCommandError::Conflict(
                "PWE selected source Claim shadowed",
            ));
        }
        let document = cmd::field(source, "payload")?;
        let path = self.frames.source_path(id)?.to_owned();
        let native_ref = crate::source_native_cli::private_assessment::owner_reference(source)?;
        let ref_raw = canonical(&native_ref)?;
        let reference: Value = serde_json::from_slice(&ref_raw)
            .map_err(|_| invalid("PWE native Claim reference conversion"))?;
        exact_match(&reference, document, "claim_id", "claim_version")?;
        let raw = canonical(document)?;
        self.frames.retain(
            raw.len()
                .checked_mul(64)
                .ok_or(unsupported("PWE Claim holder overflow"))?,
        )?;
        let claim: Value =
            serde_json::from_slice(&raw).map_err(|_| invalid("PWE native Claim conversion"))?;
        let prepared = session
            .prepare(&claim["object"])
            .map_err(|_| invalid("PWE selected account value shape"))?;
        session
            .check_claim_content(&prepared, &claim)
            .map_err(|_| invalid("PWE source wrapper/content mismatch"))?;
        session
            .check_endpoints(&prepared)
            .map_err(|_| invalid("PWE account endpoints/member closure"))?;
        let spec = self.registry.select(prepared.kind())?;
        if cmd::text(document, "schema_version")? != spec.schema_version
            || cmd::text(document, "predicate")? != spec.predicate
            || cmd::text(document, "assertion_layer")? != spec.assertion_layer
        {
            return Err(SourceCommandError::Conflict(
                "PWE source Claim/registered profile mismatch",
            ));
        }
        self.frames
            .check_schema(&path, document, &spec.schema_path)?;
        let kind = prepared.kind();
        let arguments = spec.arguments.clone();
        let basis = NativeBasis::capture(
            &mut self.frames,
            self.dependencies,
            &mut self.roles,
            session,
            &prepared,
        )?;
        basis.check_metadata(&mut self.frames, metadata)?;
        session
            .check_role_matrix(&prepared, metadata)
            .map_err(|_| {
                unsupported("PWE actual metadata responsibility/identity role binding differs")
            })?;
        let qualified = session
            .qualify(prepared)
            .map_err(|_| unsupported("PWE account qualification is unresolved"))?;
        self.check_application(&qualified, &arguments)?;
        for (reference, role) in [
            (qualified.context(), Role::Context),
            (qualified.scope(), Role::Scope),
        ] {
            self.role_value(reference, role)?;
        }
        if kind == Kind::Identity {
            let criterion = claim
                .pointer("/object/criterion/value")
                .ok_or(unsupported("PWE criterion binding absent"))?;
            self.role_value(criterion, Role::CriterionPolicy)?;
        }
        let report = self.frames.assess(plan)?;
        // These retained exact fields came from native source selection and
        // C4 application verification, not from a caller-created DispatchRow.
        let scope = self.frames.domain_value(&plan.scope)?;
        let policy = report
            .current_admission()
            .get("policy")
            .ok_or(invalid("PWE native policy ref absent"))?;
        self.frames.retain(
            super::encoded_len(policy)?
                .checked_mul(64)
                .ok_or(unsupported("PWE policy reference holder overflow"))?,
        )?;
        let policy = policy.clone();
        self.frames.retain(
            raw.len()
                .checked_mul(64)
                .and_then(|n| n.checked_add(8192))
                .ok_or(unsupported("PWE dispatch holder overflow"))?,
        )?;
        let selected = ApprovedDispatch {
            claim: claim.clone(),
            reference: reference.clone(),
            kind,
            signature: qualified.signature().clone(),
            predicate: qualified.predicate().clone(),
            profile: qualified.qualification_profile().clone(),
            arguments,
            assessment_scope: scope,
            assessment_policy: policy,
            basis_pins: basis.pins(&mut self.frames)?,
        };
        // Charge the retained duplicate content before cloning into the cache.
        // The source Claim and evidence remain separate in NativeAccount.
        self.approved
            .try_reserve_exact(1)
            .map_err(|_| unsupported("PWE dispatch reservation"))?;
        self.approved.push(selected);
        self.frames.verify()?;
        Ok(NativeAccount {
            claim,
            reference,
            report,
            basis,
        })
    }
    fn check_application(
        &mut self,
        account: &QualifiedAccount<'_>,
        arguments: &[String],
    ) -> SourceCommandResult<()> {
        let signature = self.role_value(account.signature(), Role::Signature)?;
        let predicate = self.role_value(account.predicate(), Role::Predicate)?;
        let profile =
            self.role_value(account.qualification_profile(), Role::QualificationProfile)?;
        if signature.get("schema_version").and_then(Value::as_str) != Some("tos_pwe_signature_v1")
            || predicate.get("schema_version").and_then(Value::as_str)
                != Some("tos_pwe_predicate_v1")
            || profile.get("schema_version").and_then(Value::as_str)
                != Some("tos_pwe_qualification_profile_v1")
            || !signature
                .pointer("/body/predicates")
                .and_then(Value::as_array)
                .is_some_and(|p| p.contains(account.predicate()))
            || predicate.pointer("/body/value_kind") != account.value().get("kind")
            || !predicate
                .pointer("/body/arguments")
                .and_then(Value::as_array)
                .is_some_and(|p| {
                    p.iter()
                        .filter_map(Value::as_str)
                        .eq(arguments.iter().map(String::as_str))
                })
            || profile.pointer("/body/signature_ref") != Some(account.signature())
            || profile.pointer("/body/predicate_ref") != Some(account.predicate())
            || profile.pointer("/body/modality")
                != account.value().pointer("/qualification/modality")
        {
            return Err(SourceCommandError::Conflict(
                "PWE source-owned signature/predicate/qualification application differs",
            ));
        }
        Ok(())
    }
    fn verify_view(&mut self, view: &ViewBasis<'_>) -> SourceCommandResult<()> {
        self.frames.verify()?;
        let worker = CutSchemaExecutor::prepared_execution_binding(&*self.frames.worker)
            .ok_or(unsupported("PWE prepared schema worker binding absent"))?;
        let binding = AssessmentSchemaBindingV1::SourceCut {
            source_revision: self.frames.revision(),
            worker,
        };
        if view.source_revision != self.frames.revision()
            || view.source_input_identity != self.frames.source_identity()
            || view.assessment_schema_binding != binding
            || view.observed_now != self.frames.context.recorded_at
        {
            return Err(SourceCommandError::Conflict(
                "PWE view source/worker/time selector differs",
            ));
        }
        for (reference, role) in [
            (view.context, Role::Context),
            (view.scope, Role::Scope),
            (view.criterion, Role::CriterionPolicy),
            (view.joint_carrier, Role::JointCarrier),
            (view.scenario_ref, Role::Scenario),
        ] {
            self.role_value(reference, role)?;
        }
        let carrier = self.role_value(view.joint_carrier, Role::JointCarrier)?;
        if carrier.get("schema_version").and_then(Value::as_str)
            != Some("tos_pwe_identity_carrier_v1")
            || carrier.pointer("/body/context_ref") != Some(view.context)
            || carrier.pointer("/body/scope_ref") != Some(view.scope)
            || carrier.pointer("/body/criterion_ref") != Some(view.criterion)
            || !carrier
                .pointer("/body/scenario_refs")
                .and_then(Value::as_array)
                .is_some_and(|rows| rows.contains(view.scenario_ref))
        {
            return Err(SourceCommandError::Conflict(
                "PWE identity carrier selected scenario/context/scope/criterion differs",
            ));
        }
        if !self.approved.iter().any(|a| {
            a.assessment_scope == *view.assessment_scope
                && a.assessment_policy == *view.assessment_policy
        }) {
            return Err(SourceCommandError::Denied(
                "PWE query access scope/policy has no held protected selection",
            ));
        }
        self.frames.verify()
    }
}
impl<R: ExactDependencyReader> ConsumerOwner for NativeConsumerOwner<'_, '_, R> {
    fn current_fence(&mut self, view: &ViewBasis<'_>) -> tos_compiler::pwe_native::Result<()> {
        self.verify_view(view).map_err(refuse)
    }
    fn assessment_scope<'o>(
        &'o mut self,
        view: &'o ViewBasis<'_>,
        claim_ref: &Value,
    ) -> tos_compiler::pwe_native::Result<&'o Value> {
        self.verify_view(view).map_err(refuse)?;
        let selected = self
            .approved
            .iter()
            .find(|a| a.reference == *claim_ref)
            .ok_or(tos_compiler::pwe_native::Error::Denied)?;
        // requested_use is shared by the query. Record/layer/language/maker and
        // the rest remain each Claim's actual protected native scope.
        if selected.assessment_policy != *view.assessment_policy
            || selected.assessment_scope.get("requested_use")
                != view.assessment_scope.get("requested_use")
        {
            return Err(tos_compiler::pwe_native::Error::Denied);
        }
        Ok(&selected.assessment_scope)
    }
    fn account_dispatch<'o>(
        &'o mut self,
        view: &ViewBasis<'_>,
        account: &QualifiedAccount<'_>,
        claim: &Value,
        basis: &ResolvedBasis<'_>,
    ) -> tos_compiler::pwe_native::Result<DispatchRow<'o>> {
        self.verify_view(view).map_err(refuse)?;
        let index = self
            .approved
            .iter()
            .position(|a| a.claim == *claim && a.kind == account_kind(account))
            .ok_or(tos_compiler::pwe_native::Error::NeedsBinding(
                "Claim was not prepared by this held native owner",
            ))?;
        let scenario = self
            .role_value(view.scenario_ref, Role::Scenario)
            .map_err(refuse)?;
        let body = &scenario["body"];
        if scenario.get("schema_version").and_then(Value::as_str)
            != Some("tos_pwe_identity_scenario_v1")
            || body.get("interpretation").and_then(Value::as_str)
                != Some("co-selected-account-assertions-v1")
            || body.get("carrier_ref") != Some(view.joint_carrier)
            || body.get("context_ref") != Some(view.context)
            || body.get("scope_ref") != Some(view.scope)
            || body.get("criterion_ref") != Some(view.criterion)
            || !body
                .get("claim_refs")
                .and_then(Value::as_array)
                .is_some_and(|rows| {
                    rows.len() <= MAX_RECORDS && rows.contains(&self.approved[index].reference)
                })
        {
            return Err(tos_compiler::pwe_native::Error::NeedsBinding(
                "account is not co-selected in this actual scenario",
            ));
        }
        let selected = &self.approved[index];
        if !NativeBasis::matches_pins(&selected.basis_pins, basis, &mut self.frames)
            .map_err(refuse)?
        {
            return Err(tos_compiler::pwe_native::Error::ExactMismatch);
        }
        if selected.assessment_policy != *view.assessment_policy
            || selected.assessment_scope.get("requested_use")
                != view.assessment_scope.get("requested_use")
        {
            return Err(tos_compiler::pwe_native::Error::QualificationMismatch(
                "protected access scope/policy",
            ));
        }
        // Argument order is fixed by each actual selected schema/binding row.
        let arguments = match selected.kind {
            Kind::Name => &[
                "subject",
                "subject_role",
                "assignment",
                "name",
                "name_function",
                "named_bearer",
            ][..],
            Kind::Responsibility => &[
                "subject",
                "role",
                "attribution_mode",
                "assignees",
                "responsibility_scope",
                "role_basis",
            ][..],
            Kind::Identity => &[
                "left",
                "right",
                "identity_level",
                "criterion",
                "asserted_outcome",
            ][..],
        };
        if !selected
            .arguments
            .iter()
            .map(String::as_str)
            .eq(arguments.iter().copied())
        {
            return Err(tos_compiler::pwe_native::Error::QualificationMismatch(
                "native dispatch arguments",
            ));
        }
        Ok(DispatchRow {
            outer_predicate: selected.kind.outer_predicate(),
            kind: selected.kind,
            signature: &selected.signature,
            predicate: &selected.predicate,
            qualification_profile: &selected.profile,
            arguments,
        })
    }
}
fn account_kind(account: &QualifiedAccount<'_>) -> Kind {
    match account.value().get("kind").and_then(Value::as_str) {
        Some("name-assignment") => Kind::Name,
        Some("responsibility-account") => Kind::Responsibility,
        _ => Kind::Identity,
    }
}
