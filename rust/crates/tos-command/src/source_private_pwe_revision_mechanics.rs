//! Prospectively admitted PWE revision mechanics, not a Claim/target issuer.
//! The closed interpreter must still bind every actual receipt/archive, live
//! selection, immutable creation, form reconstruction and native source law.
use super::super::{MAX_CLAIM_BYTES, MAX_CLAIM_ROWS, claim_id};
use super::{Error, OperationBudget, Result, ScopeAccess, admitted_document, family, overflow};
use crate::source_command as cmd;
use crate::source_native_roles::admitted_canonical;
use std::{mem::size_of, ops::Range, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::{Digest256, Digest256Hasher, JsonDocument, JsonString, JsonValue};

const FIELDS: &[&str] = &[
    "qualifiers",
    "evidence_refs",
    "counterevidence_refs",
    "alternative_claim_refs",
    "supporting_quotes",
    "epistemic_status",
    "confidence",
];

fn set(
    value: &mut JsonValue,
    key: &str,
    new: JsonValue,
    access: &mut ScopeAccess<'_>,
) -> Result<()> {
    access.inspect(size_of::<JsonValue>(), 1)?;
    let JsonValue::Object(fields) = value else {
        return Err(Error::Invalid("expected object"));
    };
    for (name, old) in fields.iter_mut() {
        access.inspect(size_of::<(JsonString, JsonValue)>(), 1)?;
        if let Some(name) = name.as_str() {
            if access.text_equal(name, key)? {
                *old = new;
                return Ok(());
            }
        }
    }
    // Actual logical slot, UTF16<=UTF8 units and UTF8 backing BEFORE adding a
    // new key. The retained-value copy was paid separately before this call.
    access.inspect(
        size_of::<(JsonString, JsonValue)>()
            .checked_add(key.len().checked_mul(3).ok_or_else(overflow)?)
            .ok_or_else(overflow)?,
        key.len().checked_mul(2).ok_or_else(overflow)?,
    )?;
    fields.try_reserve(1).map_err(|_| overflow())?;
    fields.push((JsonString::from_utf8(key), new));
    Ok(())
}
fn integer(value: &JsonValue, key: &str, access: &mut ScopeAccess<'_>) -> Result<u64> {
    let value = access.field(value, key)?;
    if let JsonValue::Number(number) = value {
        access.inspect(number.lexeme.len(), 1)?;
    }
    value
        .as_u64()
        .ok_or(Error::Invalid("expected integer field"))
}

/// Same correction law as source_claims::advance_claim, restricted to the
/// three genuine closed PWE syntactic families. Generic Claim dispatch and
/// the identity-transition proposal route remain unchanged. This function
/// never runs a schema stub or turns this DATA transform into permission.
pub(crate) fn advance_private_pwe_claim(
    record: &JsonValue,
    fields: &JsonValue,
    layer: Option<&JsonValue>,
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<JsonValue> {
    crate::source_creation_store::active(deadline, cancelled)?;
    budget.charge_bytes(2 * size_of::<JsonValue>() + size_of::<Result<JsonValue>>())?;
    let mut access = ScopeAccess::admitted_operation(budget, deadline, cancelled);
    family(record, &mut access)?;
    access.inspect(size_of::<JsonValue>(), 1)?;
    let members = fields
        .as_object()
        .ok_or(Error::Invalid("Claim field patch"))?;
    if members.is_empty() {
        return Err(Error::Invalid("empty Claim correction"));
    }
    for (key, _) in members {
        access.inspect(size_of::<(JsonString, JsonValue)>(), 1)?;
        let key = key.as_str().ok_or(Error::Invalid("field name"))?;
        let allowed = if layer.is_some() {
            access.text_equal(key, "assertion_layer")?
        } else {
            let mut allowed = access.text_equal(key, "object")?;
            for candidate in FIELDS {
                allowed |= access.text_equal(key, candidate)?;
            }
            allowed
        };
        if !allowed {
            return Err(Error::Denied("immutable Claim field"));
        }
    }
    if let Some(transition) = layer {
        access.exact_keys(transition, &["from", "to"])?;
        let from = access.text(transition, "from")?;
        let to = access.text(transition, "to")?;
        let old = access.text(record, "assertion_layer")?;
        let new = access.text(fields, "assertion_layer")?;
        if access.text_equal(from, to)?
            || !access.text_equal(from, old)?
            || !access.text_equal(to, new)?
        {
            return Err(Error::Denied("exact predecessor layer transition"));
        }
    }
    let mut revised = access.copied(record)?;
    for (key, value) in members {
        access.inspect(size_of::<(JsonString, JsonValue)>(), 1)?;
        let key = key.as_str().ok_or(Error::Invalid("field name"))?;
        let replacement = if access.text_equal(key, "qualifiers")? {
            let mut qualifiers = match access.get(record, "qualifiers")? {
                Some(value) => access.copied(value)?,
                None => {
                    access.inspect(size_of::<JsonValue>(), 1)?;
                    JsonValue::Object(Vec::new())
                }
            };
            access.inspect(size_of::<JsonValue>(), 1)?;
            for (name, value) in value
                .as_object()
                .ok_or(Error::Invalid("qualifier patch object"))?
            {
                access.inspect(size_of::<(JsonString, JsonValue)>(), 1)?;
                let name = name.as_str().ok_or(Error::Invalid("qualifier name"))?;
                let copied = access.copied(value)?;
                set(&mut qualifiers, name, copied, &mut access)?;
            }
            qualifiers
        } else {
            access.copied(value)?
        };
        if access.text_equal(key, "object")? {
            let before = access.field(record, key)?;
            access.inspect(2 * size_of::<JsonValue>(), 1)?;
            if before.as_object().is_none() || value.as_object().is_none() {
                return Err(Error::Denied("Claim endpoint kind is immutable"));
            }
        }
        set(&mut revised, key, replacement, &mut access)?;
    }
    // PWE families were fixed above: none is the separate generic identity
    // transition proposal predicate, so its topology branch is inapplicable.
    if access.same(record, &revised)? {
        return Err(Error::Invalid("Claim correction has no change"));
    }
    let version = integer(record, "claim_version", &mut access)?
        .checked_add(1)
        .filter(|v| *v <= 9_007_199_254_740_991)
        .ok_or(Error::Invalid("Claim version overflow"))?;
    access.inspect(size_of::<JsonValue>() + 20, 20)?;
    set(
        &mut revised,
        "claim_version",
        cmd::number(version),
        &mut access,
    )?;
    Ok(revised)
}

struct StreamRow {
    document: JsonDocument,
    whole: Range<usize>,
}
fn stream_rows(
    raw: &[u8],
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<StreamRow>> {
    crate::source_creation_store::active(deadline, cancelled)?;
    if raw.len() > MAX_CLAIM_BYTES {
        return Err(Error::Invalid("private Claim stream byte budget"));
    }
    budget.charge_work(raw.len().checked_mul(2).ok_or_else(overflow)?)?;
    budget.charge_bytes(
        MAX_CLAIM_ROWS
            .checked_mul(size_of::<StreamRow>())
            .ok_or_else(overflow)?
            + size_of::<Vec<StreamRow>>(),
    )?;
    let mut rows: Vec<StreamRow> = Vec::new();
    rows.try_reserve_exact(MAX_CLAIM_ROWS)
        .map_err(|_| overflow())?;
    let mut offset = 0usize;
    for line in raw.split_inclusive(|b| *b == b'\n') {
        crate::source_creation_store::active(deadline, cancelled)?;
        let start = offset;
        offset = offset.checked_add(line.len()).ok_or_else(overflow)?;
        if crate::source_claims::python_bytes_blank(line) {
            continue;
        }
        if rows.len() == MAX_CLAIM_ROWS {
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
        for previous in &rows {
            access.inspect(size_of::<StreamRow>(), 1)?;
            let previous = access.text(previous.document.root(), "claim_id")?;
            if access.text_equal(previous, id)? {
                return Err(Error::Conflict(
                    "private Claim stream has duplicate or excessive rows",
                ));
            }
        }
        rows.push(StreamRow {
            document,
            whole: start..offset,
        });
    }
    Ok(rows)
}

/// Paid counterpart of replace_claim_row: validate the WHOLE stream first,
/// retain each sibling byte and existing LF/CRLF/no-final-newline behavior.
/// All slices refer to this original immutable raw stream. No second parse,
/// whole-stream clone, early selected-row return or unpaid canonical output.
pub(crate) fn replace_private_pwe_claim_row(
    raw: &[u8],
    revised: &JsonValue,
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    let rows = stream_rows(raw, budget, deadline, cancelled)?;
    replace_rows(raw, &rows, revised, budget, deadline, cancelled)
}
fn replace_rows(
    raw: &[u8],
    rows: &[StreamRow],
    revised: &JsonValue,
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>> {
    let selected = {
        let mut access = ScopeAccess::admitted_operation(budget, deadline, cancelled);
        family(revised, &mut access)?;
        let wanted = access.text(revised, "claim_id")?;
        let mut selected = None;
        for row in rows {
            access.inspect(size_of::<StreamRow>(), 1)?;
            let id = access.text(row.document.root(), "claim_id")?;
            if access.text_equal(id, wanted)? {
                selected = Some(row.whole.clone());
            }
        }
        selected.ok_or(Error::Conflict("selected Claim absent"))?
    };
    let encoded = admitted_canonical(revised, budget, deadline, cancelled)?;
    budget.charge_work(2)?;
    let previous = &raw[selected.clone()];
    let ending: &[u8] = if previous.ends_with(b"\r\n") {
        b"\r\n"
    } else if previous.ends_with(b"\n") {
        b"\n"
    } else {
        b""
    };
    let output_len = raw
        .len()
        .checked_sub(previous.len())
        .and_then(|n| n.checked_add(encoded.len()))
        .and_then(|n| n.checked_add(ending.len()))
        .ok_or_else(overflow)?;
    budget.charge_bytes(
        output_len
            .checked_add(size_of::<Vec<u8>>() + size_of::<Result<Vec<u8>>>())
            .ok_or_else(overflow)?,
    )?;
    budget.charge_work(output_len)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(output_len)
        .map_err(|_| overflow())?;
    output.extend_from_slice(&raw[..selected.start]);
    output.extend_from_slice(&encoded);
    output.extend_from_slice(ending);
    output.extend_from_slice(&raw[selected.end..]);
    crate::source_creation_store::active(deadline, cancelled)?;
    Ok(output)
}

fn hash(raw: &[u8], budget: &mut OperationBudget) -> Result<Digest256> {
    budget.charge_work(raw.len().checked_add(1).ok_or_else(overflow)?)?;
    budget.charge_bytes(size_of::<Digest256Hasher>() + size_of::<Digest256>())?;
    Ok(Digest256::of_bytes(raw))
}
fn digest_json(
    value: &JsonValue,
    budget: &mut OperationBudget,
    d: Instant,
    c: &AtomicBool,
) -> Result<String> {
    let raw = admitted_canonical(value, budget, d, c)?;
    let digest = hash(&raw, budget)?;
    digest_spelling(digest, budget)
}
fn digest_spelling(digest: Digest256, budget: &mut OperationBudget) -> Result<String> {
    // Actual Foundation to_hex temporary64 + format!(sha256:{}) output71,
    // two String controllers and this Result carrier, BEFORE the API call.
    // No refund when the temporary dies; allocator/RSS/provider tariff OPEN.
    budget.charge_bytes(2 * size_of::<String>() + size_of::<Result<String>>() + 64 + 71)?;
    budget.charge_work(64 + 71)?;
    Ok(digest.to_prefixed())
}
fn reference(
    value: &JsonValue,
    budget: &mut OperationBudget,
    d: Instant,
    c: &AtomicBool,
) -> Result<JsonValue> {
    let digest = digest_json(value, budget, d, c)?;
    let mut access = ScopeAccess::admitted_operation(budget, d, c);
    let id = access.field(value, "claim_id")?;
    let id = access.copied(id)?;
    let version = integer(value, "claim_version", &mut access)?;
    // Same metadata_subject Claim reference: id, integer version, digest of
    // canonical complete record. Existing foundation groups the command and
    // record digest profiles under the same PythonCompact implementation.
    access.inspect(
        size_of::<JsonValue>()
            + 3 * size_of::<(&str, JsonValue)>()
            + 3 * size_of::<(JsonString, JsonValue)>()
            + 3 * (2 + 7 + 6 + 71)
            + 20,
        3 * 71 + 20,
    )?;
    Ok(cmd::object(vec![
        ("id", id),
        ("version", cmd::number(version)),
        ("digest", cmd::string(&digest)),
    ]))
}
fn nonblank(value: &str, access: &mut ScopeAccess<'_>) -> Result<bool> {
    access.inspect(value.len().checked_mul(2).ok_or_else(overflow)?, 2)?;
    Ok(
        !tos_foundation::python_strip_unicode16_v1(value, value.len())
            .map_err(|_| Error::Invalid("private Claim commit preconditions"))?
            .is_empty(),
    )
}
fn receipt_request<'a>(
    receipt: &'a JsonValue,
    budget: &mut OperationBudget,
    d: Instant,
    c: &AtomicBool,
) -> Result<&'a JsonValue> {
    let request = {
        let mut a = ScopeAccess::admitted_operation(budget, d, c);
        a.exact_keys(
            receipt,
            &[
                "command_id",
                "request_digest",
                "principal_id",
                "authority_ref",
                "owner_configuration",
                "recorded_at",
                "reason",
                "previous_source",
                "source",
                "previous_revision",
                "archive_path",
                "dependencies",
                "source_bindings",
                "changed_fields",
                "forms",
                "grants_admission",
                "request",
            ],
        )?;
        let request = a.field(receipt, "request")?;
        a.exact_keys(
            request,
            &[
                "schema_version",
                "operation",
                "claim_id",
                "fields",
                "forms",
                "reason",
                "command_id",
                "expected_configuration",
                "expected_source",
                "expected_revision",
                "expected_dependencies",
                "expected_inputs",
            ],
        )?;
        let schema = a.text(request, "schema_version")?;
        if !a.text_equal(schema, "tos_local_source_command_v1")? {
            return Err(Error::Unsupported("private Claim request version"));
        }
        let operation = a.text(request, "operation")?;
        if !a.text_equal(operation, "claim.revise")? {
            return Err(Error::Conflict(
                "private Claim revision receipt differs from retained request",
            ));
        }
        let command = a.text(request, "command_id")?;
        a.inspect(command.len(), 1)?;
        if !(1..=256).contains(&command.chars().count()) {
            return Err(Error::Invalid("private Claim command identity"));
        }
        for key in ["expected_configuration", "expected_dependencies"] {
            let text = a.text(request, key)?;
            if !nonblank(text, &mut a)? {
                return Err(Error::Invalid("private Claim commit preconditions"));
            }
        }
        let reason = a.text(request, "reason")?;
        a.inspect(reason.len().checked_mul(3).ok_or_else(overflow)?, 3)?;
        let stripped = tos_foundation::python_strip_unicode16_v1(reason, reason.len())
            .map_err(|_| Error::Invalid("private Claim revision reason"))?;
        if !(1..=4096).contains(&stripped.chars().count()) {
            return Err(Error::Invalid("private Claim revision reason"));
        }
        let fields = a.field(request, "fields")?;
        if fields.as_object().is_none_or(|f| f.is_empty()) {
            return Err(Error::Invalid("private Claim revision fields"));
        }
        let forms = a
            .field(request, "forms")?
            .as_array()
            .ok_or(Error::Invalid("expected array field"))?;
        if forms.is_empty() || forms.len() > 32 {
            return Err(Error::Invalid("private Claim revision form selections"));
        }
        let recorded = a.text(receipt, "recorded_at")?;
        a.inspect(recorded.len(), 1)?;
        crate::source_command::validate_instant(recorded)?;
        request
    };
    let expected_digest = digest_json(request, budget, d, c)?;
    let mut a = ScopeAccess::admitted_operation(budget, d, c);
    let receipt_digest = a.text(receipt, "request_digest")?;
    let receipt_command = a.text(receipt, "command_id")?;
    let request_command = a.text(request, "command_id")?;
    if !a.text_equal(receipt_digest, &expected_digest)?
        || !a.text_equal(receipt_command, request_command)?
    {
        return Err(Error::Conflict(
            "private Claim revision receipt differs from retained request",
        ));
    }
    for (left, right, canonical) in [
        ("previous_source", "expected_source", true),
        ("previous_revision", "expected_revision", false),
        ("owner_configuration", "expected_configuration", false),
        ("dependencies", "expected_dependencies", false),
        ("source_bindings", "expected_inputs", true),
        ("reason", "reason", false),
    ] {
        let left = a.field(receipt, left)?;
        let right = a.field(request, right)?;
        let equal = if canonical {
            a.same(left, right)?
        } else {
            a.optional_equal(Some(left), Some(right))?
        };
        if !equal {
            return Err(Error::Conflict(
                "private Claim revision receipt differs from retained request",
            ));
        }
    }
    let grants = a.field(receipt, "grants_admission")?;
    if !a.optional_equal(Some(grants), Some(&JsonValue::Bool(false)))? {
        return Err(Error::Conflict(
            "private Claim revision receipt differs from retained request",
        ));
    }
    Ok(request)
}

fn declared_rows(
    rows: &[StreamRow],
    declared: &JsonValue,
    budget: &mut OperationBudget,
    d: Instant,
    c: &AtomicBool,
) -> Result<bool> {
    if rows.is_empty() || rows.len() > 32 {
        return Err(Error::Denied(
            "private Claim package contains an undelegated identity",
        ));
    }
    let mut a = ScopeAccess::admitted_operation(budget, d, c);
    let allowed = a
        .field(declared, "allowed_claim_ids")?
        .as_array()
        .ok_or(Error::Invalid("expected array field"))?;
    if allowed.is_empty() || allowed.len() > 32 {
        return Err(Error::Invalid("private PWE producer allowed count"));
    }
    let mut initial = true;
    for row in rows {
        a.inspect(size_of::<StreamRow>(), 1)?;
        let id = a.text(row.document.root(), "claim_id")?;
        let mut permitted = false;
        for allowed in allowed {
            a.inspect(size_of::<JsonValue>(), 1)?;
            let allowed = allowed
                .as_str()
                .ok_or(Error::Invalid("private PWE producer allowed identity"))?;
            permitted |= a.text_equal(id, allowed)?;
        }
        if !permitted {
            return Err(Error::Denied(
                "private Claim package contains an undelegated identity",
            ));
        }
        initial &= integer(row.document.root(), "claim_version", &mut a)? == 1;
    }
    Ok(initial)
}
/// Immutable stream facts only; even a fully linked stream lacks the required
/// creation/form reconstruction and live semantic target/basis authority.
pub(crate) struct PrivatePweStreamRevision {
    pub(crate) predecessor: Digest256,
    pub(crate) successor: Digest256,
    pub(crate) all_predecessors_initial: bool,
}
pub(crate) fn inspect_private_pwe_stream(
    raw: &[u8],
    declared: &JsonValue,
    budget: &mut OperationBudget,
    d: Instant,
    c: &AtomicBool,
) -> Result<(Digest256, bool)> {
    let rows = stream_rows(raw, budget, d, c)?;
    let initial = declared_rows(&rows, declared, budget, d, c)?;
    Ok((hash(raw, budget)?, initial))
}
pub(crate) fn reconstruct_private_pwe_stream_revision(
    raw: &[u8],
    declared: &JsonValue,
    receipt: &JsonValue,
    budget: &mut OperationBudget,
    d: Instant,
    c: &AtomicBool,
) -> Result<PrivatePweStreamRevision> {
    budget.charge_bytes(
        size_of::<PrivatePweStreamRevision>() + size_of::<Result<PrivatePweStreamRevision>>(),
    )?;
    let request = receipt_request(receipt, budget, d, c)?;
    let rows = stream_rows(raw, budget, d, c)?;
    let initial = declared_rows(&rows, declared, budget, d, c)?;
    let prior = {
        let mut a = ScopeAccess::admitted_operation(budget, d, c);
        let wanted = a.text(request, "claim_id")?;
        let mut prior = None;
        for row in &rows {
            a.inspect(size_of::<StreamRow>(), 1)?;
            let id = a.text(row.document.root(), "claim_id")?;
            if a.text_equal(id, wanted)? {
                prior = Some(row.document.root());
            }
        }
        prior.ok_or(Error::Conflict(
            "private Claim archived predecessor is absent",
        ))?
    };
    let previous = reference(prior, budget, d, c)?;
    let fields = {
        let mut a = ScopeAccess::admitted_operation(budget, d, c);
        let expected = a.field(receipt, "previous_source")?;
        if !a.same(expected, &previous)? {
            return Err(Error::Conflict(
                "private Claim revision predecessor differs",
            ));
        }
        a.field(request, "fields")?
    };
    // validate_request_shape's exact closed request keys exclude a transition
    // operand; use the same None used by the actual retained private route.
    let revised = advance_private_pwe_claim(prior, fields, None, budget, d, c)?;
    let successor = reference(&revised, budget, d, c)?;
    {
        let mut a = ScopeAccess::admitted_operation(budget, d, c);
        let expected = a.field(receipt, "source")?;
        if !a.same(expected, &successor)? {
            return Err(Error::Conflict("private Claim revision successor differs"));
        }
        let changed = a
            .field(receipt, "changed_fields")?
            .as_array()
            .ok_or(Error::Invalid("Claim changed field list"))?;
        let fields = fields
            .as_object()
            .ok_or(Error::Invalid("Claim revision field patch"))?;
        a.inspect(
            fields
                .len()
                .checked_mul(size_of::<&str>())
                .ok_or_else(overflow)?,
            1,
        )?;
        let mut keys = Vec::new();
        keys.try_reserve_exact(fields.len())
            .map_err(|_| overflow())?;
        for (key, _) in fields {
            a.inspect(size_of::<(JsonString, JsonValue)>(), 1)?;
            keys.push(key.as_str().ok_or(Error::Invalid("field name"))?);
        }
        // Small exact selection sort: every actual comparison is admitted,
        // rather than assuming a tariff for an opaque sorting implementation.
        for i in 0..keys.len() {
            let mut least = i;
            for j in i + 1..keys.len() {
                a.inspect(
                    keys[j]
                        .len()
                        .checked_add(keys[least].len())
                        .ok_or_else(overflow)?,
                    1,
                )?;
                if keys[j] < keys[least] {
                    least = j;
                }
            }
            a.inspect(2 * size_of::<&str>(), 1)?;
            keys.swap(i, least);
        }
        if changed.len() != keys.len() {
            return Err(Error::Conflict(
                "private Claim changed-field receipt differs",
            ));
        }
        for (changed, expected) in changed.iter().zip(keys) {
            a.inspect(size_of::<JsonValue>(), 1)?;
            let changed = changed.as_str().ok_or(Error::Conflict(
                "private Claim changed-field receipt differs",
            ))?;
            if !a.text_equal(changed, expected)? {
                return Err(Error::Conflict(
                    "private Claim changed-field receipt differs",
                ));
            }
        }
    }
    let output = replace_rows(raw, &rows, &revised, budget, d, c)?;
    Ok(PrivatePweStreamRevision {
        predecessor: hash(raw, budget)?,
        successor: hash(&output, budget)?,
        all_predecessors_initial: initial,
    })
}

#[cfg(test)]
mod tests {
    use super::super::PrivatePweClaimFamily;
    use super::*;
    use serde_json::{Value, json};
    use std::{sync::atomic::Ordering, time::Duration};
    fn setup() -> (Instant, AtomicBool, OperationBudget) {
        let d = Instant::now() + Duration::from_secs(30);
        (
            d,
            AtomicBool::new(false),
            OperationBudget::new(
                &json!({"budgets":{
                    "worker_cpu_seconds":1,"worker_address_space_bytes":1048576
                }}),
                d,
            )
            .unwrap(),
        )
    }
    fn value(v: Value) -> JsonValue {
        cmd::parse(&serde_json::to_vec(&v).unwrap()).unwrap()
    }
    fn record(family: PrivatePweClaimFamily) -> JsonValue {
        let (predicate, schema, kind, layer) = family.shape();
        value(
            json!({"claim_id":"tos.claim.selected","claim_version":1,"schema_version":schema,
            "claim_type":"relation","predicate":predicate,"visibility":"local_only","assertion_layer":layer,
            "object":{"kind":kind,"source_wording":"Парменид Элейский"},
            "qualifiers":{"statement":"исходное имя","statement_language":"ru"}}),
        )
    }
    #[test]
    fn private_pwe_admitted_revision_matches_maintained_advance_for_all_three_families() {
        for family in [
            PrivatePweClaimFamily::Name,
            PrivatePweClaimFamily::Responsibility,
            PrivatePweClaimFamily::Identity,
        ] {
            let original = record(family);
            let fields =
                value(json!({"qualifiers":{"statement":"уточнённое имя"},"confidence":"probable"}));
            let expected = crate::source_claims::advance_claim(&original, &fields, None).unwrap();
            let (d, c, mut b) = setup();
            let before = b.test_work_remaining();
            let actual =
                advance_private_pwe_claim(&original, &fields, None, &mut b, d, &c).unwrap();
            assert_eq!(actual, expected);
            assert!(b.test_work_remaining() < before);
            assert_eq!(cmd::integer(&actual, "claim_version").unwrap(), 2);
            assert_eq!(
                cmd::text(
                    cmd::field(&actual, "qualifiers").unwrap(),
                    "statement_language"
                )
                .unwrap(),
                "ru"
            );
        }
    }
    #[test]
    fn private_pwe_admitted_revision_preserves_immutable_nochange_endpoint_and_overflow_refusals() {
        let original = record(PrivatePweClaimFamily::Name);
        for fields in [
            value(json!({"predicate":"other"})),
            value(json!({"object":"wrong"})),
            value(json!({"qualifiers":{"statement":"исходное имя"}})),
        ] {
            let expected =
                crate::source_claims::advance_claim(&original, &fields, None).unwrap_err();
            let (d, c, mut b) = setup();
            let actual =
                advance_private_pwe_claim(&original, &fields, None, &mut b, d, &c).unwrap_err();
            assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
        }
        let mut original = original;
        cmd::set(
            &mut original,
            "claim_version",
            cmd::number(9_007_199_254_740_991),
        )
        .unwrap();
        let fields = value(json!({"confidence":"probable"}));
        let (d, c, mut b) = setup();
        assert!(matches!(
            advance_private_pwe_claim(&original, &fields, None, &mut b, d, &c),
            Err(Error::Invalid("Claim version overflow"))
        ));
    }
    #[test]
    fn private_pwe_admitted_row_replacement_preserves_sibling_bytes_and_each_line_ending() {
        let original = record(PrivatePweClaimFamily::Name);
        let patch = value(json!({"confidence":"probable"}));
        let revised = crate::source_claims::advance_claim(&original, &patch, None).unwrap();
        for ending in [b"\r\n".as_slice(), b"\n".as_slice(), b"".as_slice()] {
            let mut raw =
                b" \t\r\n{\"claim_id\":\"tos.claim.sibling\",\"odd\": [ 1,  2 ]}\r\n".to_vec();
            raw.extend_from_slice(&cmd::canonical(&original).unwrap());
            raw.extend_from_slice(ending);
            let expected = crate::source_claims::replace_claim_row(&raw, &revised).unwrap();
            let (d, c, mut b) = setup();
            let actual = replace_private_pwe_claim_row(&raw, &revised, &mut b, d, &c).unwrap();
            assert_eq!(actual, expected);
            assert!(actual.starts_with(
                b" \t\r\n{\"claim_id\":\"tos.claim.sibling\",\"odd\": [ 1,  2 ]}\r\n"
            ));
        }
    }
    #[test]
    fn private_pwe_admitted_row_replacement_rejects_duplicate_or_malformed_trailing_row() {
        let original = record(PrivatePweClaimFamily::Name);
        let mut first = cmd::canonical(&original).unwrap();
        first.push(b'\n');
        for tail in [cmd::canonical(&original).unwrap(), b"{broken}".to_vec()] {
            let mut raw = first.clone();
            raw.extend_from_slice(&tail);
            let (d, c, mut b) = setup();
            assert!(replace_private_pwe_claim_row(&raw, &original, &mut b, d, &c).is_err());
        }
    }
    #[test]
    fn private_pwe_revision_cancel_and_failed_prefix_never_mutate_source_or_restore_budget() {
        let original = record(PrivatePweClaimFamily::Name);
        let fields = value(json!({"confidence":"probable"}));
        let saved = original.clone();
        let (d, c, mut b) = setup();
        let before = b.test_work_remaining();
        c.store(true, Ordering::Relaxed);
        assert!(advance_private_pwe_claim(&original, &fields, None, &mut b, d, &c).is_err());
        assert_eq!(b.test_work_remaining(), before);
        assert_eq!(original, saved);
        let (d, c, mut b) = setup();
        b.lower_remaining(1_048_576, 1).unwrap();
        assert!(advance_private_pwe_claim(&original, &fields, None, &mut b, d, &c).is_err());
        // The first admitted field-header visit spends the one remaining unit;
        // its next visit fails. Keep that actual successful prefix spent.
        assert_eq!(b.test_work_remaining(), 0);
        assert!(b.limits().is_err());
        assert_eq!(original, saved);
    }

    fn stream_fixture(record: &JsonValue) -> Vec<u8> {
        let mut raw = b" \t\r\n".to_vec();
        raw.extend_from_slice(&cmd::canonical(record).unwrap());
        raw.extend_from_slice(b"\r\n");
        raw
    }
    fn receipt(original: &JsonValue, patch: &JsonValue, command: &str) -> (JsonValue, JsonValue) {
        // DATA-only retained request fixture. It asserts no genuine source
        // homes, form reconstruction, grant, maker/event or native issuer.
        let revised = crate::source_claims::advance_claim(original, patch, None).unwrap();
        let convert =
            |v: &JsonValue| serde_json::from_slice::<Value>(&cmd::published(v).unwrap()).unwrap();
        let previous = crate::source_forms::metadata_subject(original).unwrap();
        let successor = crate::source_forms::metadata_subject(&revised).unwrap();
        let mut changed = patch
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, _)| k.as_str().unwrap())
            .collect::<Vec<_>>();
        changed.sort();
        let request = value(
            json!({"schema_version":"tos_local_source_command_v1","operation":"claim.revise",
            "claim_id":"tos.claim.selected","fields":convert(patch),"forms":[{"form_id":"tos.form.fixture","field_id":"claim.statement"}],
            "reason":" уточнение ","command_id":command,"expected_configuration":"sha256:configuration-fixture",
            "expected_source":convert(&previous),"expected_revision":"sha256:archive-package-fixture",
            "expected_dependencies":"sha256:dependency-fixture","expected_inputs":[]}),
        );
        let receipt = value(
            json!({"command_id":command,"request_digest":cmd::record_digest(&request).unwrap().to_prefixed(),
            "principal_id":"fixture-principal-data","authority_ref":"fixture-authority-data",
            "owner_configuration":"sha256:configuration-fixture","recorded_at":"2026-10-25T12:00:00Z",
            "reason":" уточнение ","previous_source":convert(&previous),"source":convert(&successor),
            "previous_revision":"sha256:archive-package-fixture","archive_path":"fixture-archive-data",
            "dependencies":"sha256:dependency-fixture","source_bindings":[],"changed_fields":changed,
            "forms":[],"grants_admission":false,"request":convert(&request)}),
        );
        (receipt, revised)
    }
    fn declared() -> JsonValue {
        value(json!({"allowed_claim_ids":["tos.claim.selected"]}))
    }
    #[test]
    fn private_pwe_retained_revision_reconstructs_two_exact_stream_links_for_all_three_families() {
        for family in [
            PrivatePweClaimFamily::Name,
            PrivatePweClaimFamily::Responsibility,
            PrivatePweClaimFamily::Identity,
        ] {
            let first = record(family);
            let patch = value(json!({"confidence":"probable"}));
            let (r1, second) = receipt(&first, &patch, "revision-1");
            let raw1 = stream_fixture(&first);
            let raw2 = crate::source_claims::replace_claim_row(&raw1, &second).unwrap();
            let (d, c, mut b) = setup();
            let one =
                reconstruct_private_pwe_stream_revision(&raw1, &declared(), &r1, &mut b, d, &c)
                    .unwrap();
            assert!(one.all_predecessors_initial);
            assert_eq!(one.predecessor, Digest256::of_bytes(&raw1));
            assert_eq!(one.successor, Digest256::of_bytes(&raw2));
            let patch = value(json!({"confidence":"certain"}));
            let (r2, third) = receipt(&second, &patch, "revision-2");
            let raw3 = crate::source_claims::replace_claim_row(&raw2, &third).unwrap();
            let (d, c, mut b) = setup();
            let two =
                reconstruct_private_pwe_stream_revision(&raw2, &declared(), &r2, &mut b, d, &c)
                    .unwrap();
            assert!(!two.all_predecessors_initial);
            assert_eq!(two.predecessor, one.successor);
            assert_eq!(two.successor, Digest256::of_bytes(&raw3));
        }
    }
    #[test]
    fn private_pwe_retained_revision_refuses_changed_request_predecessor_successor_and_field_list()
    {
        let original = record(PrivatePweClaimFamily::Name);
        let raw = stream_fixture(&original);
        let patch = value(json!({"confidence":"probable","qualifiers":{"statement":"уточнение"}}));
        let (receipt, _) = receipt(&original, &patch, "revision-1");
        for field in [
            "request_digest",
            "owner_configuration",
            "previous_source",
            "source",
            "changed_fields",
        ] {
            let mut corrupt = receipt.clone();
            cmd::set(&mut corrupt, field, JsonValue::Null).unwrap();
            let (d, c, mut b) = setup();
            assert!(
                reconstruct_private_pwe_stream_revision(&raw, &declared(), &corrupt, &mut b, d, &c)
                    .is_err()
            );
        }
        let mut corrupt = receipt.clone();
        cmd::set(&mut corrupt, "grants_admission", JsonValue::Bool(true)).unwrap();
        let (d, c, mut b) = setup();
        assert!(
            reconstruct_private_pwe_stream_revision(&raw, &declared(), &corrupt, &mut b, d, &c)
                .is_err()
        );
    }
    #[test]
    fn private_pwe_stream_inspection_rejects_undelegated_rows_and_records_noninitial_state() {
        let original = record(PrivatePweClaimFamily::Name);
        let raw = stream_fixture(&original);
        let (d, c, mut b) = setup();
        let (digest, initial) =
            inspect_private_pwe_stream(&raw, &declared(), &mut b, d, &c).unwrap();
        assert!(initial);
        assert_eq!(digest, Digest256::of_bytes(&raw));
        let other = value(json!({"allowed_claim_ids":["tos.claim.other"]}));
        let (d, c, mut b) = setup();
        assert!(matches!(
            inspect_private_pwe_stream(&raw, &other, &mut b, d, &c),
            Err(Error::Denied(
                "private Claim package contains an undelegated identity"
            ))
        ));
        let mut revised = original;
        cmd::set(&mut revised, "claim_version", cmd::number(2)).unwrap();
        let (d, c, mut b) = setup();
        assert!(
            !inspect_private_pwe_stream(&stream_fixture(&revised), &declared(), &mut b, d, &c)
                .unwrap()
                .1
        );
    }
    #[test]
    fn private_pwe_revision_digest_spelling_pays_both_hex_and_prefixed_characters() {
        let digest = Digest256::of_bytes(b"exact digest DATA");
        let (_, _, mut b) = setup();
        b.lower_remaining(1_048_576, 64 + 71 - 1).unwrap();
        assert!(matches!(
            digest_spelling(digest, &mut b),
            Err(Error::Invalid("assessment cumulative work budget"))
        ));
        assert_eq!(b.test_work_remaining(), 64 + 71 - 1);
        assert!(b.limits().is_err());
        let (_, _, mut b) = setup();
        assert_eq!(
            digest_spelling(digest, &mut b).unwrap(),
            digest.to_prefixed()
        );
    }
}
