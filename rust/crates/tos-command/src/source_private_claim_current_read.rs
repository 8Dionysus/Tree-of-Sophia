//! Current protected Assessment read scope, independent of historical Claim
//! producer whitelists. This factory accepts no historical Grant/history JSON
//! and cannot issue a mutation, admission, role or external content witness.
use super::{CLAIM_STREAM, claim_id};
use crate::source_assessment_journal::ProtectedAssessmentPgGrant;
use crate::source_command::{
    self as cmd, SourceCommandError as Error, SourceCommandResult as Result,
};
use crate::source_native_cli::private_assessment::original_payer::OriginalAssessmentPayer;
use crate::source_native_cli::private_assessment::scope_admission::ScopeAccess;
use crate::source_text_owner::OwnerTextContext;
use std::mem::size_of;
use tos_compiler::pwe_native::Session;
use tos_foundation::JsonValue;

/// Constructed only from the LIVE protected Assessment configuration. It is
/// deliberately not ClaimPackageReadPolicy: current read access is a different
/// obligation from historical producer allowance/creation/history integrity.
/// This borrowed token is scoped to a callback, with no Clone/owned constructor.
pub(crate) struct CurrentClaimReadScope<'read> {
    row: &'read JsonValue,
    path: &'read str,
    claim: &'read str,
    content_requested: bool,
}
impl CurrentClaimReadScope<'_> {
    pub(crate) fn path(&self) -> &str {
        self.path
    }
    pub(crate) fn claim_id(&self) -> &str {
        self.claim
    }
    pub(crate) fn assessment_selection(&self) -> &JsonValue {
        self.row
    }
    /// Necessary current selection only, not sufficient content provenance,
    /// custody/rights/identity/native binding or qualified semantic authority.
    pub(crate) fn content_requested(&self) -> bool {
        self.content_requested
    }
}

fn overflow() -> Error {
    Error::Unsupported("current Claim read-scope work overflow")
}

fn selected_scope<'read>(
    access: &mut ScopeAccess<'_>,
    config: &'read JsonValue,
    owner_uid: u32,
    owner_context_path: &str,
    private_prefix: &str,
    requested_path: &str,
    requested_claim: &str,
    content_requested: bool,
) -> Result<CurrentClaimReadScope<'read>> {
    let uid = access.field(config, "uid")?;
    if let JsonValue::Number(number) = uid {
        access.inspect(number.lexeme.len(), 1)?;
    }
    if uid.as_u64() != Some(u64::from(owner_uid)) {
        return Err(Error::Denied("current Claim read account differs"));
    }
    let context = access.text(config, "source_context_ref")?;
    if !access.text_equal(context, owner_context_path)? {
        return Err(Error::Denied("current Claim read context differs"));
    }
    let rows = access
        .get(config, "owner_local_source_claims")?
        .and_then(JsonValue::as_array)
        .ok_or(Error::Denied(
            "current Assessment has no Claim read selection",
        ))?;
    if rows.len() > 32 {
        return Err(Error::Invalid("current Claim read selection count"));
    }
    let mut selected = None;
    for row in rows {
        // Same exact maintained selector fields. Full endpoint/profile/form
        // qualification still belongs to preflight_assessment_claim_selections
        // and the actual Claim resolver; this token does not bypass either.
        access.exact_keys(
            row,
            &[
                "path",
                "claim_id",
                "relation_type_id",
                "origin_id",
                "source_access",
                "source_records",
                "native_bindings",
                "verify_content",
                "form_ids",
            ],
        )?;
        let claim = access.text(row, "claim_id")?;
        if !access.text_equal(claim, requested_claim)? {
            continue;
        }
        if selected.is_some() {
            return Err(Error::Denied("current Claim read identity is ambiguous"));
        }
        let path = access.text(row, "path")?;
        if !access.text_equal(path, requested_path)? {
            return Err(Error::Denied("current Claim read path differs"));
        }
        let path_work = path
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(private_prefix.len()))
            .and_then(|n| n.checked_add(claim.len()))
            .ok_or_else(overflow)?;
        access.inspect(path_work, 1)?;
        if !claim_id(claim) {
            return Err(Error::Invalid("current Claim read identity"));
        }
        // Same named-package grammar as owner_path_claim_stream, using the
        // actual typed owner's prefix and no allocated Vec/format/string copy.
        let tail = path
            .strip_prefix(private_prefix)
            .ok_or(Error::Denied("current Claim read leaves owner prefix"))?;
        let mut parts = tail.split('/');
        let family = parts.next();
        let slug = parts.next().unwrap_or("");
        let stream = parts.next();
        if family != Some("claims")
            || stream != Some(CLAIM_STREAM)
            || parts.next().is_some()
            || slug.is_empty()
            || slug.split(['.', '-']).any(|part| {
                part.is_empty()
                    || !part
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            })
        {
            return Err(Error::Denied(
                "current Claim read is not a named owner package",
            ));
        }
        let source_access = access.field(row, "source_access")?;
        access.exact_keys(
            source_access,
            &["read_scope", "access_allowed", "authority_ref"],
        )?;
        let scope = access.text(source_access, "read_scope")?;
        let metadata = access.text_equal(scope, "metadata_only")?;
        let exact = access.text_equal(scope, "exact_owner_local")?;
        let allowed = access.field(source_access, "access_allowed")?;
        let authority = access.text(source_access, "authority_ref")?;
        access.inspect(authority.len().checked_mul(2).ok_or_else(overflow)?, 1)?;
        if (!metadata && !exact) || allowed.as_bool() != Some(true) || !cmd::nonblank(authority) {
            return Err(Error::Denied(
                "current Claim read source access is not explicit",
            ));
        }
        let verify_content = access
            .field(row, "verify_content")?
            .as_bool()
            .ok_or(Error::Invalid("current Claim read content mode"))?;
        if verify_content && !exact || content_requested && (!verify_content || !exact) {
            return Err(Error::Denied(
                "current Claim content is outside selected read scope",
            ));
        }
        selected = Some(CurrentClaimReadScope {
            row,
            path,
            claim,
            content_requested,
        });
    }
    selected.ok_or(Error::Denied(
        "Claim is not selected by the current Assessment",
    ))
}

/// A real current permission boundary, not a semantic Claim verifier. Actual
/// protected grant and typed owner are reread before/after the callback on the
/// original Session. The callback still needs genuine producer/read-policy,
/// retained semantic kernels, actual PG role/source/worker pins and funding.
/// No whole mutable operation-budget loan crosses that callback.
#[allow(clippy::too_many_arguments)]
pub(crate) fn with_current_pg_claim_read_scope<'pure, T>(
    payer: &OriginalAssessmentPayer<'_, '_>,
    same: &mut Session<'pure>,
    grant: &ProtectedAssessmentPgGrant,
    owner: &OwnerTextContext,
    path: &str,
    claim: &str,
    content_requested: bool,
    run: impl for<'read> FnOnce(CurrentClaimReadScope<'read>, &'read mut Session<'pure>) -> Result<T>,
) -> Result<T> {
    with_current_pg_claim_read_scope_using(
        payer,
        same,
        grant,
        owner,
        path,
        claim,
        content_requested,
        false,
        run,
    )
}

/// Same actual selector/permission/fences, using the already maintained exact
/// physical request profile. This grants no new source/form/content scope.
#[allow(clippy::too_many_arguments)]
pub(crate) fn with_current_pg_claim_read_scope_exact_source<'pure, T>(
    payer: &OriginalAssessmentPayer<'_, '_>,
    same: &mut Session<'pure>,
    grant: &ProtectedAssessmentPgGrant,
    owner: &OwnerTextContext,
    path: &str,
    claim: &str,
    content_requested: bool,
    run: impl for<'read> FnOnce(CurrentClaimReadScope<'read>, &'read mut Session<'pure>) -> Result<T>,
) -> Result<T> {
    with_current_pg_claim_read_scope_using(
        payer,
        same,
        grant,
        owner,
        path,
        claim,
        content_requested,
        true,
        run,
    )
}

#[allow(clippy::too_many_arguments)]
fn with_current_pg_claim_read_scope_using<'pure, T>(
    payer: &OriginalAssessmentPayer<'_, '_>,
    same: &mut Session<'pure>,
    grant: &ProtectedAssessmentPgGrant,
    owner: &OwnerTextContext,
    path: &str,
    claim: &str,
    content_requested: bool,
    exact_requests: bool,
    run: impl for<'read> FnOnce(CurrentClaimReadScope<'read>, &'read mut Session<'pure>) -> Result<T>,
) -> Result<T> {
    payer.charge_sto_source_measures(same, 0, 0, 0, 0, 0)?;
    let carriers = size_of::<CurrentClaimReadScope<'_>>()
        .checked_add(size_of::<Result<CurrentClaimReadScope<'_>>>())
        .and_then(|n| n.checked_add(size_of::<Result<T>>()))
        .and_then(|n| n.checked_add(3 * size_of::<Result<()>>()))
        .and_then(|n| n.checked_add(std::mem::size_of_val(&run)))
        .and_then(|n| n.checked_add(size_of::<bool>()))
        .ok_or_else(overflow)?;
    payer.reserve(carriers)?;
    if exact_requests {
        payer.recheck_current_pg_grant_exact_source(same, grant)?;
        payer.recheck_owner_context_exact_source(same, owner)?;
    } else {
        payer.recheck_current_pg_grant_source(same, grant)?;
        payer.recheck_owner_context_source(same, owner)?;
    }
    let (deadline, cancelled) = payer.source_cutoff();
    let result = (|| {
        let selected = payer.with_operation(|operation| {
            let mut access = ScopeAccess::admitted_operation(operation, deadline, cancelled);
            // Price path UTF-8 validation before Path::to_str. The retained typed
            // owner path, never caller-provided context JSON, supplies this side.
            access.inspect(
                owner
                    .configuration_path()
                    .as_os_str()
                    .as_encoded_bytes()
                    .len(),
                1,
            )?;
            let context_path = owner
                .configuration_path()
                .to_str()
                .ok_or(Error::Invalid("current owner context path UTF-8"))?;
            selected_scope(
                &mut access,
                grant.configuration(),
                owner.account_uid(),
                context_path,
                owner.private_prefix(),
                path,
                claim,
                content_requested,
            )
        })?;
        run(selected, same)
    })();
    // Attempt BOTH actual protected exits even after body Err; sticky or
    // cancellation may deny physical I/O. Never claim unconditional I/O on Err.
    let grant_exit = if exact_requests {
        payer.recheck_current_pg_grant_exact_source(same, grant)
    } else {
        payer.recheck_current_pg_grant_source(same, grant)
    };
    let owner_exit = if exact_requests {
        payer.recheck_owner_context_exact_source(same, owner)
    } else {
        payer.recheck_owner_context_source(same, owner)
    };
    let cutoff = payer.charge_sto_source_measures(same, 0, 0, 0, 0, 0);
    match (result, grant_exit, owner_exit, cutoff) {
        (Err(original), _, _, _)
        | (Ok(_), Err(original), _, _)
        | (Ok(_), Ok(()), Err(original), _)
        | (Ok(_), Ok(()), Ok(()), Err(original)) => Err(original),
        (Ok(value), Ok(()), Ok(()), Ok(())) => Ok(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_native_cli::private_assessment::OperationBudget;
    use serde_json::{Value, json};
    use std::{
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };

    const PREFIX: &str = "ToS/source-witnesses/owner-local/selected/";
    const PATH: &str =
        "ToS/source-witnesses/owner-local/selected/claims/context/source-claims.jsonl";
    const CLAIM: &str = "tos.claim.selected";
    fn row(scope: &str, verify: bool) -> Value {
        json!({
            "path":PATH,"claim_id":CLAIM,"relation_type_id":"tos.relation.fixture",
            "origin_id":"tos.origin.fixture",
            "source_access":{"read_scope":scope,"access_allowed":true,"authority_ref":"owner:fixture"},
            "source_records":[],"native_bindings":[],"verify_content":verify,"form_ids":[]
        })
    }
    // Partial configuration operands for the PRIVATE pure selector only.
    // These are not a ProtectedAssessmentPgGrant or an issuer fixture, and
    // cannot enter the public factory's actual physical grant/context guards.
    fn config(rows: Vec<Value>) -> JsonValue {
        cmd::parse(
            &serde_json::to_vec(&json!({
                "uid":17,"source_context_ref":"/selected/context.json",
                "owner_local_source_claims":rows
            }))
            .unwrap(),
        )
        .unwrap()
    }
    fn check(config: &JsonValue, requested_content: bool) -> Result<(bool, String)> {
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut budget = OperationBudget::new(
            &json!({"budgets":{
                "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
            }}),
            deadline,
        )
        .unwrap();
        let mut access = ScopeAccess::admitted_operation(&mut budget, deadline, &cancelled);
        let selected = selected_scope(
            &mut access,
            config,
            17,
            "/selected/context.json",
            PREFIX,
            PATH,
            CLAIM,
            requested_content,
        )?;
        Ok((selected.content_requested(), selected.path().to_owned()))
    }
    fn producer_policy(config: &JsonValue, declared: &JsonValue) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancel = AtomicBool::new(false);
        let mut original = OperationBudget::new(
            &json!({"budgets":{
                "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
            }}),
            deadline,
        )?;
        let mut access = ScopeAccess::admitted_operation(&mut original, deadline, &cancel);
        let selected = selected_scope(
            &mut access,
            config,
            17,
            "/selected/context.json",
            PREFIX,
            PATH,
            CLAIM,
            false,
        )?;
        crate::source_native_cli::private_owner::claim::validate_pg_claim_producer_policy(
            declared,
            &selected,
            &mut access,
        )
    }
    fn declared(claims: Value, forms: Value) -> JsonValue {
        cmd::parse(&serde_json::to_vec(&json!({
            "schema_version":"tos_local_source_command_result_v1", "authentication":"local-unix-account",
            "visibility":"local_only", "source_path":PATH, "target_exists":true,
            "publication_authorized":false,"grants_admission":false,
            "owner_configuration":format!("sha256:{}", "1".repeat(64)),
            "allowed_claim_ids":claims,"allowed_form_ids":forms
        })).unwrap()).unwrap()
    }
    #[test]
    fn producer_policy_predicate_keeps_historical_declaration_separate_from_current_scope() {
        // Private predicate operands only: no native producer/owner factory.
        let current = config(vec![row("metadata_only", false)]);
        assert!(producer_policy(&current, &declared(json!([CLAIM]), json!([]))).is_ok());
        assert!(matches!(
            producer_policy(&current, &declared(json!(["tos.claim.other"]), json!([]))),
            Err(Error::Denied(
                "selected Claim is outside producer declaration"
            ))
        ));
        assert!(matches!(
            producer_policy(&current, &declared(json!([CLAIM, CLAIM]), json!([]))),
            Err(Error::Invalid("Claim producer repeats declared identity"))
        ));
    }
    #[test]
    fn producer_policy_predicate_rejects_admission_posture_and_foreign_source() {
        let current = config(vec![row("metadata_only", false)]);
        let good = declared(json!([CLAIM]), json!([]));
        for key in ["publication_authorized", "grants_admission"] {
            let mut bad = good.clone();
            cmd::set(&mut bad, key, JsonValue::Bool(true)).unwrap();
            assert!(producer_policy(&current, &bad).is_err());
        }
        let mut bad = good;
        cmd::set(
            &mut bad,
            "source_path",
            cmd::string("foreign/source-claims.jsonl"),
        )
        .unwrap();
        assert!(matches!(
            producer_policy(&current, &bad),
            Err(Error::Conflict(
                "Claim producer read-policy protocol differs"
            ))
        ));
    }
    #[test]
    fn producer_policy_predicate_requires_actual_selected_form_in_independent_declaration() {
        let mut selected = row("metadata_only", false);
        selected["form_ids"] = json!(["tos.form.selected"]);
        let current = config(vec![selected]);
        assert!(
            producer_policy(
                &current,
                &declared(json!([CLAIM]), json!(["tos.form.selected"]))
            )
            .is_ok()
        );
        assert!(matches!(
            producer_policy(&current, &declared(json!([CLAIM]), json!([]))),
            Err(Error::Denied(
                "current Claim form outside producer declaration"
            ))
        ));
    }

    #[test]
    fn metadata_scope_does_not_promote_historical_or_unreviewed_content_access() {
        let metadata = config(vec![row("metadata_only", false)]);
        assert_eq!(check(&metadata, false).unwrap(), (false, PATH.to_owned()));
        assert!(matches!(
            check(&metadata, true),
            Err(Error::Denied(
                "current Claim content is outside selected read scope"
            ))
        ));
        let exact_without_verify = config(vec![row("exact_owner_local", false)]);
        assert!(check(&exact_without_verify, true).is_err());
        let exact = config(vec![row("exact_owner_local", true)]);
        assert_eq!(check(&exact, true).unwrap(), (true, PATH.to_owned()));
    }
    #[test]
    fn selected_claim_rejects_duplicate_identity_wrong_path_and_implicit_permission() {
        assert!(matches!(
            check(&config(vec![row("metadata_only", false); 2]), false),
            Err(Error::Denied("current Claim read identity is ambiguous"))
        ));
        let mut changed = row("metadata_only", false);
        changed["path"] =
            json!("ToS/source-witnesses/owner-local/selected/claims/other/source-claims.jsonl");
        assert!(matches!(
            check(&config(vec![changed]), false),
            Err(Error::Denied("current Claim read path differs"))
        ));
        let mut denied = row("metadata_only", false);
        denied["source_access"]["access_allowed"] = json!(false);
        assert!(matches!(
            check(&config(vec![denied]), false),
            Err(Error::Denied(
                "current Claim read source access is not explicit"
            ))
        ));
        assert!(check(&config(Vec::new()), false).is_err());
    }
    #[test]
    fn selected_claim_binds_actual_account_context_and_named_package() {
        let mut wrong = json!({
            "uid":18,"source_context_ref":"/selected/context.json",
            "owner_local_source_claims":[row("metadata_only", false)]
        });
        assert!(matches!(
            check(
                &cmd::parse(&serde_json::to_vec(&wrong).unwrap()).unwrap(),
                false
            ),
            Err(Error::Denied("current Claim read account differs"))
        ));
        wrong["uid"] = json!(17);
        wrong["source_context_ref"] = json!("/other/context.json");
        assert!(matches!(
            check(
                &cmd::parse(&serde_json::to_vec(&wrong).unwrap()).unwrap(),
                false
            ),
            Err(Error::Denied("current Claim read context differs"))
        ));
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let mut budget = OperationBudget::new(
            &json!({"budgets":{
                "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
            }}),
            deadline,
        )
        .unwrap();
        let mut access = ScopeAccess::admitted_operation(&mut budget, deadline, &cancelled);
        let mut escaped = row("metadata_only", false);
        let outside = "ToS/source-witnesses/owner-local/other/claims/context/source-claims.jsonl";
        escaped["path"] = json!(outside);
        let escaped = config(vec![escaped]);
        assert!(matches!(
            selected_scope(
                &mut access,
                &escaped,
                17,
                "/selected/context.json",
                PREFIX,
                outside,
                CLAIM,
                false
            ),
            Err(Error::Denied("current Claim read leaves owner prefix"))
        ));
    }
}
