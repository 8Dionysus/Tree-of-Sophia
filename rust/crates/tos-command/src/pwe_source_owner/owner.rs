use super::{
    BindingRegistry, ExactDependencyReader, HeldFrames, MAX_RECORDS, NativeBasis, RoleCache,
    invalid, refuse, unsupported,
};
use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_native_cli::private_assessment::AssessmentEvaluationPlan;
use crate::source_native_roles::{NativeRole as Role, RoleRegistrySelection};
use serde_json::Value;
use tos_compiler::pwe_native::{
    CheckedMetadata, ConsumerOwner, DispatchRow, Error, Kind, PreparedAccount, QualifiedAccount,
    ResolvedBasis, Session, ViewBasis, ViewSource,
};
use tos_foundation::ExactRecordRef;
use tos_validation::assessment::{AssessmentMechanicsReport, AssessmentSchemaBindingV1};
use tos_validation::source_cut::CutSchemaExecutor;
use std::rc::Rc;
use super::source_budget::{SourcePayer, OriginalPayer};
use crate::source_native_cli::private_assessment::original_payer::OriginalAssessmentPayer;
use crate::source_native_cli::private_assessment::AssessmentEvaluationTarget;

/// Preserve the original source payer's error before the host's paid Err fence
/// or Common's finish observes its sticky refusal. Semantic negatives retain
/// the operation-specific refusal; no error cell or replacement ledger exists.
pub(super) fn host_session_result<T>(
    result: tos_compiler::pwe_native::Result<T>,
    payer: &dyn OriginalPayer,
    semantic: SourceCommandError,
) -> SourceCommandResult<T> {
    result.map_err(|error| match error {
        Error::Upstream(_) | Error::Cancelled | Error::Deadline | Error::ResourceLimit => {
            payer.error(error, "PWE held Session upstream operation")
        }
        _ => semantic,
    })
}

/// The actual prepare_target content stage, separately addressable without
/// constructing a native owner, ticket, grant or fabricated mechanics report.
fn prepare_target_content<'value>(
    session: &mut Session<'_>,
    payer: &dyn OriginalPayer,
    object: &'value Value,
    claim: &Value,
) -> SourceCommandResult<PreparedAccount<'value>> {
    let prepared = host_session_result(
        session.prepare(object), payer, invalid("PWE selected account value shape"),
    )?;
    host_session_result(
        session.check_claim_content(&prepared, claim), payer,
        invalid("PWE source wrapper/content mismatch"),
    )?;
    host_session_result(
        session.check_endpoints(&prepared), payer,
        invalid("PWE account endpoints/member closure"),
    )?;
    Ok(prepared)
}

/// Only constructed from the actual protected plan and native evaluator.
/// It grants no new operation and is not an AssessmentMechanicsReport factory.
pub(crate) struct NativeAccount {
    claim: Value,
    reference: Value,
    report: AssessmentMechanicsReport,
    basis: NativeBasis,
    host_lease: Rc<HostLease>,
    frame_lease: Rc<super::frames::FrameLease>,
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
// A strong handle is retained by every account and parent witness. No raw
// address survives independently of its allocation. This is not a source ID.
pub(super) struct HostLease { _private: u8 }
pub(crate) struct ApplicationCheckedTicket<'checked> {
    pub(super) target: AssessmentEvaluationTarget<'checked, 'checked>,
    pub(super) claim: &'checked Value,
    pub(super) reference: &'checked Value,
    pub(super) signature: &'checked Value,
    pub(super) predicate: &'checked Value,
    pub(super) profile: &'checked Value,
    pub(super) arguments: &'checked [String],
    host_lease: Rc<HostLease>,
    frame_lease: Rc<super::frames::FrameLease>,
}
pub(crate) struct CheckedPweParent<'account, 'plan> {
    parent: &'account NativeAccount,
    selected_form: &'plan AssessmentEvaluationPlan<'plan>,
    host_lease: Rc<HostLease>,
    frame_lease: Rc<super::frames::FrameLease>,
}
pub(crate) struct NativeAssessmentHost<'host, 'a, 'scope, 'cancel, R: ExactDependencyReader> {
    owner: &'host mut NativeConsumerOwner<'a, 'scope, R>,
    session: &'host mut Session<'cancel>,
    payer: &'host dyn OriginalPayer,
    lease: Rc<HostLease>,
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
    fn with_native_session<'host, 'cancel, T>(
        &'host mut self,
        session: &'host mut Session<'cancel>,
        payer: &'host dyn OriginalPayer,
        run: impl FnOnce(&mut NativeAssessmentHost<'host, 'a, 'scope, 'cancel, R>) -> SourceCommandResult<T>,
    ) -> SourceCommandResult<T> {
        self.frames.verify()?;
        let prepared = (|| {
            self.frames.inspect_access(0, 2)?;
            self.frames.check_session_payer(session, payer)?;
            if !session.has_source_payer(payer.admission()) {
                return Err(unsupported("PWE scoped host requires its actual original Session payer"));
            }
            let holders = std::mem::size_of::<NativeAssessmentHost<'_, '_, '_, '_, R>>()
                .checked_add(std::mem::size_of::<HostLease>())
                .and_then(|n| n.checked_add(4 * std::mem::size_of::<usize>()))
                .ok_or(unsupported("PWE scoped host lease holder"))?;
            self.frames.reserve_scoped(session, payer, holders)?;
            Ok(())
        })();
        if let Err(error) = prepared {
            let _ = self.frames.verify();
            return Err(error);
        }
        let mut host = NativeAssessmentHost {
            owner: self, session, payer, lease: Rc::new(HostLease { _private: 0 }),
        };
        let result = run(&mut host);
        let fenced = host.owner.frames.verify();
        match (result, fenced) {
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
    pub(crate) fn with_claim_session<'host, 'cancel, 'payer, T>(
        &'host mut self, session: &'host mut Session<'cancel>, payer: &'host SourcePayer<'payer>,
        run: impl FnOnce(&mut NativeAssessmentHost<'host, 'a, 'scope, 'cancel, R>) -> SourceCommandResult<T>,
    ) -> SourceCommandResult<T> { self.with_native_session(session, payer, run) }
    pub(crate) fn with_assessment_session<'host, 'cancel, 'budget, 'payer_cancel, T>(
        &'host mut self, session: &'host mut Session<'cancel>,
        payer: &'host OriginalAssessmentPayer<'budget, 'payer_cancel>,
        run: impl FnOnce(&mut NativeAssessmentHost<'host, 'a, 'scope, 'cancel, R>) -> SourceCommandResult<T>,
    ) -> SourceCommandResult<T> { self.with_native_session(session, payer, run) }
    pub(crate) fn prepare_account(
        &mut self, plan: &AssessmentEvaluationPlan<'_>, metadata: &[CheckedMetadata<'_>],
        session: &mut Session<'_>, payer: &OriginalAssessmentPayer<'_, '_>,
    ) -> SourceCommandResult<NativeAccount> {
        self.with_assessment_session(session, payer, |host| host.prepare_account(plan, metadata))
    }
    pub(crate) fn prepare_claim_account(
        &mut self, plan: &AssessmentEvaluationPlan<'_>, metadata: &[CheckedMetadata<'_>],
        session: &mut Session<'_>, payer: &SourcePayer<'_>,
    ) -> SourceCommandResult<NativeAccount> {
        self.with_claim_session(session, payer, |host| host.prepare_account(plan, metadata))
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
        self.frames.inspect_access(0, 1)?;
        if !Rc::ptr_eq(&self.frames.lease, &account.frame_lease) {
            return Err(SourceCommandError::Conflict("PWE account protected frame differs"));
        }
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
    fn prepare_target(
        &mut self,
        target: AssessmentEvaluationTarget<'_, '_>,
        metadata: &[CheckedMetadata<'_>],
        session: &mut Session<'_>,
        payer: &dyn OriginalPayer,
        host_lease: &Rc<HostLease>,
        parent: Option<CheckedPweParent<'_, '_>>,
    ) -> SourceCommandResult<NativeAccount> {
        self.frames.verify()?;
        self.frames.inspect_access(0, 4)?;
        let plan = target.plan();
        let target_is_parent = target.is_parent();
        if self.approved.len() >= MAX_RECORDS {
            return Err(unsupported("PWE selected account count cap"));
        }
        let id = self.frames.native_text(target.record(), "id")?;
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
        if !self.frames.same(source, target.record())? {
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
        let prepared = prepare_target_content(session, payer, object, &claim)?;
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
            payer,
        )?;
        basis.check_metadata(&mut self.frames, metadata)?;
        host_session_result(
            session.check_role_matrix(&prepared, metadata), payer,
            unsupported("PWE actual metadata responsibility/identity role binding differs"),
        )?;
        let qualified = host_session_result(
            session.qualify(prepared), payer,
            unsupported("PWE account qualification is unresolved"),
        )?;
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
        // Fixed ticket and retained lease handles are paid before cloning.
        self.frames.reserve_scoped(session, payer,
            std::mem::size_of::<ApplicationCheckedTicket<'_>>()
                + std::mem::size_of::<super::frames::HeldPweEvaluation<'_>>()
                + 4 * std::mem::size_of::<Rc<HostLease>>())?;
        let frame_lease = Rc::clone(&self.frames.lease);
        let ticket = ApplicationCheckedTicket {
            target,
            claim: &claim,
            reference: &reference,
            signature: qualified.signature(),
            predicate: qualified.predicate(),
            profile: qualified.qualification_profile(),
            arguments: &arguments,
            host_lease: Rc::clone(host_lease),
            frame_lease: Rc::clone(&frame_lease),
        };
        let report = self.frames.assess_checked(ticket, parent)?;
        // These retained exact fields came from native source selection and
        // C4 application verification, not from a caller-created DispatchRow.
        let scope = self.frames.domain_value(if target_is_parent {
            &plan.parent_context.as_ref().ok_or(invalid("PWE parent context absent"))?.scope
        } else { &plan.scope })?;
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
            host_lease: Rc::clone(host_lease),
            frame_lease,
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
        if view.source != (ViewSource::SourceCut {
                revision: self.frames.revision(),
                input_identity: self.frames.source_identity(),
            })
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

impl<R: ExactDependencyReader> NativeAssessmentHost<'_, '_, '_, '_, R> {
    fn reserve_target(&mut self) -> SourceCommandResult<()> {
        self.owner.frames.reserve_scoped(self.session, self.payer,
            std::mem::size_of::<AssessmentEvaluationTarget<'_, '_>>())
    }
    pub(crate) fn prepare_account(
        &mut self,
        plan: &AssessmentEvaluationPlan<'_>,
        metadata: &[CheckedMetadata<'_>],
    ) -> SourceCommandResult<NativeAccount> {
        self.reserve_target()?;
        self.owner.prepare_target(plan.current_target(), metadata, self.session, self.payer, &self.lease, None)
    }
    pub(crate) fn prepare_parent(
        &mut self,
        selected_form: &AssessmentEvaluationPlan<'_>,
        metadata: &[CheckedMetadata<'_>],
    ) -> SourceCommandResult<NativeAccount> {
        self.reserve_target()?;
        let target = selected_form.parent_target().ok_or(invalid("PWE actual parent target absent"))?;
        self.owner.prepare_target(target, metadata, self.session, self.payer, &self.lease, None)
    }
    pub(crate) fn checked_parent<'account, 'plan>(
        &mut self,
        parent: &'account NativeAccount,
        selected_form: &'plan AssessmentEvaluationPlan<'plan>,
    ) -> SourceCommandResult<CheckedPweParent<'account, 'plan>> {
        self.owner.frames.inspect_access(0, 2)?;
        if !Rc::ptr_eq(&self.lease, &parent.host_lease)
            || !Rc::ptr_eq(&self.owner.frames.lease, &parent.frame_lease)
        {
            return Err(SourceCommandError::Conflict("PWE parent belongs to another host/frame"));
        }
        self.owner.admitted_account(parent)?;
        self.reserve_target()?;
        let target = selected_form.parent_target().ok_or(invalid("PWE selected form has no parent"))?;
        let reference = self.owner.frames.claim_reference(target.record())?;
        let document = self.owner.frames.native_field(target.record(), "payload")?;
        let claim = self.owner.frames.domain_value(document)?;
        let scope = self.owner.frames.domain_value(target.scope())?;
        if !self.owner.frames.equal(&reference, &parent.reference)?
            || !self.owner.frames.equal(&claim, &parent.claim)?
            || !self.owner.frames.equal(&scope, parent.report.scope())?
        {
            return Err(SourceCommandError::Conflict("PWE exact selected parent record/scope differs"));
        }
        self.owner.frames.reserve_scoped(self.session, self.payer,
            std::mem::size_of::<CheckedPweParent<'_, '_>>() + 2 * std::mem::size_of::<Rc<HostLease>>())?;
        Ok(CheckedPweParent { parent, selected_form,
            host_lease: Rc::clone(&self.lease), frame_lease: Rc::clone(&self.owner.frames.lease) })
    }
    pub(crate) fn prepare_account_with_parent(
        &mut self,
        plan: &AssessmentEvaluationPlan<'_>,
        metadata: &[CheckedMetadata<'_>],
        parent: CheckedPweParent<'_, '_>,
    ) -> SourceCommandResult<NativeAccount> {
        self.owner.frames.inspect_access(0, 1)?;
        if !Rc::ptr_eq(&self.lease, &parent.host_lease) {
            return Err(SourceCommandError::Conflict("PWE checked parent belongs to another scoped host"));
        }
        self.reserve_target()?;
        self.owner.prepare_target(plan.current_target(), metadata, self.session, self.payer, &self.lease, Some(parent))
    }
    pub(crate) fn assess_form_with_parent(
        &mut self,
        plan: &AssessmentEvaluationPlan<'_>,
        parent: CheckedPweParent<'_, '_>,
    ) -> SourceCommandResult<AssessmentMechanicsReport> {
        self.owner.frames.inspect_access(0, 1)?;
        if !Rc::ptr_eq(&self.lease, &parent.host_lease) {
            return Err(SourceCommandError::Conflict("PWE form parent belongs to another scoped host"));
        }
        self.owner.frames.reserve_scoped(self.session, self.payer,
            std::mem::size_of::<super::frames::HeldPweEvaluation<'_>>()
                + std::mem::size_of::<AssessmentEvaluationTarget<'_, '_>>())?;
        self.owner.frames.assess_form_with_parent(plan, parent, &self.lease)
    }
}
impl ApplicationCheckedTicket<'_> {
    pub(crate) fn target(&self) -> &AssessmentEvaluationTarget<'_, '_> { &self.target }
    pub(crate) fn claim(&self) -> &Value { self.claim }
    pub(crate) fn reference(&self) -> &Value { self.reference }
    pub(crate) fn signature(&self) -> &Value { self.signature }
    pub(crate) fn predicate(&self) -> &Value { self.predicate }
    pub(crate) fn profile(&self) -> &Value { self.profile }
    pub(crate) fn arguments(&self) -> &[String] { self.arguments }
    pub(super) fn check_frame(&self, frames: &mut HeldFrames<'_, '_>) -> SourceCommandResult<()> {
        frames.inspect_access(0, 1)?;
        if !Rc::ptr_eq(&self.frame_lease, &frames.lease) {
            return Err(SourceCommandError::Conflict("PWE ticket protected frame differs"));
        }
        let native = frames.native_field(self.target.record(), "payload")?;
        frames.exact_match(self.reference, native, "claim_id", "claim_version")?;
        let claimed = frames.domain_value(native)?;
        if !frames.equal(&claimed, self.claim)? {
            return Err(SourceCommandError::Conflict("PWE ticket exact account content differs"));
        }
        Ok(())
    }
    pub(super) fn check_parent(&self, parent: &CheckedPweParent<'_, '_>, frames: &mut HeldFrames<'_, '_>) -> SourceCommandResult<()> {
        frames.inspect_access(0, 1)?;
        if !Rc::ptr_eq(&self.host_lease, &parent.host_lease) {
            return Err(SourceCommandError::Conflict("PWE current and parent scoped hosts differ"));
        }
        parent.check_frame(frames, self.target.plan())
    }
}
impl CheckedPweParent<'_, '_> {
    pub(crate) fn report(&self) -> &AssessmentMechanicsReport { &self.parent.report }
    pub(super) fn check_host(&self, frames: &mut HeldFrames<'_, '_>, lease: &Rc<HostLease>) -> SourceCommandResult<()> {
        frames.inspect_access(0, 1)?;
        if !Rc::ptr_eq(&self.host_lease, lease) {
            return Err(SourceCommandError::Conflict("PWE parent inactive/foreign host lease"));
        }
        Ok(())
    }
    pub(super) fn check_frame(&self, frames: &mut HeldFrames<'_, '_>, plan: &AssessmentEvaluationPlan<'_>) -> SourceCommandResult<()> {
        frames.inspect_access(0, 2)?;
        if !Rc::ptr_eq(&self.frame_lease, &frames.lease) || !std::ptr::eq(self.selected_form, plan) {
            return Err(SourceCommandError::Conflict("PWE checked parent frame/selected plan differs"));
        }
        let target_holder = std::mem::size_of::<AssessmentEvaluationTarget<'_, '_>>();
        frames.budget.charge_work(target_holder)?;
        frames.budget.charge_bytes(target_holder)?;
        frames.retain(target_holder)?;
        let target = plan.parent_target().ok_or(invalid("PWE checked parent target absent"))?;
        let reference = frames.claim_reference(target.record())?;
        let payload = frames.native_field(target.record(), "payload")?;
        frames.exact_match(&reference, payload, "claim_id", "claim_version")?;
        if !frames.equal(&reference, &self.parent.reference)?
            || { let claim = frames.domain_value(payload)?; !frames.equal(&claim, &self.parent.claim)? }
            || { let scope = frames.domain_value(target.scope())?; !frames.equal(&scope, self.parent.report.scope())? }
        {
            return Err(SourceCommandError::Conflict("PWE checked parent exact target differs"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod original_error_regression {
    use super::*;
    use crate::source_native_cli::private_assessment::{
        OperationBudget, original_payer::with_original_pwe_session,
    };
    use std::{sync::atomic::{AtomicBool, Ordering}, time::{Duration, Instant}};
    use tos_compiler::pwe_native::Limits;

    fn limits(deadline: Instant) -> Limits {
        Limits {
            max_input_bytes: 1_048_576, max_nodes: 65_536, max_work: 262_144,
            max_refs: 4096, max_depth: 96, max_items: 4096,
            max_state_bytes: 1_048_576, deadline,
        }
    }

    #[test]
    fn prepare_target_content_preserves_original_payer_and_cancellation_errors() {
        for lane in 0..4 {
            let deadline = Instant::now() + Duration::from_secs(30);
            let cancelled = AtomicBool::new(false);
            let mut original = OperationBudget::new(&serde_json::json!({"budgets": {
                "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
            }}), deadline).unwrap();
            let mut selected = limits(deadline);
            if lane == 3 { selected.max_work = 1; }
            let object = if lane == 3 { Value::String("x".into()) } else { Value::Null };
            let result = with_original_pwe_session(&mut original, selected, &cancelled, |payer, session| {
                if lane == 0 {
                    payer.with_operation(|operation| {
                        let work = operation.limits()?.max_work;
                        operation.lower_remaining(1, work)
                    })?;
                }
                if lane == 1 { cancelled.store(true, Ordering::Relaxed); }
                let before = session.charges();
                // This is the production prepare_target content-stage call,
                // not a codec or a fabricated native issuer/assessment.
                let body = prepare_target_content(session, payer, &object, &Value::Null)
                    .map(|_| ());
                if lane == 0 {
                    assert!(matches!(&body, Err(SourceCommandError::Invalid(
                        "assessment cumulative byte budget"
                    ))));
                    assert!(session.charges().2 > before.2);
                }
                body // actual Common finish must preserve this original Err
            });
            match lane {
                0 => assert!(matches!(result, Err(SourceCommandError::Invalid(
                    "assessment cumulative byte budget"
                )))),
                1 => assert!(matches!(result, Err(SourceCommandError::Denied(
                    "PWE Assessment Session cancelled"
                )))),
                2 => {
                    assert!(matches!(result, Err(SourceCommandError::Invalid(
                        "PWE selected account value shape"
                    ))));
                    assert!(original.limits().is_ok()); // genuine malformed, no resource refusal
                }
                _ => assert!(matches!(result, Err(SourceCommandError::Unsupported(
                    "PWE Assessment Session exhausted"
                )))),
            }
        }
    }

    #[test]
    fn prepare_target_later_session_boundaries_keep_original_and_semantic_errors() {
        // Existing unbound construction value: no registration, source grant,
        // historical fact, native account or successful evaluator is invented.
        let candidate: Value = serde_json::from_str(include_str!(
            "../../../tos-compiler/src/pwe_native/fixtures/prepared-pwe-values.json"
        )).unwrap();
        let object = &candidate["body"]["values"][0];
        for stage in 0..8 {
            let deadline = Instant::now() + Duration::from_secs(30);
            let cancelled = AtomicBool::new(false);
            let mut original = OperationBudget::new(&serde_json::json!({"budgets": {
                "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
            }}), deadline).unwrap();
            let result = with_original_pwe_session(&mut original, limits(deadline), &cancelled, |payer, session| {
                let prepared = host_session_result(session.prepare(object), payer,
                    invalid("PWE selected account value shape"))?;
                if stage < 5 {
                    payer.with_operation(|operation| {
                        let work = operation.limits()?.max_work;
                        operation.lower_remaining(1, work)
                    })?;
                }
                let before = session.charges();
                let body = match stage {
                    0 => host_session_result(session.check_claim_content(&prepared, &Value::Null),
                        payer, invalid("PWE source wrapper/content mismatch")),
                    1 => host_session_result(session.check_endpoints(&prepared),
                        payer, invalid("PWE account endpoints/member closure")),
                    2 | 6 => host_session_result(session.exact_basis(&prepared), payer,
                        unsupported("PWE exact basis requires all declared bindings")).map(|_| ()),
                    3 | 7 => host_session_result(session.check_role_matrix(&prepared, &[]), payer,
                        unsupported("PWE actual metadata responsibility/identity role binding differs")),
                    _ => host_session_result(session.qualify(prepared), payer,
                        unsupported("PWE account qualification is unresolved")).map(|_| ()),
                };
                if stage < 5 {
                    assert!(matches!(&body, Err(SourceCommandError::Invalid(
                        "assessment cumulative byte budget"
                    ))));
                    assert!(session.charges().2 > before.2);
                }
                body
            });
            if stage < 5 {
                assert!(matches!(result, Err(SourceCommandError::Invalid(
                    "assessment cumulative byte budget"
                ))));
            } else {
                let expected = match stage {
                    5 => "PWE account qualification is unresolved",
                    6 => "PWE exact basis requires all declared bindings",
                    _ => "PWE actual metadata responsibility/identity role binding differs",
                };
                assert!(matches!(result, Err(SourceCommandError::Unsupported(reason)) if reason == expected));
                assert!(original.limits().is_ok());
            }
        }
    }
}
