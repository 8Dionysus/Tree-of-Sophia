//! Closed private PWE source parsing, separate from generic Claim readers.
//! These values prove only selected bytes/syntax/registered schema request.
//! They are NOT package lineage, NativeAccount, PG NativeBasis or admission.
use super::{
    BASE_PACKAGE_FILES, CLAIM_STREAM, CurrentClaimReadScope, MAX_ASSESSMENT_CLAIM_FILE_BYTES,
    MAX_CLAIM_ROWS, MAX_PACKAGE_BYTES, MAX_PACKAGE_FILES, claim_id,
};
use crate::source_command::{SourceCommandError as Error, SourceCommandResult as Result};
use crate::source_native_cli::private_assessment::{OperationBudget, scope_admission::ScopeAccess};
use crate::source_native_roles::{PgClaimProducerReadPolicy, admitted_document};
use std::{
    collections::BTreeMap, mem::size_of, ops::Range, sync::atomic::AtomicBool, time::Instant,
};
use tos_foundation::{JsonDocument, JsonValue};
use tos_validation::assessment::MAX_RECORD_BYTES;

#[path = "source_private_pwe_archive_plan.rs"]
mod archive_plan;
pub(crate) use archive_plan::{
    PrivatePweArchivePlan, PrivatePweArchiveRequest, parse_selected_archive_requests,
};

#[path = "source_private_pwe_revision_mechanics.rs"]
mod revision_mechanics;
pub(crate) use revision_mechanics::{
    PrivatePweStreamRevision, advance_private_pwe_claim, inspect_private_pwe_stream,
    reconstruct_private_pwe_stream_revision, replace_private_pwe_claim_row,
};
#[path = "source_private_pwe_creation_scope.rs"]
mod creation_scope;
pub(crate) use creation_scope::check_private_pwe_initial_declaration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrivatePweClaimFamily {
    Name,
    Responsibility,
    Identity,
}
impl PrivatePweClaimFamily {
    pub(crate) fn schema(self) -> &'static str {
        match self {
            Self::Name => "ToS/contracts/pwe-name-assignment-claim.schema.json",
            Self::Responsibility => "ToS/contracts/pwe-responsibility-account-claim.schema.json",
            Self::Identity => "ToS/contracts/pwe-identity-correspondence-claim.schema.json",
        }
    }
    fn shape(self) -> (&'static str, &'static str, &'static str, &'static str) {
        match self {
            Self::Name => (
                "pwe_name_assignment",
                "tos_pwe_name_assignment_claim_v1",
                "name-assignment",
                "bibliographic_assertion",
            ),
            Self::Responsibility => (
                "pwe_responsibility_account",
                "tos_pwe_responsibility_account_claim_v1",
                "responsibility-account",
                "bibliographic_assertion",
            ),
            Self::Identity => (
                "pwe_identity_correspondence",
                "tos_pwe_identity_correspondence_claim_v1",
                "identity-correspondence",
                "identity_assertion",
            ),
        }
    }
}

/// No raw/from-JSON constructor. The public parser requires the genuine
/// borrowed producer witness AND current protected read selection. The byte
/// range refers to that exact witness's CLAIM_STREAM, not a public PG member.
pub(crate) struct ParsedPrivatePweClaim {
    document: JsonDocument,
    raw: Range<usize>,
    family: PrivatePweClaimFamily,
}
impl ParsedPrivatePweClaim {
    pub(crate) fn document(&self) -> &JsonDocument {
        &self.document
    }
    pub(crate) fn family(&self) -> PrivatePweClaimFamily {
        self.family
    }
    pub(crate) fn raw_range(&self) -> Range<usize> {
        self.raw.clone()
    }

    /// Check real selected PWE/relation registry DATA after strict parsing.
    /// The caller must separately execute their actual schemas/dependencies
    /// and this Claim's schema on the same closed native executor.
    pub(crate) fn check_registered_source<'a>(
        &self,
        bindings: &'a JsonValue,
        relations: &'a JsonValue,
        selected: &CurrentClaimReadScope<'_>,
        access: &mut ScopeAccess<'_>,
    ) -> Result<&'a [JsonValue]> {
        registered_source(
            self.document.root(),
            self.family,
            bindings,
            relations,
            selected.assessment_selection(),
            access,
        )
    }
}

fn overflow() -> Error {
    Error::Unsupported("private PWE source accounting overflow")
}
fn mismatch() -> Error {
    Error::Conflict("private PWE selected source route differs")
}
fn array<'a>(
    access: &mut ScopeAccess<'_>,
    value: &'a JsonValue,
    key: &str,
) -> Result<&'a [JsonValue]> {
    access
        .field(value, key)?
        .as_array()
        .ok_or(Error::Invalid("private PWE source array"))
}
fn equals(
    access: &mut ScopeAccess<'_>,
    value: &JsonValue,
    field: &str,
    expected: &str,
) -> Result<()> {
    let actual = access.text(value, field)?;
    if access.text_equal(actual, expected)? {
        Ok(())
    } else {
        Err(mismatch())
    }
}
fn family(row: &JsonValue, access: &mut ScopeAccess<'_>) -> Result<PrivatePweClaimFamily> {
    for family in [
        PrivatePweClaimFamily::Name,
        PrivatePweClaimFamily::Responsibility,
        PrivatePweClaimFamily::Identity,
    ] {
        let (predicate, schema, kind, layer) = family.shape();
        let actual = access.text(row, "predicate")?;
        if !access.text_equal(actual, predicate)? {
            continue;
        }
        equals(access, row, "schema_version", schema)?;
        equals(access, row, "claim_type", "relation")?;
        equals(access, row, "visibility", "local_only")?;
        equals(access, row, "assertion_layer", layer)?;
        let object = access.field(row, "object")?;
        equals(access, object, "kind", kind)?;
        return Ok(family);
    }
    Err(Error::Unsupported(
        "selected private Claim has no closed PWE family",
    ))
}

pub(crate) fn parse_selected_claim_stream<'source>(
    policy: &'source PgClaimProducerReadPolicy<'_>,
    selected: &CurrentClaimReadScope<'_>,
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(ParsedPrivatePweClaim, &'source [u8])> {
    // Both operands have private constructors. A caller raw Claim, name of a
    // profile, hash or self-declared allowed list cannot enter this function.
    let raw = package_stream(
        &policy.producer().entry().members,
        budget,
        deadline,
        cancelled,
    )?;
    {
        let mut access = ScopeAccess::admitted_operation(budget, deadline, cancelled);
        equals(
            &mut access,
            policy.declared(),
            "source_path",
            selected.path(),
        )?;
    }
    select_stream_with_loan(
        raw,
        policy.declared(),
        selected.claim_id(),
        budget,
        deadline,
        cancelled,
    )
}

fn select_stream_with_loan<'source>(
    raw: &'source [u8],
    declared: &JsonValue,
    requested: &str,
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(ParsedPrivatePweClaim, &'source [u8])> {
    // Original immutable stream loan selected by package_stream, not a second
    // map search or buffer copy. Price the additional tuple/Result holders
    // before parsing/construction. Whole-stream parsing/error order is kept.
    budget.charge_bytes(
        size_of::<(ParsedPrivatePweClaim, &[u8])>()
            + size_of::<Result<(ParsedPrivatePweClaim, &[u8])>>(),
    )?;
    let parsed = select_stream(raw, declared, requested, budget, deadline, cancelled)?;
    Ok((parsed, raw))
}

fn package_stream<'a>(
    members: &'a BTreeMap<String, Vec<u8>>,
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<&'a [u8]> {
    if members.len() > MAX_PACKAGE_FILES {
        return Err(Error::Invalid("private Claim package size budget"));
    }
    budget.charge_bytes(
        size_of::<[bool; BASE_PACKAGE_FILES.len()]>()
            + size_of::<Option<&[u8]>>()
            + size_of::<usize>(),
    )?;
    let mut found = [false; BASE_PACKAGE_FILES.len()];
    let mut stream = None;
    let mut bytes = 0usize;
    let mut access = ScopeAccess::admitted_operation(budget, deadline, cancelled);
    for (name, raw) in members {
        access.inspect(0, 1)?;
        bytes = bytes.checked_add(raw.len()).ok_or_else(overflow)?;
        if bytes > MAX_PACKAGE_BYTES {
            return Err(Error::Invalid("private Claim package size budget"));
        }
        for (index, required) in BASE_PACKAGE_FILES.iter().enumerate() {
            if access.text_equal(name, required)? {
                found[index] = true;
                if *required == CLAIM_STREAM {
                    stream = Some(raw.as_slice());
                }
            }
        }
    }
    access.inspect(0, BASE_PACKAGE_FILES.len())?;
    if found.iter().any(|present| !*present) {
        return Err(Error::Conflict(
            "private Claim package evidence is incomplete",
        ));
    }
    // Physical base-member closure only. Their receipt/form/archive contents
    // still require the unchanged package_state and creation-integrity gates.
    stream.ok_or(Error::Conflict("selected private PWE Claim stream absent"))
}

fn select_stream(
    raw: &[u8],
    declared: &JsonValue,
    requested: &str,
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<ParsedPrivatePweClaim> {
    crate::source_creation_store::active(deadline, cancelled)?;
    if raw.len() > MAX_ASSESSMENT_CLAIM_FILE_BYTES {
        return Err(Error::Invalid(
            "private assessment Claim stream byte budget",
        ));
    }
    // Two actual bounded scans: delimiter search and blank-line inspection.
    // The legacy source count/row/stream caps are preserved. No map/JSON tree
    // clone is needed to detect duplicates in the whole selected stream.
    budget.charge_work(raw.len().checked_mul(2).ok_or_else(overflow)?)?;
    budget.charge_bytes(
        MAX_CLAIM_ROWS
            .checked_mul(size_of::<String>())
            .ok_or_else(overflow)?
            .checked_add(
                size_of::<Vec<String>>()
                    + size_of::<Option<ParsedPrivatePweClaim>>()
                    + size_of::<Result<ParsedPrivatePweClaim>>(),
            )
            .ok_or_else(overflow)?,
    )?;
    let mut ids: Vec<String> = Vec::new();
    ids.try_reserve_exact(MAX_CLAIM_ROWS)
        .map_err(|_| Error::Unsupported("private PWE source ID backing"))?;
    let mut found = None;
    let mut offset = 0usize;
    for line in raw.split(|byte| *byte == b'\n') {
        crate::source_creation_store::active(deadline, cancelled)?;
        let start = offset;
        offset = offset
            .checked_add(line.len())
            .and_then(|n| n.checked_add(1))
            .ok_or_else(overflow)?;
        if crate::source_claims::python_bytes_blank(line) {
            continue;
        }
        if line.len() > MAX_RECORD_BYTES {
            return Err(Error::Invalid("private assessment Claim row byte budget"));
        }
        if ids.len() == MAX_CLAIM_ROWS {
            return Err(Error::Conflict(
                "private Claim stream has duplicate or excessive rows",
            ));
        }
        let document = admitted_document(line, budget, deadline, cancelled)?;
        let mut access = ScopeAccess::admitted_operation(budget, deadline, cancelled);
        let id = access.text(document.root(), "claim_id")?;
        access.inspect(id.len(), 1)?;
        if !claim_id(id) {
            return Err(Error::Invalid("private PWE source Claim identity"));
        }
        for previous in &ids {
            if access.text_equal(previous, id)? {
                return Err(Error::Conflict(
                    "private Claim stream has duplicate or excessive rows",
                ));
            }
        }
        let allowed = array(&mut access, declared, "allowed_claim_ids")?;
        if allowed.is_empty() || allowed.len() > 32 {
            return Err(Error::Invalid("private PWE producer allowed count"));
        }
        let mut delegated = false;
        for value in allowed {
            let allowed = value
                .as_str()
                .ok_or(Error::Invalid("private PWE producer allowed identity"))?;
            delegated |= access.text_equal(allowed, id)?;
        }
        if !delegated {
            return Err(Error::Denied(
                "private Claim package contains an undelegated identity",
            ));
        }
        let selected = access.text_equal(id, requested)?;
        access.inspect(id.len(), 1)?; // original-ledger backing BEFORE to_owned
        ids.push(id.to_owned());
        if selected {
            let family = family(document.root(), &mut access)?;
            found = Some(ParsedPrivatePweClaim {
                document,
                raw: start..start + line.len(),
                family,
            });
        }
    }
    crate::source_creation_store::active(deadline, cancelled)?;
    found.ok_or(Error::Invalid(
        "selected private Claim is absent from its source stream",
    ))
}

fn registered_source<'a>(
    claim: &JsonValue,
    family: PrivatePweClaimFamily,
    bindings: &'a JsonValue,
    relations: &'a JsonValue,
    selection: &JsonValue,
    access: &mut ScopeAccess<'_>,
) -> Result<&'a [JsonValue]> {
    let (predicate, schema, kind, layer) = family.shape();
    equals(access, bindings, "schema_version", "tos_pwe_bindings_v1")?;
    equals(access, bindings, "registry_id", "tos.pwe.bindings")?;
    let rows = array(access, bindings, "bindings")?;
    if rows.len() != 3 {
        return Err(Error::Invalid(
            "PWE registry must declare exactly three account readers",
        ));
    }
    for expected in [
        PrivatePweClaimFamily::Name,
        PrivatePweClaimFamily::Responsibility,
        PrivatePweClaimFamily::Identity,
    ] {
        let mut count = 0usize;
        for value in rows {
            let actual = access.text(value, "predicate")?;
            if access.text_equal(actual, expected.shape().0)? {
                count += 1;
            }
        }
        if count != 1 {
            return Err(mismatch());
        }
    }
    let mut row = None;
    for value in rows {
        let actual = access.text(value, "predicate")?;
        if access.text_equal(actual, predicate)? {
            if row.replace(value).is_some() {
                return Err(mismatch());
            }
        }
    }
    let row = row.ok_or(mismatch())?;
    equals(access, row, "schema_version", schema)?;
    equals(access, row, "schema_ref", family.schema())?;
    equals(access, row, "value_kind", kind)?;
    equals(access, row, "reader", "pwe-exact-basis-v1")?;
    equals(access, row, "assertion_layer", layer)?;
    equals(access, claim, "predicate", predicate)?;
    let declared = access.text(row, "relation_type_id")?;
    equals(access, selection, "relation_type_id", declared)?;
    let mut relation = None;
    for value in array(access, relations, "relations")? {
        for mapping in array(access, value, "source_mappings")? {
            let graph = access.text(mapping, "source_graph")?;
            if !access.text_equal(graph, "source-claims")? {
                continue;
            }
            let actual = access
                .get(mapping, "source_predicate_id")?
                .and_then(JsonValue::as_str);
            if let Some(actual) = actual {
                if access.text_equal(actual, predicate)? {
                    if relation.replace(value).is_some() {
                        return Err(mismatch());
                    }
                }
            }
        }
    }
    let relation = relation.ok_or(Error::Unsupported(
        "PWE predicate has no actual registered source mapping",
    ))?;
    equals(access, relation, "relation_type_id", declared)?;
    let profile = access.field(relation, "source_claim_profile")?;
    equals(access, profile, "reader", "pwe-exact-basis-v1")?;
    equals(access, profile, "value_kind", kind)?;
    for value in [
        access.field(row, "profile_version")?,
        access.field(profile, "profile_version")?,
    ] {
        if let JsonValue::Number(number) = value {
            access.inspect(number.lexeme.len(), 1)?;
        }
        if value.as_u64() != Some(1) {
            return Err(mismatch());
        }
    }
    let mut route = None;
    for value in array(access, profile, "schemas")? {
        let actual = access.text(value, "schema_version")?;
        if access.text_equal(actual, schema)? {
            if route.replace(value).is_some() {
                return Err(mismatch());
            }
        }
    }
    let route = route.ok_or(Error::Unsupported(
        "PWE source schema version not registered",
    ))?;
    equals(access, route, "schema_ref", family.schema())?;
    let mut layer_found = false;
    for value in array(access, profile, "assertion_layers")? {
        let actual = value
            .as_str()
            .ok_or(Error::Invalid("private PWE assertion layer"))?;
        layer_found |= access.text_equal(actual, layer)?;
    }
    if !layer_found {
        return Err(mismatch());
    }
    let dependencies = array(access, route, "schema_dependencies")?;
    if dependencies.len() > 128 {
        return Err(Error::Invalid("private PWE schema closure count"));
    }
    // Prepay the later dependency iteration/UTF8/trait operands before the
    // short operation loan ends. Actual worker hooks run sequentially later.
    for value in dependencies {
        let name = value
            .as_str()
            .ok_or(Error::Invalid("private PWE schema dependency"))?;
        access.inspect(name.len(), 1)?;
    }
    Ok(dependencies)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_command as cmd;
    use serde_json::{Value, json};
    use std::{sync::atomic::Ordering, time::Duration};

    // Ordinary DATA fixtures for private pure predicates only. These cannot
    // create a producer witness/current read scope or call the closed issuer.
    fn operand(family: PrivatePweClaimFamily) -> Value {
        let (predicate, schema, kind, layer) = family.shape();
        json!({"claim_id":"tos.claim.selected","claim_version":1,
            "schema_version":schema,"predicate":predicate,"claim_type":"relation",
            "visibility":"local_only","assertion_layer":layer,
            "object":{"kind":kind,"source_wording":"Парменид Элейский"}})
    }
    fn setup() -> (Instant, AtomicBool, OperationBudget) {
        let deadline = Instant::now() + Duration::from_secs(30);
        let cancelled = AtomicBool::new(false);
        let budget = OperationBudget::new(
            &json!({"budgets":{
                "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
            }}),
            deadline,
        )
        .unwrap();
        (deadline, cancelled, budget)
    }
    fn declared(ids: &[&str]) -> JsonValue {
        cmd::parse(&serde_json::to_vec(&json!({"allowed_claim_ids":ids})).unwrap()).unwrap()
    }
    fn parse(
        raw: &[u8],
        ids: &[&str],
        budget: &mut OperationBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<ParsedPrivatePweClaim> {
        select_stream(
            raw,
            &declared(ids),
            "tos.claim.selected",
            budget,
            deadline,
            cancelled,
        )
    }
    fn registry_data() -> (JsonValue, JsonValue) {
        (
            cmd::parse(include_bytes!(
                "../../../../ToS/doctrine/semantic-interchange/pwe-bindings.v1.json"
            ))
            .unwrap(),
            cmd::parse(include_bytes!(
                "../../../../ToS/doctrine/semantic-interchange/relation-types.v1.json"
            ))
            .unwrap(),
        )
    }
    fn selected_relation(family: PrivatePweClaimFamily) -> JsonValue {
        let id = match family {
            PrivatePweClaimFamily::Name => "tos.relation.pwe-name-assignment",
            PrivatePweClaimFamily::Responsibility => "tos.relation.pwe-responsibility-account",
            PrivatePweClaimFamily::Identity => "tos.relation.pwe-identity-correspondence",
        };
        cmd::parse(&serde_json::to_vec(&json!({"relation_type_id":id})).unwrap()).unwrap()
    }

    #[test]
    fn private_pwe_selected_stream_loan_preserves_exact_unicode_bytes_without_another_lookup() {
        let (deadline, cancelled, mut budget) = setup();
        let line = serde_json::to_vec(&operand(PrivatePweClaimFamily::Name)).unwrap();
        let mut raw = b"\n \t\r\n".to_vec();
        let start = raw.len();
        raw.extend_from_slice(&line);
        raw.push(b'\n');
        let declaration = declared(&["tos.claim.selected"]);
        let (parsed, loan) = select_stream_with_loan(
            &raw,
            &declaration,
            "tos.claim.selected",
            &mut budget,
            deadline,
            &cancelled,
        )
        .unwrap();
        assert_eq!(loan.as_ptr(), raw.as_ptr());
        assert_eq!(loan.len(), raw.len());
        assert_eq!(parsed.raw_range(), start..start + line.len());
        // Later use is only a checked slice of the already selected loan.
        // Exhausting the budget cannot trigger a second map search/clone here.
        budget.lower_remaining(1, 1).unwrap();
        assert!(budget.charge_work(2).is_err());
        let before = budget.test_work_remaining();
        let selected = loan.get(parsed.raw_range()).unwrap();
        assert_eq!(selected.as_ptr(), raw[start..].as_ptr());
        assert_eq!(selected, line.as_slice());
        assert_eq!(budget.test_work_remaining(), before);
    }

    #[test]
    fn private_pwe_source_parses_all_three_families_and_preserves_raw_unicode_range() {
        for family in [
            PrivatePweClaimFamily::Name,
            PrivatePweClaimFamily::Responsibility,
            PrivatePweClaimFamily::Identity,
        ] {
            let (deadline, cancelled, mut budget) = setup();
            let line = serde_json::to_vec(&operand(family)).unwrap();
            let mut stream = b"\n \t\r\n".to_vec();
            let start = stream.len();
            stream.extend_from_slice(&line);
            stream.push(b'\n');
            let parsed = parse(
                &stream,
                &["tos.claim.selected"],
                &mut budget,
                deadline,
                &cancelled,
            )
            .unwrap();
            assert_eq!(parsed.family(), family);
            assert_eq!(parsed.raw_range(), start..start + line.len());
            assert_eq!(&stream[parsed.raw_range()], line.as_slice());
            assert_eq!(
                cmd::text(
                    cmd::field(parsed.document().root(), "object").unwrap(),
                    "source_wording"
                )
                .unwrap(),
                "Парменид Элейский"
            );
        }
    }

    #[test]
    fn private_pwe_source_rejects_duplicate_and_undelegated_rows_after_selected_row() {
        let selected = serde_json::to_vec(&operand(PrivatePweClaimFamily::Name)).unwrap();
        let mut duplicate = selected.clone();
        duplicate.push(b'\n');
        duplicate.extend_from_slice(&selected);
        let (deadline, cancelled, mut budget) = setup();
        assert!(matches!(
            parse(
                &duplicate,
                &["tos.claim.selected"],
                &mut budget,
                deadline,
                &cancelled
            ),
            Err(Error::Conflict(
                "private Claim stream has duplicate or excessive rows"
            ))
        ));
        let mut other = operand(PrivatePweClaimFamily::Name);
        other["claim_id"] = json!("tos.claim.other");
        let mut stream = selected;
        stream.push(b'\n');
        stream.extend_from_slice(&serde_json::to_vec(&other).unwrap());
        let (deadline, cancelled, mut budget) = setup();
        assert!(matches!(
            parse(
                &stream,
                &["tos.claim.selected"],
                &mut budget,
                deadline,
                &cancelled
            ),
            Err(Error::Denied(
                "private Claim package contains an undelegated identity"
            ))
        ));
    }

    #[test]
    fn private_pwe_source_refuses_schema_kind_and_visibility_substitution() {
        for (key, replacement) in [
            ("schema_version", "tos_pwe_identity_correspondence_claim_v1"),
            ("visibility", "public"),
            ("assertion_layer", "identity_assertion"),
        ] {
            let mut row = operand(PrivatePweClaimFamily::Name);
            row[key] = json!(replacement);
            let (deadline, cancelled, mut budget) = setup();
            assert!(matches!(
                parse(
                    &serde_json::to_vec(&row).unwrap(),
                    &["tos.claim.selected"],
                    &mut budget,
                    deadline,
                    &cancelled
                ),
                Err(Error::Conflict("private PWE selected source route differs"))
            ));
        }
        let mut row = operand(PrivatePweClaimFamily::Name);
        row["object"]["kind"] = json!("responsibility-account");
        let (deadline, cancelled, mut budget) = setup();
        assert!(
            parse(
                &serde_json::to_vec(&row).unwrap(),
                &["tos.claim.selected"],
                &mut budget,
                deadline,
                &cancelled
            )
            .is_err()
        );
    }

    #[test]
    fn private_pwe_source_keeps_original_cancel_and_failed_debit_prefix() {
        let line = serde_json::to_vec(&operand(PrivatePweClaimFamily::Name)).unwrap();
        let (deadline, cancelled, mut budget) = setup();
        let before = budget.test_work_remaining();
        cancelled.store(true, Ordering::Relaxed);
        assert!(
            parse(
                &line,
                &["tos.claim.selected"],
                &mut budget,
                deadline,
                &cancelled
            )
            .is_err()
        );
        assert_eq!(budget.test_work_remaining(), before);
        let (deadline, cancelled, mut budget) = setup();
        budget.lower_remaining(1_048_576, 100).unwrap();
        budget.charge_work(7).unwrap();
        let prefix = budget.test_work_remaining();
        assert!(matches!(
            parse(
                &line,
                &["tos.claim.selected"],
                &mut budget,
                deadline,
                &cancelled
            ),
            Err(Error::Invalid("assessment cumulative work budget"))
        ));
        assert_eq!(budget.test_work_remaining(), prefix);
        assert!(budget.limits().is_err()); // swallowed refusal cannot yield a fresh allowance
    }

    #[test]
    fn private_pwe_source_routes_all_three_against_actual_registered_source_data() {
        let (bindings, relations) = registry_data();
        for family in [
            PrivatePweClaimFamily::Name,
            PrivatePweClaimFamily::Responsibility,
            PrivatePweClaimFamily::Identity,
        ] {
            let (deadline, cancelled, mut budget) = setup();
            let raw = serde_json::to_vec(&operand(family)).unwrap();
            let parsed = parse(
                &raw,
                &["tos.claim.selected"],
                &mut budget,
                deadline,
                &cancelled,
            )
            .unwrap();
            let mut access = ScopeAccess::admitted_operation(&mut budget, deadline, &cancelled);
            let selected = selected_relation(family);
            let dependencies = registered_source(
                parsed.document().root(),
                family,
                &bindings,
                &relations,
                &selected,
                &mut access,
            )
            .unwrap();
            assert!(!dependencies.is_empty());
            assert!(
                dependencies
                    .iter()
                    .any(|v| v.as_str() == Some("ToS/contracts/pwe-values.schema.json"))
            );
        }
    }

    #[test]
    fn private_pwe_source_rejects_selected_relation_and_registry_ambiguity() {
        let (bindings, relations) = registry_data();
        let (deadline, cancelled, mut budget) = setup();
        let claim = cmd::parse(&serde_json::to_vec(&operand(PrivatePweClaimFamily::Name)).unwrap())
            .unwrap();
        let mut access = ScopeAccess::admitted_operation(&mut budget, deadline, &cancelled);
        assert!(
            registered_source(
                &claim,
                PrivatePweClaimFamily::Name,
                &bindings,
                &relations,
                &selected_relation(PrivatePweClaimFamily::Identity),
                &mut access
            )
            .is_err()
        );
        let mut changed: Value = serde_json::from_slice(include_bytes!(
            "../../../../ToS/doctrine/semantic-interchange/pwe-bindings.v1.json"
        ))
        .unwrap();
        changed["bindings"][2] = changed["bindings"][1].clone();
        let changed = cmd::parse(&serde_json::to_vec(&changed).unwrap()).unwrap();
        let (deadline, cancelled, mut budget) = setup();
        let mut access = ScopeAccess::admitted_operation(&mut budget, deadline, &cancelled);
        assert!(
            registered_source(
                &claim,
                PrivatePweClaimFamily::Name,
                &changed,
                &relations,
                &selected_relation(PrivatePweClaimFamily::Name),
                &mut access
            )
            .is_err()
        );
    }

    #[test]
    fn private_pwe_source_checks_whole_stream_json_before_returning_selected_row() {
        let mut raw = serde_json::to_vec(&operand(PrivatePweClaimFamily::Name)).unwrap();
        raw.extend_from_slice(
            b"\n{\"claim_id\":\"tos.claim.other\",\"claim_id\":\"tos.claim.third\"}",
        );
        let (deadline, cancelled, mut budget) = setup();
        assert!(
            parse(
                &raw,
                &["tos.claim.selected", "tos.claim.other", "tos.claim.third"],
                &mut budget,
                deadline,
                &cancelled
            )
            .is_err()
        );
        let (deadline, cancelled, mut budget) = setup();
        assert!(matches!(
            parse(
                b"\n\r\n",
                &["tos.claim.selected"],
                &mut budget,
                deadline,
                &cancelled
            ),
            Err(Error::Invalid(
                "selected private Claim is absent from its source stream"
            ))
        ));
    }

    #[test]
    fn private_pwe_package_envelope_requires_all_original_base_members() {
        let mut members = BTreeMap::new();
        for name in BASE_PACKAGE_FILES {
            members.insert((*name).to_owned(), b"{}".to_vec());
        }
        let stream = b"authentic source bytes are not replaced by envelope validation".to_vec();
        members.insert(CLAIM_STREAM.to_owned(), stream.clone());
        let (deadline, cancelled, mut budget) = setup();
        assert_eq!(
            package_stream(&members, &mut budget, deadline, &cancelled).unwrap(),
            stream
        );
        members.remove(super::super::CREATE_PROVENANCE);
        let (deadline, cancelled, mut budget) = setup();
        assert!(matches!(
            package_stream(&members, &mut budget, deadline, &cancelled),
            Err(Error::Conflict(
                "private Claim package evidence is incomplete"
            ))
        ));
    }
}
