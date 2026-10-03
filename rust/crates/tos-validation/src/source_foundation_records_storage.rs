//! Bounded storage seam for the source-foundation Records and Item kernel.
//!
//! The command owner supplies this store over its already-profiled candidate
//! index scope. This trait transports facts and bounded reads only; successful
//! writes, point lookups, and cursor pages are never admission evidence.

use super::{
    BiblioCurrentRecord, SourceFoundationEventInsertion, SourceFoundationItemRecordSelection,
    SourceFoundationRecordsIssue, SourceFoundationRecordsSchemaCheck,
};
use crate::item_rules::{ItemIssue, ItemRefusal};
use crate::record_biblio_cut::{SourceCutRecordUsage, SourceCutSchemaDiagnostic};
use crate::record_rules::{
    GlobalIdFact, IdCarrier, LinkUriFact, PathReferenceCheck, RecordObservation, TypedIdRefFact,
};
use crate::source_cut::CutPreparedSchemaExecutionBinding;
use serde_json::Value;
use std::num::NonZeroUsize;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tos_foundation::Digest256;
use tos_source_store::SourceMembershipV1;

pub const NATIVE_ARTIFACT_RECORD_SCHEMA_URI: &str =
    "https://tree-of-sophia.local/ToS/contracts/artifact-source-witness-v2.schema.json";

/// Collections exposed by the completed stored report. The store preserves
/// the legacy Python owner order for each collection: source insertion order
/// for records, Item selections, editions, events, issues, schema positions,
/// and checks; sorted key order for set and file-index projections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFoundationRecordsCollection {
    CurrentRecords,
    UsedDeclaredProfileKinds,
    ItemRecordSelections,
    RecordSchemaPositions,
    FileDescriptors,
    ItemFileMemberships,
    RightsIds,
    ItemEditions,
    SourceEventInsertions,
    OrderedIssues,
    SchemaChecks,
    ItemIssues,
    ManifestItemIds,
    RecordObservations,
    RecordSchemaDiagnostics,
    LinkUriOwners,
}

/// Sorted join inputs derived from the ordered Biblio observation stream.
/// Each fact page is complete and stable: key first, original observation
/// ordinal second (except path references, which retain source order).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFoundationRecordFactCollection {
    Observations,
    GlobalIdFacts,
    LinkUriFacts,
    TypedIdRefFacts,
    PathReferenceFacts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFoundationRecordIdCarrier {
    Standalone,
    NativePacket,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFoundationGlobalIdFact {
    pub ordinal: u64,
    pub id: String,
    pub kind: String,
    pub path: String,
    pub carrier: SourceFoundationRecordIdCarrier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFoundationLinkUriFact {
    pub ordinal: u64,
    pub uri: String,
    pub id: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFoundationTypedIdRefFact {
    pub ordinal: u64,
    pub target_id: String,
    pub expected_kind: String,
    pub from_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFoundationRecordPathReference {
    pub ordinal: u64,
    pub from_path: String,
    pub target_path: String,
    pub check: PathReferenceCheck,
}

#[derive(Debug, Clone)]
pub enum SourceFoundationRecordFact {
    Observation(SourceFoundationRecordObservation),
    GlobalId(SourceFoundationGlobalIdFact),
    LinkUri(SourceFoundationLinkUriFact),
    TypedIdRef(SourceFoundationTypedIdRefFact),
    PathReference(SourceFoundationRecordPathReference),
}

#[derive(Debug, Clone)]
pub struct SourceFoundationRecordObservation {
    pub ordinal: u64,
    pub observation: RecordObservation,
}

#[derive(Debug, Clone)]
pub struct SourceFoundationRecordSchemaDiagnostic {
    pub ordinal: u64,
    pub diagnostic: SourceCutSchemaDiagnostic,
}

#[derive(Debug, Clone)]
pub struct SourceFoundationRecordFactPage {
    pub rows: Vec<SourceFoundationRecordFact>,
    pub next_cursor: Option<SourceFoundationRecordsCursor>,
    pub charged_state_bytes: usize,
}

impl From<(u64, GlobalIdFact)> for SourceFoundationGlobalIdFact {
    fn from((ordinal, fact): (u64, GlobalIdFact)) -> Self {
        Self {
            ordinal,
            id: fact.id,
            kind: fact.kind,
            path: fact.path,
            carrier: match fact.carrier {
                IdCarrier::Standalone => SourceFoundationRecordIdCarrier::Standalone,
                IdCarrier::NativePacket => SourceFoundationRecordIdCarrier::NativePacket,
            },
        }
    }
}

impl From<SourceFoundationGlobalIdFact> for (u64, GlobalIdFact) {
    fn from(fact: SourceFoundationGlobalIdFact) -> Self {
        let ordinal = fact.ordinal;
        (
            ordinal,
            GlobalIdFact {
                id: fact.id,
                kind: fact.kind,
                path: fact.path,
                carrier: match fact.carrier {
                    SourceFoundationRecordIdCarrier::Standalone => IdCarrier::Standalone,
                    SourceFoundationRecordIdCarrier::NativePacket => IdCarrier::NativePacket,
                },
            },
        )
    }
}

impl From<(u64, LinkUriFact)> for SourceFoundationLinkUriFact {
    fn from((ordinal, fact): (u64, LinkUriFact)) -> Self {
        Self {
            ordinal,
            uri: fact.uri,
            id: fact.id,
            path: fact.path,
        }
    }
}

impl From<SourceFoundationLinkUriFact> for (u64, LinkUriFact) {
    fn from(fact: SourceFoundationLinkUriFact) -> Self {
        let ordinal = fact.ordinal;
        (
            ordinal,
            LinkUriFact {
                uri: fact.uri,
                id: fact.id,
                path: fact.path,
            },
        )
    }
}

impl From<(u64, TypedIdRefFact)> for SourceFoundationTypedIdRefFact {
    fn from((ordinal, fact): (u64, TypedIdRefFact)) -> Self {
        Self {
            ordinal,
            target_id: fact.target_id,
            expected_kind: fact.expected_kind,
            from_path: fact.from_path,
        }
    }
}

impl From<SourceFoundationTypedIdRefFact> for (u64, TypedIdRefFact) {
    fn from(fact: SourceFoundationTypedIdRefFact) -> Self {
        let ordinal = fact.ordinal;
        (
            ordinal,
            TypedIdRefFact {
                target_id: fact.target_id,
                expected_kind: fact.expected_kind,
                from_path: fact.from_path,
            },
        )
    }
}

/// Opaque pagination position returned by the store. It is not an identity,
/// receipt, or proof object. Adapters should keep it small and validate that
/// it belongs to the requested collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFoundationRecordsCursor(Vec<u8>);

impl SourceFoundationRecordsCursor {
    /// Construct a cursor token in the command-owned store adapter.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Inspect the opaque cursor bytes when implementing the store adapter.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Finite caller-supplied page bounds. The adapter must account the retained
/// row objects, nested values, and cursor in `charged_state_bytes` and reject
/// any page that exceeds one of these ceilings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceFoundationRecordsPageBudget {
    pub max_rows: NonZeroUsize,
    pub max_state_bytes: NonZeroUsize,
    pub max_cursor_bytes: NonZeroUsize,
}

/// One source fact from the completed streamed report. Every row is bounded
/// by the selected page budget; no whole family collection is exposed.
#[derive(Debug, Clone)]
pub enum SourceFoundationRecordsStoredFact {
    CurrentRecord {
        record_id: String,
        record: BiblioCurrentRecord,
    },
    UsedDeclaredProfileKind(String),
    ItemRecordSelection(SourceFoundationItemRecordSelection),
    RecordSchemaPosition {
        path: String,
        before_issue: usize,
    },
    FileDescriptor {
        file_id: String,
        sha256: Value,
        byte_size: Value,
        media_type: Value,
    },
    ItemFileMembership {
        file_id: String,
        item_id: String,
    },
    LinkUriOwner {
        uri: String,
        record_id: String,
    },
    RightsId(String),
    ItemEdition {
        item_id: String,
        embodiment_ref: String,
    },
    SourceEventInsertion(SourceFoundationEventInsertion),
    OrderedIssue(SourceFoundationRecordsIssue),
    SchemaCheck(SourceFoundationRecordsSchemaCheck),
    ItemIssue(ItemIssue),
    ManifestItemId(String),
    RecordObservation(SourceFoundationRecordObservation),
    RecordSchemaDiagnostic(SourceFoundationRecordSchemaDiagnostic),
}

/// A page returned from the caller-owned index. `charged_state_bytes` is the
/// adapter's checked logical charge for rows and cursor retained by this page;
/// it is not process memory, an RSS claim, or a storage completion receipt.
#[derive(Debug, Clone)]
pub struct SourceFoundationRecordsCursorPage {
    pub rows: Vec<SourceFoundationRecordsStoredFact>,
    pub next_cursor: Option<SourceFoundationRecordsCursor>,
    pub charged_state_bytes: usize,
}

/// Bounded result of a current-record point lookup.
#[derive(Debug, Clone)]
pub struct SourceFoundationRecordsLookup {
    pub record: BiblioCurrentRecord,
    pub charged_state_bytes: usize,
}

/// Bounded point lookup of the first retained current Record at an exact path.
#[derive(Debug, Clone)]
pub struct SourceFoundationCurrentRecordPathLookup {
    pub record: BiblioCurrentRecord,
    pub charged_state_bytes: usize,
}

/// Bounded point result from the candidate's held index while joining current
/// native Artifact Records to the exact source-path stream. `schema_matches`
/// belongs to the first Record ID in binary order, matching the former sorted
/// page merge; duplicate path counts remain explicit.
#[derive(Debug, Clone, Copy)]
pub struct SourceFoundationArtifactRecordPathSummary {
    pub record_count: usize,
    pub schema_matches: bool,
    pub charged_state_bytes: usize,
}

/// Keyset page over candidate Artifact schema-proof paths stored beside the
/// streamed Records facts. Proof payloads stay in the same SQLite scope; only
/// this bounded path page is materialized for live input-presence probes.
#[derive(Debug, Clone)]
pub struct SourceFoundationCandidateArtifactProofPathPage {
    pub paths: Vec<String>,
    pub has_more: bool,
    pub charged_state_bytes: usize,
}

/// Bounded lookup of the final Item edition value.
#[derive(Debug, Clone)]
pub struct SourceFoundationItemEditionLookup {
    pub embodiment_ref: String,
    pub charged_state_bytes: usize,
}

/// Keyset page over DISTINCT current Records in binary ID order. The ordinary
/// `CurrentRecords` collection page remains in its original insertion order.
#[derive(Debug, Clone)]
pub struct SourceFoundationCurrentRecordsPage {
    pub rows: Vec<(String, BiblioCurrentRecord)>,
    pub next_after_id: Option<String>,
    pub charged_state_bytes: usize,
}

/// Bounded lookup of the current selected Item value. The store applies the
/// owner's dict law: later values replace while retaining first insertion
/// position.
#[derive(Debug, Clone)]
pub struct SourceFoundationItemSelectionLookup {
    pub selection: SourceFoundationItemRecordSelection,
    pub charged_state_bytes: usize,
}

/// Bounded lookup of a first-observed Link URI owner.
#[derive(Debug, Clone)]
pub struct SourceFoundationUriOwnerLookup {
    pub record_id: String,
    pub charged_state_bytes: usize,
}

/// First-observed File descriptor fields remain distinct from whether a
/// particular Item has a membership relation to that file.
#[derive(Debug, Clone)]
pub struct SourceFoundationFileDescriptorLookup {
    pub file_id: String,
    pub sha256: Value,
    pub byte_size: Value,
    pub media_type: Value,
    pub charged_state_bytes: usize,
}

/// Storage-only API implemented by the command adapter over the candidate's
/// existing index scope and connection. Callback invocation order is the
/// kernel's source order. The adapter must apply the indicated insertion laws:
/// current records first value wins; Item selections and editions replace the
/// value while retaining first insertion order; rights/profile kinds are sets;
/// source events append every insertion, including repeated IDs.
pub trait SourceFoundationRecordsStore {
    fn current_record_first(
        &mut self,
        id: &str,
        record: &BiblioCurrentRecord,
    ) -> Result<(), ItemRefusal>;

    fn used_profile_kind(&mut self, kind: &str) -> Result<(), ItemRefusal>;

    fn item_record_selection(
        &mut self,
        row: &SourceFoundationItemRecordSelection,
    ) -> Result<(), ItemRefusal>;

    fn item_edition(&mut self, item_id: &str, embodiment_ref: &str) -> Result<(), ItemRefusal>;

    fn rights_id(&mut self, id: &str) -> Result<(), ItemRefusal>;

    fn source_event_insertion(
        &mut self,
        row: &SourceFoundationEventInsertion,
    ) -> Result<(), ItemRefusal>;

    fn file_descriptor_first(
        &mut self,
        file_id: &str,
        sha256: &Value,
        byte_size: &Value,
        media_type: &Value,
    ) -> Result<(), ItemRefusal>;

    fn item_file_membership(&mut self, file_id: &str, item_id: &str) -> Result<(), ItemRefusal>;

    fn record_schema_position(
        &mut self,
        path: &str,
        before_issue: usize,
    ) -> Result<(), ItemRefusal>;

    fn ordered_issue(&mut self, row: &SourceFoundationRecordsIssue) -> Result<(), ItemRefusal>;

    fn schema_check(&mut self, row: &SourceFoundationRecordsSchemaCheck)
    -> Result<(), ItemRefusal>;

    fn item_issue(&mut self, row: &ItemIssue) -> Result<(), ItemRefusal>;

    fn manifest_item_id(&mut self, id: &str) -> Result<(), ItemRefusal>;

    /// Retain one observation in exact RecordFamily emission order. The
    /// adapter also maintains the complete sorted join indexes from this row;
    /// ties retain this source ordinal.
    fn record_observation(
        &mut self,
        ordinal: u64,
        row: &RecordObservation,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal>;

    /// Retain a Biblio schema diagnostic at its exact report position.
    fn record_schema_diagnostic(
        &mut self,
        ordinal: u64,
        row: &SourceCutSchemaDiagnostic,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), ItemRefusal>;

    /// Read one bounded sorted join page. The adapter binds cursors to the
    /// collection and uses the original operation deadline/cancellation token
    /// while charging every repeated read to the candidate's shared ledgers.
    fn record_fact_page(
        &self,
        collection: SourceFoundationRecordFactCollection,
        after: Option<&SourceFoundationRecordsCursor>,
        budget: SourceFoundationRecordsPageBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<SourceFoundationRecordFactPage, ItemRefusal>;

    fn lookup_current_record(
        &self,
        id: &str,
        max_state_bytes: NonZeroUsize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceFoundationRecordsLookup>, ItemRefusal>;

    fn lookup_current_record_by_path(
        &self,
        path: &str,
        max_state_bytes: NonZeroUsize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceFoundationCurrentRecordPathLookup>, ItemRefusal>;

    fn lookup_item_edition(
        &self,
        item_id: &str,
        max_state_bytes: NonZeroUsize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceFoundationItemEditionLookup>, ItemRefusal>;

    fn current_records_by_id_page(
        &self,
        after_id: Option<&str>,
        budget: SourceFoundationRecordsPageBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<SourceFoundationCurrentRecordsPage, ItemRefusal>;

    /// Insert the first URI owner, returning the prior owner when this URI
    /// already appeared. This retains source-order collision semantics.
    fn link_uri_owner_first(
        &mut self,
        uri: &str,
        record_id: &str,
        max_state_bytes: NonZeroUsize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceFoundationUriOwnerLookup>, ItemRefusal>;

    fn lookup_item_record_selection(
        &self,
        item_id: &str,
        max_state_bytes: NonZeroUsize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceFoundationItemSelectionLookup>, ItemRefusal>;

    fn lookup_file_descriptor(
        &self,
        file_id: &str,
        max_state_bytes: NonZeroUsize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceFoundationFileDescriptorLookup>, ItemRefusal>;

    fn contains_item_file_membership(
        &self,
        file_id: &str,
        item_id: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal>;

    fn contains_rights_id(
        &self,
        id: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal>;

    fn page(
        &self,
        collection: SourceFoundationRecordsCollection,
        after: Option<&SourceFoundationRecordsCursor>,
        budget: SourceFoundationRecordsPageBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<SourceFoundationRecordsCursorPage, ItemRefusal>;
}

/// Read-only view held by a completed streamed report. Its lifetime is tied to
/// the command-owned candidate scope, so the source and index stay together.
pub struct SourceFoundationRecordsIndex<'a> {
    store: &'a dyn SourceFoundationRecordsStore,
}

fn current_record_minimum_state(record: &BiblioCurrentRecord) -> Result<usize, ItemRefusal> {
    std::mem::size_of::<BiblioCurrentRecord>()
        .checked_add(record.path.len())
        .and_then(|bytes| bytes.checked_add(record.kind.len()))
        .and_then(|bytes| {
            crate::record_biblio_cut::decoded_state(&record.value)
                .ok()
                .and_then(|value| bytes.checked_add(value))
        })
        .ok_or(ItemRefusal::Budget)
}

fn current_record_page_minimum_state(
    page: &SourceFoundationCurrentRecordsPage,
) -> Result<usize, ItemRefusal> {
    let mut bytes = std::mem::size_of::<SourceFoundationCurrentRecordsPage>()
        .checked_add(
            page.rows
                .len()
                .checked_mul(std::mem::size_of::<(String, BiblioCurrentRecord)>())
                .ok_or(ItemRefusal::Budget)?,
        )
        .ok_or(ItemRefusal::Budget)?;
    for (id, record) in &page.rows {
        bytes = bytes
            .checked_add(id.len())
            .and_then(|used| used.checked_add(current_record_minimum_state(record).ok()?))
            .ok_or(ItemRefusal::Budget)?;
    }
    bytes = bytes
        .checked_add(page.next_after_id.as_ref().map_or(0, String::len))
        .ok_or(ItemRefusal::Budget)?;
    Ok(bytes)
}

impl<'a> SourceFoundationRecordsIndex<'a> {
    pub(crate) fn new(store: &'a dyn SourceFoundationRecordsStore) -> Self {
        Self { store }
    }

    /// Report whether this read-only view is tied to the exact caller-owned
    /// store object, without reopening a connection or granting authority.
    pub fn is_backed_by(&self, store: &dyn SourceFoundationRecordsStore) -> bool {
        std::ptr::eq(self.store, store)
    }

    pub fn lookup_current_record(
        &self,
        id: &str,
        max_state_bytes: NonZeroUsize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceFoundationRecordsLookup>, ItemRefusal> {
        check_current(deadline, cancelled)?;
        let found = self
            .store
            .lookup_current_record(id, max_state_bytes, deadline, cancelled)?;
        check_current(deadline, cancelled)?;
        let minimum = found
            .as_ref()
            .map(|lookup| {
                std::mem::size_of::<SourceFoundationRecordsLookup>()
                    .checked_add(lookup.record.path.len())
                    .and_then(|bytes| bytes.checked_add(lookup.record.kind.len()))
                    .and_then(|bytes| {
                        crate::record_biblio_cut::decoded_state(&lookup.record.value)
                            .ok()
                            .and_then(|value| bytes.checked_add(value))
                    })
                    .ok_or(ItemRefusal::Budget)
            })
            .transpose()?;
        if found.as_ref().is_some_and(|lookup| {
            lookup.charged_state_bytes > max_state_bytes.get()
                || minimum.is_some_and(|minimum| lookup.charged_state_bytes < minimum)
        }) {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation stored current-record lookup state",
                used: found.map(|lookup| lookup.charged_state_bytes as u64),
                limit: Some(max_state_bytes.get() as u64),
            });
        }
        Ok(found)
    }

    pub fn lookup_current_record_by_path(
        &self,
        path: &str,
        max_state_bytes: NonZeroUsize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceFoundationCurrentRecordPathLookup>, ItemRefusal> {
        check_current(deadline, cancelled)?;
        let found =
            self.store
                .lookup_current_record_by_path(path, max_state_bytes, deadline, cancelled)?;
        check_current(deadline, cancelled)?;
        let minimum = found
            .as_ref()
            .map(|lookup| current_record_minimum_state(&lookup.record))
            .transpose()?;
        if found.as_ref().is_some_and(|lookup| {
            lookup.charged_state_bytes > max_state_bytes.get()
                || minimum.is_some_and(|minimum| lookup.charged_state_bytes < minimum)
        }) {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation stored current-record path lookup state",
                used: found.map(|lookup| lookup.charged_state_bytes as u64),
                limit: Some(max_state_bytes.get() as u64),
            });
        }
        Ok(found)
    }

    pub fn lookup_item_edition(
        &self,
        item_id: &str,
        max_state_bytes: NonZeroUsize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceFoundationItemEditionLookup>, ItemRefusal> {
        check_current(deadline, cancelled)?;
        let found =
            self.store
                .lookup_item_edition(item_id, max_state_bytes, deadline, cancelled)?;
        check_current(deadline, cancelled)?;
        let minimum = found
            .as_ref()
            .map(|lookup| {
                std::mem::size_of::<SourceFoundationItemEditionLookup>()
                    .checked_add(lookup.embodiment_ref.len())
                    .ok_or(ItemRefusal::Budget)
            })
            .transpose()?;
        if found.as_ref().is_some_and(|lookup| {
            lookup.charged_state_bytes > max_state_bytes.get()
                || minimum.is_some_and(|minimum| lookup.charged_state_bytes < minimum)
        }) {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation stored Item-edition lookup state",
                used: found.map(|lookup| lookup.charged_state_bytes as u64),
                limit: Some(max_state_bytes.get() as u64),
            });
        }
        Ok(found)
    }

    pub fn current_records_by_id_page(
        &self,
        after_id: Option<&str>,
        budget: SourceFoundationRecordsPageBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<SourceFoundationCurrentRecordsPage, ItemRefusal> {
        check_current(deadline, cancelled)?;
        if after_id.is_some_and(|id| id.len() > budget.max_cursor_bytes.get()) {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation current Record ID cursor bytes",
                used: after_id.map(|id| id.len() as u64),
                limit: Some(budget.max_cursor_bytes.get() as u64),
            });
        }
        let page = self
            .store
            .current_records_by_id_page(after_id, budget, deadline, cancelled)?;
        check_current(deadline, cancelled)?;
        let minimum = current_record_page_minimum_state(&page)?;
        let next_matches_last = match page.next_after_id.as_deref() {
            Some(next) => page.rows.last().is_some_and(|(last, _)| last == next),
            None => true,
        };
        for (index, (id, _)) in page.rows.iter().enumerate() {
            let previous = if index == 0 {
                after_id
            } else {
                Some(page.rows[index - 1].0.as_str())
            };
            if previous.is_some_and(|prior| prior >= id.as_str()) {
                return Err(ItemRefusal::Source(
                    "source-foundation current Record page order changed".into(),
                ));
            }
        }
        if page.rows.len() > budget.max_rows.get()
            || page.charged_state_bytes > budget.max_state_bytes.get()
            || page.charged_state_bytes < minimum
            || page
                .next_after_id
                .as_deref()
                .is_some_and(|id| id.len() > budget.max_cursor_bytes.get())
            || page
                .next_after_id
                .as_deref()
                .is_some_and(|next| !next_matches_last)
            || page.rows.is_empty() && page.next_after_id.is_some()
        {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation current Record page envelope",
                used: Some(page.charged_state_bytes as u64),
                limit: Some(budget.max_state_bytes.get() as u64),
            });
        }
        Ok(page)
    }

    pub fn lookup_item_record_selection(
        &self,
        item_id: &str,
        max_state_bytes: NonZeroUsize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceFoundationItemSelectionLookup>, ItemRefusal> {
        check_current(deadline, cancelled)?;
        let found = self.store.lookup_item_record_selection(
            item_id,
            max_state_bytes,
            deadline,
            cancelled,
        )?;
        check_current(deadline, cancelled)?;
        let minimum = found
            .as_ref()
            .map(|lookup| {
                std::mem::size_of::<SourceFoundationItemSelectionLookup>()
                    .checked_add(lookup.selection.record_id.len())
                    .and_then(|bytes| bytes.checked_add(lookup.selection.path.len()))
                    .and_then(|bytes| {
                        crate::record_biblio_cut::decoded_state(&lookup.selection.value)
                            .ok()
                            .and_then(|value| bytes.checked_add(value))
                    })
                    .ok_or(ItemRefusal::Budget)
            })
            .transpose()?;
        if found.as_ref().is_some_and(|lookup| {
            lookup.charged_state_bytes > max_state_bytes.get()
                || minimum.is_some_and(|minimum| lookup.charged_state_bytes < minimum)
        }) {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation stored Item-selection lookup state",
                used: found.map(|lookup| lookup.charged_state_bytes as u64),
                limit: Some(max_state_bytes.get() as u64),
            });
        }
        Ok(found)
    }

    pub fn lookup_file_descriptor(
        &self,
        file_id: &str,
        max_state_bytes: NonZeroUsize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<SourceFoundationFileDescriptorLookup>, ItemRefusal> {
        check_current(deadline, cancelled)?;
        let found =
            self.store
                .lookup_file_descriptor(file_id, max_state_bytes, deadline, cancelled)?;
        check_current(deadline, cancelled)?;
        let minimum = found
            .as_ref()
            .map(|lookup| {
                [&lookup.sha256, &lookup.byte_size, &lookup.media_type]
                    .into_iter()
                    .try_fold(
                        std::mem::size_of::<SourceFoundationFileDescriptorLookup>()
                            .checked_add(lookup.file_id.len())
                            .ok_or(ItemRefusal::Budget)?,
                        |used, value| {
                            used.checked_add(crate::record_biblio_cut::decoded_state(value)?)
                                .ok_or(ItemRefusal::Budget)
                        },
                    )
            })
            .transpose()?;
        if found.as_ref().is_some_and(|lookup| {
            lookup.charged_state_bytes > max_state_bytes.get()
                || minimum.is_some_and(|minimum| lookup.charged_state_bytes < minimum)
        }) {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation stored File-descriptor lookup state",
                used: found.map(|lookup| lookup.charged_state_bytes as u64),
                limit: Some(max_state_bytes.get() as u64),
            });
        }
        Ok(found)
    }

    pub fn contains_item_file_membership(
        &self,
        file_id: &str,
        item_id: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        check_current(deadline, cancelled)?;
        let contains = self
            .store
            .contains_item_file_membership(file_id, item_id, deadline, cancelled)?;
        check_current(deadline, cancelled)?;
        Ok(contains)
    }

    pub fn contains_rights_id(
        &self,
        id: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<bool, ItemRefusal> {
        check_current(deadline, cancelled)?;
        let contains = self.store.contains_rights_id(id, deadline, cancelled)?;
        check_current(deadline, cancelled)?;
        Ok(contains)
    }

    pub fn page(
        &self,
        collection: SourceFoundationRecordsCollection,
        after: Option<&SourceFoundationRecordsCursor>,
        budget: SourceFoundationRecordsPageBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<SourceFoundationRecordsCursorPage, ItemRefusal> {
        check_current(deadline, cancelled)?;
        if after.is_some_and(|cursor| cursor.as_bytes().len() > budget.max_cursor_bytes.get()) {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation stored cursor input bytes",
                used: after.map(|cursor| cursor.as_bytes().len() as u64),
                limit: Some(budget.max_cursor_bytes.get() as u64),
            });
        }
        let page = self
            .store
            .page(collection, after, budget, deadline, cancelled)?;
        check_current(deadline, cancelled)?;
        if (page.next_cursor.is_some() && page.next_cursor.as_ref() == after)
            || (page.rows.is_empty() && page.next_cursor.is_some())
        {
            return Err(ItemRefusal::Source(
                "source-foundation stored cursor did not advance".into(),
            ));
        }
        if page.rows.len() > budget.max_rows.get()
            || page.charged_state_bytes > budget.max_state_bytes.get()
            || page
                .rows
                .iter()
                .any(|row| !stored_fact_matches_collection(collection, row))
            || page.charged_state_bytes
                < source_foundation_page_minimum_state_bytes(&page.rows, page.next_cursor.as_ref())?
            || page
                .next_cursor
                .as_ref()
                .is_some_and(|cursor| cursor.as_bytes().len() > budget.max_cursor_bytes.get())
        {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation stored cursor page envelope",
                used: Some(page.charged_state_bytes as u64),
                limit: Some(budget.max_state_bytes.get() as u64),
            });
        }
        Ok(page)
    }

    pub(crate) fn record_fact_page(
        &self,
        collection: SourceFoundationRecordFactCollection,
        after: Option<&SourceFoundationRecordsCursor>,
        budget: SourceFoundationRecordsPageBudget,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<SourceFoundationRecordFactPage, ItemRefusal> {
        if after.is_some_and(|cursor| cursor.as_bytes().len() > budget.max_cursor_bytes.get()) {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation Biblio cursor input bytes",
                used: after.map(|cursor| cursor.as_bytes().len() as u64),
                limit: Some(budget.max_cursor_bytes.get() as u64),
            });
        }
        if Instant::now() >= deadline {
            return Err(ItemRefusal::Deadline);
        }
        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(ItemRefusal::Source("Biblio fact page cancelled".into()));
        }
        let page = self
            .store
            .record_fact_page(collection, after, budget, deadline, cancelled)?;
        let minimum =
            source_foundation_fact_page_minimum_state_bytes(&page.rows, page.next_cursor.as_ref())?;
        if (page.next_cursor.is_some() && page.next_cursor.as_ref() == after)
            || (page.rows.is_empty() && page.next_cursor.is_some())
            || page
                .rows
                .iter()
                .any(|row| !record_fact_matches_collection(collection, row))
            || page.rows.len() > budget.max_rows.get()
            || page.charged_state_bytes > budget.max_state_bytes.get()
            || page.charged_state_bytes < minimum
            || page
                .next_cursor
                .as_ref()
                .is_some_and(|cursor| cursor.as_bytes().len() > budget.max_cursor_bytes.get())
        {
            return Err(ItemRefusal::BudgetCheck {
                check: "source-foundation Biblio fact page envelope",
                used: Some(page.charged_state_bytes as u64),
                limit: Some(budget.max_state_bytes.get() as u64),
            });
        }
        if Instant::now() >= deadline {
            return Err(ItemRefusal::Deadline);
        }
        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(ItemRefusal::Source("Biblio fact page cancelled".into()));
        }
        Ok(page)
    }
}

fn record_fact_matches_collection(
    collection: SourceFoundationRecordFactCollection,
    row: &SourceFoundationRecordFact,
) -> bool {
    matches!(
        (collection, row),
        (
            SourceFoundationRecordFactCollection::Observations,
            SourceFoundationRecordFact::Observation(_)
        ) | (
            SourceFoundationRecordFactCollection::GlobalIdFacts,
            SourceFoundationRecordFact::GlobalId(_)
        ) | (
            SourceFoundationRecordFactCollection::LinkUriFacts,
            SourceFoundationRecordFact::LinkUri(_)
        ) | (
            SourceFoundationRecordFactCollection::TypedIdRefFacts,
            SourceFoundationRecordFact::TypedIdRef(_)
        ) | (
            SourceFoundationRecordFactCollection::PathReferenceFacts,
            SourceFoundationRecordFact::PathReference(_)
        )
    )
}

fn source_foundation_page_minimum_state_bytes(
    rows: &[SourceFoundationRecordsStoredFact],
    cursor: Option<&SourceFoundationRecordsCursor>,
) -> Result<usize, ItemRefusal> {
    let mut bytes = std::mem::size_of::<SourceFoundationRecordsCursorPage>();
    bytes = bytes
        .checked_add(
            rows.len()
                .checked_mul(std::mem::size_of::<SourceFoundationRecordsStoredFact>())
                .ok_or(ItemRefusal::Budget)?,
        )
        .ok_or(ItemRefusal::Budget)?;
    for row in rows {
        bytes = bytes
            .checked_add(source_foundation_stored_fact_minimum_state_bytes(row)?)
            .ok_or(ItemRefusal::Budget)?;
    }
    bytes = bytes
        .checked_add(cursor.map_or(0, |token| token.as_bytes().len()))
        .ok_or(ItemRefusal::Budget)?;
    Ok(bytes)
}

fn stored_fact_matches_collection(
    collection: SourceFoundationRecordsCollection,
    row: &SourceFoundationRecordsStoredFact,
) -> bool {
    matches!(
        (collection, row),
        (
            SourceFoundationRecordsCollection::CurrentRecords,
            SourceFoundationRecordsStoredFact::CurrentRecord { .. }
        ) | (
            SourceFoundationRecordsCollection::UsedDeclaredProfileKinds,
            SourceFoundationRecordsStoredFact::UsedDeclaredProfileKind(_)
        ) | (
            SourceFoundationRecordsCollection::ItemRecordSelections,
            SourceFoundationRecordsStoredFact::ItemRecordSelection(_)
        ) | (
            SourceFoundationRecordsCollection::RecordSchemaPositions,
            SourceFoundationRecordsStoredFact::RecordSchemaPosition { .. }
        ) | (
            SourceFoundationRecordsCollection::FileDescriptors,
            SourceFoundationRecordsStoredFact::FileDescriptor { .. }
        ) | (
            SourceFoundationRecordsCollection::ItemFileMemberships,
            SourceFoundationRecordsStoredFact::ItemFileMembership { .. }
        ) | (
            SourceFoundationRecordsCollection::RightsIds,
            SourceFoundationRecordsStoredFact::RightsId(_)
        ) | (
            SourceFoundationRecordsCollection::ItemEditions,
            SourceFoundationRecordsStoredFact::ItemEdition { .. }
        ) | (
            SourceFoundationRecordsCollection::SourceEventInsertions,
            SourceFoundationRecordsStoredFact::SourceEventInsertion(_)
        ) | (
            SourceFoundationRecordsCollection::OrderedIssues,
            SourceFoundationRecordsStoredFact::OrderedIssue(_)
        ) | (
            SourceFoundationRecordsCollection::SchemaChecks,
            SourceFoundationRecordsStoredFact::SchemaCheck(_)
        ) | (
            SourceFoundationRecordsCollection::ItemIssues,
            SourceFoundationRecordsStoredFact::ItemIssue(_)
        ) | (
            SourceFoundationRecordsCollection::ManifestItemIds,
            SourceFoundationRecordsStoredFact::ManifestItemId(_)
        ) | (
            SourceFoundationRecordsCollection::RecordObservations,
            SourceFoundationRecordsStoredFact::RecordObservation(_)
        ) | (
            SourceFoundationRecordsCollection::RecordSchemaDiagnostics,
            SourceFoundationRecordsStoredFact::RecordSchemaDiagnostic(_)
        ) | (
            SourceFoundationRecordsCollection::LinkUriOwners,
            SourceFoundationRecordsStoredFact::LinkUriOwner { .. }
        )
    )
}

fn check_current(deadline: Instant, cancelled: &AtomicBool) -> Result<(), ItemRefusal> {
    if Instant::now() >= deadline {
        return Err(ItemRefusal::Deadline);
    }
    if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(ItemRefusal::Source(
            "source-foundation indexed read cancelled".into(),
        ));
    }
    Ok(())
}

fn source_foundation_fact_page_minimum_state_bytes(
    rows: &[SourceFoundationRecordFact],
    cursor: Option<&SourceFoundationRecordsCursor>,
) -> Result<usize, ItemRefusal> {
    let mut bytes = std::mem::size_of::<SourceFoundationRecordFactPage>();
    bytes = bytes
        .checked_add(
            rows.len()
                .checked_mul(std::mem::size_of::<SourceFoundationRecordFact>())
                .ok_or(ItemRefusal::Budget)?,
        )
        .ok_or(ItemRefusal::Budget)?;
    for row in rows {
        let payload = match row {
            SourceFoundationRecordFact::Observation(row) => {
                record_observation_minimum_state_bytes(&row.observation)?
            }
            SourceFoundationRecordFact::GlobalId(fact) => {
                add_state_fields(&[fact.id.len(), fact.kind.len(), fact.path.len()])?
            }
            SourceFoundationRecordFact::LinkUri(fact) => {
                add_state_fields(&[fact.uri.len(), fact.id.len(), fact.path.len()])?
            }
            SourceFoundationRecordFact::TypedIdRef(fact) => add_state_fields(&[
                fact.target_id.len(),
                fact.expected_kind.len(),
                fact.from_path.len(),
            ])?,
            SourceFoundationRecordFact::PathReference(fact) => {
                add_state_fields(&[fact.from_path.len(), fact.target_path.len()])?
            }
        };
        bytes = bytes.checked_add(payload).ok_or(ItemRefusal::Budget)?;
    }
    bytes = bytes
        .checked_add(cursor.map_or(0, |token| token.as_bytes().len()))
        .ok_or(ItemRefusal::Budget)?;
    Ok(bytes)
}

fn source_foundation_stored_fact_minimum_state_bytes(
    row: &SourceFoundationRecordsStoredFact,
) -> Result<usize, ItemRefusal> {
    let size = std::mem::size_of::<SourceFoundationRecordsStoredFact>();
    let payload = match row {
        SourceFoundationRecordsStoredFact::CurrentRecord { record_id, record } => {
            add_state_fields(&[
                record_id.len(),
                record.path.len(),
                record.kind.len(),
                crate::record_biblio_cut::decoded_state(&record.value)?,
            ])?
        }
        SourceFoundationRecordsStoredFact::UsedDeclaredProfileKind(kind)
        | SourceFoundationRecordsStoredFact::RightsId(kind)
        | SourceFoundationRecordsStoredFact::ManifestItemId(kind) => kind.len(),
        SourceFoundationRecordsStoredFact::ItemRecordSelection(row) => add_state_fields(&[
            row.record_id.len(),
            row.path.len(),
            crate::record_biblio_cut::decoded_state(&row.value)?,
        ])?,
        SourceFoundationRecordsStoredFact::RecordSchemaPosition { path, .. } => path.len(),
        SourceFoundationRecordsStoredFact::FileDescriptor {
            file_id,
            sha256,
            byte_size,
            media_type,
        } => add_state_fields(&[
            file_id.len(),
            crate::record_biblio_cut::decoded_state(sha256)?,
            crate::record_biblio_cut::decoded_state(byte_size)?,
            crate::record_biblio_cut::decoded_state(media_type)?,
        ])?,
        SourceFoundationRecordsStoredFact::ItemFileMembership { file_id, item_id } => {
            add_state_fields(&[file_id.len(), item_id.len()])?
        }
        SourceFoundationRecordsStoredFact::LinkUriOwner { uri, record_id } => {
            add_state_fields(&[uri.len(), record_id.len()])?
        }
        SourceFoundationRecordsStoredFact::ItemEdition {
            item_id,
            embodiment_ref,
        } => add_state_fields(&[item_id.len(), embodiment_ref.len()])?,
        SourceFoundationRecordsStoredFact::SourceEventInsertion((event_id, value)) => {
            add_state_fields(&[
                event_id.len(),
                crate::record_biblio_cut::decoded_state(value)?,
            ])?
        }
        SourceFoundationRecordsStoredFact::OrderedIssue(row) => {
            add_state_fields(&[row.location.len(), row.message.len()])?
        }
        SourceFoundationRecordsStoredFact::SchemaCheck(row) => add_state_fields(&[
            row.location.len(),
            row.contract.len(),
            row.decoded_instance
                .as_ref()
                .map(crate::record_biblio_cut::decoded_state)
                .transpose()?
                .unwrap_or(0),
            row.legacy_raw_instance.as_ref().map(Vec::len).unwrap_or(0),
        ])?,
        SourceFoundationRecordsStoredFact::ItemIssue(row) => {
            add_state_fields(&[row.path.len(), row.code.len()])?
        }
        SourceFoundationRecordsStoredFact::RecordObservation(row) => {
            record_observation_minimum_state_bytes(&row.observation)?
        }
        SourceFoundationRecordsStoredFact::RecordSchemaDiagnostic(row) => {
            row.diagnostic.accounted_state_bytes
        }
    };
    size.checked_add(payload).ok_or(ItemRefusal::Budget)
}

fn add_state_fields(fields: &[usize]) -> Result<usize, ItemRefusal> {
    fields.iter().try_fold(0usize, |sum, field| {
        sum.checked_add(*field).ok_or(ItemRefusal::Budget)
    })
}

fn record_observation_minimum_state_bytes(row: &RecordObservation) -> Result<usize, ItemRefusal> {
    let payload = match row {
        RecordObservation::ExactPath { path, raw_sha256 } => {
            add_state_fields(&[path.len(), raw_sha256.len()])?
        }
        RecordObservation::Registry {
            path,
            version,
            raw_sha256,
        } => add_state_fields(&[path.len(), version.len(), raw_sha256.len()])?,
        RecordObservation::Schema {
            path,
            uri,
            raw_sha256,
        } => add_state_fields(&[path.len(), uri.len(), raw_sha256.len()])?,
        RecordObservation::Profile {
            path,
            kind,
            schema_version,
            ..
        } => add_state_fields(&[path.len(), kind.len(), schema_version.len()])?,
        RecordObservation::Reference {
            from_path,
            target_path,
            ..
        } => add_state_fields(&[from_path.len(), target_path.len()])?,
        RecordObservation::RecordIdReference {
            from_path,
            target_id,
            expected_kind,
        } => add_state_fields(&[from_path.len(), target_id.len(), expected_kind.len()])?,
        RecordObservation::LinkUriOwner { uri, id, path } => {
            add_state_fields(&[uri.len(), id.len(), path.len()])?
        }
        RecordObservation::IdOwner {
            id,
            kind,
            path,
            raw_sha256,
            ..
        } => add_state_fields(&[id.len(), kind.len(), path.len(), raw_sha256.len()])?,
        RecordObservation::IdKindOwner { kind, id, path } => {
            add_state_fields(&[kind.len(), id.len(), path.len()])?
        }
        RecordObservation::NativeReservation {
            id,
            packet_path,
            raw_sha256,
        } => add_state_fields(&[id.len(), packet_path.len(), raw_sha256.len()])?,
        RecordObservation::Issue { path, code } => add_state_fields(&[path.len(), code.len()])?,
    };
    std::mem::size_of::<RecordObservation>()
        .checked_add(payload)
        .ok_or(ItemRefusal::Budget)
}

/// Compact Item family summary for the streamed form. Row collections remain
/// available through `SourceFoundationRecordsIndex`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFoundationRecordsStreamedItemSummary {
    pub issue_count: usize,
    pub manifest_item_id_count: usize,
    pub metadata_bytes: u64,
    pub unavailable_payloads: u64,
    pub accounted_state_upper_bound_bytes: usize,
    pub inventory_set_scan_steps: usize,
    pub source_admission_complete: bool,
}

/// Prepared, selected Item schema identity used by the candidate receiver.
/// The ordered resource set itself remains owned by the candidate adapter;
/// this projection carries its exact set and selection digests after each
/// selected path, size, digest, and worker check has been verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceFoundationCandidateSchemaIdentity {
    prepared_execution: CutPreparedSchemaExecutionBinding,
    contract_selection_digest: Digest256,
    selected_resource_count: u64,
    selected_resource_bytes: u64,
}

impl SourceFoundationCandidateSchemaIdentity {
    pub(super) fn new(
        prepared_execution: CutPreparedSchemaExecutionBinding,
        contract_selection_digest: Digest256,
        selected_resource_count: u64,
        selected_resource_bytes: u64,
    ) -> Self {
        Self {
            prepared_execution,
            contract_selection_digest,
            selected_resource_count,
            selected_resource_bytes,
        }
    }

    pub fn profile(&self) -> crate::FormatProfile {
        self.prepared_execution.schema_profile
    }

    pub fn schema_set_digest(&self) -> Digest256 {
        self.prepared_execution.schema_set_sha256
    }

    pub fn worker_digest(&self) -> Digest256 {
        self.prepared_execution.worker_sha256
    }

    pub fn prepared_execution_binding(&self) -> CutPreparedSchemaExecutionBinding {
        self.prepared_execution
    }

    pub fn contract_selection_digest(&self) -> Digest256 {
        self.contract_selection_digest
    }

    pub fn selected_resource_count(&self) -> u64 {
        self.selected_resource_count
    }

    pub fn selected_resource_bytes(&self) -> u64 {
        self.selected_resource_bytes
    }
}

/// Completed stream result with an opaque source identity. It deliberately
/// does not require `SourceRevision`: a candidate fence stays its own input
/// identity while `source_membership` carries the exact enumerated set.
/// Construction is private to the source-foundation kernel after its complete
/// read and final currentness checks.
pub struct SourceFoundationRecordsStreamedReport<'a, I> {
    input_identity: I,
    source_membership: SourceMembershipV1,
    record_usage: SourceCutRecordUsage,
    items: SourceFoundationRecordsStreamedItemSummary,
    cost: super::SourceFoundationRecordsCost,
    candidate_schema_identity: Option<SourceFoundationCandidateSchemaIdentity>,
    index: SourceFoundationRecordsIndex<'a>,
}

impl<'a, I> SourceFoundationRecordsStreamedReport<'a, I> {
    pub(super) fn new_completed(
        input_identity: I,
        source_membership: SourceMembershipV1,
        record_usage: SourceCutRecordUsage,
        items: SourceFoundationRecordsStreamedItemSummary,
        cost: super::SourceFoundationRecordsCost,
        candidate_schema_identity: Option<SourceFoundationCandidateSchemaIdentity>,
        store: &'a dyn SourceFoundationRecordsStore,
    ) -> Self {
        Self {
            input_identity,
            source_membership,
            record_usage,
            items,
            cost,
            candidate_schema_identity,
            index: SourceFoundationRecordsIndex::new(store),
        }
    }

    pub fn input_identity(&self) -> &I {
        &self.input_identity
    }

    pub fn source_membership(&self) -> &SourceMembershipV1 {
        &self.source_membership
    }

    pub fn record_usage(&self) -> &SourceCutRecordUsage {
        &self.record_usage
    }

    pub fn items(&self) -> &SourceFoundationRecordsStreamedItemSummary {
        &self.items
    }

    pub fn cost(&self) -> &super::SourceFoundationRecordsCost {
        &self.cost
    }

    pub fn candidate_schema_identity(&self) -> Option<&SourceFoundationCandidateSchemaIdentity> {
        self.candidate_schema_identity.as_ref()
    }

    pub fn index(&self) -> &SourceFoundationRecordsIndex<'a> {
        &self.index
    }
}
