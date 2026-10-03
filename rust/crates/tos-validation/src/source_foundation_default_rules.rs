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
    ArtifactCorrectionReplayMap, Cost as DiscoveryCost, CurrentArtifactInvalidSchemaProofs,
    Issue as DiscoveryIssue, SchemaRequest as DiscoverySchemaRequest, SourcePhysicalFacts,
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
    /// Bytes returned through the wrapper's counted current/recorded/payload
    /// calls in the later four districts. Existence semantics stay delegated
    /// to the exact source and its own read ledger.
    pub later_counted_source_bytes: u64,
    /// Selected baseline for the already-retained Records report: the full
    /// operation ceiling in the legacy fixed entry, or the Records kernel's
    /// logical accounted-retention upper bound in the rolling entry. This is
    /// not a process-memory measurement.
    pub records_state_reservation_upper_bound_bytes: usize,
    /// Retained-state estimates charged by Labs, Gold, Discovery and Closure.
    pub later_district_state_bytes: usize,
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
    let mut bytes = 0usize;
    for (location, message) in &labs.ordered_issues {
        add_text_bytes(&mut bytes, location)?;
        add_text_bytes(&mut bytes, message)?;
    }
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
