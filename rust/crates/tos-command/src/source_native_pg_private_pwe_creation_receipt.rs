//! Bound creation receipt/request observations, NOT full creation integrity.
//! In particular receipt hashes cannot issue current permission, native basis,
//! a protected evaluator input, report provenance or application admission.
use super::{PgPrivatePweCreationSource, member, overflow};
use crate::source_command::{SourceCommandError as Error, SourceCommandResult as Result};
use crate::source_native_cli::private_assessment::{OperationBudget, scope_admission::ScopeAccess};
use crate::source_native_roles::{admitted_canonical, admitted_document};
use std::{mem::size_of, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::{Digest256, Digest256Hasher, JsonDocument, JsonValue};

pub(crate) struct PgPrivatePweCreationReceipt<'history> {
    source: PgPrivatePweCreationSource<'history>,
    request: JsonDocument,
    receipt: JsonDocument,
    normalized_configuration: JsonDocument,
    initial_response: JsonDocument,
    normalized_configuration_digest: Digest256,
}
impl PgPrivatePweCreationReceipt<'_> {
    pub(crate) fn source(&self) -> &PgPrivatePweCreationSource<'_> {
        &self.source
    }
    pub(crate) fn request(&self) -> &JsonDocument {
        &self.request
    }
    pub(crate) fn receipt(&self) -> &JsonDocument {
        &self.receipt
    }
    pub(crate) fn normalized_configuration(&self) -> &JsonDocument {
        &self.normalized_configuration
    }
    pub(crate) fn initial_response(&self) -> &JsonDocument {
        &self.initial_response
    }
    /// An observation of stored normalized bytes. It is NOT the authenticated
    /// broker raw-grant hash or the response's compound configuration basis.
    pub(crate) fn normalized_configuration_digest(&self) -> Digest256 {
        self.normalized_configuration_digest
    }
}
pub(super) fn hash(raw: &[u8], budget: &mut OperationBudget) -> Result<Digest256> {
    budget.charge_work(raw.len().checked_add(1).ok_or_else(overflow)?)?;
    budget.charge_bytes(size_of::<Digest256Hasher>() + size_of::<Digest256>())?;
    Ok(Digest256::of_bytes(raw))
}
// Exact lower-case prefixed spelling comparison with NO allocated String or
// formatting API. Whole input,32 digest bytes, prefix and64 comparisons paid
// before accessing either spelling. Neither temporary64 nor result71 exists.
pub(super) fn exact_digest(
    text: &str,
    digest: Digest256,
    access: &mut ScopeAccess<'_>,
) -> Result<bool> {
    access.inspect(
        text.len().checked_add(32 + 7 + 64).ok_or_else(overflow)?,
        64,
    )?;
    if text.len() != 71 || !text.starts_with("sha256:") {
        return Ok(false);
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let raw = text.as_bytes();
    for (index, byte) in digest.as_bytes().iter().enumerate() {
        if raw[7 + index * 2] != HEX[(byte >> 4) as usize]
            || raw[8 + index * 2] != HEX[(byte & 15) as usize]
        {
            return Ok(false);
        }
    }
    Ok(true)
}
pub(super) fn integer(value: &JsonValue, key: &str, access: &mut ScopeAccess<'_>) -> Result<u64> {
    let value = access.field(value, key)?;
    if let JsonValue::Number(n) = value {
        access.inspect(n.lexeme.len(), 1)?;
    }
    value
        .as_u64()
        .ok_or(Error::Invalid("private PWE creation integer field"))
}
fn canonical_lf(raw: &[u8], canonical: &[u8], access: &mut ScopeAccess<'_>) -> Result<bool> {
    access.inspect(
        raw.len()
            .checked_add(canonical.len())
            .ok_or_else(overflow)?,
        1,
    )?;
    Ok(
        raw.len() == canonical.len().checked_add(1).ok_or_else(overflow)?
            && raw.last() == Some(&b'\n')
            && raw.get(..canonical.len()) == Some(canonical),
    )
}
fn check_header(
    request: &JsonValue,
    receipt: &JsonValue,
    retained: &JsonValue,
    response: &JsonValue,
    actual_command: &str,
    declared_path: &str,
    request_digest: Digest256,
    access: &mut ScopeAccess<'_>,
) -> Result<()> {
    access.exact_keys(
        receipt,
        &[
            "schema_version",
            "command_id",
            "request_digest",
            "principal_id",
            "authority_ref",
            "owner_configuration",
            "recorded_at",
            "source_path",
            "dependencies",
            "source_bindings",
            "claims",
            "files",
            "grants_admission",
        ],
    )?;
    access.exact_keys(
        request,
        &[
            "schema_version",
            "operation",
            "claims",
            "forms",
            "command_id",
            "expected_configuration",
            "expected_source",
            "expected_revision",
            "expected_dependencies",
            "expected_inputs",
        ],
    )?;
    for (value, key, expected) in [
        (
            receipt,
            "schema_version",
            "tos_local_claim_create_receipt_v1",
        ),
        (request, "schema_version", "tos_local_source_command_v1"),
        (request, "operation", "claims.create"),
    ] {
        let text = access.text(value, key)?;
        if !access.text_equal(text, expected)? {
            return Err(Error::Conflict("private PWE creation protocol differs"));
        }
    }
    for value in [request, receipt] {
        let command = access.text(value, "command_id")?;
        if !access.text_equal(command, actual_command)? {
            return Err(Error::Conflict(
                "private PWE creation broker command differs",
            ));
        }
    }
    let digest = access.text(receipt, "request_digest")?;
    if !exact_digest(digest, request_digest, access)? {
        return Err(Error::Conflict(
            "private PWE creation receipt request digest differs",
        ));
    }
    let grants = access.field(receipt, "grants_admission")?;
    if !access.optional_equal(Some(grants), Some(&JsonValue::Bool(false)))? {
        return Err(Error::Conflict("private PWE creation grants admission"));
    }
    for value in [receipt, retained] {
        let path = access.text(value, "source_path")?;
        if !access.text_equal(path, declared_path)? {
            return Err(Error::Conflict("private PWE creation source path differs"));
        }
    }
    for key in ["principal_id", "authority_ref"] {
        let left = access.text(receipt, key)?;
        let right = access.text(retained, key)?;
        if !access.text_equal(left, right)? {
            return Err(Error::Conflict(
                "private PWE creation historical principal differs",
            ));
        }
    }
    for (left, right, canonical) in [
        ("owner_configuration", "expected_configuration", false),
        ("dependencies", "expected_dependencies", false),
        ("source_bindings", "expected_inputs", true),
    ] {
        let left = access.field(receipt, left)?;
        let right = access.field(request, right)?;
        let equal = if canonical {
            access.same(left, right)?
        } else {
            access.optional_equal(Some(left), Some(right))?
        };
        if !equal {
            return Err(Error::Conflict(
                "private PWE creation retained request differs",
            ));
        }
    }
    // This is the actual initial RESPONSE's compound basis, not its broker
    // raw grant digest, not normalized CONFIG_FILE and not current authority.
    let compound = access.field(receipt, "owner_configuration")?;
    let response_compound = access.field(response, "owner_configuration")?;
    if !access.optional_equal(Some(compound), Some(response_compound))? {
        return Err(Error::Conflict(
            "private PWE creation producer compound basis differs",
        ));
    }
    for key in ["expected_source", "expected_revision"] {
        let value = access.field(request, key)?;
        access.inspect(size_of::<JsonValue>(), 1)?;
        if !value.is_null() {
            return Err(Error::Conflict(
                "private PWE creation had an existing source",
            ));
        }
    }
    let operations = access
        .field(retained, "allowed_operations")?
        .as_array()
        .ok_or(Error::Invalid("private PWE creation operations"))?;
    if operations.is_empty() || operations.len() > 4 {
        return Err(Error::Invalid("private PWE creation operation count"));
    }
    let mut create = false;
    for operation in operations {
        access.inspect(size_of::<JsonValue>(), 1)?;
        let operation = operation
            .as_str()
            .ok_or(Error::Invalid("private PWE creation operation"))?;
        create |= access.text_equal(operation, "claims.create")?;
    }
    if !create {
        return Err(Error::Conflict(
            "private PWE original policy excludes creation",
        ));
    }
    let recorded = access.text(receipt, "recorded_at")?;
    access.inspect(recorded.len(), 1)?;
    crate::source_command::validate_instant(recorded)?;
    Ok(())
}
fn check_initial_claims(
    request: &JsonValue,
    receipt: &JsonValue,
    stream: &[u8],
    budget: &mut OperationBudget,
    d: Instant,
    c: &AtomicBool,
) -> Result<()> {
    let (claims, refs) = {
        let mut a = ScopeAccess::admitted_operation(budget, d, c);
        let claims = a
            .field(request, "claims")?
            .as_array()
            .ok_or(Error::Invalid("private PWE creation claims"))?;
        let refs = a
            .field(receipt, "claims")?
            .as_array()
            .ok_or(Error::Invalid("private PWE creation refs"))?;
        let forms = a
            .field(request, "forms")?
            .as_array()
            .ok_or(Error::Invalid("private PWE creation form selections"))?;
        if claims.is_empty()
            || claims.len() > 32
            || claims.len() != refs.len()
            || forms.is_empty()
            || forms.len() > 32
        {
            return Err(Error::Conflict(
                "private PWE creation subject/form closure differs",
            ));
        }
        (claims, refs)
    };
    budget.charge_bytes(
        claims
            .len()
            .checked_mul(size_of::<&str>())
            .ok_or_else(overflow)?
            + size_of::<Vec<&str>>(),
    )?;
    let mut ids: Vec<&str> = Vec::new();
    ids.try_reserve_exact(claims.len())
        .map_err(|_| overflow())?;
    let mut offset = 0usize;
    for (claim, reference) in claims.iter().zip(refs) {
        let id = {
            let mut a = ScopeAccess::admitted_operation(budget, d, c);
            a.inspect(2 * size_of::<JsonValue>(), 1)?;
            let id = a.text(claim, "claim_id")?;
            for previous in &ids {
                if a.text_equal(id, previous)? {
                    return Err(Error::Conflict(
                        "private PWE creation repeats Claim identity",
                    ));
                }
            }
            let version = integer(claim, "claim_version", &mut a)?;
            if version != 1 {
                return Err(Error::Conflict(
                    "private PWE creation starts after version one",
                ));
            }
            a.exact_keys(reference, &["id", "version", "digest"])?;
            let actual = a.text(reference, "id")?;
            if !a.text_equal(id, actual)? || integer(reference, "version", &mut a)? != version {
                return Err(Error::Conflict(
                    "private PWE creation subject reference differs",
                ));
            }
            id
        };
        let canonical = admitted_canonical(claim, budget, d, c)?;
        let digest = hash(&canonical, budget)?;
        let mut a = ScopeAccess::admitted_operation(budget, d, c);
        let expected = a.text(reference, "digest")?;
        if !exact_digest(expected, digest, &mut a)? {
            return Err(Error::Conflict(
                "private PWE creation subject digest differs",
            ));
        }
        let end = offset
            .checked_add(canonical.len())
            .and_then(|n| n.checked_add(1))
            .ok_or_else(overflow)?;
        let actual = stream.get(offset..end).ok_or(Error::Conflict(
            "private PWE creation initial stream differs",
        ))?;
        if !canonical_lf(actual, &canonical, &mut a)? {
            return Err(Error::Conflict(
                "private PWE creation initial stream differs",
            ));
        }
        a.inspect(size_of::<&str>() + size_of::<usize>(), 1)?;
        ids.push(id);
        offset = end;
    }
    ScopeAccess::admitted_operation(budget, d, c).inspect(size_of::<usize>(), 1)?;
    if offset != stream.len() {
        return Err(Error::Conflict(
            "private PWE creation initial stream has extra bytes",
        ));
    }
    Ok(())
}

fn check_receipt_files(
    receipt: &JsonValue,
    members: &std::collections::BTreeMap<String, Vec<u8>>,
    budget: &mut OperationBudget,
    d: Instant,
    c: &AtomicBool,
) -> Result<()> {
    // Equivalent DATA closure to validate_receipt_ref_files(file_refs(...,
    // false)), but no copied package, JSON map, hex String or join buffer.
    // The creation receipt itself is written AFTER this projection and is
    // therefore the only excluded member. Forms' actual source-copy/kernel
    // reconstruction remains a separate required law, not a hash inference.
    let refs = {
        let mut a = ScopeAccess::admitted_operation(budget, d, c);
        a.field(receipt, "files")?
            .as_object()
            .ok_or(Error::Invalid("private PWE creation file refs"))?
    };
    let mut expected = 0usize;
    for (name, _) in members {
        let mut a = ScopeAccess::admitted_operation(budget, d, c);
        a.inspect(size_of::<(String, Vec<u8>)>(), 1)?;
        if !a.text_equal(name, "source-create-receipt.json")? {
            expected = expected.checked_add(1).ok_or_else(overflow)?;
        }
    }
    if refs.len() != expected {
        return Err(Error::Conflict("private PWE creation file closure differs"));
    }
    for (name, binding) in refs {
        let raw = {
            let mut a = ScopeAccess::admitted_operation(budget, d, c);
            a.inspect(size_of::<(tos_foundation::JsonString, JsonValue)>(), 1)?;
            let name = name
                .as_str()
                .ok_or(Error::Invalid("private PWE creation filename"))?;
            if a.text_equal(name, "source-create-receipt.json")? {
                return Err(Error::Conflict(
                    "private PWE creation receipt includes itself",
                ));
            }
            a.exact_keys(binding, &["sha256", "bytes"])?;
            let raw = member(members, name, &mut a)?;
            a.inspect(size_of::<usize>() + size_of::<u64>(), 1)?;
            let actual = u64::try_from(raw.len()).map_err(|_| overflow())?;
            if integer(binding, "bytes", &mut a)? != actual {
                return Err(Error::Conflict(
                    "private PWE creation file byte count differs",
                ));
            }
            raw
        };
        let digest = hash(raw, budget)?;
        let mut a = ScopeAccess::admitted_operation(budget, d, c);
        let expected = a.text(binding, "sha256")?;
        if !exact_digest(expected, digest, &mut a)? {
            return Err(Error::Conflict("private PWE creation file digest differs"));
        }
    }
    Ok(())
}

pub(super) fn read<'history>(
    source: PgPrivatePweCreationSource<'history>,
    budget: &mut OperationBudget,
    d: Instant,
    c: &AtomicBool,
) -> Result<PgPrivatePweCreationReceipt<'history>> {
    crate::source_creation_store::active(d, c)?;
    budget.charge_bytes(
        size_of::<PgPrivatePweCreationReceipt<'_>>()
            + size_of::<Result<PgPrivatePweCreationReceipt<'_>>>(),
    )?;
    budget.charge_work(size_of::<
        crate::durable_private_packages::PrivateOwnerPackageHistoryEntryV1,
    >())?;
    let initial = source.initial.initial_entry();
    let (request_raw, receipt_raw, retained_raw, stream) = {
        let mut a = ScopeAccess::admitted_operation(budget, d, c);
        (
            member(&initial.members, "source-create-request.json", &mut a)?,
            member(&initial.members, "source-create-receipt.json", &mut a)?,
            member(
                &initial.members,
                "source-create-owner-configuration.json",
                &mut a,
            )?,
            member(&initial.members, "source-claims.jsonl", &mut a)?,
        )
    };
    let request = admitted_document(request_raw, budget, d, c)?;
    let receipt = admitted_document(receipt_raw, budget, d, c)?;
    let retained = admitted_document(retained_raw, budget, d, c)?;
    let response = admitted_document(&initial.response_bytes, budget, d, c)?;
    let canonical = admitted_canonical(request.root(), budget, d, c)?;
    {
        let mut a = ScopeAccess::admitted_operation(budget, d, c);
        if !canonical_lf(request_raw, &canonical, &mut a)? {
            return Err(Error::Conflict(
                "private PWE creation request bytes are not exact canonical LF",
            ));
        }
    }
    let request_digest = hash(&canonical, budget)?;
    {
        let mut a = ScopeAccess::admitted_operation(budget, d, c);
        a.inspect(2 * size_of::<Digest256>(), 1)?;
        if request_digest != initial.request_digest {
            return Err(Error::Conflict(
                "private PWE creation request differs from actual broker digest",
            ));
        }
        let declared = a.text(source.current.declared(), "source_path")?;
        check_header(
            request.root(),
            receipt.root(),
            retained.root(),
            response.root(),
            &initial.command_id,
            declared,
            request_digest,
            &mut a,
        )?;
    }
    check_initial_claims(request.root(), receipt.root(), stream, budget, d, c)?;
    check_receipt_files(receipt.root(), &initial.members, budget, d, c)?;
    let normalized_configuration_digest = hash(retained_raw, budget)?;
    crate::source_creation_store::active(d, c)?;
    Ok(PgPrivatePweCreationReceipt {
        source,
        request,
        receipt,
        normalized_configuration: retained,
        initial_response: response,
        normalized_configuration_digest,
    })
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
    fn value(value: &serde_json::Value) -> JsonValue {
        cmd::parse(&serde_json::to_vec(value).unwrap()).unwrap()
    }
    fn initial() -> (serde_json::Value, serde_json::Value, Vec<u8>) {
        // DATA-only comparison fixture, not a PWE Claim, source history,
        // grant, owner, schema executor or creation-integrity factory.
        let claims = [
            serde_json::json!({"claim_id":"tos.claim.data.first","claim_version":1,"text":"Парменид"}),
            serde_json::json!({"claim_id":"tos.claim.data.second","claim_version":1,"text":"second"}),
        ];
        let mut refs = Vec::new();
        let mut stream = Vec::new();
        for claim in &claims {
            let claim = value(claim);
            let reference = crate::source_forms::metadata_subject(&claim).unwrap();
            refs.push(
                serde_json::from_slice::<serde_json::Value>(&cmd::canonical(&reference).unwrap())
                    .unwrap(),
            );
            stream.extend(cmd::canonical(&claim).unwrap());
            stream.push(b'\n');
        }
        (
            serde_json::json!({"claims":claims,"forms":[{}]}),
            serde_json::json!({"claims":refs}),
            stream,
        )
    }
    fn initial_check(
        request: &serde_json::Value,
        receipt: &serde_json::Value,
        stream: &[u8],
    ) -> Result<()> {
        let (d, c, mut b) = setup();
        check_initial_claims(&value(request), &value(receipt), stream, &mut b, d, &c)
    }
    fn header_check(
        request: &serde_json::Value,
        receipt: &serde_json::Value,
        configuration: &serde_json::Value,
        response: &serde_json::Value,
    ) -> Result<()> {
        let request = value(request);
        let receipt = value(receipt);
        let configuration = value(configuration);
        let response = value(response);
        let (d, c, mut b) = setup();
        check_header(
            &request,
            &receipt,
            &configuration,
            &response,
            "actual-command",
            "private/claims/data/source-claims.jsonl",
            cmd::record_digest(&request).unwrap(),
            &mut ScopeAccess::admitted_operation(&mut b, d, &c),
        )
    }
    #[test]
    fn private_pwe_creation_header_binds_request_and_compound_without_raw_grant_substitution() {
        let request = serde_json::json!({"schema_version":"tos_local_source_command_v1","operation":"claims.create",
            "claims":[],"forms":[],"command_id":"actual-command","expected_configuration":"actual-compound",
            "expected_source":null,"expected_revision":null,"expected_dependencies":"dependencies",
            "expected_inputs":[{"path":"actual source","digest":"source pin"}]});
        let receipt = serde_json::json!({"schema_version":"tos_local_claim_create_receipt_v1","command_id":"actual-command",
            "request_digest":cmd::record_digest(&value(&request)).unwrap().to_prefixed(),"principal_id":"historical principal",
            "authority_ref":"historical authority","owner_configuration":"actual-compound","recorded_at":"2026-10-25T00:00:00Z",
            "source_path":"private/claims/data/source-claims.jsonl","dependencies":"dependencies",
            "source_bindings":[{"path":"actual source","digest":"source pin"}],"claims":[],"files":{},"grants_admission":false});
        let configuration = serde_json::json!({"principal_id":"historical principal","authority_ref":"historical authority",
            "source_path":"private/claims/data/source-claims.jsonl","allowed_operations":["claims.create"]});
        let response = serde_json::json!({"owner_configuration":"actual-compound"});
        // These are DATA predicates only. Empty subjects here cannot pass the
        // separate initial-subject check or construct any live source witness.
        assert!(header_check(&request, &receipt, &configuration, &response).is_ok());
        let mut changed = receipt.clone();
        changed["request_digest"] = serde_json::json!("sha256:wrong");
        assert!(matches!(
            header_check(&request, &changed, &configuration, &response),
            Err(Error::Conflict(
                "private PWE creation receipt request digest differs"
            ))
        ));
        let mut changed = response.clone();
        changed["owner_configuration"] = serde_json::json!("raw-grant-digest-is-not-compound");
        assert!(matches!(
            header_check(&request, &receipt, &configuration, &changed),
            Err(Error::Conflict(
                "private PWE creation producer compound basis differs"
            ))
        ));
        let mut changed = receipt.clone();
        changed["grants_admission"] = serde_json::json!(true);
        assert!(matches!(
            header_check(&request, &changed, &configuration, &response),
            Err(Error::Conflict("private PWE creation grants admission"))
        ));
        let mut changed = configuration.clone();
        changed["principal_id"] = serde_json::json!("other principal");
        assert!(matches!(
            header_check(&request, &receipt, &changed, &response),
            Err(Error::Conflict(
                "private PWE creation historical principal differs"
            ))
        ));
        let mut changed = configuration.clone();
        changed["allowed_operations"] = serde_json::json!(["claim.revise"]);
        assert!(matches!(
            header_check(&request, &receipt, &changed, &response),
            Err(Error::Conflict(
                "private PWE original policy excludes creation"
            ))
        ));
    }
    #[test]
    fn private_pwe_creation_digest_comparison_is_exact_lowercase_and_admitted_before_use() {
        let digest = Digest256::of_bytes(b"actual source operand");
        let spelling = digest.to_prefixed();
        let (d, c, mut b) = setup();
        let mut a = ScopeAccess::admitted_operation(&mut b, d, &c);
        assert!(exact_digest(&spelling, digest, &mut a).unwrap());
        assert!(!exact_digest(&spelling.to_uppercase(), digest, &mut a).unwrap());
        assert!(!exact_digest(&spelling, Digest256::of_bytes(b"different"), &mut a).unwrap());
        assert!(!exact_digest(&spelling[..70], digest, &mut a).unwrap());
        let (d, c, mut b) = setup();
        b.lower_remaining(1_048_576, 237).unwrap();
        // text71 + digest32 + prefix7 + hex64 + comparison visits64 =238.
        assert!(matches!(
            exact_digest(
                &spelling,
                digest,
                &mut ScopeAccess::admitted_operation(&mut b, d, &c)
            ),
            Err(Error::Invalid("assessment cumulative work budget"))
        ));
        assert_eq!(b.test_work_remaining(), 237);
        assert!(b.limits().is_err());
    }
    #[test]
    fn private_pwe_creation_canonical_lf_requires_exact_bytes_without_join_allocation() {
        let canonical = cmd::canonical(&value(&serde_json::json!({"text":"Парменид"}))).unwrap();
        let mut raw = canonical.clone();
        raw.push(b'\n');
        let (d, c, mut b) = setup();
        let mut a = ScopeAccess::admitted_operation(&mut b, d, &c);
        assert!(canonical_lf(&raw, &canonical, &mut a).unwrap());
        assert!(!canonical_lf(&canonical, &canonical, &mut a).unwrap());
        raw.push(b'\n');
        assert!(!canonical_lf(&raw, &canonical, &mut a).unwrap());
        raw.pop();
        raw.insert(0, b' ');
        assert!(!canonical_lf(&raw, &canonical, &mut a).unwrap());
    }
    #[test]
    fn private_pwe_creation_initial_stream_matches_maintained_subject_ref_recipe() {
        let (request, receipt, stream) = initial();
        assert!(initial_check(&request, &receipt, &stream).is_ok());
        let mut changed = receipt.clone();
        changed["claims"][0]["digest"] = serde_json::json!("sha256:wrong");
        assert!(matches!(
            initial_check(&request, &changed, &stream),
            Err(Error::Conflict(
                "private PWE creation subject digest differs"
            ))
        ));
        let mut changed = receipt.clone();
        changed["claims"][0]["id"] = serde_json::json!("tos.claim.other");
        assert!(matches!(
            initial_check(&request, &changed, &stream),
            Err(Error::Conflict(
                "private PWE creation subject reference differs"
            ))
        ));
        let mut changed = request.clone();
        changed["claims"][1]["claim_id"] = changed["claims"][0]["claim_id"].clone();
        assert!(matches!(
            initial_check(&changed, &receipt, &stream),
            Err(Error::Conflict(
                "private PWE creation repeats Claim identity"
            ))
        ));
        let mut changed = request.clone();
        changed["claims"][0]["claim_version"] = serde_json::json!(2);
        assert!(matches!(
            initial_check(&changed, &receipt, &stream),
            Err(Error::Conflict(
                "private PWE creation starts after version one"
            ))
        ));
    }
    #[test]
    fn private_pwe_creation_initial_stream_rejects_missing_extra_and_reordered_raw() {
        let (request, receipt, stream) = initial();
        assert!(initial_check(&request, &receipt, &stream[..stream.len() - 1]).is_err());
        let mut extra = stream.clone();
        extra.push(b'\n');
        assert!(matches!(
            initial_check(&request, &receipt, &extra),
            Err(Error::Conflict(
                "private PWE creation initial stream has extra bytes"
            ))
        ));
        let mut changed = request.clone();
        changed["claims"].as_array_mut().unwrap().swap(0, 1);
        let mut refs = receipt.clone();
        refs["claims"].as_array_mut().unwrap().swap(0, 1);
        assert!(matches!(
            initial_check(&changed, &refs, &stream),
            Err(Error::Conflict(
                "private PWE creation initial stream differs"
            ))
        ));
        let mut empty = request.clone();
        empty["forms"] = serde_json::json!([]);
        assert!(matches!(
            initial_check(&empty, &receipt, &stream),
            Err(Error::Conflict(
                "private PWE creation subject/form closure differs"
            ))
        ));
    }
    #[test]
    fn private_pwe_creation_number_and_cancel_refuse_before_unpaid_conversion_or_hash() {
        use tos_foundation::{JsonNumber, JsonNumberKind, JsonString};
        let numeric = JsonValue::Object(vec![(
            JsonString::from_utf8("bytes"),
            JsonValue::Number(JsonNumber {
                kind: JsonNumberKind::Int,
                lexeme: "9".repeat(4096),
            }),
        )]);
        let (d, c, mut b) = setup();
        b.lower_remaining(1_048_576, 32).unwrap();
        assert!(matches!(
            integer(
                &numeric,
                "bytes",
                &mut ScopeAccess::admitted_operation(&mut b, d, &c)
            ),
            Err(Error::Invalid("assessment cumulative work budget"))
        ));
        assert_eq!(b.test_work_remaining(), 19);
        assert!(b.limits().is_err());
        let digest = Digest256::of_bytes(b"known");
        let spelling = digest.to_prefixed();
        let (d, c, mut b) = setup();
        let before = b.test_work_remaining();
        c.store(true, Ordering::Relaxed);
        assert!(
            exact_digest(
                &spelling,
                digest,
                &mut ScopeAccess::admitted_operation(&mut b, d, &c)
            )
            .is_err()
        );
        assert_eq!(b.test_work_remaining(), before);
    }
    #[test]
    fn private_pwe_creation_file_closure_matches_maintained_refs_and_rejects_tampering() {
        // Ordinary DATA map only. No history/owner/creation source constructor.
        let mut files = std::collections::BTreeMap::from([
            (
                "source-claims.jsonl".to_owned(),
                b"actual initial stream\n".to_vec(),
            ),
            (
                "source-create-owner-configuration.json".to_owned(),
                b"normalized, not raw grant".to_vec(),
            ),
        ]);
        let refs = crate::source_revisions::file_refs(&files, false);
        let receipt = cmd::object(vec![("files", refs.clone())]);
        files.insert(
            "source-create-receipt.json".to_owned(),
            b"receipt excluded".to_vec(),
        );
        let (d, c, mut b) = setup();
        assert!(check_receipt_files(&receipt, &files, &mut b, d, &c).is_ok());
        let mut changed = files.clone();
        changed.get_mut("source-claims.jsonl").unwrap()[0] = b'!';
        let (d, c, mut b) = setup();
        assert!(matches!(
            check_receipt_files(&receipt, &changed, &mut b, d, &c),
            Err(Error::Conflict("private PWE creation file digest differs"))
        ));
        let mut changed = files.clone();
        changed.get_mut("source-claims.jsonl").unwrap().push(b'!');
        let (d, c, mut b) = setup();
        assert!(matches!(
            check_receipt_files(&receipt, &changed, &mut b, d, &c),
            Err(Error::Conflict(
                "private PWE creation file byte count differs"
            ))
        ));
        let mut changed = files.clone();
        changed.insert("unexpected".to_owned(), Vec::new());
        let (d, c, mut b) = setup();
        assert!(matches!(
            check_receipt_files(&receipt, &changed, &mut b, d, &c),
            Err(Error::Conflict("private PWE creation file closure differs"))
        ));
        let with_self = cmd::object(vec![(
            "files",
            crate::source_revisions::file_refs(&files, false),
        )]);
        let (d, c, mut b) = setup();
        assert!(matches!(
            check_receipt_files(&with_self, &files, &mut b, d, &c),
            Err(Error::Conflict("private PWE creation file closure differs"))
        ));
        let mut refs = refs;
        let JsonValue::Object(ref mut bindings) = refs else {
            unreachable!()
        };
        bindings[0].0 = tos_foundation::JsonString::from_utf8("source-create-receipt.json");
        let includes_self = cmd::object(vec![("files", refs)]);
        let (d, c, mut b) = setup();
        assert!(matches!(
            check_receipt_files(&includes_self, &files, &mut b, d, &c),
            Err(Error::Conflict(
                "private PWE creation receipt includes itself"
            ))
        ));
    }
}
