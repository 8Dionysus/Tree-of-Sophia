//! Public-operand PWE application checks on the actual retained PG owner.
//!
//! This scoped result is pre-admission evidence. It neither implements
//! ConsumerOwner nor creates NativeAccount/ApplicationCheckedTicket. In
//! particular, a mechanics report has no protected-owner token: matching its
//! public coordinates cannot authorize a Claim or substitute for dispatch,
//! complete exact-basis/metadata closure, private policy or parent admission.
use super::access::BorrowedAccess;
use super::owner::host_session_result;
use super::source_budget::{SourceLedger, assessment_canonical};
use super::{invalid, unsupported};
use crate::source_command::{SourceCommandError, SourceCommandResult};
use crate::source_native_roles::{
    CurrentPgRoleOwner, NativePgRoleDocument, NativeRole, PgRoleSchemaAccess,
};
use crate::{DurableError, DurableResult};
use serde_json::Value;
use std::mem::{size_of, size_of_val};
use tos_compiler::pwe_native::{Kind, QualifiedAccount};
use tos_foundation::{Digest256, ExactRecordRef, JsonValue};
use tos_validation::assessment::{AssessmentMechanicsReport, AssessmentSchemaBindingV1};

/// Borrowed native documents, not an issuer interface. Each record has the
/// Common-owned private constructor; every use rechecks current permission,
/// epoch and unique metadata ownership. Combining documents grants nothing.
#[derive(Clone, Copy)]
pub(crate) struct PgPublicRoles<'a> {
    pub(crate) signature: &'a NativePgRoleDocument,
    pub(crate) predicate: &'a NativePgRoleDocument,
    pub(crate) qualification_profile: &'a NativePgRoleDocument,
    pub(crate) context: &'a NativePgRoleDocument,
    pub(crate) scope: &'a NativePgRoleDocument,
    pub(crate) criterion: Option<&'a NativePgRoleDocument>,
}

/// Cannot escape the scoped original Claim conversion. There is deliberately
/// no public constructor, report factory, can_use accessor, native-parent
/// conversion or authority trait implementation. The actual original owner
/// remains exclusively loaned for this complete scope; metadata equality cannot
/// replace that owner, host, worker, payer or cumulative Session.
pub(crate) struct PgPublicApplication<'scope, 'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure> {
    qualified: QualifiedAccount<'scope>,
    claim: &'scope Value,
    exact: &'scope ExactRecordRef,
    roles: PgPublicRoles<'scope>,
    // The actual mutable owner loan is retained, not an address/configuration
    // stamp. It transitively holds the same private host, worker, payer and
    // cumulative Session; no accessor or replacement surface is provided.
    current: &'scope mut CurrentPgRoleOwner<'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure>,
}
impl PgPublicApplication<'_, '_, '_, '_, '_, '_, '_, '_> {
    pub(crate) fn qualified(&self) -> &QualifiedAccount<'_> { &self.qualified }
    pub(crate) fn claim(&self) -> &Value { self.claim }
    pub(crate) fn exact(&self) -> &ExactRecordRef { self.exact }
}

fn source<T>(result: SourceCommandResult<T>) -> DurableResult<T> {
    result.map_err(DurableError::Source)
}
fn finish<T>(body: DurableResult<T>, exit: DurableResult<()>) -> DurableResult<T> {
    match (body, exit) {
        (Err(original), _) | (Ok(_), Err(original)) => Err(original),
        (Ok(value), Ok(())) => Ok(value),
    }
}
fn mismatch() -> SourceCommandError {
    SourceCommandError::Conflict("PWE PG public application binding differs")
}

impl PgPublicRoles<'_> {
    fn current(&self, current: &mut CurrentPgRoleOwner<'_, '_, '_, '_, '_, '_, '_>) -> DurableResult<()> {
        current.check_current()?;
        // Pay the fixed borrowed list/result carrier before its construction.
        current.with_source_epoch(|_, payer, same| {
            let mut ledger = source(SourceLedger::new(same, payer))?;
            source(ledger.reserve(
                6 * size_of::<Option<&NativePgRoleDocument>>() + size_of::<DurableResult<()>>(),
            ))
        })?;
        for document in [Some(self.signature), Some(self.predicate),
            Some(self.qualification_profile), Some(self.context), Some(self.scope), self.criterion] {
            if let Some(document) = document {
                document.record().check_current_use(current)?;
            }
        }
        Ok(())
    }
}

/// The public application stage, using the SAME original Session and closed
/// worker/epoch owner. `claim_exact` is a required operand, not a registration
/// or source-Claim authority token. Native source-target/schema/dispatch and
/// metadata-basis admission MUST precede any eventual application ticket.
/// The higher-ranked callback prevents retaining the converted Claim/account
/// or original mutable owner loan. No parallel raw owner argument is returned.
#[allow(clippy::too_many_arguments)]
pub(crate) fn with_pg_public_application<'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure, T>(
    current: &mut CurrentPgRoleOwner<'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure>,
    native_claim: &JsonValue,
    claim_exact: &ExactRecordRef,
    roles: PgPublicRoles<'_>,
    run: impl for<'claim> FnOnce(
        PgPublicApplication<'claim, 'call, 'owner, 'view, 'store, 'budget, 'cancel, 'pure>,
    ) -> DurableResult<T>,
) -> DurableResult<T> {
    // Added logical activation/result/callback and hash descriptors are paid
    // before constructing this adapter's body. This is no RSS/driver bound.
    current.with_source_epoch(|_, payer, same| {
        let fixed = size_of::<PgPublicApplication<'_, '_, '_, '_, '_, '_, '_, '_>>()
            .checked_add(3 * size_of::<DurableResult<T>>())
            .and_then(|n| n.checked_add(size_of_val(&run)))
            .and_then(|n| n.checked_add(2048))
            .ok_or(DurableError::Source(unsupported("PWE PG application activation")))?;
        let mut ledger = source(SourceLedger::new(same, payer))?;
        source(ledger.reserve(fixed))?;
        source(ledger.inspect(0, 32))
    })?;
    let body = (|| {
        roles.current(current)?;
        let claim = current.with_source_epoch(|_, payer, same| {
            let raw = source(assessment_canonical(native_claim, same, payer))?;
            let mut ledger = source(SourceLedger::new(same, payer))?;
            // One raw operand is reused for hashing and conversion; both
            // passes and all owned conversion backing are paid prospectively.
            source(ledger.inspect(raw.len(), 1))?;
            let digest = Digest256::of_bytes(&raw);
            source(ledger.inspect(raw.len(), 1))?;
            let holder = raw.len().checked_mul(64).and_then(|n| n.checked_add(256))
                .ok_or(DurableError::Source(unsupported("PWE PG Claim conversion holder")))?;
            source(ledger.reserve(holder))?;
            let claim: Value = serde_json::from_slice(&raw)
                .map_err(|_| DurableError::Source(invalid("PWE PG Claim conversion")))?;
            let id = source(ledger.field(&claim, "claim_id"))?.as_str()
                .ok_or(DurableError::Source(invalid("PWE PG Claim ID")))?;
            let version = source(ledger.field(&claim, "claim_version"))?.as_u64();
            if !source(ledger.text_equal(id, claim_exact.id.as_str()))?
                || version != Some(claim_exact.version.get()) || digest != claim_exact.digest {
                return Err(DurableError::Source(mismatch()));
            }
            source(ledger.reserve(size_of::<PgPublicApplication<'_, '_, '_, '_, '_, '_, '_, '_>>() + size_of::<DurableResult<T>>()))?;
            Ok(claim)
        })?;
        let qualified = current.with_source_epoch(|_, payer, same| {
            let object = {
                let mut ledger = source(SourceLedger::new(same, payer))?;
                source(ledger.field(&claim, "object"))?
            };
            let prepared = source(host_session_result(same.prepare(object), payer,
                invalid("PWE PG account value shape")))?;
            source(host_session_result(same.check_claim_content(&prepared, &claim), payer,
                invalid("PWE PG source wrapper/content mismatch")))?;
            // check_claim_content includes the maintained endpoint law.
            let kind = prepared.kind();
            let account = source(host_session_result(same.qualify(prepared), payer,
                unsupported("PWE PG account qualification unresolved")))?;
            let mut ledger = source(SourceLedger::new(same, payer))?;
            source(check_public_bindings(&mut ledger, &account, kind, roles))?;
            Ok(account)
        })?;
        run(PgPublicApplication { qualified, claim: &claim, exact: claim_exact, roles, current: &mut *current })
    })();
    // Attempt paid exit on Ok AND Err, preserving the actual body error.
    let exit = roles.current(current);
    finish(body, exit)
}

fn typed_role<P: super::source_budget::OriginalPayer>(
    ledger: &mut SourceLedger<'_, '_, P>, reference: &Value,
    actual: &NativePgRoleDocument, role: NativeRole, registry: &NativePgRoleDocument,
) -> SourceCommandResult<()> {
    ledger.inspect(0, 1)?;
    if actual.record().role() != role
        || !ledger.text_equal(actual.record().role_registry_path(), super::registry::BINDINGS)?
        || !ledger.exact_equal(actual.record().role_registry(), registry.record().role_registry())?
        || !ledger.exact_equal(actual.record().kind_binding(), registry.record().kind_binding())? {
        return Err(mismatch());
    }
    if reference.as_object().map(|v| v.len()) != Some(2) { return Err(mismatch()); }
    let record = ledger.field(reference, "record")?;
    let kind = ledger.field(reference, "kind_binding")?;
    let record = ledger.exact(record)?;
    let kind = ledger.exact(kind)?;
    if !ledger.exact_equal(&record, actual.record().exact())?
        || !ledger.exact_equal(&kind, actual.record().kind_binding())? { return Err(mismatch()); }
    Ok(())
}

fn arguments(kind: Kind) -> &'static [&'static str] {
    // Fixed accepted value grammar; this is NOT a fabricated DispatchRow.
    match kind {
        Kind::Name => &["subject", "subject_role", "assignment", "name", "name_function", "named_bearer"],
        Kind::Responsibility => &["subject", "role", "attribution_mode", "assignees", "responsibility_scope", "role_basis"],
        Kind::Identity => &["left", "right", "identity_level", "criterion", "asserted_outcome"],
    }
}
fn check_public_bindings<P: super::source_budget::OriginalPayer>(
    ledger: &mut SourceLedger<'_, '_, P>, account: &QualifiedAccount<'_>, kind: Kind,
    roles: PgPublicRoles<'_>,
) -> SourceCommandResult<()> {
    ledger.reserve(5 * size_of::<(&Value, &NativePgRoleDocument, NativeRole)>())?;
    for (wire, document, role) in [
        (account.signature(), roles.signature, NativeRole::Signature),
        (account.predicate(), roles.predicate, NativeRole::Predicate),
        (account.qualification_profile(), roles.qualification_profile, NativeRole::QualificationProfile),
        (account.context(), roles.context, NativeRole::Context),
        (account.scope(), roles.scope, NativeRole::Scope),
    ] { typed_role(ledger, wire, document, role, roles.signature)?; }
    if kind == Kind::Identity {
        let wire = ledger.pointer(account.value(), "/criterion/value")?.ok_or(mismatch())?;
        let document = roles.criterion.ok_or(unsupported("PWE PG actual identity criterion absent"))?;
        typed_role(ledger, wire, document, NativeRole::CriterionPolicy, roles.signature)?;
    } else if roles.criterion.is_some() {
        return Err(mismatch()); // no unused policy operand silently accepted
    }
    let signature = ledger.domain_value(roles.signature.document().root())?;
    let predicate = ledger.domain_value(roles.predicate.document().root())?;
    let profile = ledger.domain_value(roles.qualification_profile.document().root())?;
    let predicates = ledger.pointer(&signature, "/body/predicates")?
        .and_then(Value::as_array).ok_or(mismatch())?;
    if !ledger.text_is(&signature, "schema_version", "tos_pwe_signature_v1")?
        || !ledger.text_is(&predicate, "schema_version", "tos_pwe_predicate_v1")?
        || !ledger.text_is(&profile, "schema_version", "tos_pwe_qualification_profile_v1")?
        || !ledger.contains(predicates, account.predicate())? { return Err(mismatch()); }
    let value_kind = ledger.get(account.value(), "kind")?;
    let predicate_kind = ledger.pointer(&predicate, "/body/value_kind")?;
    if !ledger.optional_equal(predicate_kind, value_kind)? { return Err(mismatch()); }
    let actual = ledger.pointer(&predicate, "/body/arguments")?
        .and_then(Value::as_array).ok_or(mismatch())?;
    ledger.inspect(0, 1)?;
    if actual.len() != arguments(kind).len() { return Err(mismatch()); }
    for (actual, expected) in actual.iter().zip(arguments(kind)) {
        let actual = actual.as_str().ok_or(mismatch())?;
        if !ledger.text_equal(actual, expected)? { return Err(mismatch()); }
        ledger.field(account.value(), expected)?;
    }
    for (path, expected) in [("/body/signature_ref", account.signature()),
        ("/body/predicate_ref", account.predicate())] {
        let actual = ledger.pointer(&profile, path)?.ok_or(mismatch())?;
        if !ledger.equal(actual, expected)? { return Err(mismatch()); }
    }
    let actual = ledger.pointer(&profile, "/body/modality")?.ok_or(mismatch())?;
    let expected = ledger.pointer(account.value(), "/qualification/modality")?.ok_or(mismatch())?;
    if !ledger.equal(actual, expected)? { return Err(mismatch()); }
    Ok(())
}

/// These coordinates are comparison operands, never a source of permission.
/// Content scope/context are kept in QualifiedAccount, separately from this
/// actual requested access scope and policy. The caller must bind these to its
/// real protected plan before any final admission route can consume a result.
pub(crate) struct PgReportCoordinates<'a> {
    pub(crate) access_scope: &'a Value,
    pub(crate) assessment_policy: &'a Value,
    pub(crate) observed_now: &'a str,
    /// Actual evaluator input pin selected by the protected plan, NOT the
    /// Claim digest or a self-declared source/coverage manifest digest.
    pub(crate) evaluator_input_sha256: Digest256,
}
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PgReportComparison {
    /// A mechanical coordinate match ONLY, never authorization or NativeAccount.
    MatchingMechanics,
    NegativeMechanics,
}

impl PgPublicApplication<'_, '_, '_, '_, '_, '_, '_, '_> {
    pub(crate) fn compare_native_report(
        &mut self, report: &AssessmentMechanicsReport, coordinates: PgReportCoordinates<'_>,
    ) -> DurableResult<PgReportComparison> {
        // No caller-selected current: every operation reborrows the private
        // original loan, including sequential schema access and Err exits.
        let roles = self.roles;
        let exact = self.exact;
        let current = &mut *self.current;
        current.with_source_epoch(|_, payer, same| {
            let mut ledger = source(SourceLedger::new(same, payer))?;
            source(ledger.reserve(2048 + 3 * size_of::<DurableResult<PgReportComparison>>()
                + size_of::<PgReportCoordinates<'_>>()))?;
            source(ledger.inspect(0, 32))
        })?;
        let body = (|| {
            roles.current(current)?;
            // Sequential closed schema access; no reentrant owner/worker loan.
            let (_, worker) = current.schema_validation_binding()?;
            current.with_source_epoch(|epoch, payer, same| {
                source(same.admit_source_work(report.work_used()).map_err(|e| payer.error(e)))?;
                let mut ledger = source(SourceLedger::new(same, payer))?;
                source(ledger.reserve(size_of::<PgReportComparison>() + size_of::<AssessmentSchemaBindingV1>()))?;
                // PgRecordRevisionIdentity::digest hashes only fixed fields.
                // sizeof includes all fields/padding; 64 pays the fixed domain
                // prefix and finalization margin. No filesystem revision alias.
                source(ledger.inspect(0, size_of::<crate::source_cohort::revision::PgRecordRevisionIdentity>() + 64))?;
                let identity = current_digest(epoch, payer)?;
                source(ledger.inspect(0, 2 * size_of::<AssessmentSchemaBindingV1>() + 128))?;
                if report.source_revision().is_some() || report.source_input_identity() != identity
                    || report.input_sha256() != coordinates.evaluator_input_sha256
                    || report.schema_binding() != (AssessmentSchemaBindingV1::SelectedInput {
                        source_identity: identity, worker,
                    }) || !source(ledger.text_equal(report.observed_now(), coordinates.observed_now))? {
                    return Err(DurableError::Source(mismatch()));
                }
                source(compare_report_fields(&mut ledger, exact, report, &coordinates))
            })
        })();
        let exit = roles.current(current);
        finish(body, exit)
    }
}

#[cfg(test)]
#[path = "pg_application_cases.rs"]
mod actual_cases;

// Actual-fixture consumers only. Common registers these in its cfg(test)
// fixture composition; production exports/authority and the v26 body stay fixed.
#[cfg(test)]
pub(crate) use actual_cases::{
    fixture_body_error_and_cancelled_exit, fixture_cancelled_comparison,
    fixture_cumulative_comparison_twice, fixture_scope_exit_after_original_work_drain,
    OriginalWorkDrainPhase,
};

fn current_digest(
    epoch: &mut crate::source_cohort::revision::SourceRevisionReadSession<'_>,
    payer: &crate::source_native_cli::private_assessment::original_payer::OriginalAssessmentPayer<'_, '_>,
) -> DurableResult<Digest256> {
    let (deadline, cancelled) = payer.source_cutoff();
    let identity = source(payer.with_operation(|budget| {
        epoch.selection().identity_with_data_admission(budget, deadline, cancelled)
    }))?;
    Ok(identity.digest())
}
fn compare_report_fields<P: super::source_budget::OriginalPayer>(
    ledger: &mut SourceLedger<'_, '_, P>, exact: &ExactRecordRef,
    report: &AssessmentMechanicsReport, coordinates: &PgReportCoordinates<'_>,
) -> SourceCommandResult<PgReportComparison> {
    if !ledger.equal(report.scope(), coordinates.access_scope)? { return Err(mismatch()); }
    let admission = report.current_admission();
    let subject = ledger.field(admission, "subject")?;
    let subject = ledger.exact(subject)?;
    if !ledger.exact_equal(&subject, exact)? { return Err(mismatch()); }
    let actual = ledger.field(admission, "policy")?;
    if !ledger.equal(actual, coordinates.assessment_policy)? { return Err(mismatch()); }
    let actual = ledger.field(admission, "use")?;
    let requested = ledger.field(coordinates.access_scope, "requested_use")?;
    if !ledger.equal(actual, requested)? { return Err(mismatch()); }
    ledger.inspect(0, 4)?;
    if report.source_read_required() && !report.source_read_ready() {
        return Ok(PgReportComparison::NegativeMechanics);
    }
    let can_use = ledger.field(admission, "can_use")?.as_bool()
        .ok_or(invalid("PWE native admission can_use"))?;
    let status = ledger.field(admission, "status")?.as_str()
        .ok_or(invalid("PWE native admission status"))?;
    if can_use && ledger.text_equal(status, "admitted-with-limits")? {
        return Err(unsupported("PWE current admission limits interpreter absent"));
    }
    if can_use && ledger.text_equal(status, "admitted")? {
        Ok(PgReportComparison::MatchingMechanics)
    } else { Ok(PgReportComparison::NegativeMechanics) }
}
