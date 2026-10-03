//! Ordered composition for the source-foundation default owner districts.
//!
//! This assembles bounded findings over caller-authenticated inputs. It does
//! not execute schema diagnostics, construct a catalog, or issue a complete
//! source verdict. Diagnostic-v2 requests remain attached to their district
//! and retain their local insertion ordinals.

use crate::biblio_rules::BiblioClaim;
use crate::item_rules::{ItemLimits, ItemRefusal};
use crate::layer_family_rules::{LayerFamilySource, LayerPayload};
use crate::native_compound::NativeRecordHistoryReadObservation;
use crate::record_rules::RecordObservation;
use crate::source_foundation_closure::{
    SourceFoundationClosureReport, inspect_source_foundation_closure,
};
use crate::source_foundation_discovery::{
    ArtifactCorrectionReplayMap, CandidateArtifactCorrectionReplayMap,
    CandidateArtifactEvidenceProvider, CandidateArtifactInvalidSchemaProofs, Cost as DiscoveryCost,
    CurrentArtifactInvalidSchemaProofs, DiscoverySeenIds, Issue as DiscoveryIssue,
    SchemaRequest as DiscoverySchemaRequest, SourcePhysicalFacts,
    UnsupportedScope as DiscoveryUnsupported, inspect_with_cut_and_artifact_replays,
    inspect_with_cut_and_artifact_replays_and_records,
    inspect_with_cut_and_artifact_replays_and_records_with_proofs,
};
use crate::source_foundation_goldsets::{
    SourceFoundationGoldsetsReport, inspect_source_foundation_goldsets,
};
use crate::source_foundation_labs::{
    SourceFoundationLabsReport, inspect_source_foundation_labs_with_physical,
};
use crate::source_foundation_records::{
    SourceFoundationRecordKernelOutcome, SourceFoundationRecordsReport,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::mem::size_of;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_source_store::CorpusCutReader;

/// Repeatable, authenticated current-path access. Each traversal preserves
/// the owning input's sorted path order and reaches its real metadata EOF.
/// A candidate implementation must retain the original deadline, cancellation
/// token and read ledger; this view creates no published source revision.
pub trait SourceFoundationDefaultPaths {
    fn contains(&self, path: &str) -> Result<bool, ItemRefusal>;
    fn contains_with_checkpoint(
        &self,
        path: &str,
        checkpoint: &mut dyn FnMut() -> Result<(), ItemRefusal>,
    ) -> Result<bool, ItemRefusal> {
        let mut found = false;
        self.for_each_path(&mut |candidate| {
            checkpoint()?;
            found |= candidate == path;
            Ok(())
        })?;
        Ok(found)
    }
    fn for_each_path(
        &self,
        visit: &mut dyn FnMut(&str) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal>;
}

pub struct SliceDefaultPaths<'a>(pub &'a [String]);

impl SourceFoundationDefaultPaths for SliceDefaultPaths<'_> {
    fn contains(&self, path: &str) -> Result<bool, ItemRefusal> {
        Ok(self.0.iter().any(|candidate| candidate == path))
    }
    fn contains_with_checkpoint(
        &self,
        path: &str,
        checkpoint: &mut dyn FnMut() -> Result<(), ItemRefusal>,
    ) -> Result<bool, ItemRefusal> {
        for candidate in self.0 {
            checkpoint()?;
            if candidate == path {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn for_each_path(
        &self,
        visit: &mut dyn FnMut(&str) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        for path in self.0 {
            visit(path)?;
        }
        Ok(())
    }
}

/// Facts from the actual completed Records owner. Borrowed cold facts and
/// bounded stored lookup results use one district predicate kernel. Owned
/// lookup results remain charged caller workspace until the caller drops them.
pub trait SourceFoundationDefaultRecordsLookup {
    fn current_record(
        &self,
        id: &str,
    ) -> Result<
        Option<std::borrow::Cow<'_, crate::record_biblio_cut::BiblioCurrentRecord>>,
        ItemRefusal,
    >;
    fn record_by_path(
        &self,
        path: &str,
    ) -> Result<
        Option<std::borrow::Cow<'_, crate::record_biblio_cut::BiblioCurrentRecord>>,
        ItemRefusal,
    >;
    /// Point lookup for a streamed candidate caller that must reserve an
    /// owned row before reading it. The default permits caller-owned borrowed
    /// records; owned adapters must override this method to enforce the row
    /// budget before materialization and return their charged workspace.
    fn record_by_path_with_state_budget(
        &self,
        _path: &str,
        _max_state_bytes: usize,
    ) -> Result<
        (
            Option<std::borrow::Cow<'_, crate::record_biblio_cut::BiblioCurrentRecord>>,
            usize,
        ),
        ItemRefusal,
    > {
        Err(ItemRefusal::Unsupported(
            "Records path lookup lacks a precharged state adapter".into(),
        ))
    }
    fn item_edition(&self, id: &str) -> Result<Option<std::borrow::Cow<'_, str>>, ItemRefusal>;
    fn rights_contains(&self, id: &str) -> Result<bool, ItemRefusal>;
    fn file_contains(&self, item: &Value, file: &Value) -> Result<bool, ItemRefusal>;
    fn file_sha256(&self, file: &Value)
    -> Result<Option<std::borrow::Cow<'_, Value>>, ItemRefusal>;
    fn for_each_current_record(
        &self,
        visit: &mut dyn FnMut(
            &str,
            &crate::record_biblio_cut::BiblioCurrentRecord,
        ) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal>;
    fn for_each_profile_kind(
        &self,
        visit: &mut dyn FnMut(&str) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal>;
}

/// Final-value event lookup and traversal in sorted ID order, matching the
/// maintained district's keyed event map. Records insertions must
/// be folded by their existing replacement law before exposing this view.
/// This is distinct from an append-only insertion stream.
pub trait SourceFoundationDefaultEventLookup {
    fn event(&self, id: &str) -> Result<Option<std::borrow::Cow<'_, Value>>, ItemRefusal>;
    fn event_contains(&self, id: &str) -> Result<bool, ItemRefusal> {
        Ok(self.event(id)?.is_some())
    }
    fn for_each_event(
        &self,
        visit: &mut dyn FnMut(&str, &Value) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal>;
}

/// One scoped, disk-backed event projection, retaining the owner dictionary
/// law: replace the value for an existing ID without moving its first slot.
/// Implementations charge writes and repeated reads to the request's existing
/// ledgers. Persistent bytes are not reported as retained process state.
pub trait SourceFoundationDefaultEventStore: SourceFoundationDefaultEventLookup {
    fn insert_event(
        &mut self,
        id: &str,
        value: &Value,
        max_json_bytes: usize,
        max_state_bytes: usize,
    ) -> Result<(), ItemRefusal>;
    fn cost(&self) -> Result<SourceFoundationDefaultEventStoreCost, ItemRefusal>;
    fn event_lookup(&self) -> &dyn SourceFoundationDefaultEventLookup;
}

#[derive(Debug, Clone, Copy)]
pub struct SourceFoundationDefaultEventStoreCost {
    pub retained_state_bytes: usize,
    pub workspace_state_bytes: usize,
    pub merged_event_json_bytes: usize,
}

/// Actual bibliographic claims in their original observation order. The
/// bounded point lookup returns the first observed row at one physical
/// location and its original ordinal, making duplicate checks possible
/// without rebuilding the complete path/line map.
pub trait SourceFoundationDefaultClaims {
    /// First observation with this textual `claim_id`, preserving the owner
    /// source-order lookup law; duplicate admission is checked separately.
    fn claim_by_id(
        &self,
        id: &str,
    ) -> Result<Option<(u64, std::borrow::Cow<'_, BiblioClaim>)>, ItemRefusal>;
    fn for_each_claim(
        &self,
        visit: &mut dyn FnMut(u64, &BiblioClaim) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal>;
    fn first_claim_at(
        &self,
        path: &str,
        line: usize,
    ) -> Result<Option<(u64, std::borrow::Cow<'_, BiblioClaim>)>, ItemRefusal>;
    fn last_claim_at(
        &self,
        path: &str,
        line: usize,
    ) -> Result<Option<(u64, std::borrow::Cow<'_, BiblioClaim>)>, ItemRefusal>;
    fn claim_count_for_path(&self, path: &str) -> Result<u64, ItemRefusal>;
    fn distinct_nonzero_claim_lines_for_path(&self, path: &str) -> Result<u64, ItemRefusal>;
    fn for_each_claim_at(
        &self,
        path: &str,
        line: usize,
        visit: &mut dyn FnMut(u64, &BiblioClaim) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal>;
}

pub struct SliceDefaultClaims<'a>(pub &'a [BiblioClaim]);

impl SourceFoundationDefaultClaims for SliceDefaultClaims<'_> {
    fn claim_by_id(
        &self,
        id: &str,
    ) -> Result<Option<(u64, std::borrow::Cow<'_, BiblioClaim>)>, ItemRefusal> {
        self.0
            .iter()
            .enumerate()
            .find(|(_, claim)| claim.value.get("claim_id").and_then(Value::as_str) == Some(id))
            .map(|(ordinal, claim)| {
                Ok((
                    u64::try_from(ordinal).map_err(|_| ItemRefusal::Budget)?,
                    std::borrow::Cow::Borrowed(claim),
                ))
            })
            .transpose()
    }
    fn for_each_claim(
        &self,
        visit: &mut dyn FnMut(u64, &BiblioClaim) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        for (ordinal, claim) in self.0.iter().enumerate() {
            visit(
                u64::try_from(ordinal).map_err(|_| ItemRefusal::Budget)?,
                claim,
            )?;
        }
        Ok(())
    }
    fn first_claim_at(
        &self,
        path: &str,
        line: usize,
    ) -> Result<Option<(u64, std::borrow::Cow<'_, BiblioClaim>)>, ItemRefusal> {
        self.0
            .iter()
            .enumerate()
            .find(|(_, claim)| claim.path == path && claim.line == line)
            .map(|(ordinal, claim)| {
                Ok((
                    u64::try_from(ordinal).map_err(|_| ItemRefusal::Budget)?,
                    std::borrow::Cow::Borrowed(claim),
                ))
            })
            .transpose()
    }
    fn last_claim_at(
        &self,
        path: &str,
        line: usize,
    ) -> Result<Option<(u64, std::borrow::Cow<'_, BiblioClaim>)>, ItemRefusal> {
        self.0
            .iter()
            .enumerate()
            .rev()
            .find(|(_, claim)| claim.path == path && claim.line == line)
            .map(|(ordinal, claim)| {
                Ok((
                    u64::try_from(ordinal).map_err(|_| ItemRefusal::Budget)?,
                    std::borrow::Cow::Borrowed(claim),
                ))
            })
            .transpose()
    }
    fn claim_count_for_path(&self, path: &str) -> Result<u64, ItemRefusal> {
        u64::try_from(self.0.iter().filter(|claim| claim.path == path).count())
            .map_err(|_| ItemRefusal::Budget)
    }
    fn distinct_nonzero_claim_lines_for_path(&self, path: &str) -> Result<u64, ItemRefusal> {
        let mut count = 0u64;
        for (ordinal, claim) in self.0.iter().enumerate() {
            if claim.path == path
                && claim.line != 0
                && !self.0[..ordinal]
                    .iter()
                    .any(|prior| prior.path == path && prior.line == claim.line)
            {
                count = count.checked_add(1).ok_or(ItemRefusal::Budget)?;
            }
        }
        Ok(count)
    }
    fn for_each_claim_at(
        &self,
        path: &str,
        line: usize,
        visit: &mut dyn FnMut(u64, &BiblioClaim) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        for (ordinal, claim) in self
            .0
            .iter()
            .enumerate()
            .filter(|(_, claim)| claim.path == path && claim.line == line)
        {
            visit(
                u64::try_from(ordinal).map_err(|_| ItemRefusal::Budget)?,
                claim,
            )?;
        }
        Ok(())
    }
}

impl SourceFoundationDefaultEventLookup for BTreeMap<String, Value> {
    fn event(&self, id: &str) -> Result<Option<std::borrow::Cow<'_, Value>>, ItemRefusal> {
        Ok(self.get(id).map(std::borrow::Cow::Borrowed))
    }
    fn event_contains(&self, id: &str) -> Result<bool, ItemRefusal> {
        Ok(self.contains_key(id))
    }
    fn for_each_event(
        &self,
        visit: &mut dyn FnMut(&str, &Value) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        for (id, value) in self {
            visit(id, value)?;
        }
        Ok(())
    }
}

pub struct BorrowedDefaultRecords<'a> {
    pub current_records: &'a BTreeMap<String, crate::record_biblio_cut::BiblioCurrentRecord>,
    pub item_editions: &'a BTreeMap<String, String>,
    pub rights_ids: &'a std::collections::BTreeSet<String>,
    pub file_memberships: &'a crate::source_witness_foundation::SourceFileMembershipIndex,
    pub declared_profile_kinds: &'a std::collections::BTreeSet<String>,
}

impl SourceFoundationDefaultRecordsLookup for BorrowedDefaultRecords<'_> {
    fn current_record(
        &self,
        id: &str,
    ) -> Result<
        Option<std::borrow::Cow<'_, crate::record_biblio_cut::BiblioCurrentRecord>>,
        ItemRefusal,
    > {
        Ok(self.current_records.get(id).map(std::borrow::Cow::Borrowed))
    }
    fn record_by_path(
        &self,
        path: &str,
    ) -> Result<
        Option<std::borrow::Cow<'_, crate::record_biblio_cut::BiblioCurrentRecord>>,
        ItemRefusal,
    > {
        Ok(self
            .current_records
            .values()
            .find(|record| record.path == path)
            .map(std::borrow::Cow::Borrowed))
    }
    fn record_by_path_with_state_budget(
        &self,
        path: &str,
        _max_state_bytes: usize,
    ) -> Result<
        (
            Option<std::borrow::Cow<'_, crate::record_biblio_cut::BiblioCurrentRecord>>,
            usize,
        ),
        ItemRefusal,
    > {
        Ok((self.record_by_path(path)?, 0))
    }
    fn item_edition(&self, id: &str) -> Result<Option<std::borrow::Cow<'_, str>>, ItemRefusal> {
        Ok(self
            .item_editions
            .get(id)
            .map(|value| std::borrow::Cow::Borrowed(value.as_str())))
    }
    fn rights_contains(&self, id: &str) -> Result<bool, ItemRefusal> {
        Ok(self.rights_ids.contains(id))
    }
    fn file_contains(&self, item: &Value, file: &Value) -> Result<bool, ItemRefusal> {
        Ok(self.file_memberships.contains(item, file))
    }
    fn file_sha256(
        &self,
        file: &Value,
    ) -> Result<Option<std::borrow::Cow<'_, Value>>, ItemRefusal> {
        Ok(self
            .file_memberships
            .sha256_for(file)
            .map(std::borrow::Cow::Borrowed))
    }
    fn for_each_current_record(
        &self,
        visit: &mut dyn FnMut(
            &str,
            &crate::record_biblio_cut::BiblioCurrentRecord,
        ) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        for (id, record) in self.current_records {
            visit(id, record)?;
        }
        Ok(())
    }
    fn for_each_profile_kind(
        &self,
        visit: &mut dyn FnMut(&str) -> Result<(), ItemRefusal>,
    ) -> Result<(), ItemRefusal> {
        for kind in self.declared_profile_kinds {
            visit(kind)?;
        }
        Ok(())
    }
}

/// Limits for one ordered district composition.
#[derive(Debug, Clone, Copy)]
pub struct SourceFoundationDefaultRulesLimits {
    /// Aggregate caps shared across Labs, Gold, Discovery and Closure. The
    /// already-run Records report contributes its own conservative state/read
    /// reservations before any later district is invoked.
    pub operation: ItemLimits,
    /// Maximum exact JSON encoding size of the merged source-event map handed
    /// to Closure and returned to the caller. This is not a cap on CLI output.
    pub max_event_map_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFoundationDefaultRulesCost {
    /// `None` remains unknown when the Records kernel refused after partial
    /// reads. The separate reservation below bounds that uncertainty.
    pub records_observed_read_bytes: Option<u64>,
    /// Exact Records+Item bytes when the record kernel completed; otherwise a
    /// conservative ceiling using the full record read budget.
    pub records_read_reservation_bytes: u64,
    /// Counted current/recorded/payload bytes in the later four districts,
    /// including candidate current reads charged directly by their exact
    /// input. Existence semantics remain with that input's own read ledger.
    pub later_counted_source_bytes: u64,
    /// Selected baseline for the already-retained Records report: the full
    /// operation ceiling in the legacy fixed entry, or the Records kernel's
    /// logical accounted-retention upper bound in the rolling entry. This is
    /// not a process-memory measurement.
    pub records_state_reservation_upper_bound_bytes: usize,
    /// Retained-state estimates charged by Labs, Gold, Discovery and Closure.
    pub later_district_state_bytes: usize,
    /// High-water workspace of the candidate's disk-backed Discovery
    /// uniqueness lookups, reported separately from retained row state.
    pub discovery_seen_ids_peak_workspace_state_bytes: usize,
    /// Additional clone state for the merged event map and first-insertion
    /// order vector.
    pub merged_event_state_bytes: usize,
    /// Aggregate retained-state reservation, including the Records upper
    /// bound and this report's fixed containers.
    pub aggregate_state_reservation_bytes: usize,
    /// Exact bounded serialized size of the final event map.
    pub merged_event_json_bytes: usize,
    /// Sum of UTF-8 bytes in direct owner issue strings retained by the five
    /// districts. CLI framing and pending schema diagnostics are not included.
    pub owner_issue_bytes: usize,
    /// Direct owner issue count. Queued schema documents are reported
    /// separately because diagnostic-v2 may emit a variable number of issues.
    pub direct_owner_issue_count: usize,
    /// Number of distinct queued diagnostic-v2 document checks. Per-district
    /// DTOs remain authoritative for locations, controls, and ordinals.
    pub queued_schema_document_count: usize,
}

/// Discovery findings without its local `ScopeStatus`; an assembly report
/// carries no complete/incomplete verdict. Every actual unsupported row and
/// owner issue remains available here.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceFoundationDefaultDiscoveryFindings {
    pub issues: Vec<DiscoveryIssue>,
    pub schema_requests: Vec<DiscoverySchemaRequest>,
    pub source_event_insertions: Vec<(String, Value)>,
    pub unsupported: Vec<DiscoveryUnsupported>,
    pub cost: DiscoveryCost,
}

/// Findings returned in the maintained source-foundation order:
/// Labs → Records → Gold → Discovery → Closure. District schema requests and
/// coverage gaps stay typed and local; no whole-source verdict is provided.
pub struct SourceFoundationDefaultRulesReport {
    pub labs: SourceFoundationLabsReport,
    pub records: SourceFoundationRecordsReport,
    pub goldsets: SourceFoundationGoldsetsReport,
    pub discovery: SourceFoundationDefaultDiscoveryFindings,
    pub closure: SourceFoundationClosureReport,
    /// Last value per ID, with Python dictionary first-insertion ordering
    /// available separately. The map supplies keyed Closure lookups.
    pub source_events: BTreeMap<String, Value>,
    pub source_event_order: Vec<String>,
    pub cost: SourceFoundationDefaultRulesCost,
}

/// Default district findings from one genuine stored Records operation.
/// No Records collection, event dictionary or source-path inventory is
/// reconstructed in this result. Schema request DTOs remain owned by their
/// existing bounded districts so the caller can run the same diagnostics.
pub struct SourceFoundationDefaultRulesStoredReport<I> {
    pub input_identity: I,
    pub source_membership: tos_source_store::SourceMembershipV1,
    pub labs: SourceFoundationLabsReport,
    pub goldsets: SourceFoundationGoldsetsReport,
    pub discovery: SourceFoundationDefaultDiscoveryFindings,
    pub closure: SourceFoundationClosureReport,
    pub cost: SourceFoundationDefaultRulesCost,
}

#[derive(Debug, Clone, Copy)]
pub struct SourceFoundationDefaultStoredLimits {
    pub page_budget: crate::source_foundation_records::SourceFoundationRecordsPageBudget,
    /// Aggregate rows admitted through this composition's Records index
    /// traversals. Every repeated row is charged again; zero is refused.
    pub max_scan_rows: u64,
}

#[derive(Default)]
struct StoredRecordsSummary {
    direct_issue_count: usize,
    owner_issue_bytes: usize,
    schema_document_count: usize,
}

fn visit_stored_records<I>(
    records: &crate::source_foundation_records::SourceFoundationRecordsStreamedReport<'_, I>,
    collection: crate::source_foundation_records::SourceFoundationRecordsCollection,
    limits: SourceFoundationDefaultStoredLimits,
    operation: ItemLimits,
    cancelled: &AtomicBool,
    scanned: &mut u64,
    visit: &mut dyn FnMut(
        &crate::source_foundation_records::SourceFoundationRecordsStoredFact,
    ) -> Result<(), ItemRefusal>,
) -> Result<(), ItemRefusal> {
    if limits.max_scan_rows == 0 || limits.max_scan_rows == u64::MAX {
        return Err(ItemRefusal::Budget);
    }
    let page_peak = limits
        .page_budget
        .max_state_bytes
        .get()
        .checked_add(limits.page_budget.max_cursor_bytes.get())
        .and_then(|bytes| bytes.checked_add(size_of::<StoredRecordsSummary>()))
        .ok_or(ItemRefusal::Budget)?;
    if page_peak > operation.max_state_bytes {
        return Err(budget_refusal(
            "source-foundation stored page workspace",
            page_peak as u64,
            operation.max_state_bytes as u64,
        ));
    }
    let mut after = None;
    loop {
        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(ItemRefusal::Source(
                "source-foundation stored traversal cancelled".into(),
            ));
        }
        if Instant::now() >= operation.deadline {
            return Err(ItemRefusal::Deadline);
        }
        let page = records.index().page(
            collection,
            after.as_ref(),
            limits.page_budget,
            operation.deadline,
            cancelled,
        )?;
        *scanned = scanned
            .checked_add(u64::try_from(page.rows.len()).map_err(|_| ItemRefusal::Budget)?)
            .filter(|rows| *rows <= limits.max_scan_rows)
            .ok_or(ItemRefusal::Budget)?;
        for row in &page.rows {
            if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(ItemRefusal::Source(
                    "source-foundation stored traversal cancelled".into(),
                ));
            }
            if Instant::now() >= operation.deadline {
                return Err(ItemRefusal::Deadline);
            }
            visit(row)?;
        }
        after = page.next_cursor;
        if after.is_none() {
            break;
        }
    }
    Ok(())
}

fn stored_records_summary<I>(
    records: &crate::source_foundation_records::SourceFoundationRecordsStreamedReport<'_, I>,
    limits: SourceFoundationDefaultStoredLimits,
    operation: ItemLimits,
    cancelled: &AtomicBool,
    scanned: &mut u64,
) -> Result<StoredRecordsSummary, ItemRefusal> {
    use crate::source_foundation_records::{
        SourceFoundationRecordsCollection as C, SourceFoundationRecordsStoredFact as F,
    };
    let mut result = StoredRecordsSummary::default();
    for collection in [
        C::OrderedIssues,
        C::ItemIssues,
        C::RecordObservations,
        C::SchemaChecks,
    ] {
        visit_stored_records(
            records,
            collection,
            limits,
            operation,
            cancelled,
            scanned,
            &mut |row| {
                let issue = match row {
                    F::OrderedIssue(issue) => {
                        Some((issue.location.as_str(), issue.message.as_str()))
                    }
                    F::ItemIssue(issue) => Some((issue.path.as_str(), issue.code)),
                    F::RecordObservation(row) => match &row.observation {
                        RecordObservation::Issue { path, code } => Some((path.as_str(), *code)),
                        _ => None,
                    },
                    F::SchemaCheck(check) => {
                        if check.decoded_instance.is_some() || check.legacy_raw_instance.is_some() {
                            result.schema_document_count = result
                                .schema_document_count
                                .checked_add(1)
                                .ok_or(ItemRefusal::Budget)?;
                        }
                        None
                    }
                    _ => {
                        return Err(ItemRefusal::Source(
                            "source-foundation stored summary collection mismatch".into(),
                        ));
                    }
                };
                if let Some((location, message)) = issue {
                    result.direct_issue_count = result
                        .direct_issue_count
                        .checked_add(1)
                        .ok_or(ItemRefusal::Budget)?;
                    add_text_bytes(&mut result.owner_issue_bytes, location)?;
                    add_text_bytes(&mut result.owner_issue_bytes, message)?;
                    if result.direct_issue_count > operation.max_issues {
                        return Err(ItemRefusal::Budget);
                    }
                }
                Ok(())
            },
        )?;
    }
    Ok(result)
}

fn charge_stored_events(
    events: &dyn SourceFoundationDefaultEventStore,
    used_state: &mut usize,
    prior_charge: &mut usize,
    operation: ItemLimits,
    max_json_bytes: usize,
) -> Result<SourceFoundationDefaultEventStoreCost, ItemRefusal> {
    let cost = events.cost()?;
    if cost.merged_event_json_bytes > max_json_bytes {
        return Err(budget_refusal(
            "source-foundation stored event output",
            cost.merged_event_json_bytes as u64,
            max_json_bytes as u64,
        ));
    }
    let charge = cost
        .retained_state_bytes
        .checked_add(cost.workspace_state_bytes)
        .ok_or(ItemRefusal::Budget)?;
    *used_state = used_state
        .checked_sub(*prior_charge)
        .and_then(|bytes| bytes.checked_add(charge))
        .filter(|bytes| *bytes <= operation.max_state_bytes)
        .ok_or(ItemRefusal::Budget)?;
    *prior_charge = charge;
    Ok(cost)
}

fn insert_stored_event(
    events: &mut dyn SourceFoundationDefaultEventStore,
    id: &str,
    value: &Value,
    used_state: &mut usize,
    event_charge: &mut usize,
    limits: SourceFoundationDefaultRulesLimits,
) -> Result<(), ItemRefusal> {
    let other_state = used_state
        .checked_sub(*event_charge)
        .ok_or(ItemRefusal::Budget)?;
    let available = limits
        .operation
        .max_state_bytes
        .checked_sub(other_state)
        .ok_or(ItemRefusal::Budget)?;
    events.insert_event(id, value, limits.max_event_map_bytes, available)?;
    charge_stored_events(
        events,
        used_state,
        event_charge,
        limits.operation,
        limits.max_event_map_bytes,
    )?;
    Ok(())
}

fn later_district_owner_issue_bytes(
    labs: &SourceFoundationLabsReport,
    goldsets: &SourceFoundationGoldsetsReport,
    discovery: &SourceFoundationDefaultDiscoveryFindings,
    closure: &SourceFoundationClosureReport,
) -> Result<usize, ItemRefusal> {
    let mut bytes = 0;
    for (location, message) in &labs.ordered_issues {
        add_text_bytes(&mut bytes, location)?;
        add_text_bytes(&mut bytes, message)?;
    }
    for (location, message) in &goldsets.ordered_issues {
        add_text_bytes(&mut bytes, location)?;
        add_text_bytes(&mut bytes, message)?;
    }
    for issue in &discovery.issues {
        add_text_bytes(&mut bytes, &issue.location)?;
        add_text_bytes(&mut bytes, issue.code)?;
        add_text_bytes(&mut bytes, &issue.detail)?;
    }
    for (location, message) in &closure.issues {
        add_text_bytes(&mut bytes, location)?;
        add_text_bytes(&mut bytes, message)?;
    }
    Ok(bytes)
}

/// Run the maintained default district order over the actual stored Records
/// report and bounded provider views belonging to that same input. The caller
/// keeps the candidate scope and schema worker alive for the complete call.
/// The scoped event provider has already folded the actual Records insertion
/// stream once, before bibliography reads its readonly event projection. This
/// call continues that same projection with Gold and Discovery writes.
/// This returns findings only; it never creates source-admission authority.
#[allow(clippy::too_many_arguments)]
pub fn inspect_source_foundation_default_rules_from_input_stored<
    I: Copy + Eq,
    S: LayerFamilySource + ?Sized,
>(
    source: &mut S,
    input: &dyn crate::record_biblio_cut::SourceCutInputWithIdentity<I>,
    coverage: &crate::record_biblio_cut::SourceCutInputCoverage,
    records: &crate::source_foundation_records::SourceFoundationRecordsStreamedReport<'_, I>,
    records_lookup: &dyn SourceFoundationDefaultRecordsLookup,
    paths: &dyn SourceFoundationDefaultPaths,
    events: &mut dyn SourceFoundationDefaultEventStore,
    native_histories: &BTreeMap<
        String,
        crate::native_compound::CandidateNativeRecordHistoryReadObservation<I>,
    >,
    claims: &dyn SourceFoundationDefaultClaims,
    physical: &SourcePhysicalFacts,
    artifact_replays: &crate::source_foundation_discovery::CandidateArtifactCorrectionReplayMap<
        '_,
        I,
    >,
    invalid_artifact_proofs: &crate::source_foundation_discovery::CandidateArtifactInvalidSchemaProofs<'_, '_, I>,
    require_local_payloads: bool,
    limits: SourceFoundationDefaultRulesLimits,
    stored_limits: SourceFoundationDefaultStoredLimits,
    cancelled: &AtomicBool,
) -> Result<SourceFoundationDefaultRulesStoredReport<I>, ItemRefusal> {
    inspect_source_foundation_default_rules_from_input_stored_inner(
        source,
        input,
        coverage,
        records,
        records_lookup,
        paths,
        events,
        Some(native_histories),
        claims,
        physical,
        Some(artifact_replays),
        Some(invalid_artifact_proofs),
        None,
        None,
        require_local_payloads,
        limits,
        stored_limits,
        cancelled,
    )
}

/// Run the maintained default district order while reconstructing one
/// candidate Artifact evidence packet at a time inside Discovery. The
/// provider remains CMD-owned; this portable composition only calls its
/// exact-path callback.
#[allow(clippy::too_many_arguments)]
pub fn inspect_source_foundation_default_rules_from_input_stored_with_artifact_evidence_provider<
    I: Copy + Eq,
    S: LayerFamilySource + ?Sized,
>(
    source: &mut S,
    input: &dyn crate::record_biblio_cut::SourceCutInputWithIdentity<I>,
    coverage: &crate::record_biblio_cut::SourceCutInputCoverage,
    records: &crate::source_foundation_records::SourceFoundationRecordsStreamedReport<'_, I>,
    records_lookup: &dyn SourceFoundationDefaultRecordsLookup,
    paths: &dyn SourceFoundationDefaultPaths,
    events: &mut dyn SourceFoundationDefaultEventStore,
    claims: &dyn SourceFoundationDefaultClaims,
    physical: &SourcePhysicalFacts,
    evidence_provider: &mut dyn CandidateArtifactEvidenceProvider<I>,
    require_local_payloads: bool,
    limits: SourceFoundationDefaultRulesLimits,
    stored_limits: SourceFoundationDefaultStoredLimits,
    cancelled: &AtomicBool,
) -> Result<SourceFoundationDefaultRulesStoredReport<I>, ItemRefusal> {
    inspect_source_foundation_default_rules_from_input_stored_inner(
        source,
        input,
        coverage,
        records,
        records_lookup,
        paths,
        events,
        None,
        claims,
        physical,
        None,
        None,
        Some(evidence_provider),
        None,
        require_local_payloads,
        limits,
        stored_limits,
        cancelled,
    )
}

/// Candidate stored composition with the held disk-backed exact-ID scratch
/// index used by Discovery to preserve duplicate and negative-membership laws.
#[allow(clippy::too_many_arguments)]
pub fn inspect_source_foundation_default_rules_from_input_stored_with_artifact_evidence_provider_and_seen_ids<
    I: Copy + Eq,
    S: LayerFamilySource + ?Sized,
>(
    source: &mut S,
    input: &dyn crate::record_biblio_cut::SourceCutInputWithIdentity<I>,
    coverage: &crate::record_biblio_cut::SourceCutInputCoverage,
    records: &crate::source_foundation_records::SourceFoundationRecordsStreamedReport<'_, I>,
    records_lookup: &dyn SourceFoundationDefaultRecordsLookup,
    paths: &dyn SourceFoundationDefaultPaths,
    events: &mut dyn SourceFoundationDefaultEventStore,
    claims: &dyn SourceFoundationDefaultClaims,
    physical: &SourcePhysicalFacts,
    evidence_provider: &mut dyn CandidateArtifactEvidenceProvider<I>,
    discovery_seen_ids: &mut dyn DiscoverySeenIds,
    require_local_payloads: bool,
    limits: SourceFoundationDefaultRulesLimits,
    stored_limits: SourceFoundationDefaultStoredLimits,
    cancelled: &AtomicBool,
) -> Result<SourceFoundationDefaultRulesStoredReport<I>, ItemRefusal> {
    inspect_source_foundation_default_rules_from_input_stored_inner(
        source,
        input,
        coverage,
        records,
        records_lookup,
        paths,
        events,
        None,
        claims,
        physical,
        None,
        None,
        Some(evidence_provider),
        Some(discovery_seen_ids),
        require_local_payloads,
        limits,
        stored_limits,
        cancelled,
    )
}

#[allow(clippy::too_many_arguments)]
fn inspect_source_foundation_default_rules_from_input_stored_inner<
    I: Copy + Eq,
    S: LayerFamilySource + ?Sized,
>(
    source: &mut S,
    input: &dyn crate::record_biblio_cut::SourceCutInputWithIdentity<I>,
    coverage: &crate::record_biblio_cut::SourceCutInputCoverage,
    records: &crate::source_foundation_records::SourceFoundationRecordsStreamedReport<'_, I>,
    records_lookup: &dyn SourceFoundationDefaultRecordsLookup,
    paths: &dyn SourceFoundationDefaultPaths,
    events: &mut dyn SourceFoundationDefaultEventStore,
    native_histories: Option<
        &BTreeMap<String, crate::native_compound::CandidateNativeRecordHistoryReadObservation<I>>,
    >,
    claims: &dyn SourceFoundationDefaultClaims,
    physical: &SourcePhysicalFacts,
    artifact_replays: Option<&CandidateArtifactCorrectionReplayMap<'_, I>>,
    invalid_artifact_proofs: Option<&CandidateArtifactInvalidSchemaProofs<'_, '_, I>>,
    mut evidence_provider: Option<&mut dyn CandidateArtifactEvidenceProvider<I>>,
    mut discovery_seen_ids: Option<&mut dyn DiscoverySeenIds>,
    require_local_payloads: bool,
    limits: SourceFoundationDefaultRulesLimits,
    stored_limits: SourceFoundationDefaultStoredLimits,
    cancelled: &AtomicBool,
) -> Result<SourceFoundationDefaultRulesStoredReport<I>, ItemRefusal> {
    let operation = limits.operation;
    source.checkpoint(operation.deadline)?;
    if !std::ptr::eq(source.cancellation(), cancelled)
        || input.input_identity() != records.input_identity()
        || coverage.membership() != *records.source_membership()
    {
        return Err(ItemRefusal::Source(
            "source-foundation stored input/report binding differs".into(),
        ));
    }
    input.verify_current_fence(coverage, operation.deadline, cancelled)?;
    if limits.max_event_map_bytes < 2 {
        return Err(ItemRefusal::Budget);
    }
    let records_cost = records.cost();
    let records_read_reservation_bytes = records_cost
        .record_observed_read_bytes
        .unwrap_or(records_cost.record_read_limit_bytes)
        .checked_add(records_cost.item_observed_read_bytes)
        .and_then(|bytes| bytes.checked_add(records_cost.item_schema_resource_bytes))
        .ok_or(ItemRefusal::Budget)?;
    let records_state_reservation_upper_bound_bytes = records_cost
        .rolling_accounted_state_upper_bound_bytes
        .unwrap_or(records_cost.operation_state_limit_bytes);
    let mut used_state = add_state(
        records_state_reservation_upper_bound_bytes,
        size_of::<SourceFoundationDefaultRulesStoredReport<I>>(),
        operation.max_state_bytes,
    )?;
    let mut event_charge = 0usize;
    charge_stored_events(
        events,
        &mut used_state,
        &mut event_charge,
        operation,
        limits.max_event_map_bytes,
    )?;
    let summary_limits =
        remaining_limits(operation, records_read_reservation_bytes, 0, used_state, 0)?;
    let mut scanned = 0;
    let summary = stored_records_summary(
        records,
        stored_limits,
        summary_limits,
        cancelled,
        &mut scanned,
    )?;
    let mut aggregate_source = AggregateLayerFamilySource {
        inner: source,
        read_allowance: operation
            .max_total_bytes
            .checked_sub(records_read_reservation_bytes)
            .ok_or(ItemRefusal::Budget)?,
        read_bytes: 0,
    };
    let labs =
        crate::source_foundation_labs::inspect_source_foundation_labs_with_physical_from_paths(
            &mut aggregate_source,
            remaining_limits(
                operation,
                records_read_reservation_bytes,
                0,
                used_state,
                summary.direct_issue_count,
            )?,
            paths,
            physical,
        )?;
    used_state = add_state(
        used_state,
        labs.cost.retained_state_bytes,
        operation.max_state_bytes,
    )?;
    let mut direct_owner_issue_count = summary
        .direct_issue_count
        .checked_add(labs.ordered_issues.len())
        .ok_or(ItemRefusal::Budget)?;
    let gold_limits = remaining_limits(
        operation,
        records_read_reservation_bytes,
        aggregate_source.read_bytes,
        used_state,
        direct_owner_issue_count,
    )?;
    let goldsets =
        crate::source_foundation_goldsets::inspect_source_foundation_goldsets_with_lookups(
            &mut aggregate_source,
            paths,
            events.event_lookup(),
            records_lookup,
            physical,
            require_local_payloads,
            gold_limits,
        )?;
    direct_owner_issue_count = direct_owner_issue_count
        .checked_add(goldsets.ordered_issues.len())
        .ok_or(ItemRefusal::Budget)?;
    used_state = add_state(
        used_state,
        goldsets.retained_state_bytes,
        operation.max_state_bytes,
    )?;
    for (id, value) in &goldsets.source_events {
        insert_stored_event(
            events,
            id,
            value,
            &mut used_state,
            &mut event_charge,
            limits,
        )?;
    }
    let discovery_limits = remaining_limits(
        operation,
        records_read_reservation_bytes,
        aggregate_source.read_bytes,
        used_state,
        direct_owner_issue_count,
    )?;
    let discovery = if let Some(provider) = evidence_provider.as_deref_mut() {
        if let Some(seen_ids) = discovery_seen_ids.as_deref_mut() {
            crate::source_foundation_discovery::inspect_candidate_with_artifact_evidence_provider_and_seen_ids(
                &mut aggregate_source,
                input,
                coverage,
                paths,
                events.event_lookup(),
                records_lookup,
                records,
                discovery_limits,
                physical,
                provider,
                seen_ids,
                require_local_payloads,
            )?
        } else {
            crate::source_foundation_discovery::inspect_candidate_with_artifact_evidence_provider(
                &mut aggregate_source,
                input,
                coverage,
                paths,
                events.event_lookup(),
                records_lookup,
                records,
                discovery_limits,
                physical,
                provider,
                require_local_payloads,
            )?
        }
    } else {
        crate::source_foundation_discovery::inspect_candidate_with_artifact_replays_and_records_with_proofs(
            &mut aggregate_source,
            input,
            coverage,
            paths,
            events.event_lookup(),
            records_lookup,
            records,
            discovery_limits,
            physical,
            native_histories.ok_or_else(|| {
                ItemRefusal::Source("candidate native Artifact histories are missing".into())
            })?,
            artifact_replays.ok_or_else(|| {
                ItemRefusal::Source("candidate Artifact replay map is missing".into())
            })?,
            invalid_artifact_proofs.ok_or_else(|| {
                ItemRefusal::Source("candidate Artifact schema proof is missing".into())
            })?,
            require_local_payloads,
        )?
    };
    if discovery.input_identity() != input.input_identity()
        || discovery.source_membership() != *records.source_membership()
    {
        return Err(ItemRefusal::Source(
            "source-foundation Discovery report belongs to another stored input".into(),
        ));
    }
    // Candidate current reads use the exact input directly; the remaining
    // district read allowance must still consume those bytes once. Recorded
    // and payload calls already passed through AggregateLayerFamilySource.
    aggregate_source.charge_read(discovery.candidate_direct_source_bytes())?;
    let discovery = discovery.into_report();
    direct_owner_issue_count = direct_owner_issue_count
        .checked_add(discovery.issues.len())
        .ok_or(ItemRefusal::Budget)?;
    used_state = add_state(
        used_state,
        discovery.cost.state_bytes,
        operation.max_state_bytes,
    )?;
    for (id, value) in &discovery.source_event_insertions {
        insert_stored_event(
            events,
            id,
            value,
            &mut used_state,
            &mut event_charge,
            limits,
        )?;
    }
    let discovery = SourceFoundationDefaultDiscoveryFindings {
        issues: discovery.issues,
        schema_requests: discovery.schema_requests,
        source_event_insertions: discovery.source_event_insertions,
        unsupported: discovery.unsupported,
        cost: discovery.cost,
    };
    let closure_limits = remaining_limits(
        operation,
        records_read_reservation_bytes,
        aggregate_source.read_bytes,
        used_state,
        direct_owner_issue_count,
    )?;
    let closure =
        crate::source_foundation_closure::inspect_source_foundation_closure_with_identity(
            &mut aggregate_source,
            input,
            input.input_identity(),
            coverage,
            events.event_lookup(),
            records_lookup,
            paths,
            claims,
            closure_limits,
        )?;
    direct_owner_issue_count = direct_owner_issue_count
        .checked_add(closure.issues.len())
        .ok_or(ItemRefusal::Budget)?;
    used_state = add_state(
        used_state,
        closure.cost.reserved_state_bytes,
        operation.max_state_bytes,
    )?;
    let final_event_cost = charge_stored_events(
        events,
        &mut used_state,
        &mut event_charge,
        operation,
        limits.max_event_map_bytes,
    )?;
    input.verify_current_fence(coverage, operation.deadline, cancelled)?;
    aggregate_source.checkpoint(operation.deadline)?;
    let owner_issue_bytes = summary
        .owner_issue_bytes
        .checked_add(later_district_owner_issue_bytes(
            &labs, &goldsets, &discovery, &closure,
        )?)
        .ok_or(ItemRefusal::Budget)?;
    let queued_schema_document_count = summary
        .schema_document_count
        .checked_add(labs.schema_checks.len())
        .and_then(|count| count.checked_add(goldsets.schema_requests.len()))
        .and_then(|count| count.checked_add(discovery.schema_requests.len()))
        .and_then(|count| count.checked_add(closure.schema_requests.len()))
        .ok_or(ItemRefusal::Budget)?;
    let later_district_state_bytes = labs
        .cost
        .retained_state_bytes
        .checked_add(goldsets.retained_state_bytes)
        .and_then(|bytes| bytes.checked_add(discovery.cost.state_bytes))
        .and_then(|bytes| bytes.checked_add(closure.cost.reserved_state_bytes))
        .ok_or(ItemRefusal::Budget)?;
    if direct_owner_issue_count > operation.max_issues {
        return Err(ItemRefusal::Budget);
    }
    Ok(SourceFoundationDefaultRulesStoredReport {
        input_identity: *input.input_identity(),
        source_membership: *records.source_membership(),
        labs,
        goldsets,
        discovery,
        closure,
        cost: SourceFoundationDefaultRulesCost {
            records_observed_read_bytes: records_cost.record_observed_read_bytes,
            records_read_reservation_bytes,
            later_counted_source_bytes: aggregate_source.read_bytes,
            records_state_reservation_upper_bound_bytes,
            later_district_state_bytes,
            discovery_seen_ids_peak_workspace_state_bytes: discovery
                .cost
                .candidate_discovery_seen_ids_peak_workspace_state_bytes,
            merged_event_state_bytes: final_event_cost.retained_state_bytes,
            aggregate_state_reservation_bytes: used_state,
            merged_event_json_bytes: final_event_cost.merged_event_json_bytes,
            owner_issue_bytes,
            direct_owner_issue_count,
            queued_schema_document_count,
        },
    })
}

/// Run the default authored foundation districts over actual owner inputs.
/// `records`, native histories, biblio claims, physical observations and
/// correction replay evidence are borrowed or moved from their existing
/// owners; this function never fabricates empty substitutes for them.
pub fn inspect_source_foundation_default_rules<S: LayerFamilySource + ?Sized>(
    source: &mut S,
    cut: &CorpusCutReader,
    current_paths: &[String],
    records: SourceFoundationRecordsReport,
    native_histories: &BTreeMap<String, NativeRecordHistoryReadObservation>,
    bibliographic_claims: &[BiblioClaim],
    physical: &SourcePhysicalFacts,
    artifact_replays: &ArtifactCorrectionReplayMap<'_>,
    require_local_payloads: bool,
    limits: SourceFoundationDefaultRulesLimits,
) -> Result<SourceFoundationDefaultRulesReport, ItemRefusal> {
    inspect_source_foundation_default_rules_internal(
        source,
        cut,
        current_paths,
        records,
        native_histories,
        bibliographic_claims,
        physical,
        artifact_replays,
        None,
        require_local_payloads,
        limits,
    )
}

/// Default districts with the shared exact-cut Artifact invalid-schema proof
/// prepared by CMD from this same completed Records report.
#[allow(clippy::too_many_arguments)]
pub fn inspect_source_foundation_default_rules_with_invalid_artifact_proofs<
    S: LayerFamilySource + ?Sized,
>(
    source: &mut S,
    cut: &CorpusCutReader,
    current_paths: &[String],
    records: SourceFoundationRecordsReport,
    native_histories: &BTreeMap<String, NativeRecordHistoryReadObservation>,
    bibliographic_claims: &[BiblioClaim],
    physical: &SourcePhysicalFacts,
    artifact_replays: &ArtifactCorrectionReplayMap<'_>,
    invalid_artifact_proofs: &CurrentArtifactInvalidSchemaProofs<'_>,
    require_local_payloads: bool,
    limits: SourceFoundationDefaultRulesLimits,
) -> Result<SourceFoundationDefaultRulesReport, ItemRefusal> {
    inspect_source_foundation_default_rules_internal(
        source,
        cut,
        current_paths,
        records,
        native_histories,
        bibliographic_claims,
        physical,
        artifact_replays,
        Some(invalid_artifact_proofs),
        require_local_payloads,
        limits,
    )
}

#[allow(clippy::too_many_arguments)]
fn inspect_source_foundation_default_rules_internal<S: LayerFamilySource + ?Sized>(
    source: &mut S,
    cut: &CorpusCutReader,
    current_paths: &[String],
    records: SourceFoundationRecordsReport,
    native_histories: &BTreeMap<String, NativeRecordHistoryReadObservation>,
    bibliographic_claims: &[BiblioClaim],
    physical: &SourcePhysicalFacts,
    artifact_replays: &ArtifactCorrectionReplayMap<'_>,
    invalid_artifact_proofs: Option<&CurrentArtifactInvalidSchemaProofs<'_>>,
    require_local_payloads: bool,
    limits: SourceFoundationDefaultRulesLimits,
) -> Result<SourceFoundationDefaultRulesReport, ItemRefusal> {
    if limits.max_event_map_bytes < 2 {
        return Err(budget_refusal(
            "source-foundation empty event map output",
            2,
            limits.max_event_map_bytes as u64,
        ));
    }
    let operation = limits.operation;
    let selected_revision = cut.current().revision();
    let selected_membership = cut
        .stream(selected_revision)
        .map_err(|_| {
            ItemRefusal::Source("source-foundation exact cut membership unavailable".into())
        })?
        .expectation();
    if records.source_revision != selected_revision
        || records.source_membership != selected_membership
    {
        return Err(ItemRefusal::Source(
            "source-foundation Records report differs from the exact captured cut".into(),
        ));
    }
    check_current_paths(cut, current_paths, operation.deadline)?;
    let records_observed_read_bytes = records.cost.record_observed_read_bytes;
    let records_read_reservation_bytes = records_read_reservation(&records)?;
    if records_read_reservation_bytes > operation.max_total_bytes {
        return Err(budget_refusal(
            "source-foundation ordered read reservation",
            records_read_reservation_bytes,
            operation.max_total_bytes,
        ));
    }

    // The legacy fixed entry continues to reserve the complete Records
    // operation ceiling. The additive rolling Records entry supplies its
    // same-kernel logical-retention baseline instead; it does not change the
    // operation's absolute ceiling.
    let records_state_reservation_upper_bound_bytes = records
        .cost
        .rolling_accounted_state_upper_bound_bytes
        .unwrap_or(records.cost.operation_state_limit_bytes);
    let fixed_output_state_bytes = size_of::<SourceFoundationDefaultRulesReport>();
    let mut used_state_bytes = records_state_reservation_upper_bound_bytes
        .checked_add(fixed_output_state_bytes)
        .filter(|used| *used <= operation.max_state_bytes)
        .ok_or_else(|| {
            budget_refusal(
                "source-foundation ordered state reservation",
                records_state_reservation_upper_bound_bytes.saturating_add(fixed_output_state_bytes)
                    as u64,
                operation.max_state_bytes as u64,
            )
        })?;

    let records_issue_count = records_direct_issue_count(&records)?;
    if records_issue_count > operation.max_issues {
        return Err(budget_refusal(
            "source-foundation ordered issue reservation",
            records_issue_count as u64,
            operation.max_issues as u64,
        ));
    }

    let later_read_allowance = operation
        .max_total_bytes
        .checked_sub(records_read_reservation_bytes)
        .ok_or(ItemRefusal::Budget)?;
    let mut aggregate_source = AggregateLayerFamilySource {
        inner: source,
        read_allowance: later_read_allowance,
        read_bytes: 0,
    };

    // Labs run first and preserve their own direct/schema order. The wrapper
    // makes each later read consume the same remaining aggregate budget.
    let labs_limits = remaining_limits(
        operation,
        records_read_reservation_bytes,
        aggregate_source.read_bytes,
        used_state_bytes,
        records_issue_count,
    )?;
    let labs = inspect_source_foundation_labs_with_physical(
        &mut aggregate_source,
        labs_limits,
        current_paths,
        physical,
    )?;
    let mut direct_owner_issue_count = records_issue_count
        .checked_add(labs.ordered_issues.len())
        .ok_or(ItemRefusal::Budget)?;
    used_state_bytes = add_state(
        used_state_bytes,
        labs.cost.retained_state_bytes,
        operation.max_state_bytes,
    )?;

    // Records event writes are folded before Gold. The source rows remain
    // intact inside their owner report while this map is the one charged
    // keyed projection shared by the later districts.
    let mut events = EventLedger::new();
    merge_event_rows(
        &mut events,
        &records.source_event_insertions,
        &mut used_state_bytes,
        operation.max_state_bytes,
        limits.max_event_map_bytes,
    )?;

    let gold_limits = remaining_limits(
        operation,
        records_read_reservation_bytes,
        aggregate_source.read_bytes,
        used_state_bytes,
        direct_owner_issue_count,
    )?;
    let goldsets = inspect_source_foundation_goldsets(
        &mut aggregate_source,
        current_paths,
        &events.by_id,
        &records.current_records,
        &records.item_editions,
        &records.rights_ids,
        &records.file_memberships,
        physical,
        require_local_payloads,
        gold_limits,
    )?;
    direct_owner_issue_count = direct_owner_issue_count
        .checked_add(goldsets.ordered_issues.len())
        .ok_or(ItemRefusal::Budget)?;
    used_state_bytes = add_state(
        used_state_bytes,
        goldsets.retained_state_bytes,
        operation.max_state_bytes,
    )?;
    merge_event_rows(
        &mut events,
        &goldsets.source_events,
        &mut used_state_bytes,
        operation.max_state_bytes,
        limits.max_event_map_bytes,
    )?;

    let discovery_limits = remaining_limits(
        operation,
        records_read_reservation_bytes,
        aggregate_source.read_bytes,
        used_state_bytes,
        direct_owner_issue_count,
    )?;
    let discovery_report = match &records.records {
        SourceFoundationRecordKernelOutcome::Complete(record_report) => {
            match invalid_artifact_proofs {
                Some(proofs) => inspect_with_cut_and_artifact_replays_and_records_with_proofs(
                    &mut aggregate_source,
                    current_paths,
                    &events.by_id,
                    discovery_limits,
                    physical,
                    cut,
                    native_histories,
                    artifact_replays,
                    record_report,
                    proofs,
                    require_local_payloads,
                )?,
                None => inspect_with_cut_and_artifact_replays_and_records(
                    &mut aggregate_source,
                    current_paths,
                    &events.by_id,
                    discovery_limits,
                    physical,
                    cut,
                    native_histories,
                    artifact_replays,
                    record_report,
                    require_local_payloads,
                )?,
            }
        }
        SourceFoundationRecordKernelOutcome::Refused { .. } => {
            if invalid_artifact_proofs.is_some() {
                return Err(ItemRefusal::Source(
                    "source-foundation Artifact proof requires complete Records coverage".into(),
                ));
            }
            inspect_with_cut_and_artifact_replays(
                &mut aggregate_source,
                current_paths,
                &events.by_id,
                discovery_limits,
                physical,
                cut,
                native_histories,
                artifact_replays,
                require_local_payloads,
            )?
        }
    };
    direct_owner_issue_count = direct_owner_issue_count
        .checked_add(discovery_report.issues.len())
        .ok_or(ItemRefusal::Budget)?;
    used_state_bytes = add_state(
        used_state_bytes,
        discovery_report.cost.state_bytes,
        operation.max_state_bytes,
    )?;
    merge_event_rows(
        &mut events,
        &discovery_report.source_event_insertions,
        &mut used_state_bytes,
        operation.max_state_bytes,
        limits.max_event_map_bytes,
    )?;
    let discovery = SourceFoundationDefaultDiscoveryFindings {
        issues: discovery_report.issues,
        schema_requests: discovery_report.schema_requests,
        source_event_insertions: discovery_report.source_event_insertions,
        unsupported: discovery_report.unsupported,
        cost: discovery_report.cost,
    };

    let closure_limits = remaining_limits(
        operation,
        records_read_reservation_bytes,
        aggregate_source.read_bytes,
        used_state_bytes,
        direct_owner_issue_count,
    )?;
    let closure = inspect_source_foundation_closure(
        &mut aggregate_source,
        cut,
        &events.by_id,
        &records.current_records,
        &records.item_editions,
        current_paths,
        &records.file_memberships,
        &records.rights_ids,
        &records.used_declared_profile_kinds,
        bibliographic_claims,
        closure_limits,
    )?;
    direct_owner_issue_count = direct_owner_issue_count
        .checked_add(closure.issues.len())
        .ok_or(ItemRefusal::Budget)?;
    used_state_bytes = add_state(
        used_state_bytes,
        closure.cost.reserved_state_bytes,
        operation.max_state_bytes,
    )?;

    let owner_issue_bytes = owner_issue_bytes(&labs, &records, &goldsets, &discovery, &closure)?;
    let queued_schema_document_count =
        schema_document_count(&labs, &records, &goldsets, &discovery, &closure)?;
    let later_district_state_bytes = labs
        .cost
        .retained_state_bytes
        .checked_add(goldsets.retained_state_bytes)
        .and_then(|bytes| bytes.checked_add(discovery.cost.state_bytes))
        .and_then(|bytes| bytes.checked_add(closure.cost.reserved_state_bytes))
        .ok_or(ItemRefusal::Budget)?;
    let aggregate_state_reservation_bytes = used_state_bytes;
    let later_source_read_bytes = aggregate_source.read_bytes;
    let merged_event_json_bytes = events.json_bytes;
    let merged_event_state_bytes = events.retained_state_bytes;

    Ok(SourceFoundationDefaultRulesReport {
        labs,
        records,
        goldsets,
        discovery,
        closure,
        source_events: events.by_id,
        source_event_order: events.order,
        cost: SourceFoundationDefaultRulesCost {
            records_observed_read_bytes,
            records_read_reservation_bytes,
            later_counted_source_bytes: later_source_read_bytes,
            records_state_reservation_upper_bound_bytes,
            later_district_state_bytes,
            discovery_seen_ids_peak_workspace_state_bytes: discovery
                .cost
                .candidate_discovery_seen_ids_peak_workspace_state_bytes,
            merged_event_state_bytes,
            aggregate_state_reservation_bytes,
            merged_event_json_bytes,
            owner_issue_bytes,
            direct_owner_issue_count,
            queued_schema_document_count,
        },
    })
}

struct AggregateLayerFamilySource<'a, S: LayerFamilySource + ?Sized> {
    inner: &'a mut S,
    read_allowance: u64,
    read_bytes: u64,
}

impl<S: LayerFamilySource + ?Sized> AggregateLayerFamilySource<'_, S> {
    fn remaining_read_bytes(&self) -> Result<u64, ItemRefusal> {
        self.read_allowance
            .checked_sub(self.read_bytes)
            .ok_or(ItemRefusal::Budget)
    }

    fn bounded_request(&self, requested: usize) -> Result<usize, ItemRefusal> {
        let remaining = usize::try_from(self.remaining_read_bytes()?).unwrap_or(usize::MAX);
        Ok(requested.min(remaining))
    }

    fn charge_read(&mut self, bytes: u64) -> Result<(), ItemRefusal> {
        self.read_bytes = self
            .read_bytes
            .checked_add(bytes)
            .filter(|used| *used <= self.read_allowance)
            .ok_or(ItemRefusal::Budget)?;
        Ok(())
    }
}

impl<S: LayerFamilySource + ?Sized> LayerFamilySource for AggregateLayerFamilySource<'_, S> {
    fn current(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<Option<Vec<u8>>, ItemRefusal> {
        let request_limit = self.bounded_request(max_bytes)?;
        let raw = self.inner.current(path, request_limit, deadline)?;
        if let Some(bytes) = &raw {
            if bytes.len() > request_limit {
                return Err(ItemRefusal::Budget);
            }
            self.charge_read(bytes.len() as u64)?;
        }
        Ok(raw)
    }

    fn recorded(
        &mut self,
        path: &str,
        digest: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<Option<Vec<u8>>, ItemRefusal> {
        let request_limit = self.bounded_request(max_bytes)?;
        let raw = self.inner.recorded(path, digest, request_limit, deadline)?;
        if let Some(bytes) = &raw {
            if bytes.len() > request_limit {
                return Err(ItemRefusal::Budget);
            }
            self.charge_read(bytes.len() as u64)?;
        }
        Ok(raw)
    }

    fn schema(
        &mut self,
        path: &str,
        raw: &[u8],
        contract: &str,
        deadline: Instant,
    ) -> Result<bool, ItemRefusal> {
        self.inner.schema(path, raw, contract, deadline)
    }

    fn payload(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<LayerPayload, ItemRefusal> {
        let request_limit = self.bounded_request(max_bytes)?;
        let payload = self.inner.payload(path, request_limit, deadline)?;
        if let LayerPayload::File { byte_size, .. } = &payload {
            let bounded_limit = u64::try_from(request_limit).unwrap_or(u64::MAX);
            if *byte_size > bounded_limit {
                return Err(ItemRefusal::Budget);
            }
            self.charge_read(*byte_size)?;
        }
        Ok(payload)
    }

    fn exists(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<bool, ItemRefusal> {
        let request_limit = self.bounded_request(max_bytes)?;
        self.inner.exists(path, request_limit, deadline)
    }

    fn discovered_item_manifest(
        &mut self,
        path: &str,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<bool, ItemRefusal> {
        let request_limit = self.bounded_request(max_bytes)?;
        self.inner
            .discovered_item_manifest(path, request_limit, deadline)
    }

    fn cancellation(&self) -> &AtomicBool {
        self.inner.cancellation()
    }

    fn generation(&self) -> String {
        self.inner.generation()
    }

    fn checkpoint(&self, deadline: Instant) -> Result<(), ItemRefusal> {
        self.inner.checkpoint(deadline)
    }
}

#[derive(Default)]
struct EventLedger {
    by_id: BTreeMap<String, Value>,
    order: Vec<String>,
    retained_state_bytes: usize,
    json_bytes: usize,
}

impl EventLedger {
    fn new() -> Self {
        Self {
            json_bytes: 2,
            ..Self::default()
        }
    }
}

fn merge_event_rows(
    ledger: &mut EventLedger,
    rows: &[(String, Value)],
    used_state_bytes: &mut usize,
    state_limit: usize,
    event_output_limit: usize,
) -> Result<(), ItemRefusal> {
    for (id, value) in rows {
        let new_value_state = estimate_value_storage(value)?;
        let new_value_json = bounded_json_len(value, event_output_limit)?;
        let key_json = bounded_json_len(id, event_output_limit)?;
        let key_clone_state = estimate_string_storage(id)?;
        let existing = ledger.by_id.get(id);
        let existing_value_state = existing
            .map(estimate_value_storage)
            .transpose()?
            .unwrap_or(0);
        let existing_value_json = existing
            .map(|old| bounded_json_len(old, event_output_limit))
            .transpose()?
            .unwrap_or(0);

        let is_new = existing.is_none();
        let next_json_bytes = if is_new {
            ledger
                .json_bytes
                .checked_add(key_json)
                .and_then(|bytes| bytes.checked_add(1)) // colon
                .and_then(|bytes| bytes.checked_add(new_value_json))
                .and_then(|bytes| {
                    if ledger.by_id.is_empty() {
                        Some(bytes)
                    } else {
                        bytes.checked_add(1) // comma
                    }
                })
                .ok_or(ItemRefusal::Budget)?
        } else {
            ledger
                .json_bytes
                .checked_sub(existing_value_json)
                .and_then(|bytes| bytes.checked_add(new_value_json))
                .ok_or(ItemRefusal::Budget)?
        };
        if next_json_bytes > event_output_limit {
            return Err(budget_refusal(
                "source-foundation merged event output",
                next_json_bytes as u64,
                event_output_limit as u64,
            ));
        }

        let next_event_state = if is_new {
            let event_node_state = size_of::<(String, Value)>()
                .checked_add(96)
                .ok_or(ItemRefusal::Budget)?;
            let order_slot_state = size_of::<String>()
                .checked_mul(2)
                .and_then(|bytes| bytes.checked_add(32))
                .ok_or(ItemRefusal::Budget)?;
            ledger
                .retained_state_bytes
                .checked_add(key_clone_state)
                .and_then(|bytes| bytes.checked_add(new_value_state))
                .and_then(|bytes| bytes.checked_add(event_node_state))
                .and_then(|bytes| bytes.checked_add(key_clone_state))
                .and_then(|bytes| bytes.checked_add(order_slot_state))
                .ok_or(ItemRefusal::Budget)?
        } else {
            ledger
                .retained_state_bytes
                .checked_sub(existing_value_state)
                .and_then(|bytes| bytes.checked_add(new_value_state))
                .ok_or(ItemRefusal::Budget)?
        };
        let transient_extra = if is_new {
            0
        } else {
            key_clone_state
                .checked_add(new_value_state)
                .ok_or(ItemRefusal::Budget)?
        };
        let peak_state = used_state_bytes
            .checked_add(transient_extra)
            .filter(|bytes| *bytes <= state_limit)
            .ok_or_else(|| {
                budget_refusal(
                    "source-foundation event clone peak state",
                    used_state_bytes.saturating_add(transient_extra) as u64,
                    state_limit as u64,
                )
            })?;
        let new_total_state = used_state_bytes
            .checked_sub(ledger.retained_state_bytes)
            .and_then(|base| base.checked_add(next_event_state))
            .filter(|bytes| *bytes <= state_limit)
            .ok_or_else(|| {
                budget_refusal(
                    "source-foundation event retained state",
                    used_state_bytes
                        .saturating_sub(ledger.retained_state_bytes)
                        .saturating_add(next_event_state) as u64,
                    state_limit as u64,
                )
            })?;
        let _ = peak_state;

        if is_new {
            ledger
                .order
                .try_reserve(1)
                .map_err(|_| ItemRefusal::Budget)?;
        }
        let id_key = id.clone();
        let value_copy = value.clone();
        ledger.by_id.insert(id_key, value_copy);
        if is_new {
            ledger.order.push(id.clone());
        }
        ledger.retained_state_bytes = next_event_state;
        ledger.json_bytes = next_json_bytes;
        *used_state_bytes = new_total_state;
    }
    Ok(())
}

fn records_read_reservation(records: &SourceFoundationRecordsReport) -> Result<u64, ItemRefusal> {
    let record_bytes = records
        .cost
        .record_observed_read_bytes
        .unwrap_or(records.cost.record_read_limit_bytes);
    record_bytes
        .checked_add(records.cost.item_observed_read_bytes)
        .and_then(|bytes| bytes.checked_add(records.cost.item_schema_resource_bytes))
        .ok_or(ItemRefusal::Budget)
}

fn check_current_paths(
    cut: &CorpusCutReader,
    current_paths: &[String],
    deadline: Instant,
) -> Result<(), ItemRefusal> {
    let mut members = cut.current().members();
    for supplied in current_paths {
        if Instant::now() >= deadline {
            return Err(ItemRefusal::Deadline);
        }
        let Some(member) = members.next() else {
            return Err(ItemRefusal::Source(
                "source-foundation current paths differ from the exact captured cut".into(),
            ));
        };
        if member.path.as_str() != supplied {
            return Err(ItemRefusal::Source(
                "source-foundation current paths differ from the exact captured cut".into(),
            ));
        }
    }
    if members.next().is_some() {
        return Err(ItemRefusal::Source(
            "source-foundation current paths differ from the exact captured cut".into(),
        ));
    }
    Ok(())
}

fn records_direct_issue_count(
    records: &SourceFoundationRecordsReport,
) -> Result<usize, ItemRefusal> {
    let mut count = records
        .ordered_issues
        .len()
        .checked_add(records.items.issues.len())
        .ok_or(ItemRefusal::Budget)?;
    if let crate::source_foundation_records::SourceFoundationRecordKernelOutcome::Complete(report) =
        &records.records
    {
        count = count
            .checked_add(
                usize::try_from(report.record_family.issue_count)
                    .map_err(|_| ItemRefusal::Budget)?,
            )
            .ok_or(ItemRefusal::Budget)?;
    }
    Ok(count)
}

fn remaining_limits(
    operation: ItemLimits,
    records_read_reservation_bytes: u64,
    later_read_bytes: u64,
    used_state_bytes: usize,
    used_issue_count: usize,
) -> Result<ItemLimits, ItemRefusal> {
    let max_total_bytes = operation
        .max_total_bytes
        .checked_sub(records_read_reservation_bytes)
        .and_then(|remaining| remaining.checked_sub(later_read_bytes))
        .ok_or(ItemRefusal::Budget)?;
    let max_state_bytes = operation
        .max_state_bytes
        .checked_sub(used_state_bytes)
        .ok_or(ItemRefusal::Budget)?;
    let max_issues = operation
        .max_issues
        .checked_sub(used_issue_count)
        .ok_or(ItemRefusal::Budget)?;
    Ok(ItemLimits {
        max_member_bytes: operation.max_member_bytes,
        max_total_bytes,
        max_state_bytes,
        max_issues,
        deadline: operation.deadline,
    })
}

fn add_state(current: usize, additional: usize, limit: usize) -> Result<usize, ItemRefusal> {
    current
        .checked_add(additional)
        .filter(|used| *used <= limit)
        .ok_or_else(|| {
            budget_refusal(
                "source-foundation aggregate retained state",
                current.saturating_add(additional) as u64,
                limit as u64,
            )
        })
}

fn owner_issue_bytes(
    labs: &SourceFoundationLabsReport,
    records: &SourceFoundationRecordsReport,
    goldsets: &SourceFoundationGoldsetsReport,
    discovery: &SourceFoundationDefaultDiscoveryFindings,
    closure: &SourceFoundationClosureReport,
) -> Result<usize, ItemRefusal> {
    let mut bytes = later_district_owner_issue_bytes(labs, goldsets, discovery, closure)?;
    for issue in &records.ordered_issues {
        add_text_bytes(&mut bytes, &issue.location)?;
        add_text_bytes(&mut bytes, &issue.message)?;
    }
    for issue in &records.items.issues {
        add_text_bytes(&mut bytes, &issue.path)?;
        add_text_bytes(&mut bytes, issue.code)?;
    }
    if let crate::source_foundation_records::SourceFoundationRecordKernelOutcome::Complete(report) =
        &records.records
    {
        for observation in &report.observations {
            if let RecordObservation::Issue { path, code } = observation {
                add_text_bytes(&mut bytes, path)?;
                add_text_bytes(&mut bytes, code)?;
            }
        }
    }
    Ok(bytes)
}

fn schema_document_count(
    labs: &SourceFoundationLabsReport,
    records: &SourceFoundationRecordsReport,
    goldsets: &SourceFoundationGoldsetsReport,
    discovery: &SourceFoundationDefaultDiscoveryFindings,
    closure: &SourceFoundationClosureReport,
) -> Result<usize, ItemRefusal> {
    let record_requests = records
        .schema_checks
        .iter()
        .filter(|check| check.decoded_instance.is_some() || check.legacy_raw_instance.is_some())
        .count();
    labs.schema_checks
        .len()
        .checked_add(record_requests)
        .and_then(|count| count.checked_add(goldsets.schema_requests.len()))
        .and_then(|count| count.checked_add(discovery.schema_requests.len()))
        .and_then(|count| count.checked_add(closure.schema_requests.len()))
        .ok_or(ItemRefusal::Budget)
}

fn add_text_bytes(total: &mut usize, text: &str) -> Result<(), ItemRefusal> {
    *total = total.checked_add(text.len()).ok_or(ItemRefusal::Budget)?;
    Ok(())
}

fn estimate_string_storage(text: &str) -> Result<usize, ItemRefusal> {
    text.len()
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(size_of::<String>() + 32))
        .ok_or(ItemRefusal::Budget)
}

fn estimate_value_storage(value: &Value) -> Result<usize, ItemRefusal> {
    fn walk(value: &Value, depth: usize) -> Result<usize, ItemRefusal> {
        if depth > 128 {
            return Err(ItemRefusal::Budget);
        }
        let mut bytes = size_of::<Value>();
        match value {
            Value::Null | Value::Bool(_) | Value::Number(_) => {
                bytes = bytes.checked_add(32).ok_or(ItemRefusal::Budget)?;
            }
            Value::String(text) => {
                bytes = bytes
                    .checked_add(estimate_string_storage(text)?)
                    .ok_or(ItemRefusal::Budget)?;
            }
            Value::Array(rows) => {
                bytes = bytes
                    .checked_add(
                        rows.len()
                            .checked_mul(size_of::<Value>() * 2)
                            .and_then(|capacity| capacity.checked_add(64))
                            .ok_or(ItemRefusal::Budget)?,
                    )
                    .ok_or(ItemRefusal::Budget)?;
                for row in rows {
                    bytes = bytes
                        .checked_add(walk(row, depth + 1)?)
                        .ok_or(ItemRefusal::Budget)?;
                }
            }
            Value::Object(object) => {
                bytes = bytes
                    .checked_add(
                        object
                            .len()
                            .checked_mul(size_of::<(String, Value)>() + 96)
                            .ok_or(ItemRefusal::Budget)?,
                    )
                    .ok_or(ItemRefusal::Budget)?;
                for (key, child) in object {
                    let key_bytes = estimate_string_storage(key)?;
                    let child_bytes = walk(child, depth + 1)?;
                    bytes = bytes
                        .checked_add(key_bytes)
                        .and_then(|current| current.checked_add(child_bytes))
                        .ok_or(ItemRefusal::Budget)?;
                }
            }
        }
        Ok(bytes)
    }
    walk(value, 0)
}

struct JsonSizeCounter {
    written: usize,
    limit: usize,
}

impl Write for JsonSizeCounter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let next = self
            .written
            .checked_add(bytes.len())
            .filter(|written| *written <= self.limit)
            .ok_or_else(|| io::Error::other("bounded JSON output exceeded"))?;
        self.written = next;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn bounded_json_len<T: serde::Serialize + ?Sized>(
    value: &T,
    limit: usize,
) -> Result<usize, ItemRefusal> {
    let mut counter = JsonSizeCounter { written: 0, limit };
    serde_json::to_writer(&mut counter, value).map_err(|_| ItemRefusal::Budget)?;
    Ok(counter.written)
}

fn budget_refusal(check: &'static str, used: u64, limit: u64) -> ItemRefusal {
    ItemRefusal::BudgetCheck {
        check,
        used: Some(used),
        limit: Some(limit),
    }
}
