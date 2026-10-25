//! Exact history-derived archive requests, not archive/lineage authority.
//! No query executes here. The future concrete SAME-RR archive adapter must
//! recheck actual current owner/policy/target and independently pay each use.
use super::super::{HISTORY, MAX_REVISIONS, claim_id};
use super::{
    CurrentClaimReadScope, Error, OperationBudget, PgClaimProducerReadPolicy, Result, ScopeAccess,
    admitted_document, package_stream,
};
use std::{mem::size_of, sync::atomic::AtomicBool, time::Instant};
use tos_foundation::{Digest256, Digest256Hasher, JsonDocument, JsonValue};

/// Private construction follows an actual immutable producer history. This
/// only identifies a request needed by package integrity; it grants no query,
/// source Claim, NativeAccount, native basis, current permission or admission.
pub(crate) struct PrivatePweArchiveRequest {
    receipt_index: usize,
    reference: String,
    subject: String,
    revision: String,
}
impl PrivatePweArchiveRequest {
    pub(crate) fn receipt_index(&self) -> usize {
        self.receipt_index
    }
    pub(crate) fn reference(&self) -> &str {
        &self.reference
    }
    pub(crate) fn subject(&self) -> &str {
        &self.subject
    }
    pub(crate) fn revision(&self) -> &str {
        &self.revision
    }
}
pub(crate) struct PrivatePweArchivePlan<'history> {
    producer: PgClaimProducerReadPolicy<'history>,
    history: Option<JsonDocument>,
    requests: Vec<PrivatePweArchiveRequest>,
}
impl PrivatePweArchivePlan<'_> {
    pub(crate) fn producer(&self) -> &PgClaimProducerReadPolicy<'_> {
        &self.producer
    }
    pub(crate) fn history_document(&self) -> Option<&JsonDocument> {
        self.history.as_ref()
    }
    pub(crate) fn requests(&self) -> &[PrivatePweArchiveRequest] {
        &self.requests
    }
}
fn overflow() -> Error {
    Error::Unsupported("private PWE archive plan overflow")
}

/// Data plan ONLY: source bytes come from a privately constructed producer,
/// current namespace from a privately constructed read scope. It cannot mint
/// permission from an archive string, a raw JSON history or copied whitelist.
/// The actual protected factory must establish SAME owner again at query use;
/// immutable metadata reuse does not carry prepayment or authority to B.
pub(crate) fn parse_selected_archive_requests<'history>(
    producer: PgClaimProducerReadPolicy<'history>,
    selected: &CurrentClaimReadScope<'_>,
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<PrivatePweArchivePlan<'history>> {
    package_stream(
        &producer.producer().entry().members,
        budget,
        deadline,
        cancelled,
    )?;
    let prefix = {
        let mut access = ScopeAccess::admitted_operation(budget, deadline, cancelled);
        let path = access.text(producer.declared(), "source_path")?;
        if !access.text_equal(path, selected.path())? {
            return Err(Error::Conflict("private PWE archive source path differs"));
        }
        // selected_scope already proved EXACT prefix/claims/<slug>/stream.
        // Borrow that actual namespace, never a caller-provided context map.
        access.inspect(
            selected.path().len().checked_mul(3).ok_or_else(overflow)?,
            3,
        )?;
        let package = selected
            .path()
            .rsplit_once('/')
            .ok_or(Error::Invalid("selected Claim package path"))?
            .0;
        let claims = package
            .rsplit_once('/')
            .ok_or(Error::Invalid("selected Claim package home"))?
            .0;
        claims
            .strip_suffix("claims")
            .ok_or(Error::Invalid("selected Claim family home"))?
    };
    let raw = {
        let mut found = None;
        let mut access = ScopeAccess::admitted_operation(budget, deadline, cancelled);
        for (name, raw) in &producer.producer().entry().members {
            if access.text_equal(name, HISTORY)? {
                found = Some(raw.as_slice());
            }
        }
        found
    };
    let source_path = selected.path();
    let (history, requests) = parse_requests(
        raw,
        producer.declared(),
        source_path,
        prefix,
        budget,
        deadline,
        cancelled,
    )?;
    Ok(PrivatePweArchivePlan {
        producer,
        history,
        requests,
    })
}

fn parse_requests(
    raw: Option<&[u8]>,
    declared: &JsonValue,
    source_path: &str,
    prefix: &str,
    budget: &mut OperationBudget,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(Option<JsonDocument>, Vec<PrivatePweArchiveRequest>)> {
    crate::source_creation_store::active(deadline, cancelled)?;
    budget.charge_bytes(
        size_of::<PrivatePweArchivePlan<'_>>()
            + size_of::<Result<PrivatePweArchivePlan<'_>>>()
            + size_of::<Option<JsonDocument>>()
            + size_of::<Vec<PrivatePweArchiveRequest>>(),
    )?;
    let Some(raw) = raw else {
        // Actual physical absence is a navigation observation only. The old
        // package_state must still reject v>1 without history; this returns
        // neither a ready source target nor a successful lineage verdict.
        return Ok((None, Vec::new()));
    };
    let document = admitted_document(raw, budget, deadline, cancelled)?;
    let mut access = ScopeAccess::admitted_operation(budget, deadline, cancelled);
    access.exact_keys(
        document.root(),
        &["schema_version", "source_path", "receipts"],
    )?;
    super::equals(
        &mut access,
        document.root(),
        "schema_version",
        "tos_claim_revision_history_v1",
    )?;
    super::equals(&mut access, document.root(), "source_path", source_path)?;
    let receipts = super::array(&mut access, document.root(), "receipts")?;
    if receipts.len() > MAX_REVISIONS {
        return Err(Error::Invalid("private Claim history capacity"));
    }
    // Actual SOURCE maximum, no top-N truncation or new reservation currency.
    access.inspect(
        receipts
            .len()
            .checked_mul(size_of::<PrivatePweArchiveRequest>())
            .ok_or_else(overflow)?,
        1,
    )?;
    let mut requests: Vec<PrivatePweArchiveRequest> = Vec::new();
    requests
        .try_reserve_exact(receipts.len())
        .map_err(|_| Error::Unsupported("private PWE archive request backing"))?;
    for (receipt_index, receipt) in receipts.iter().enumerate() {
        let previous = access.field(receipt, "previous_source")?;
        let subject = access.text(previous, "id")?;
        access.inspect(subject.len(), 1)?;
        if !claim_id(subject) {
            return Err(Error::Invalid("private PWE archive subject identity"));
        }
        let mut allowed = false;
        let ids = super::array(&mut access, declared, "allowed_claim_ids")?;
        if ids.is_empty() || ids.len() > 32 {
            return Err(Error::Invalid("private PWE archive declared identities"));
        }
        for value in ids {
            let id = value
                .as_str()
                .ok_or(Error::Invalid("private PWE archive declared identity"))?;
            allowed |= access.text_equal(id, subject)?;
        }
        if !allowed {
            return Err(Error::Denied(
                "private PWE archive subject outside producer declaration",
            ));
        }
        let revision = access.text(receipt, "previous_revision")?;
        access.inspect(revision.len(), 1)?;
        let rev = revision
            .strip_prefix("sha256:")
            .ok_or(Error::Invalid("private Claim archive revision"))?;
        let reference = canonical_reference(prefix, subject, rev, &mut access)?;
        super::equals(&mut access, receipt, "archive_path", &reference)?;
        for previous in &requests {
            if access.text_equal(previous.reference(), &reference)? {
                return Err(Error::Conflict(
                    "private Claim archive path is not canonical",
                ));
            }
        }
        // The independent producer declaration is never reconstructed from
        // this receipt list. Sibling requests are needed for PACKAGE chain
        // verification, not permission to semantically use sibling Claims.
        access.inspect(
            subject
                .len()
                .checked_add(revision.len())
                .ok_or_else(overflow)?,
            2,
        )?;
        requests.push(PrivatePweArchiveRequest {
            receipt_index,
            reference,
            subject: subject.to_owned(),
            revision: revision.to_owned(),
        });
    }
    crate::source_creation_store::active(deadline, cancelled)?;
    Ok((Some(document), requests))
}

fn canonical_reference(
    prefix: &str,
    subject: &str,
    revision_suffix: &str,
    access: &mut ScopeAccess<'_>,
) -> Result<String> {
    // Exactly the unchanged archive_ref namespace/ID-hash/raw-revision recipe.
    // No lowercasing/normalization of receipt data and no latest fallback.
    let len = prefix
        .len()
        .checked_add(".record-revisions/".len())
        .and_then(|n| n.checked_add(64 + 1))
        .and_then(|n| n.checked_add(revision_suffix.len()))
        .ok_or_else(overflow)?;
    access.inspect(
        size_of::<Digest256Hasher>() + size_of::<Digest256>() + 2 * size_of::<String>() + 64,
        subject.len().checked_add(1).ok_or_else(overflow)?,
    )?;
    access.inspect(len, len.checked_add(1).ok_or_else(overflow)?)?;
    let hash = Digest256::of_bytes(subject.as_bytes()).to_hex();
    let mut reference = String::new();
    reference
        .try_reserve_exact(len)
        .map_err(|_| Error::Unsupported("private PWE canonical archive path backing"))?;
    reference.push_str(prefix);
    reference.push_str(".record-revisions/");
    reference.push_str(&hash);
    reference.push('-');
    reference.push_str(revision_suffix);
    Ok(reference)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_command as cmd;
    use serde_json::json;
    use std::time::Duration;

    const PATH: &str =
        "ToS/source-witnesses/owner-local/selected/claims/rehearsal/source-claims.jsonl";
    const PREFIX: &str = "ToS/source-witnesses/owner-local/selected/";
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
    fn raw_history(duplicate: bool) -> Vec<u8> {
        let id = "tos.claim.selected";
        let revision = format!("sha256:{}", "a".repeat(64));
        let archive = format!(
            "{}.record-revisions/{}-{}",
            PREFIX,
            Digest256::of_bytes(id.as_bytes()).to_hex(),
            &revision[7..]
        );
        let receipt = json!({"previous_source":{"id":id},"previous_revision":revision,"archive_path":archive});
        let receipts = if duplicate {
            vec![receipt.clone(), receipt]
        } else {
            vec![receipt]
        };
        serde_json::to_vec(&json!({"schema_version":"tos_claim_revision_history_v1","source_path":PATH,"receipts":receipts})).unwrap()
    }
    fn declaration() -> JsonValue {
        cmd::parse(
            &serde_json::to_vec(
                &json!({"source_path":PATH,"allowed_claim_ids":["tos.claim.selected"]}),
            )
            .unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn private_pwe_archive_request_recipe_matches_actual_legacy_required_archives() {
        let raw = raw_history(false);
        let declared = declaration();
        let (d, c, mut b) = setup();
        let (history, requests) =
            parse_requests(Some(&raw), &declared, PATH, PREFIX, &mut b, d, &c).unwrap();
        let mut package = std::collections::BTreeMap::new();
        package.insert(HISTORY.to_owned(), raw);
        let context =
            cmd::parse(&serde_json::to_vec(&json!({"private_prefix":PREFIX})).unwrap()).unwrap();
        let legacy = super::super::super::required_archives(&package, &declared, &context).unwrap();
        assert!(history.is_some());
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].receipt_index(), 0);
        assert_eq!(requests[0].reference(), legacy[0]);
        assert_eq!(requests[0].subject(), "tos.claim.selected");
    }
    #[test]
    fn private_pwe_archive_requests_reject_duplicate_and_undelegated_subject() {
        let declared = declaration();
        let raw = raw_history(true);
        let (d, c, mut b) = setup();
        assert!(matches!(
            parse_requests(Some(&raw), &declared, PATH, PREFIX, &mut b, d, &c),
            Err(Error::Conflict(
                "private Claim archive path is not canonical"
            ))
        ));
        let declared =
            cmd::parse(br#"{"source_path":"unused","allowed_claim_ids":["tos.claim.other"]}"#)
                .unwrap();
        let raw = raw_history(false);
        let (d, c, mut b) = setup();
        assert!(matches!(
            parse_requests(Some(&raw), &declared, PATH, PREFIX, &mut b, d, &c),
            Err(Error::Denied(
                "private PWE archive subject outside producer declaration"
            ))
        ));
    }
    #[test]
    fn private_pwe_archive_missing_history_is_only_an_empty_request_observation() {
        let (d, c, mut b) = setup();
        let (history, requests) =
            parse_requests(None, &declaration(), PATH, PREFIX, &mut b, d, &c).unwrap();
        assert!(history.is_none());
        assert!(requests.is_empty());
        // This helper exposes neither PackageState nor NativeAccount/target;
        // the unchanged package_state enforces versions/initial-stream rules.
    }
    #[test]
    fn private_pwe_archive_plan_keeps_all_receipt_order_and_rejects_wrong_path() {
        let mut value: serde_json::Value = serde_json::from_slice(&raw_history(false)).unwrap();
        let id = "tos.claim.another";
        let rev = format!("sha256:{}", "b".repeat(64));
        let reference = format!(
            "{}.record-revisions/{}-{}",
            PREFIX,
            Digest256::of_bytes(id.as_bytes()).to_hex(),
            &rev[7..]
        );
        value["receipts"].as_array_mut().unwrap().push(json!({
            "previous_source":{"id":id},"previous_revision":rev,"archive_path":reference
        }));
        let raw = serde_json::to_vec(&value).unwrap();
        let declared = cmd::parse(
            &serde_json::to_vec(&json!({
                "source_path":PATH,"allowed_claim_ids":["tos.claim.selected",id]
            }))
            .unwrap(),
        )
        .unwrap();
        let (d, c, mut b) = setup();
        let (_, requests) =
            parse_requests(Some(&raw), &declared, PATH, PREFIX, &mut b, d, &c).unwrap();
        let context =
            cmd::parse(&serde_json::to_vec(&json!({"private_prefix":PREFIX})).unwrap()).unwrap();
        let mut package = std::collections::BTreeMap::new();
        package.insert(HISTORY.to_owned(), raw);
        let legacy = super::super::super::required_archives(&package, &declared, &context).unwrap();
        assert_eq!(
            requests.iter().map(|r| r.reference()).collect::<Vec<_>>(),
            legacy.iter().map(String::as_str).collect::<Vec<_>>()
        );
        assert_eq!(
            requests
                .iter()
                .map(|r| r.receipt_index())
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        value["receipts"][1]["archive_path"] =
            json!("ToS/source-witnesses/owner-local/other/.record-revisions/wrong");
        let raw = serde_json::to_vec(&value).unwrap();
        let (d, c, mut b) = setup();
        assert!(parse_requests(Some(&raw), &declared, PATH, PREFIX, &mut b, d, &c).is_err());
    }
    #[test]
    fn private_pwe_archive_plan_refuses_cancel_and_full_history_over_capacity() {
        let raw = raw_history(false);
        let declared = declaration();
        let (d, c, mut b) = setup();
        let prefix = b.test_work_remaining();
        c.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(parse_requests(Some(&raw), &declared, PATH, PREFIX, &mut b, d, &c).is_err());
        assert_eq!(b.test_work_remaining(), prefix);
        let mut value: serde_json::Value = serde_json::from_slice(&raw).unwrap();
        let receipt = value["receipts"][0].clone();
        value["receipts"] = json!(vec![receipt; MAX_REVISIONS + 1]);
        let raw = serde_json::to_vec(&value).unwrap();
        let (d, c, mut b) = setup();
        assert!(matches!(
            parse_requests(Some(&raw), &declared, PATH, PREFIX, &mut b, d, &c),
            Err(Error::Invalid("private Claim history capacity"))
        ));
    }
}
