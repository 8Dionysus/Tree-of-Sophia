//! Initial PWE Claim declaration constraints, not historical authorization.
//! The closed owner caller supplies its actual RR creation receipt/request.
//! No Grant, target, report or successful native eligibility token is issued.
use super::super::{CONFIG_V1, CONFIG_V2, claim_id, event_id};
use super::{family, overflow};
use crate::source_command::{
    self as cmd, SourceCommandError as Error, SourceCommandResult as Result,
};
use crate::source_native_cli::private_assessment::scope_admission::ScopeAccess;
use std::mem::size_of;
use tos_foundation::JsonValue;

fn array<'v>(
    value: &'v JsonValue,
    key: &str,
    max: usize,
    a: &mut ScopeAccess<'_>,
) -> Result<&'v [JsonValue]> {
    let value = a.field(value, key)?;
    a.inspect(size_of::<JsonValue>() + size_of::<usize>(), 1)?;
    let rows = value
        .as_array()
        .ok_or(Error::Invalid("private PWE declaration array"))?;
    if rows.len() > max {
        return Err(Error::Invalid("private PWE declaration count budget"));
    }
    Ok(rows)
}
fn optional<'v>(
    value: &'v JsonValue,
    key: &str,
    a: &mut ScopeAccess<'_>,
) -> Result<&'v [JsonValue]> {
    match a.get(value, key)? {
        None => Ok(&[]),
        Some(value) => {
            a.inspect(size_of::<JsonValue>(), 1)?;
            value
                .as_array()
                .ok_or(Error::Invalid("private PWE declaration optional array"))
        }
    }
}
fn allowed_text(
    config: &JsonValue,
    key: &str,
    text: &str,
    max: usize,
    a: &mut ScopeAccess<'_>,
) -> Result<()> {
    let mut found = false;
    for row in array(config, key, max, a)? {
        a.inspect(size_of::<JsonValue>(), 1)?;
        let candidate = row
            .as_str()
            .ok_or(Error::Invalid("private PWE declaration text member"))?;
        found |= a.text_equal(candidate, text)?;
    }
    if found {
        Ok(())
    } else {
        Err(Error::Denied(
            "initial private PWE Claim exceeds retained declaration",
        ))
    }
}
fn equals(a: &mut ScopeAccess<'_>, row: &JsonValue, key: &str, expected: &str) -> Result<()> {
    let actual = a.text(row, key)?;
    if a.text_equal(actual, expected)? {
        Ok(())
    } else {
        Err(Error::Denied(
            "initial private PWE Claim differs from retained declaration",
        ))
    }
}
fn member_id<'v>(row: &'v JsonValue, a: &mut ScopeAccess<'_>) -> Result<&'v str> {
    let id = a.text(row, "claim_id")?;
    // Exact dotted-identity helper: prefix, delimiter walk, segment byte
    // walk and at most len+1 nonempty tests, admitted before any of them.
    let bytes = id
        .len()
        .checked_mul(3)
        .and_then(|n| n.checked_add("tos.claim.".len() + 2))
        .ok_or_else(overflow)?;
    a.inspect(bytes, 1)?;
    if !claim_id(id) {
        return Err(Error::Invalid("private PWE initial Claim identity"));
    }
    Ok(id)
}
fn selection(config: &JsonValue, id: &str, a: &mut ScopeAccess<'_>) -> Result<()> {
    let mut matches = 0usize;
    for selected in array(config, "claim_selections", 32, a)? {
        a.inspect(size_of::<JsonValue>() + size_of::<usize>(), 1)?;
        let selected = a.text(selected, "claim_id")?;
        if a.text_equal(id, selected)? {
            matches = matches.checked_add(1).ok_or_else(overflow)?;
        }
    }
    if matches != 1 {
        return Err(Error::Denied(
            "initial private PWE Claim lacks one independent selection",
        ));
    }
    Ok(())
}

/// DATA predicate only. The actual owner method reselects the creation source
/// before every use; calling this on JSON cannot issue any source authority.
/// This does not replace actual schemas/profile/endpoint or creator eligibility.
pub(crate) fn check_private_pwe_initial_declaration(
    request: &JsonValue,
    config: &JsonValue,
    a: &mut ScopeAccess<'_>,
) -> Result<()> {
    let schema = a.text(config, "schema_version")?;
    let v2 = a.text_equal(schema, CONFIG_V2)?;
    if !v2 && !a.text_equal(schema, CONFIG_V1)? {
        return Err(Error::Unsupported(
            "private PWE retained declaration version",
        ));
    }
    allowed_text(config, "allowed_operations", "claims.create", 4, a)?;
    let claims = array(request, "claims", 32, a)?;
    if claims.is_empty() {
        return Err(Error::Invalid("private PWE creation has no Claims"));
    }
    for (index, claim) in claims.iter().enumerate() {
        a.inspect(size_of::<JsonValue>() + size_of::<usize>(), 1)?;
        // All three PWE families, never legacy writer reader_allowed bypass.
        let _family = family(claim, a)?;
        let id = member_id(claim, a)?;
        for previous in &claims[..index] {
            a.inspect(size_of::<JsonValue>(), 1)?;
            let previous = member_id(previous, a)?;
            if a.text_equal(previous, id)? {
                return Err(Error::Conflict(
                    "private PWE creation repeats Claim identity",
                ));
            }
        }
        allowed_text(config, "allowed_claim_ids", id, 32, a)?;
        selection(config, id, a)?;
        let subject = a.text(claim, "subject_ref")?;
        if a.text_equal(subject, id)? {
            return Err(Error::Invalid(
                "private PWE initial Claim names itself as subject",
            ));
        }
        allowed_text(config, "allowed_subject_refs", subject, 128, a)?;
        let predicate = a.text(claim, "predicate")?;
        allowed_text(config, "allowed_predicates", predicate, 32, a)?;
        let maker = a.field(claim, "maker")?;
        let principal = a.text(config, "principal_id")?;
        let nonblank = principal.len().checked_mul(3).ok_or_else(overflow)?;
        a.inspect(
            nonblank
                .checked_add(size_of::<std::str::Chars<'_>>())
                .ok_or_else(overflow)?,
            1,
        )?;
        if !cmd::nonblank(principal) {
            return Err(Error::Invalid("private PWE retained principal is blank"));
        }
        equals(a, maker, "agent_ref", principal)?;
        let maker_type = a.text(config, "maker_type")?;
        let mut maker_allowed = false;
        for expected in ["human", "software", "model"] {
            maker_allowed |= a.text_equal(maker_type, expected)?;
        }
        if !maker_allowed {
            return Err(Error::Invalid("private PWE retained maker type"));
        }
        equals(a, maker, "maker_type", maker_type)?;
        let event = a.text(config, "provenance_event_id")?;
        let bytes = event
            .len()
            .checked_mul(3)
            .and_then(|n| n.checked_add("tos.event.".len() + 2))
            .ok_or_else(overflow)?;
        a.inspect(bytes, 1)?;
        if !event_id(event) {
            return Err(Error::Invalid("private PWE retained event identity"));
        }
        equals(a, claim, "provenance_event_ref", event)?;
        let version = a.field(claim, "claim_version")?;
        if let JsonValue::Number(number) = version {
            a.inspect(number.lexeme.len(), 1)?;
        }
        if version.as_u64() != Some(1) {
            return Err(Error::Denied(
                "initial private PWE Claim is not version one",
            ));
        }
        let assessment = optional(claim, "assessment_refs", a)?;
        let previous = a.get(claim, "supersedes_claim_ref")?;
        a.inspect(
            size_of::<usize>() + size_of::<Option<&JsonValue>>() + size_of::<JsonValue>(),
            1,
        )?;
        if !assessment.is_empty() || previous.is_some_and(|value| !value.is_null()) {
            return Err(Error::Denied(
                "initial private PWE Claim revises or carries admission",
            ));
        }
        let object = a.field(claim, "object")?;
        a.inspect(size_of::<JsonValue>(), 1)?;
        if let Some(reference) = object.as_str() {
            allowed_text(config, "allowed_object_refs", reference, 128, a)?;
        } else {
            if !v2 {
                return Err(Error::Denied(
                    "private PWE exact object needs retained v2 declaration",
                ));
            }
            let mut found = false;
            for value in array(config, "allowed_object_values", 32, a)? {
                a.inspect(size_of::<JsonValue>(), 1)?;
                if value.as_object().is_none() {
                    return Err(Error::Invalid(
                        "private PWE retained exact object is not an object",
                    ));
                }
                // Canonical equivalence uses the real admitted walker, with
                // original container/Number/String capacities retained. No
                // clone, input-length credit or unmetered JSON equality.
                found |= a.same(value, object)?;
            }
            if !found {
                return Err(Error::Denied(
                    "private PWE object exceeds retained exact values",
                ));
            }
        }
        for key in ["evidence_refs", "counterevidence_refs"] {
            for reference in optional(claim, key, a)? {
                a.inspect(size_of::<JsonValue>(), 1)?;
                let text = reference
                    .as_str()
                    .ok_or(Error::Invalid("private PWE evidence reference"))?;
                allowed_text(config, "allowed_evidence_refs", text, 128, a)?;
            }
        }
        for quote in optional(claim, "supporting_quotes", a)? {
            a.inspect(size_of::<JsonValue>(), 1)?;
            let anchor = a.text(quote, "anchor_ref")?;
            allowed_text(config, "allowed_evidence_refs", anchor, 128, a)?;
        }
        for alternative in optional(claim, "alternative_claim_refs", a)? {
            a.inspect(size_of::<JsonValue>(), 1)?;
            let text = alternative
                .as_str()
                .ok_or(Error::Invalid("private PWE alternative Claim identity"))?;
            let mut present = false;
            for claim in claims {
                a.inspect(size_of::<JsonValue>(), 1)?;
                let id = member_id(claim, a)?;
                present |= a.text_equal(id, text)?;
            }
            if !present {
                return Err(Error::Denied(
                    "private PWE alternative lacks an independently present Claim",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_command as cmd;
    use crate::source_native_cli::private_assessment::OperationBudget;
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        time::{Duration, Instant},
    };
    fn value(value: &serde_json::Value) -> JsonValue {
        cmd::parse(&serde_json::to_vec(value).unwrap()).unwrap()
    }
    fn operands() -> (serde_json::Value, serde_json::Value) {
        let families = [
            (
                "pwe_name_assignment",
                "tos_pwe_name_assignment_claim_v1",
                "name-assignment",
                "bibliographic_assertion",
            ),
            (
                "pwe_responsibility_account",
                "tos_pwe_responsibility_account_claim_v1",
                "responsibility-account",
                "bibliographic_assertion",
            ),
            (
                "pwe_identity_correspondence",
                "tos_pwe_identity_correspondence_claim_v1",
                "identity-correspondence",
                "identity_assertion",
            ),
        ];
        let claims=families.iter().enumerate().map(|(i,(predicate,schema,kind,layer))|serde_json::json!({
            "claim_id":format!("tos.claim.pwe{i}"),"claim_type":"relation","claim_version":1,
            "schema_version":schema,"predicate":predicate,"assertion_layer":layer,"visibility":"local_only",
            "subject_ref":"tos.agent.fixture","object":{"kind":kind},
            "maker":{"agent_ref":"tos.agent.maker","maker_type":"human"},"provenance_event_ref":"tos.event.fixture",
            "assessment_refs":[],"evidence_refs":["tos.anchor.fixture"],"alternative_claim_refs":[]
        })).collect::<Vec<_>>();
        let objects = claims
            .iter()
            .map(|v| v["object"].clone())
            .collect::<Vec<_>>();
        let ids = claims
            .iter()
            .map(|v| v["claim_id"].clone())
            .collect::<Vec<_>>();
        let predicates = claims
            .iter()
            .map(|v| v["predicate"].clone())
            .collect::<Vec<_>>();
        let selections = ids
            .iter()
            .map(|v| serde_json::json!({"claim_id":v}))
            .collect::<Vec<_>>();
        (
            serde_json::json!({"claims":claims}),
            serde_json::json!({
                "schema_version":CONFIG_V2,"allowed_operations":["claims.create"],
                "allowed_claim_ids":ids,"claim_selections":selections,
                "allowed_subject_refs":["tos.agent.fixture"],"allowed_predicates":predicates,
                "principal_id":"tos.agent.maker","maker_type":"human","provenance_event_id":"tos.event.fixture",
                "allowed_object_refs":[],"allowed_object_values":objects,"allowed_evidence_refs":["tos.anchor.fixture"]
            }),
        )
    }
    fn budget(d: Instant) -> OperationBudget {
        OperationBudget::new(
            &serde_json::json!({"budgets":{
                "worker_cpu_seconds":1,"worker_address_space_bytes":16_777_216
            }}),
            d,
        )
        .unwrap()
    }
    fn check(request: &serde_json::Value, config: &serde_json::Value) -> Result<()> {
        let d = Instant::now() + Duration::from_secs(30);
        let c = AtomicBool::new(false);
        let mut b = budget(d);
        check_private_pwe_initial_declaration(
            &value(request),
            &value(config),
            &mut ScopeAccess::admitted_operation(&mut b, d, &c),
        )
    }
    #[test]
    fn initial_pwe_declaration_checks_all_three_families_without_issuing_authority() {
        // Pure declaration operands only: deliberately incomplete Records,
        // not schema-positive instances, private owners or positive issuers.
        let (request, config) = operands();
        assert!(check(&request, &config).is_ok());
        let mut changed = request.clone();
        changed["claims"][0]["predicate"] = serde_json::json!("legacy-predicate");
        assert!(matches!(
            check(&changed, &config),
            Err(Error::Unsupported(
                "selected private Claim has no closed PWE family"
            ))
        ));
        let mut changed = request.clone();
        changed["claims"][2]["assertion_layer"] = serde_json::json!("bibliographic_assertion");
        assert!(check(&changed, &config).is_err());
    }
    #[test]
    fn initial_pwe_declaration_rejects_changed_maker_event_subject_and_predicate_scope() {
        let (request, config) = operands();
        for (field, actual) in [
            ("subject_ref", "tos.agent.other"),
            ("provenance_event_ref", "tos.event.other"),
        ] {
            let mut changed = request.clone();
            changed["claims"][0][field] = serde_json::json!(actual);
            assert!(check(&changed, &config).is_err());
        }
        let mut changed = request.clone();
        changed["claims"][0]["maker"]["agent_ref"] = serde_json::json!("tos.agent.other");
        assert!(check(&changed, &config).is_err());
        let mut changed = request.clone();
        let mut blank = config.clone();
        blank["principal_id"] = serde_json::json!(" ");
        changed["claims"][0]["maker"]["agent_ref"] = serde_json::json!(" ");
        assert!(matches!(
            check(&changed, &blank),
            Err(Error::Invalid("private PWE retained principal is blank"))
        ));
        let mut changed = config.clone();
        changed["allowed_predicates"] = serde_json::json!(["pwe_name_assignment"]);
        assert!(matches!(
            check(&request, &changed),
            Err(Error::Denied(
                "initial private PWE Claim exceeds retained declaration"
            ))
        ));
    }
    #[test]
    fn initial_pwe_declaration_requires_one_independent_selection_and_initial_state() {
        let (request, config) = operands();
        let mut changed = config.clone();
        changed["claim_selections"][1] = changed["claim_selections"][0].clone();
        assert!(matches!(
            check(&request, &changed),
            Err(Error::Denied(
                "initial private PWE Claim lacks one independent selection"
            ))
        ));
        for (key, bad) in [
            ("claim_version", serde_json::json!(2)),
            (
                "assessment_refs",
                serde_json::json!(["tos.assessment.fake"]),
            ),
            (
                "supersedes_claim_ref",
                serde_json::json!("tos.claim.previous"),
            ),
        ] {
            let mut changed = request.clone();
            changed["claims"][0][key] = bad;
            assert!(check(&changed, &config).is_err());
        }
        let mut changed = request.clone();
        changed["claims"][1]["claim_id"] = changed["claims"][0]["claim_id"].clone();
        assert!(matches!(
            check(&changed, &config),
            Err(Error::Conflict(
                "private PWE creation repeats Claim identity"
            ))
        ));
    }
    #[test]
    fn initial_pwe_declaration_requires_exact_v2_object_and_declared_evidence() {
        let (request, config) = operands();
        let mut changed = request.clone();
        changed["claims"][0]["object"]["extra"] = serde_json::json!("changed");
        assert!(matches!(
            check(&changed, &config),
            Err(Error::Denied(
                "private PWE object exceeds retained exact values"
            ))
        ));
        let mut changed = config.clone();
        changed["schema_version"] = serde_json::json!(CONFIG_V1);
        assert!(matches!(
            check(&request, &changed),
            Err(Error::Denied(
                "private PWE exact object needs retained v2 declaration"
            ))
        ));
        let mut changed = request.clone();
        changed["claims"][1]["supporting_quotes"] =
            serde_json::json!([{"anchor_ref":"tos.anchor.other"}]);
        assert!(check(&changed, &config).is_err());
        let mut changed = request.clone();
        changed["claims"][2]["counterevidence_refs"] = serde_json::json!(["tos.anchor.other"]);
        assert!(check(&changed, &config).is_err());
    }
    #[test]
    fn initial_pwe_declaration_requires_alternatives_present_in_exact_request() {
        let (mut request, config) = operands();
        request["claims"][0]["alternative_claim_refs"] = serde_json::json!(["tos.claim.pwe2"]);
        assert!(check(&request, &config).is_ok());
        request["claims"][0]["alternative_claim_refs"] = serde_json::json!(["tos.claim.other"]);
        assert!(matches!(
            check(&request, &config),
            Err(Error::Denied(
                "private PWE alternative lacks an independently present Claim"
            ))
        ));
    }
    #[test]
    fn initial_pwe_declaration_cancellation_and_refusal_preserve_original_spent_prefix() {
        let (request, config) = operands();
        let request = value(&request);
        let config = value(&config);
        let d = Instant::now() + Duration::from_secs(30);
        let c = AtomicBool::new(false);
        let mut b = budget(d);
        b.lower_remaining(1_048_576, 40).unwrap();
        b.charge_work(7).unwrap();
        assert!(
            check_private_pwe_initial_declaration(
                &request,
                &config,
                &mut ScopeAccess::admitted_operation(&mut b, d, &c)
            )
            .is_err()
        );
        assert!(b.test_work_remaining() <= 33);
        assert!(b.limits().is_err());
        let mut b = budget(d);
        let before = b.test_work_remaining();
        c.store(true, Ordering::Relaxed);
        assert!(
            check_private_pwe_initial_declaration(
                &request,
                &config,
                &mut ScopeAccess::admitted_operation(&mut b, d, &c)
            )
            .is_err()
        );
        assert_eq!(b.test_work_remaining(), before);
    }
}
