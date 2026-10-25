//! Closed initial statement-copy form comparison and SAME owner schema lane.
//! Not full historical grant/Claim/creation integrity or native target authority.
use super::receipt::{exact_digest, hash, integer};
use super::{CurrentPgRoleOwner, PgPrivatePweCreationReceipt, member, overflow};
use crate::durable_private_packages::PrivateOwnerSelectedHistoryRead;
use crate::source_command::{SourceCommandError as Error, SourceCommandResult as Result};
use crate::source_native_cli::private_assessment::{OperationBudget, scope_admission::ScopeAccess};
use crate::source_native_roles::{PgRoleSchemaAccess, admitted_canonical, admitted_document};
use crate::{DurableError, DurableResult};
use std::{mem::size_of, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::{JsonDocument, JsonValue};

const FORM: &str = "ToS/contracts/human-form.schema.json";
const SET: &str = "ToS/contracts/human-form-set.schema.json";
pub(crate) struct PgPrivatePweInitialForms<'history> {
    creation: PgPrivatePweCreationReceipt<'history>,
    sets: Vec<InitialSet>,
}
impl PgPrivatePweInitialForms<'_> {
    pub(crate) fn creation(&self) -> &PgPrivatePweCreationReceipt<'_> {
        &self.creation
    }
    pub(crate) fn set_count(&self) -> usize {
        self.sets.len()
    }
    pub(crate) fn document(&self, index: usize) -> Option<&JsonDocument> {
        self.sets.get(index).map(|set| &set.document)
    }
    pub(crate) fn file_name(&self, index: usize) -> Option<&str> {
        self.sets.get(index).map(|set| set.name.as_str())
    }
}
struct InitialSet {
    name: String,
    document: JsonDocument,
}
struct PreparedSet {
    set: InitialSet,
    forms: Vec<Vec<u8>>,
    canonical: Vec<u8>,
    selected: usize,
}

fn text(value: &JsonValue, key: &str, expected: &str, a: &mut ScopeAccess<'_>) -> Result<()> {
    let actual = a.text(value, key)?;
    if !a.text_equal(actual, expected)? {
        return Err(Error::Conflict("private PWE original form text differs"));
    }
    Ok(())
}
fn reference(
    value: &JsonValue,
    id: &str,
    claim: &JsonValue,
    a: &mut ScopeAccess<'_>,
    digest: tos_foundation::Digest256,
) -> Result<()> {
    a.exact_keys(value, &["id", "version", "digest"])?;
    text(value, "id", id, a)?;
    if integer(value, "version", a)? != integer(claim, "claim_version", a)? {
        return Err(Error::Conflict(
            "private PWE original form subject version differs",
        ));
    }
    let spelling = a.text(value, "digest")?;
    if !exact_digest(spelling, digest, a)? {
        return Err(Error::Conflict(
            "private PWE original form subject digest differs",
        ));
    }
    Ok(())
}
fn binding(
    value: &JsonValue,
    pointer: &str,
    id: &str,
    claim: &JsonValue,
    digest: tos_foundation::Digest256,
    a: &mut ScopeAccess<'_>,
) -> Result<()> {
    a.exact_keys(value, &["record", "pointer"])?;
    text(value, "pointer", pointer, a)?;
    reference(a.field(value, "record")?, id, claim, a, digest)
}
fn check_set(
    claim: &JsonValue,
    selections: &[JsonValue],
    configuration: &JsonValue,
    set: &JsonValue,
    digest: tos_foundation::Digest256,
    a: &mut ScopeAccess<'_>,
) -> Result<usize> {
    a.exact_keys(set, &["schema_version", "subject", "forms", "prior_forms"])?;
    text(set, "schema_version", "tos_human_form_set_v1", a)?;
    let id = a.text(claim, "claim_id")?;
    reference(a.field(set, "subject")?, id, claim, a, digest)?;
    let prior = a
        .field(set, "prior_forms")?
        .as_array()
        .ok_or(Error::Invalid("private PWE initial prior forms"))?;
    if !prior.is_empty() {
        return Err(Error::Conflict(
            "private PWE initial forms already have predecessors",
        ));
    }
    let forms = a
        .field(set, "forms")?
        .as_array()
        .ok_or(Error::Invalid("private PWE initial forms"))?;
    if forms.is_empty() || forms.len() > 32 {
        return Err(Error::Invalid("private PWE initial form count"));
    }
    let qualifiers = a.field(claim, "qualifiers")?;
    let statement = a.text(qualifiers, "statement")?;
    a.inspect(statement.len(), statement.len())?;
    if tos_foundation::python_strip_unicode16_v1(statement, statement.len())
        .map_err(|_| overflow())?
        .is_empty()
    {
        return Err(Error::Invalid("private PWE original statement empty"));
    }
    let language = a.get(qualifiers, "statement_language")?;
    let script = a.get(qualifiers, "statement_script")?;
    let principal = a.text(configuration, "principal_id")?;
    let allowed = a
        .field(configuration, "allowed_form_ids")?
        .as_array()
        .ok_or(Error::Invalid("private PWE historical form identities"))?;
    if allowed.len() > 32 {
        return Err(Error::Invalid("private PWE historical form identity count"));
    }
    let operations = a
        .field(configuration, "allowed_operations")?
        .as_array()
        .ok_or(Error::Invalid("private PWE historical operations"))?;
    if operations.len() > 4 {
        return Err(Error::Invalid("private PWE historical operation count"));
    }
    let mut delegated = false;
    for operation in operations {
        a.inspect(size_of::<JsonValue>(), 1)?;
        let operation = operation
            .as_str()
            .ok_or(Error::Invalid("private PWE historical operation"))?;
        delegated |= a.text_equal(operation, "form.create")?;
    }
    if !delegated {
        return Err(Error::Denied(
            "private PWE original policy excludes form creation",
        ));
    }
    let mut at = 0usize;
    for selection in selections {
        a.exact_keys(selection, &["claim_id", "form_id", "field_id"])?;
        let selected_claim = a.text(selection, "claim_id")?;
        if !a.text_equal(selected_claim, id)? {
            continue;
        }
        text(selection, "field_id", "claim.statement", a)?;
        let form_id = a.text(selection, "form_id")?;
        let mut eligible = false;
        for allowed in allowed {
            a.inspect(size_of::<JsonValue>(), 1)?;
            eligible |= a.text_equal(
                allowed
                    .as_str()
                    .ok_or(Error::Invalid("private PWE historical form identity"))?,
                form_id,
            )?;
        }
        if !eligible {
            return Err(Error::Denied(
                "private PWE original form outside historical identity scope",
            ));
        }
        a.inspect(size_of::<JsonValue>(), 1)?;
        let form = forms
            .get(at)
            .ok_or(Error::Conflict("private PWE original selected form absent"))?;
        a.exact_keys(
            form,
            &[
                "schema_version",
                "form_id",
                "form_version",
                "subject",
                "role",
                "language",
                "script",
                "creator_id",
                "revises",
                "bindings",
                "content",
            ],
        )?;
        for (key, expected) in [
            ("schema_version", "tos_human_form_v1"),
            ("form_id", form_id),
            ("role", "statement"),
            ("creator_id", principal),
        ] {
            text(form, key, expected, a)?;
        }
        let version = integer(form, "form_version", a)?;
        let predecessor = a.field(form, "revises")?;
        if version != 1 || !a.optional_equal(Some(predecessor), Some(&JsonValue::Null))? {
            return Err(Error::Conflict("private PWE original form is not initial"));
        }
        reference(a.field(form, "subject")?, id, claim, a, digest)?;
        for (key, expected) in [("language", language), ("script", script)] {
            let actual = a.field(form, key)?;
            if !a.same(actual, expected.unwrap_or(&JsonValue::Null))? {
                return Err(Error::Conflict(
                    "private PWE original form language differs from source",
                ));
            }
        }
        let bindings = a.field(form, "bindings")?;
        a.exact_keys(bindings, &["wording", "context-0"])?;
        binding(
            a.field(bindings, "wording")?,
            "/qualifiers/statement",
            id,
            claim,
            digest,
            a,
        )?;
        binding(a.field(bindings, "context-0")?, "", id, claim, digest, a)?;
        let content = a.field(form, "content")?;
        a.exact_keys(content, &["kind", "slot"])?;
        text(content, "kind", "source-copy", a)?;
        text(content, "slot", "wording", a)?;
        at = at.checked_add(1).ok_or_else(overflow)?;
    }
    if at != forms.len() {
        return Err(Error::Conflict(
            "private PWE original form selection order or closure differs",
        ));
    }
    Ok(at)
}
fn filename(id: &str, budget: &mut OperationBudget) -> Result<String> {
    const PREFIX: &str = "source-claims.";
    const SUFFIX: &str = ".human-forms.json";
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = hash(id.as_bytes(), budget)?;
    let bytes = PREFIX.len() + 64 + SUFFIX.len();
    budget.charge_work(bytes)?;
    budget.charge_bytes(bytes + size_of::<String>() + size_of::<Result<String>>())?;
    // Direct ASCII emission, no temporary to_hex() String.
    let mut name = String::new();
    name.try_reserve_exact(bytes).map_err(|_| overflow())?;
    name.push_str(PREFIX);
    for byte in digest.as_bytes() {
        name.push(HEX[(byte >> 4) as usize] as char);
        name.push(HEX[(byte & 15) as usize] as char);
    }
    name.push_str(SUFFIX);
    Ok(name)
}
fn selections<'a>(
    creation: &'a PgPrivatePweCreationReceipt<'_>,
    a: &mut ScopeAccess<'_>,
) -> Result<(&'a [JsonValue], &'a [JsonValue])> {
    let request = creation.request().root();
    let claims = a
        .field(request, "claims")?
        .as_array()
        .ok_or(Error::Invalid("private PWE creation subjects"))?;
    let rows = a
        .field(request, "forms")?
        .as_array()
        .ok_or(Error::Invalid("private PWE creation selections"))?;
    if claims.is_empty() || claims.len() > 32 || rows.is_empty() || rows.len() > 32 {
        return Err(Error::Invalid("private PWE creation form closure count"));
    }
    check_selection_rows(rows, a)?;
    check_selection_subjects(claims, rows, a)?;
    Ok((claims, rows))
}
fn check_selection_subjects(
    claims: &[JsonValue],
    rows: &[JsonValue],
    a: &mut ScopeAccess<'_>,
) -> Result<()> {
    for row in rows {
        let selected = a.text(row, "claim_id")?;
        let mut found = false;
        for claim in claims {
            a.inspect(size_of::<JsonValue>(), 1)?;
            let id = a.text(claim, "claim_id")?;
            found |= a.text_equal(selected, id)?;
        }
        if !found {
            return Err(Error::Denied(
                "private PWE original form selects foreign initial Claim",
            ));
        }
    }
    Ok(())
}
fn check_selection_rows(rows: &[JsonValue], a: &mut ScopeAccess<'_>) -> Result<()> {
    for (index, row) in rows.iter().enumerate() {
        a.exact_keys(row, &["claim_id", "form_id", "field_id"])?;
        let id = a.text(row, "form_id")?;
        for previous in &rows[..index] {
            let previous = a.text(previous, "form_id")?;
            if a.text_equal(id, previous)? {
                return Err(Error::Denied("private PWE original forms repeat identity"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_command as cmd;
    use std::{sync::atomic::Ordering, time::Duration};
    fn setup() -> (Instant, AtomicBool, OperationBudget) {
        let d = Instant::now() + Duration::from_secs(30);
        (
            d,
            AtomicBool::new(false),
            OperationBudget::new(
                &serde_json::json!({"budgets":{
                    "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
                }}),
                d,
            )
            .unwrap(),
        )
    }
    fn value(v: &serde_json::Value) -> JsonValue {
        cmd::parse(&serde_json::to_vec(v).unwrap()).unwrap()
    }
    fn fixture() -> (JsonValue, Vec<JsonValue>, JsonValue, JsonValue) {
        // Maintained pure kernel DATA only. This constructs no owner, grant,
        // history, PgPrivatePweCreationReceipt, actual worker or native input.
        let claim = value(
            &serde_json::json!({"claim_id":"tos.claim.form-data","claim_version":1,
            "qualifiers":{"statement":"Парменид Элейский","statement_language":"ru","statement_script":"Cyrl"}}),
        );
        let mut changes = Vec::new();
        let mut selections = Vec::new();
        for id in ["tos.form.data.first", "tos.form.data.second"] {
            changes.push(
                crate::source_forms::prepare_form_change(
                    &claim,
                    None,
                    "principal",
                    id,
                    "claim.statement",
                )
                .unwrap(),
            );
            selections.push(value(&serde_json::json!({"claim_id":"tos.claim.form-data","form_id":id,"field_id":"claim.statement"})));
        }
        let subject = crate::source_forms::metadata_subject(&claim).unwrap();
        let set = crate::source_forms::apply_form_changes(None, &subject, &changes).unwrap();
        let config = value(
            &serde_json::json!({"principal_id":"principal","allowed_form_ids":["tos.form.data.first","tos.form.data.second"],
            "allowed_operations":["claims.create","form.create"]}),
        );
        (claim, selections, config, set)
    }
    fn checked(
        claim: &JsonValue,
        rows: &[JsonValue],
        config: &JsonValue,
        set: &JsonValue,
    ) -> Result<usize> {
        let (d, c, mut b) = setup();
        check_set(
            claim,
            rows,
            config,
            set,
            cmd::record_digest(claim).unwrap(),
            &mut ScopeAccess::admitted_operation(&mut b, d, &c),
        )
    }
    fn edit(set: &JsonValue, run: impl FnOnce(&mut serde_json::Value)) -> JsonValue {
        let mut raw: serde_json::Value =
            serde_json::from_slice(&cmd::canonical(set).unwrap()).unwrap();
        run(&mut raw);
        value(&raw)
    }
    #[test]
    fn private_pwe_initial_forms_match_maintained_statement_copy_and_exact_filename() {
        let (claim, rows, config, set) = fixture();
        assert_eq!(checked(&claim, &rows, &config, &set).unwrap(), 2);
        let id = cmd::text(&claim, "claim_id").unwrap();
        let (_, _, mut b) = setup();
        assert_eq!(
            filename(id, &mut b).unwrap(),
            format!(
                "source-claims.{}.human-forms.json",
                tos_foundation::Digest256::of_bytes(id.as_bytes()).to_hex()
            )
        );
    }
    #[test]
    fn private_pwe_initial_forms_reject_changed_subject_binding_language_or_copy_kind() {
        let (claim, rows, config, set) = fixture();
        for changed in [
            edit(&set, |v| {
                v["forms"][0]["subject"]["id"] = serde_json::json!("tos.claim.other")
            }),
            edit(&set, |v| {
                v["forms"][0]["bindings"]["context-0"]["pointer"] = serde_json::json!("/qualifiers")
            }),
            edit(&set, |v| {
                v["forms"][0]["language"] = serde_json::json!("en")
            }),
            edit(&set, |v| {
                v["forms"][0]["content"]["kind"] = serde_json::json!("freeform")
            }),
            edit(&set, |v| {
                v["forms"][0]["creator_id"] = serde_json::json!("other principal")
            }),
        ] {
            assert!(checked(&claim, &rows, &config, &changed).is_err());
        }
    }
    #[test]
    fn private_pwe_initial_forms_reject_reordered_noninitial_and_undelegated_forms() {
        let (claim, rows, config, set) = fixture();
        let reordered = edit(&set, |v| v["forms"].as_array_mut().unwrap().swap(0, 1));
        assert!(checked(&claim, &rows, &config, &reordered).is_err());
        let revised = edit(&set, |v| {
            v["forms"][0]["form_version"] = serde_json::json!(2)
        });
        assert!(matches!(
            checked(&claim, &rows, &config, &revised),
            Err(Error::Conflict("private PWE original form is not initial"))
        ));
        let prior = edit(&set, |v| v["prior_forms"] = serde_json::json!([{}]));
        assert!(matches!(
            checked(&claim, &rows, &config, &prior),
            Err(Error::Conflict(
                "private PWE initial forms already have predecessors"
            ))
        ));
        let config = edit(&config, |v| {
            v["allowed_operations"] = serde_json::json!(["claims.create"])
        });
        assert!(matches!(
            checked(&claim, &rows, &config, &set),
            Err(Error::Denied(
                "private PWE original policy excludes form creation"
            ))
        ));
    }
    #[test]
    fn private_pwe_initial_form_selection_rejects_duplicate_identity_before_native_hook() {
        let (_, mut rows, _, _) = fixture();
        rows[1] = rows[0].clone();
        let (d, c, mut b) = setup();
        assert!(matches!(
            check_selection_rows(&rows, &mut ScopeAccess::admitted_operation(&mut b, d, &c)),
            Err(Error::Denied("private PWE original forms repeat identity"))
        ));
    }
    #[test]
    fn private_pwe_initial_form_selection_rejects_foreign_subject_before_native_hook() {
        let (claim, mut rows, _, _) = fixture();
        let claims = [claim];
        let (d, c, mut b) = setup();
        assert!(
            check_selection_subjects(
                &claims,
                &rows,
                &mut ScopeAccess::admitted_operation(&mut b, d, &c)
            )
            .is_ok()
        );
        rows[1] = edit(&rows[1], |v| {
            v["claim_id"] = serde_json::json!("tos.claim.foreign")
        });
        let (d, c, mut b) = setup();
        assert!(matches!(
            check_selection_subjects(
                &claims,
                &rows,
                &mut ScopeAccess::admitted_operation(&mut b, d, &c)
            ),
            Err(Error::Denied(
                "private PWE original form selects foreign initial Claim"
            ))
        ));
    }
    #[test]
    fn private_pwe_initial_forms_keep_failed_prefix_and_cancel_before_body() {
        let (claim, rows, config, set) = fixture();
        let digest = cmd::record_digest(&claim).unwrap();
        let (d, c, mut b) = setup();
        b.lower_remaining(1_048_576, 1).unwrap();
        assert!(matches!(
            check_set(
                &claim,
                &rows,
                &config,
                &set,
                digest,
                &mut ScopeAccess::admitted_operation(&mut b, d, &c)
            ),
            Err(Error::Invalid("assessment cumulative work budget"))
        ));
        assert_eq!(b.test_work_remaining(), 0);
        assert!(b.limits().is_err());
        let (d, c, mut b) = setup();
        let before = b.test_work_remaining();
        c.store(true, Ordering::Relaxed);
        assert!(
            check_set(
                &claim,
                &rows,
                &config,
                &set,
                digest,
                &mut ScopeAccess::admitted_operation(&mut b, d, &c)
            )
            .is_err()
        );
        assert_eq!(b.test_work_remaining(), before);
    }
}
fn prepare(
    creation: &PgPrivatePweCreationReceipt<'_>,
    index: usize,
    budget: &mut OperationBudget,
    d: Instant,
    c: &AtomicBool,
) -> Result<PreparedSet> {
    budget.charge_bytes(size_of::<PreparedSet>() + size_of::<Result<PreparedSet>>())?;
    let (claim, rows, id) = {
        let mut a = ScopeAccess::admitted_operation(budget, d, c);
        let (claims, rows) = selections(creation, &mut a)?;
        a.inspect(size_of::<JsonValue>(), 1)?;
        let claim = claims
            .get(index)
            .ok_or(Error::Invalid("private PWE initial form subject index"))?;
        (claim, rows, a.text(claim, "claim_id")?)
    };
    let name = filename(id, budget)?;
    let raw = member(
        &creation.source().initial().initial_entry().members,
        &name,
        &mut ScopeAccess::admitted_operation(budget, d, c),
    )?;
    let document = admitted_document(raw, budget, d, c)?;
    let canonical_claim = admitted_canonical(claim, budget, d, c)?;
    let digest = hash(&canonical_claim, budget)?;
    let selected = check_set(
        claim,
        rows,
        creation.normalized_configuration().root(),
        document.root(),
        digest,
        &mut ScopeAccess::admitted_operation(budget, d, c),
    )?;
    let forms = {
        let mut a = ScopeAccess::admitted_operation(budget, d, c);
        a.field(document.root(), "forms")?
            .as_array()
            .ok_or(Error::Invalid("private PWE initial forms"))?
    };
    budget.charge_bytes(
        forms
            .len()
            .checked_mul(size_of::<Vec<u8>>())
            .ok_or_else(overflow)?
            + size_of::<Vec<Vec<u8>>>(),
    )?;
    let mut encoded = Vec::new();
    encoded
        .try_reserve_exact(forms.len())
        .map_err(|_| overflow())?;
    for form in forms {
        ScopeAccess::admitted_operation(budget, d, c).inspect(size_of::<JsonValue>(), 1)?;
        encoded.push(admitted_canonical(form, budget, d, c)?);
    }
    let canonical = admitted_canonical(document.root(), budget, d, c)?;
    Ok(PreparedSet {
        set: InitialSet { name, document },
        forms: encoded,
        canonical,
        selected,
    })
}

impl CurrentPgRoleOwner<'_, '_, '_, '_, '_, '_, '_> {
    fn check_initial_form_schema(
        &mut self,
        path: &str,
        raw: &[u8],
        schema: &str,
    ) -> DurableResult<()> {
        let body = (|| {
            self.with_source_epoch(|_, payer, same| {
                payer
                    .charge_sto_source_measures(
                        same,
                        0,
                        2 * size_of::<DurableResult<()>>() + size_of::<DurableResult<bool>>(),
                        0,
                        0,
                        1,
                    )
                    .map_err(DurableError::Source)
            })?;
            match self.check_record_bytes(path, raw, schema)? {
                true => Ok(()),
                false => Err(DurableError::Source(Error::Invalid(
                    "private PWE initial form schema rejected bytes",
                ))),
            }
        })();
        // Real protected current-use recheck on successful AND failed native
        // results; retain the original owned source/native/semantic error.
        let exit = self.check_current();
        match (body, exit) {
            (Err(original), _) | (Ok(_), Err(original)) => Err(original),
            (Ok(()), Ok(())) => Ok(()),
        }
    }
    /// Reissues live private selection/creation input for THIS owner. An old
    /// creation observation from another owner is not an accepted argument.
    /// Native calls occur only after the original OperationBudget loan ends.
    pub(crate) fn private_pwe_initial_forms<'history>(
        &mut self,
        private: &PrivateOwnerSelectedHistoryRead<'history, '_, '_>,
        path: &str,
        claim_id: &str,
    ) -> DurableResult<PgPrivatePweInitialForms<'history>> {
        let body = (|| {
            self.check_current()?;
            self.with_source_epoch(|_, payer, same| {
                payer
                    .charge_sto_source_measures(
                        same,
                        0,
                        size_of::<PgPrivatePweInitialForms<'_>>()
                            + size_of::<DurableResult<PgPrivatePweInitialForms<'_>>>(),
                        0,
                        0,
                        1,
                    )
                    .map_err(DurableError::Source)
            })?;
            let creation = self.private_pwe_creation_receipt(private, path, claim_id)?;
            let (count, total) = self.with_source_epoch(|_, payer, _| {
                let (d, c) = payer.source_cutoff();
                payer
                    .with_operation(|budget| {
                        let (claims, rows) = selections(
                            &creation,
                            &mut ScopeAccess::admitted_operation(budget, d, c),
                        )?;
                        budget.charge_bytes(
                            claims
                                .len()
                                .checked_mul(size_of::<InitialSet>())
                                .ok_or_else(overflow)?
                                + size_of::<Vec<InitialSet>>(),
                        )?;
                        Ok((claims.len(), rows.len()))
                    })
                    .map_err(DurableError::Source)
            })?;
            let mut sets = Vec::new();
            sets.try_reserve_exact(count)
                .map_err(|_| DurableError::Source(overflow()))?;
            let mut assigned = 0usize;
            for index in 0..count {
                let prepared = self.with_source_epoch(|_, payer, _| {
                    let (d, c) = payer.source_cutoff();
                    payer
                        .with_operation(|budget| prepare(&creation, index, budget, d, c))
                        .map_err(DurableError::Source)
                })?;
                for schema in [FORM, SET] {
                    if self.selected_schema_contract_digest(schema)?.is_none() {
                        return Err(DurableError::Conflict(
                            "private PWE initial form schema outside actual worker closure",
                        ));
                    }
                }
                // SAME nonreplaceable private host/worker, SAME original
                // payer/Session/cutoff. False is a semantic rejection, never
                // an authority factory or a resource-latch substitute.
                for raw in &prepared.forms {
                    self.check_initial_form_schema(path, raw, FORM)?;
                }
                self.check_initial_form_schema(path, &prepared.canonical, SET)?;
                assigned = assigned
                    .checked_add(prepared.selected)
                    .ok_or(DurableError::Source(overflow()))?;
                sets.push(prepared.set);
            }
            if assigned != total {
                return Err(DurableError::Conflict(
                    "private PWE original form request names foreign Claim",
                ));
            }
            Ok(PgPrivatePweInitialForms { creation, sets })
        })();
        let exit = self.check_current();
        match (body, exit) {
            (Err(original), _) | (Ok(_), Err(original)) => Err(original),
            (Ok(value), Ok(())) => Ok(value),
        }
    }
}
