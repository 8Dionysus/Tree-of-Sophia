use super::{
    BindingRegistry, ExactDependencyReader, HeldFrames, MAX_RECORDS, NativeBasis, RoleCache,
    invalid, refuse, unsupported,
};
use crate::source_command::{SourceCommandError, SourceCommandResult};
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
    pub(crate) fn object<'v>(
        &'v self,
        frames: &mut HeldFrames<'_, '_>,
    ) -> SourceCommandResult<&'v Value> {
        frames.field(&self.claim, "object")
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
        let roles = RoleCache::new(role_registry, kind_binding, &mut frames)?;
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
        let mut index = None;
        for (i, candidate) in self.approved.iter().enumerate() {
            if self
                .frames
                .equal(&candidate.reference, &account.reference)?
                && self.frames.equal(&candidate.claim, &account.claim)?
            {
                index = Some(i);
                break;
            }
        }
        let selected = &self.approved[index.ok_or(SourceCommandError::Denied(
            "PWE account not selected by this held native host",
        ))?];
        let report = &account.report;
        let binding = AssessmentSchemaBindingV1::SourceCut {
            source_revision: self.frames.revision(),
            worker: self.frames.worker.prepared_execution_binding(),
        };
        let recorded_at = self.frames.context.recorded_at.as_str();
        if report.source_revision() != Some(self.frames.revision())
            || report.source_input_identity() != self.frames.source_identity()
            || report.schema_binding() != binding
            || !self.frames.text_equal(report.observed_now(), recorded_at)?
            || !self
                .frames
                .equal(report.scope(), &selected.assessment_scope)?
        {
            return Err(SourceCommandError::Conflict(
                "PWE source-account current native report differs",
            ));
        }
        let admission = report.current_admission();
        if !self
            .frames
            .field_equal(admission, "subject", &account.reference)?
            || !self
                .frames
                .field_equal(admission, "policy", &selected.assessment_policy)?
            || self
                .frames
                .get(admission, "can_use")?
                .and_then(Value::as_bool)
                != Some(true)
            || !self.frames.text_is(admission, "status", "admitted")?
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
        let id = self.frames.native_text(&plan.current, "id")?;
        let index = *self
            .frames
            .string_map_get(&plan.resolved.source_index, id)?
            .ok_or(unsupported(
                "PWE target is not an actual selected source Claim",
            ))?;
        let source = plan
            .resolved
            .source_records
            .get(index)
            .ok_or(invalid("PWE selected source index"))?;
        if !self.frames.same(source, &plan.current)? {
            return Err(SourceCommandError::Conflict(
                "PWE selected source Claim shadowed",
            ));
        }
        let document = self.frames.native_field(source, "payload")?;
        let path = self.frames.source_path(id)?;
        let reference = self.frames.claim_reference(source)?;
        self.frames
            .exact_match(&reference, document, "claim_id", "claim_version")?;
        let raw = self.frames.canonical(document)?;
        let claim = self.frames.decode_domain(&raw)?;
        let object = self.frames.field(&claim, "object")?;
        let prepared = session
            .prepare(object)
            .map_err(|_| invalid("PWE selected account value shape"))?;
        session
            .check_claim_content(&prepared, &claim)
            .map_err(|_| invalid("PWE source wrapper/content mismatch"))?;
        session
            .check_endpoints(&prepared)
            .map_err(|_| invalid("PWE account endpoints/member closure"))?;
        let spec = self.registry.select(prepared.kind())?;
        let schema_version = self.frames.native_text(document, "schema_version")?;
        let predicate = self.frames.native_text(document, "predicate")?;
        let assertion_layer = self.frames.native_text(document, "assertion_layer")?;
        if !self
            .frames
            .text_equal(schema_version, &spec.schema_version)?
            || !self.frames.text_equal(predicate, &spec.predicate)?
            || !self
                .frames
                .text_equal(assertion_layer, &spec.assertion_layer)?
        {
            return Err(SourceCommandError::Conflict(
                "PWE source Claim/registered profile mismatch",
            ));
        }
        self.frames
            .check_schema(&path, document, &spec.schema_path)?;
        let kind = prepared.kind();
        let argument_bytes = spec
            .arguments
            .iter()
            .try_fold(0usize, |n, s| {
                n.checked_add(s.len())
                    .and_then(|n| n.checked_add(std::mem::size_of::<String>()))
            })
            .ok_or(unsupported("PWE dispatch argument holder"))?;
        self.frames
            .inspect_access(argument_bytes, spec.arguments.len())?;
        self.frames.retain(argument_bytes)?;
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
            let criterion = self
                .frames
                .pointer(&claim, "/object/criterion/value")?
                .ok_or(unsupported("PWE criterion binding absent"))?;
            self.role_value(criterion, Role::CriterionPolicy)?;
        }
        let report = self.frames.assess(plan)?;
        // These retained exact fields came from native source selection and
        // C4 application verification, not from a caller-created DispatchRow.
        let scope = self.frames.domain_value(&plan.scope)?;
        let policy = self
            .frames
            .get(report.current_admission(), "policy")?
            .ok_or(invalid("PWE native policy ref absent"))?;
        self.frames.reserve_domain_copy(policy)?;
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
        let actual_args = self.frames.pointer(&predicate, "/body/arguments")?;
        let args_match = match actual_args {
            Some(args) => self.frames.string_arguments(args, arguments)?,
            None => false,
        };
        let value_kind = self.frames.get(account.value(), "kind")?;
        let predicate_kind = self.frames.pointer(&predicate, "/body/value_kind")?;
        if !self
            .frames
            .text_is(&signature, "schema_version", "tos_pwe_signature_v1")?
            || !self
                .frames
                .text_is(&predicate, "schema_version", "tos_pwe_predicate_v1")?
            || !self.frames.text_is(
                &profile,
                "schema_version",
                "tos_pwe_qualification_profile_v1",
            )?
            || !self
                .frames
                .pointer_contains(&signature, "/body/predicates", account.predicate())?
            || !self.frames.optional_equal(predicate_kind, value_kind)?
            || !args_match
            || !self
                .frames
                .pointer_equal(&profile, "/body/signature_ref", account.signature())?
            || !self
                .frames
                .pointer_equal(&profile, "/body/predicate_ref", account.predicate())?
            || !self.frames.pointers_equal(
                &profile,
                "/body/modality",
                account.value(),
                "/qualification/modality",
            )?
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
        let recorded_at = self.frames.context.recorded_at.as_str();
        if view.source_revision != self.frames.revision()
            || view.source_input_identity != self.frames.source_identity()
            || view.assessment_schema_binding != binding
            || !self.frames.text_equal(view.observed_now, recorded_at)?
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
        if !self
            .frames
            .text_is(&carrier, "schema_version", "tos_pwe_identity_carrier_v1")?
            || !self
                .frames
                .pointer_equal(&carrier, "/body/context_ref", view.context)?
            || !self
                .frames
                .pointer_equal(&carrier, "/body/scope_ref", view.scope)?
            || !self
                .frames
                .pointer_equal(&carrier, "/body/criterion_ref", view.criterion)?
            || !self
                .frames
                .pointer_contains(&carrier, "/body/scenario_refs", view.scenario_ref)?
        {
            return Err(SourceCommandError::Conflict(
                "PWE identity carrier selected scenario/context/scope/criterion differs",
            ));
        }
        let mut protected = false;
        for approved in &self.approved {
            if self
                .frames
                .equal(&approved.assessment_scope, view.assessment_scope)?
                && self
                    .frames
                    .equal(&approved.assessment_policy, view.assessment_policy)?
            {
                protected = true;
                break;
            }
        }
        if !protected {
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
        let mut index = None;
        for (i, approved) in self.approved.iter().enumerate() {
            if self
                .frames
                .equal(&approved.reference, claim_ref)
                .map_err(refuse)?
            {
                index = Some(i);
                break;
            }
        }
        let selected = &self.approved[index.ok_or(tos_compiler::pwe_native::Error::Denied)?];
        // requested_use is shared by the query. Record/layer/language/maker and
        // the rest remain each Claim's actual protected native scope.
        if !self
            .frames
            .equal(&selected.assessment_policy, view.assessment_policy)
            .map_err(refuse)?
            || !self
                .frames
                .fields_equal(
                    &selected.assessment_scope,
                    view.assessment_scope,
                    "requested_use",
                )
                .map_err(refuse)?
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
        let kind = if self
            .frames
            .text_is(account.value(), "kind", "name-assignment")
            .map_err(refuse)?
        {
            Kind::Name
        } else if self
            .frames
            .text_is(account.value(), "kind", "responsibility-account")
            .map_err(refuse)?
        {
            Kind::Responsibility
        } else {
            Kind::Identity
        };
        let mut index = None;
        for (i, approved) in self.approved.iter().enumerate() {
            if approved.kind == kind && self.frames.equal(&approved.claim, claim).map_err(refuse)? {
                index = Some(i);
                break;
            }
        }
        let index = index.ok_or(tos_compiler::pwe_native::Error::NeedsBinding(
            "Claim was not prepared by this held native owner",
        ))?;
        let scenario = self
            .role_value(view.scenario_ref, Role::Scenario)
            .map_err(refuse)?;
        let body = self.frames.field(&scenario, "body").map_err(refuse)?;
        let refs = self
            .frames
            .get(body, "claim_refs")
            .map_err(refuse)?
            .and_then(Value::as_array);
        let contains = match refs {
            Some(rows) if rows.len() <= MAX_RECORDS => self
                .frames
                .contains(rows, &self.approved[index].reference)
                .map_err(refuse)?,
            _ => false,
        };
        if !self
            .frames
            .text_is(&scenario, "schema_version", "tos_pwe_identity_scenario_v1")
            .map_err(refuse)?
            || !self
                .frames
                .text_is(body, "interpretation", "co-selected-account-assertions-v1")
                .map_err(refuse)?
            || !self
                .frames
                .field_equal(body, "carrier_ref", view.joint_carrier)
                .map_err(refuse)?
            || !self
                .frames
                .field_equal(body, "context_ref", view.context)
                .map_err(refuse)?
            || !self
                .frames
                .field_equal(body, "scope_ref", view.scope)
                .map_err(refuse)?
            || !self
                .frames
                .field_equal(body, "criterion_ref", view.criterion)
                .map_err(refuse)?
            || !contains
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
        if !self
            .frames
            .equal(&selected.assessment_policy, view.assessment_policy)
            .map_err(refuse)?
            || !self
                .frames
                .fields_equal(
                    &selected.assessment_scope,
                    view.assessment_scope,
                    "requested_use",
                )
                .map_err(refuse)?
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
        let mut argument_match = selected.arguments.len() == arguments.len();
        for (actual, expected) in selected.arguments.iter().zip(arguments) {
            if !self.frames.text_equal(actual, expected).map_err(refuse)? {
                argument_match = false;
                break;
            }
        }
        if !argument_match {
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
