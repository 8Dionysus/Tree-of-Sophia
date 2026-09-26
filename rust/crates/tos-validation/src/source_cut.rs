//! Actual source-carrier adapter for executable Item rules. Current member
//! paths and raw bytes come from an anchored CorpusCutReader; retained bases
//! stay in that reader and never become competing current record owners.
//! Carrier coverage is weaker than source-owner admission coverage.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use tos_foundation::RelativePath;
use tos_source_store::{CorpusCutReader, SourceMembershipV1};

use crate::item_rules::{
    ItemFamilyReport, ItemLimits, ItemPayload, ItemRefusal, ItemRules, ItemSource,
};
use crate::record_rules::RecordFamily;

/// Exact owner schema executor, with separately enforced process custody.
/// An unknown profile/resource or incomplete execution must refuse.
pub trait CutSchemaExecutor {
    fn check(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal>;
}

/// Payload custody is outside source metadata membership. No ambient host
/// path is opened by this adapter. The custody owner hashes selected bytes.
pub trait CutPayloadReader {
    fn inspect(
        &mut self,
        path: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<ItemPayload, ItemRefusal>;
}

/// Explicit source-owner metadata-only posture. The require-local profile
/// rejects these unavailable payloads in ItemRules instead of discovering
/// an unrelated checkout or local filesystem payload.
pub struct MetadataOnlyPayloads;
impl CutPayloadReader for MetadataOnlyPayloads {
    fn inspect(
        &mut self,
        _: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<ItemPayload, ItemRefusal> {
        check(deadline, cancelled)?;
        Ok(ItemPayload::Unavailable)
    }
}

#[derive(Debug)]
pub struct SourceCutItemReport {
    pub carrier_membership: SourceMembershipV1,
    pub item_family: ItemFamilyReport,
}

/// Run the complete current *carrier* through the record endpoint index and
/// every Item manifest and native Item record. The carrier's EOF verifies raw
/// ordered membership. This does not certify the carrier contains every
/// normative source profile or disposable derived catalog companion, and
/// never issues ValidationOutcome::MechanicallyValid.
pub fn inspect_items_from_cut(
    cut: &CorpusCutReader,
    limits: ItemLimits,
    require_local_payloads: bool,
    cancelled: &AtomicBool,
    record_routes: &RecordFamily,
    schemas: &mut impl CutSchemaExecutor,
    payloads: &mut impl CutPayloadReader,
) -> Result<SourceCutItemReport, ItemRefusal> {
    check(limits.deadline, cancelled)?;
    let revision = cut.current().revision();
    let mut stream = cut.stream(revision).map_err(store_error)?;
    let mut kinds = BTreeMap::new();
    let mut manifests = Vec::new();
    let mut items = Vec::new();
    let mut index_bytes = 0usize;
    let mut total_bytes = 0u64;
    while let Some(member) = stream
        .next_member(limits.deadline, cancelled)
        .map_err(store_error)?
    {
        check(limits.deadline, cancelled)?;
        total_bytes = total_bytes
            .checked_add(member.raw.len() as u64)
            .filter(|bytes| *bytes <= limits.max_total_bytes)
            .ok_or(ItemRefusal::Budget)?;
        let path = member.path.as_str();
        if path.starts_with("ToS/source-witnesses/") && path.ends_with("/item.manifest.json") {
            reserve(&mut index_bytes, path.len(), limits.max_state_bytes)?;
            manifests.push(member.path.clone());
        }
        // These are source-owned JSON record fields, never decoded manifest
        // keys or the weaker stable_ids index claims supplied by the carrier.
        // Native Item compatibility retains its ordinary decoded-field JSON
        // profile. Named strict declared-profile validation remains separate.
        if path.ends_with(".json") {
            if member.raw.len() > limits.max_member_bytes {
                return Err(ItemRefusal::Budget);
            }
            if let Some(carrier) = record_routes
                .classify_current_member(path, &member.raw)
                .map_err(|error| ItemRefusal::Unsupported(format!("{error:?}")))?
            {
                let id = carrier.id.as_str();
                let kind = carrier.kind.as_str();
                reserve(
                    &mut index_bytes,
                    id.len() + kind.len(),
                    limits.max_state_bytes,
                )?;
                if kinds.insert(id.to_owned(), kind.to_owned()).is_some() {
                    return Err(ItemRefusal::Source("duplicate current record ID".into()));
                }
                if kind == "item" {
                    reserve(&mut index_bytes, path.len(), limits.max_state_bytes)?;
                    items.push(member.path.clone());
                }
            }
        }
    }
    let membership = stream
        .coverage()
        .ok_or_else(|| ItemRefusal::Source("incomplete source carrier".into()))?;
    // Index state and rule state share one explicit logical allocation quota.
    let mut rule_limits = limits;
    rule_limits.max_state_bytes = limits
        .max_state_bytes
        .checked_sub(index_bytes)
        .ok_or(ItemRefusal::Budget)?;
    let mut rules = ItemRules::new(rule_limits, require_local_payloads);
    let mut source = CutItemSource {
        cut,
        kinds: &kinds,
        cancelled,
        schemas,
        payloads,
    };
    for path in manifests {
        check(limits.deadline, cancelled)?;
        rules.inspect_manifest(&mut source, path.as_str())?;
    }
    for path in items {
        check(limits.deadline, cancelled)?;
        let member = cut
            .read_member(
                revision,
                &path,
                limits.max_member_bytes as u64,
                limits.deadline,
                cancelled,
            )
            .map_err(store_error)?;
        rules.inspect_item_record(&mut source, path.as_str(), &member.raw)?;
    }
    check(limits.deadline, cancelled)?;
    Ok(SourceCutItemReport {
        carrier_membership: membership,
        item_family: rules.finish(),
    })
}

struct CutItemSource<'a, S, P> {
    cut: &'a CorpusCutReader,
    kinds: &'a BTreeMap<String, String>,
    cancelled: &'a AtomicBool,
    schemas: &'a mut S,
    payloads: &'a mut P,
}

impl<S: CutSchemaExecutor, P: CutPayloadReader> ItemSource for CutItemSource<'_, S, P> {
    fn metadata(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<Option<Vec<u8>>, ItemRefusal> {
        check(deadline, self.cancelled)?;
        let path = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("source path".into()))?;
        if self.cut.current().member(&path).is_none() {
            return Ok(None);
        }
        self.cut
            .read_member(
                self.cut.current().revision(),
                &path,
                max_bytes as u64,
                deadline,
                self.cancelled,
            )
            .map(|member| Some(member.raw))
            .map_err(store_error)
    }
    fn exists(&mut self, path: &str, deadline: Instant) -> Result<bool, ItemRefusal> {
        check(deadline, self.cancelled)?;
        let path = RelativePath::parse(path)
            .map_err(|_| ItemRefusal::Unsupported("source path".into()))?;
        Ok(self
            .cut
            .presence(self.cut.current().revision(), &path)
            .is_some())
    }
    fn schema(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
    ) -> Result<bool, ItemRefusal> {
        check(deadline, self.cancelled)?;
        self.schemas
            .check(path, raw, contract, deadline, self.cancelled)
    }
    fn payload(&mut self, path: &str, deadline: Instant) -> Result<ItemPayload, ItemRefusal> {
        check(deadline, self.cancelled)?;
        self.payloads.inspect(path, deadline, self.cancelled)
    }
    fn record_kind(&mut self, id: &str, deadline: Instant) -> Result<Option<String>, ItemRefusal> {
        check(deadline, self.cancelled)?;
        Ok(self.kinds.get(id).cloned())
    }
}

fn reserve(total: &mut usize, bytes: usize, max: usize) -> Result<(), ItemRefusal> {
    *total = total
        .checked_add(bytes)
        .filter(|n| *n <= max)
        .ok_or(ItemRefusal::Budget)?;
    Ok(())
}
fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(ItemRefusal::Source("source cut cancelled".into()));
    }
    if Instant::now() >= deadline {
        return Err(ItemRefusal::Deadline);
    }
    Ok(())
}
fn store_error(error: impl std::fmt::Display) -> ItemRefusal {
    ItemRefusal::Source(error.to_string())
}
