//! Individual archive integrity over genuine same-RR DATA. This does not
//! prove the package's revision chain, immutable creation or form history.
use super::{PgPrivatePweArchiveData, overflow};
use crate::source_command::{
    self as cmd, SourceCommandError as Error, SourceCommandResult as Result,
};
use crate::source_native_cli::private_assessment::{OperationBudget, scope_admission::ScopeAccess};
use crate::source_native_roles::{admitted_canonical, admitted_document};
use crate::source_text_private_store::PrivatePackage;
use std::{mem::size_of, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::{Digest256, Digest256Hasher, JsonDocument, JsonLimits, JsonString, JsonValue};

pub(crate) struct PrivatePweArchivedFile<'data> {
    name: String,
    raw: &'data [u8],
}
impl PrivatePweArchivedFile<'_> {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }
    pub(crate) fn raw(&self) -> &[u8] {
        self.raw
    }
}
/// No raw constructor or Clone. Borrowed blobs retain the real archive; the
/// raw manifest is strictly decoded and every logical file/digest is checked.
/// This value supplies no Claim target, permission, native basis or report.
pub(crate) struct PgPrivatePweArchivedFiles<'data, 'plan, 'history> {
    archive: &'data PgPrivatePweArchiveData<'plan, 'history>,
    checked: CheckedManifest<'data>,
}
impl PgPrivatePweArchivedFiles<'_, '_, '_> {
    pub(crate) fn files(&self) -> &[PrivatePweArchivedFile<'_>] {
        &self.checked.files
    }
    pub(crate) fn manifest(&self) -> &JsonDocument {
        &self.checked.document
    }
    pub(crate) fn archive_reference(&self) -> &str {
        self.archive.reference()
    }
}
pub(super) struct CheckedManifest<'data> {
    document: JsonDocument,
    files: Vec<PrivatePweArchivedFile<'data>>,
}

// Mirrors the old structural JsonValue equality, including object field order
// and exact Number/string representations. Canonical equality would be weaker.
fn same_value(
    a: &JsonValue,
    b: &JsonValue,
    access: &mut ScopeAccess<'_>,
    depth: usize,
) -> Result<bool> {
    access.inspect(2 * size_of::<JsonValue>(), 1)?;
    if depth > JsonLimits::default().max_depth {
        return Err(overflow());
    }
    Ok(match (a, b) {
        (JsonValue::Null, JsonValue::Null) => true,
        (JsonValue::Bool(a), JsonValue::Bool(b)) => a == b,
        (JsonValue::Number(a), JsonValue::Number(b)) => {
            access.inspect(
                a.lexeme
                    .len()
                    .checked_add(b.lexeme.len())
                    .ok_or_else(overflow)?,
                1,
            )?;
            a == b
        }
        (JsonValue::String(a), JsonValue::String(b)) => same_string(a, b, access)?,
        (JsonValue::Array(a), JsonValue::Array(b)) => {
            if a.len() != b.len() {
                return Ok(false);
            }
            for (a, b) in a.iter().zip(b) {
                if !same_value(a, b, access, depth + 1)? {
                    return Ok(false);
                }
            }
            true
        }
        (JsonValue::Object(a), JsonValue::Object(b)) => {
            if a.len() != b.len() {
                return Ok(false);
            }
            for ((ak, av), (bk, bv)) in a.iter().zip(b) {
                if !same_string(ak, bk, access)? || !same_value(av, bv, access, depth + 1)? {
                    return Ok(false);
                }
            }
            true
        }
        _ => false,
    })
}
fn same_string(a: &JsonString, b: &JsonString, access: &mut ScopeAccess<'_>) -> Result<bool> {
    let units = a
        .units()
        .len()
        .checked_add(b.units().len())
        .and_then(|n| n.checked_mul(size_of::<u16>()))
        .ok_or_else(overflow)?;
    let utf8 = a
        .as_str()
        .map_or(0, str::len)
        .checked_add(b.as_str().map_or(0, str::len))
        .ok_or_else(overflow)?;
    access.inspect(units.checked_add(utf8).ok_or_else(overflow)?, 1)?;
    Ok(a == b)
}
fn member<'data>(
    archive: &'data PrivatePackage,
    name: &str,
    access: &mut ScopeAccess<'_>,
) -> Result<Option<&'data [u8]>> {
    // Pay actual keys before comparison; no guessed BTree search tariff.
    for (key, raw) in archive {
        if access.text_equal(key, name)? {
            return Ok(Some(raw));
        }
    }
    Ok(None)
}
fn digest(raw: &[u8], budget: &mut OperationBudget) -> Result<String> {
    // SHA input, fixed hasher/result, BOTH String holders/backings and the
    // Result carrier before hashing/emission. Foundation to_prefixed calls
    // to_hex (temporary64), then formats prefix+hex (result71). Each spelling
    // has its own character work; the first is not free because it is dropped.
    // This prices these logical operands, not native provider
    // latency, allocator capacity, driver buffering, Drop or process RSS.
    budget.charge_work(raw.len().checked_add(64 + 71).ok_or_else(overflow)?)?;
    budget.charge_bytes(
        size_of::<Digest256Hasher>()
            + size_of::<Digest256>()
            + 2 * size_of::<String>()
            + size_of::<Result<String>>()
            + 64
            + 71,
    )?;
    Ok(Digest256::of_bytes(raw).to_prefixed())
}
fn json_string(raw: &str, budget: &mut OperationBudget) -> Result<JsonString> {
    // JsonString::from_utf8 stores UTF16 plus a UTF8 copy. UTF16 units <=
    // UTF8 bytes. Explicit buffer contents only; actual allocator slack OPEN.
    budget.charge_work(raw.len().checked_mul(2).ok_or_else(overflow)?)?;
    budget.charge_bytes(raw.len().checked_mul(3).ok_or_else(overflow)?)?;
    Ok(JsonString::from_utf8(raw))
}
fn manifest_bytes(binding: &JsonValue, access: &mut ScopeAccess<'_>) -> Result<u64> {
    let value = access.field(binding, "bytes")?;
    if let JsonValue::Number(number) = value {
        // Parsing the manifest previously does not pay this later numeric
        // conversion. Admit its actual lexeme before as_u64 reads it again.
        access.inspect(number.lexeme.len(), 1)?;
    }
    value
        .as_u64()
        .ok_or(Error::Invalid("expected integer field"))
}

/// Full manifest/file/fixity/package-revision law of the maintained archive
/// reader. Output uses borrowed raw blobs, avoiding its old whole-buffer
/// clones; duplicate logical files may still refer to the same genuine blob.
/// The caller must provide an authentic receipt from the bound complete plan.
fn verify<'data>(
    archive: &'data PrivatePackage,
    declared: &JsonValue,
    receipt: &JsonValue,
    reference: &str,
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<CheckedManifest<'data>> {
    crate::source_creation_store::active(deadline, cancelled)?;
    budget.charge_bytes(
        size_of::<CheckedManifest<'_>>() + size_of::<Result<CheckedManifest<'_>>>(),
    )?;
    if archive.len() > 65 {
        return Err(Error::Invalid("private Claim archive file count"));
    }
    let mut total = 0usize;
    for raw in archive.values() {
        budget.charge_work(1)?;
        total = total.checked_add(raw.len()).ok_or_else(overflow)?;
    }
    if total > 16_777_216 {
        return Err(Error::Invalid("private Claim archive byte budget"));
    }
    let manifest_raw = member(
        archive,
        "manifest.json",
        &mut ScopeAccess::admitted_operation(budget, deadline, cancelled),
    )?
    .ok_or(Error::Invalid("private Claim archive manifest absent"))?;
    let document = admitted_document(manifest_raw, budget, deadline, cancelled)?;
    let (bindings, previous_revision) = {
        let mut access = ScopeAccess::admitted_operation(budget, deadline, cancelled);
        access.exact_keys(
            document.root(),
            &[
                "schema_version",
                "source_path",
                "source",
                "revision",
                "files",
            ],
        )?;
        let schema = access.text(document.root(), "schema_version")?;
        let path = access.text(document.root(), "source_path")?;
        let expected_path = access.text(declared, "source_path")?;
        let source = access.field(document.root(), "source")?;
        let subject = access.field(receipt, "previous_source")?;
        let revision = access.text(document.root(), "revision")?;
        let expected_revision = access.text(receipt, "previous_revision")?;
        let expected_reference = access.text(receipt, "archive_path")?;
        if !access.text_equal(expected_reference, reference)? {
            return Err(Error::Conflict("private Claim archive path differs"));
        }
        if !access.text_equal(schema, "tos_source_package_archive_v1")?
            || !access.text_equal(path, expected_path)?
            || !same_value(source, subject, &mut access, 0)?
            || !access.text_equal(revision, expected_revision)?
        {
            return Err(Error::Conflict("private Claim archive binding differs"));
        }
        let bindings = access
            .field(document.root(), "files")?
            .as_object()
            .ok_or(Error::Invalid("private Claim archive file map"))?;
        if bindings.is_empty() || bindings.len() > 64 {
            return Err(Error::Invalid("private Claim archive file count"));
        }
        (bindings, expected_revision)
    };
    let slots = bindings
        .len()
        .checked_mul(size_of::<PrivatePweArchivedFile<'_>>() + size_of::<(JsonString, JsonValue)>())
        .ok_or_else(overflow)?;
    budget.charge_bytes(slots)?;
    let mut files = Vec::new();
    files
        .try_reserve_exact(bindings.len())
        .map_err(|_| overflow())?;
    let mut refs = Vec::new();
    refs.try_reserve_exact(bindings.len())
        .map_err(|_| overflow())?;
    for (name, binding) in bindings {
        ScopeAccess::admitted_operation(budget, deadline, cancelled)
            .inspect(size_of::<(JsonString, JsonValue)>(), 1)?;
        let name = name
            .as_str()
            .ok_or(Error::Invalid("Claim archive filename"))?;
        let (blob, hash, bytes) = {
            let mut access = ScopeAccess::admitted_operation(budget, deadline, cancelled);
            access.inspect(name.len(), 1)?;
            if name.is_empty() || name.contains('/') || name.starts_with('.') {
                return Err(Error::Invalid("Claim archive filename"));
            }
            access.exact_keys(binding, &["blob", "sha256", "bytes"])?;
            let hash = access.text(binding, "sha256")?;
            let blob = access.text(binding, "blob")?;
            access.inspect(hash.len().checked_add(blob.len()).ok_or_else(overflow)?, 1)?;
            if !hash.starts_with("sha256:")
                || !blob.ends_with(".blob")
                || blob.strip_suffix(".blob") != hash.get(7..)
            {
                return Err(Error::Invalid("Claim archive blob locator"));
            }
            let bytes = manifest_bytes(binding, &mut access)?;
            (blob, hash, bytes)
        };
        let raw = member(
            archive,
            blob,
            &mut ScopeAccess::admitted_operation(budget, deadline, cancelled),
        )?
        .ok_or(Error::Conflict("Claim archive blob absent"))?;
        let actual = digest(raw, budget)?;
        if !ScopeAccess::admitted_operation(budget, deadline, cancelled)
            .text_equal(&actual, hash)?
            || raw.len() as u64 != bytes
        {
            return Err(Error::Conflict("Claim archive fixity differs"));
        }
        // The exact old file_refs(files,false) projection. Explicit slots,
        // key/value strings and <=20 ASCII u64 digits precede construction.
        budget.charge_bytes(2 * size_of::<(JsonString, JsonValue)>() + 20 + name.len())?;
        budget.charge_work(name.len().checked_add(20).ok_or_else(overflow)?)?;
        let mut fields = Vec::new();
        fields.try_reserve_exact(2).map_err(|_| overflow())?;
        fields.push((
            json_string("sha256", budget)?,
            JsonValue::String(json_string(&actual, budget)?),
        ));
        fields.push((json_string("bytes", budget)?, cmd::number(raw.len() as u64)));
        refs.push((json_string(name, budget)?, JsonValue::Object(fields)));
        files.push(PrivatePweArchivedFile {
            name: name.to_owned(),
            raw,
        });
    }
    // Every physical non-manifest member must be used. This is a full scan,
    // not a top-N subset; repeated logical references are allowed as before.
    for member in archive.keys() {
        let mut access = ScopeAccess::admitted_operation(budget, deadline, cancelled);
        if access.text_equal(member, "manifest.json")? {
            continue;
        }
        let mut used = false;
        for (_, binding) in bindings {
            let blob = access.text(binding, "blob")?;
            used |= access.text_equal(member, blob)?;
        }
        if !used {
            return Err(Error::Conflict("Claim archive package differs"));
        }
    }
    let encoded = admitted_canonical(&JsonValue::Object(refs), budget, deadline, cancelled)?;
    let revision = digest(&encoded, budget)?;
    let mut access = ScopeAccess::admitted_operation(budget, deadline, cancelled);
    if !access.text_equal(&revision, previous_revision)? {
        return Err(Error::Conflict("Claim archive package differs"));
    }
    let mut stream_present = false;
    for file in &files {
        stream_present |= access.text_equal(&file.name, "source-claims.jsonl")?;
    }
    if !stream_present {
        return Err(Error::Conflict("Claim archive package differs"));
    }
    crate::source_creation_store::active(deadline, cancelled)?;
    Ok(CheckedManifest { document, files })
}

pub(super) fn check_bound_archive<'data, 'plan, 'history>(
    archive: &'data PgPrivatePweArchiveData<'plan, 'history>,
    declared: &JsonValue,
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<PgPrivatePweArchivedFiles<'data, 'plan, 'history>> {
    let request = &archive.plan.requests()[archive.request_index];
    let history = archive.plan.history_document().ok_or(Error::Conflict(
        "private PWE archive request history absent",
    ))?;
    let receipt = {
        let mut access = ScopeAccess::admitted_operation(budget, deadline, cancelled);
        access
            .field(history.root(), "receipts")?
            .as_array()
            .and_then(|a| a.get(request.receipt_index()))
            .ok_or(Error::Conflict("private PWE archive receipt absent"))?
    };
    budget.charge_bytes(
        size_of::<PgPrivatePweArchivedFiles<'_, '_, '_>>()
            + size_of::<Result<PgPrivatePweArchivedFiles<'_, '_, '_>>>(),
    )?;
    let checked = verify(
        &archive.snapshot.members,
        declared,
        receipt,
        request.reference(),
        budget,
        deadline,
        cancelled,
    )?;
    Ok(PgPrivatePweArchivedFiles { archive, checked })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::{collections::BTreeMap, sync::atomic::Ordering, time::Duration};
    const PATH: &str = "private/claims/selected/source-claims.jsonl";
    const REFERENCE: &str = "private/.record-revisions/exact-fixture";
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
    fn decoded(value: &Value) -> JsonValue {
        cmd::parse(&serde_json::to_vec(value).unwrap()).unwrap()
    }
    fn fixture() -> (PrivatePackage, PrivatePackage, JsonValue, JsonValue) {
        // Raw manifest DATA only, not a real Claim/owner/RR/permission fixture.
        // Two logical names intentionally use one physical blob, as permitted
        // by the old archive reader's used_blobs set.
        let files = BTreeMap::from([
            (
                "source-claims.jsonl".to_owned(),
                b"exact archived bytes\n".to_vec(),
            ),
            (
                "quoted-\"-Я.txt".to_owned(),
                b"exact archived bytes\n".to_vec(),
            ),
        ]);
        let revision = crate::source_revisions::revision(&files).unwrap();
        let refs = crate::source_revisions::file_refs(&files, true);
        let refs: Value = serde_json::from_slice(&cmd::published(&refs).unwrap()).unwrap();
        let subject = json!({"id":"tos.claim.selected","version":1});
        let manifest = json!({"schema_version":"tos_source_package_archive_v1","source_path":PATH,
            "source":subject,"revision":revision,"files":refs});
        let receipt = decoded(
            &json!({"previous_source":subject,"previous_revision":revision,"archive_path":REFERENCE}),
        );
        let mut archive = BTreeMap::new();
        for raw in files.values() {
            archive.insert(
                format!("{}.blob", Digest256::of_bytes(raw).to_hex()),
                raw.clone(),
            );
        }
        archive.insert(
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        );
        (
            archive,
            files,
            decoded(&json!({"source_path":PATH})),
            receipt,
        )
    }
    fn edit(archive: &mut PrivatePackage, run: impl FnOnce(&mut Value)) {
        let mut value: Value = serde_json::from_slice(&archive["manifest.json"]).unwrap();
        run(&mut value);
        archive.insert("manifest.json".into(), serde_json::to_vec(&value).unwrap());
    }
    fn checked<'a>(
        archive: &'a PrivatePackage,
        declared: &JsonValue,
        receipt: &JsonValue,
    ) -> Result<CheckedManifest<'a>> {
        let (d, c, mut b) = setup();
        verify(archive, declared, receipt, REFERENCE, &mut b, d, &c)
    }
    #[test]
    fn private_pwe_archive_manifest_preserves_legacy_revision_duplicate_blob_and_exact_borrow() {
        let (archive, files, declared, receipt) = fixture();
        let result = checked(&archive, &declared, &receipt).unwrap();
        assert_eq!(result.files.len(), files.len());
        for file in &result.files {
            let expected = &files[&file.name];
            assert_eq!(file.raw, expected.as_slice());
            let blob = format!("{}.blob", Digest256::of_bytes(expected).to_hex());
            assert_eq!(file.raw.as_ptr(), archive[&blob].as_ptr());
        }
        assert_eq!(archive.len(), 2); // manifest plus one blob for both files
        assert_eq!(
            cmd::text(result.document.root(), "revision").unwrap(),
            crate::source_revisions::revision(&files).unwrap()
        );
    }
    #[test]
    fn private_pwe_archive_manifest_rejects_changed_source_revision_and_receipt_path() {
        for field in ["source_path", "revision", "schema_version"] {
            let (mut archive, _, declared, receipt) = fixture();
            edit(&mut archive, |v| v[field] = json!("different"));
            assert!(matches!(
                checked(&archive, &declared, &receipt),
                Err(Error::Conflict("private Claim archive binding differs"))
            ));
        }
        let (archive, _, declared, mut receipt) = fixture();
        cmd::set(&mut receipt, "archive_path", cmd::string("other")).unwrap();
        assert!(matches!(
            checked(&archive, &declared, &receipt),
            Err(Error::Conflict("private Claim archive path differs"))
        ));
    }
    #[test]
    fn private_pwe_archive_manifest_rejects_wrong_fixity_unused_blob_and_missing_stream() {
        let (mut archive, _, declared, receipt) = fixture();
        edit(&mut archive, |v| {
            v["files"]["source-claims.jsonl"]["bytes"] = json!(1)
        });
        assert!(matches!(
            checked(&archive, &declared, &receipt),
            Err(Error::Conflict("Claim archive fixity differs"))
        ));
        let (mut archive, _, declared, receipt) = fixture();
        archive.insert("unused.blob".into(), b"unused".to_vec());
        assert!(matches!(
            checked(&archive, &declared, &receipt),
            Err(Error::Conflict("Claim archive package differs"))
        ));
        let (mut archive, _, declared, receipt) = fixture();
        edit(&mut archive, |v| {
            v["files"]
                .as_object_mut()
                .unwrap()
                .remove("source-claims.jsonl");
        });
        assert!(checked(&archive, &declared, &receipt).is_err());
    }
    #[test]
    fn private_pwe_archive_subject_keeps_structural_order_and_number_equality() {
        let (d, c, mut b) = setup();
        let a = cmd::parse(br#"{"id":"tos.claim.selected","version":1}"#).unwrap();
        let b_order = cmd::parse(br#"{"version":1,"id":"tos.claim.selected"}"#).unwrap();
        let b_number = cmd::parse(br#"{"id":"tos.claim.selected","version":1.0}"#).unwrap();
        let mut access = ScopeAccess::admitted_operation(&mut b, d, &c);
        assert!(same_value(&a, &a, &mut access, 0).unwrap());
        assert!(!same_value(&a, &b_order, &mut access, 0).unwrap());
        assert!(!same_value(&a, &b_number, &mut access, 0).unwrap());
    }
    #[test]
    fn private_pwe_archive_manifest_cancel_and_failed_hash_debit_keep_original_prefix() {
        let (archive, _, declared, receipt) = fixture();
        let (d, c, mut b) = setup();
        let before = b.test_work_remaining();
        c.store(true, Ordering::Relaxed);
        assert!(verify(&archive, &declared, &receipt, REFERENCE, &mut b, d, &c).is_err());
        assert_eq!(b.test_work_remaining(), before);
        let (_, _, mut b) = setup();
        b.lower_remaining(1_048_576, 1).unwrap();
        assert!(matches!(
            digest(b"actual hash input", &mut b),
            Err(Error::Invalid("assessment cumulative work budget"))
        ));
        assert_eq!(b.test_work_remaining(), 1);
        assert!(b.limits().is_err());
    }
    #[test]
    fn private_pwe_manifest_bytes_pays_numeric_lexeme_before_conversion() {
        use tos_foundation::{JsonNumber, JsonNumberKind};
        // Number DATA only: this fixture creates no archive/owner capability.
        let value = JsonValue::Object(vec![(
            JsonString::from_utf8("bytes"),
            JsonValue::Number(JsonNumber {
                kind: JsonNumberKind::Int,
                lexeme: "9".repeat(4096),
            }),
        )]);
        let (d, c, mut b) = setup();
        b.lower_remaining(1_048_576, 32).unwrap();
        assert!(matches!(
            manifest_bytes(&value, &mut ScopeAccess::admitted_operation(&mut b, d, &c)),
            Err(Error::Invalid("assessment cumulative work budget"))
        ));
        // Actual get: one header visit + one field visit + text_equal(5+5,1).
        // The subsequent lexeme debit fails before as_u64; keep that prefix.
        assert_eq!(b.test_work_remaining(), 19);
        assert!(b.limits().is_err());
        let value = cmd::object(vec![("bytes", cmd::number(255))]);
        let (d, c, mut b) = setup();
        assert_eq!(
            manifest_bytes(&value, &mut ScopeAccess::admitted_operation(&mut b, d, &c)).unwrap(),
            255
        );
    }
    #[test]
    fn private_pwe_archive_digest_prepays_temporary_hex_and_prefixed_backing() {
        let required = size_of::<Digest256Hasher>()
            + size_of::<Digest256>()
            + 2 * size_of::<String>()
            + size_of::<Result<String>>()
            + 64
            + 71;
        let (_, _, mut b) = setup();
        b.lower_remaining(required - 1, 1024).unwrap();
        assert!(matches!(
            digest(b"x", &mut b),
            Err(Error::Invalid("assessment cumulative byte budget"))
        ));
        assert_eq!(b.test_work_remaining(), 1024 - (1 + 64 + 71));
        assert!(b.limits().is_err());
        let (_, _, mut b) = setup();
        assert_eq!(
            digest(b"x", &mut b).unwrap(),
            Digest256::of_bytes(b"x").to_prefixed()
        );
    }
}
