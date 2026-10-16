//! Bounded PostgreSQL-backed derived documents over exact committed source
//! history. This is a private projection mechanic, not source admission or an
//! external owner's authority.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use postgres::{GenericClient, IsolationLevel, Row, Transaction};
use serde::{Deserialize, Serialize};
use tos_foundation::{Digest256, Digest256Hasher, RelativePath};
use tos_segment_store::SegmentStore;

use super::{DurableError, DurablePgCoordinator, DurableResult, as_i64, as_u64};

#[path = "durable_source_text_search.rs"]
pub(crate) mod durable_source_text_search;

#[path = "durable_navigation_event.rs"]
mod durable_navigation_event;
pub(crate) use durable_navigation_event::{
    DurableNavigationAdjacencyCertificateReadV1, DurableNavigationCurrentnessGuardV1,
    DurableNavigationCurrentnessV1, DurableNavigationPublishedGenerationGuardV2,
    DurableNavigationPublishedGenerationV2, navigation_owner_binding,
    navigation_selected_expectation_digest, read_navigation_adjacency_certificate_v1,
    read_navigation_published_guard_with_tx_v2, same_navigation_carrier_payload,
    verify_navigation_selected_cohort_metadata_v3,
};
pub use durable_navigation_event::{
    DurableNavigationBaseRowReadV1, DurableNavigationBootstrapApplyV1,
    DurableNavigationBootstrapOwnerSourceV1, DurableNavigationBootstrapRowKindV1,
    DurableNavigationBootstrapRowV1, DurableNavigationCurrentIndexNotReadyReasonV1,
    DurableNavigationCurrentIndexNotReadyV1, DurableNavigationCurrentIndexOwnerBindingV1,
    DurableNavigationEventAffectedOwnerKindV1, DurableNavigationEventAffectedOwnerV1,
    DurableNavigationEventContextV1, DurableNavigationEventCursorV1, DurableNavigationEventLimits,
    DurableNavigationEventRangeApplyV1, DurableNavigationEventRangeClaimMutationV1,
    DurableNavigationEventRangeCoverageV1, DurableNavigationEventRangeDependencyCauseV1,
    DurableNavigationEventRangeEventV1, DurableNavigationEventRangeMemberV1,
    DurableNavigationEventRangePendingReasonV1, DurableNavigationEventRangePriorClaimOwnerV1,
    DurableNavigationEventRangePriorOwnerBasisV1, DurableNavigationEventRangePriorRecordOwnerV1,
    DurableNavigationEventRangeSeedV1, DurableNavigationEventSourceV1,
};

#[path = "durable_projection_bootstrap.rs"]
mod durable_projection_bootstrap;
pub use durable_projection_bootstrap::{
    DurableProjectionBootstrapLimits, DurableProjectionBootstrapProgress,
};

const MAX_PROJECTION_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_DOMAIN_BYTES: usize = 1024;
const MAX_CONSUMER_ID_BYTES: usize = 256;
const MAX_PROFILE_FIELD_BYTES: usize = 128;
const MAX_DOCUMENT_ID_BYTES: usize = 256;
const MAX_SHORT_TEXT_BYTES: usize = 16 * 1024;
const MAX_QUERY_BYTES: usize = 4096;
const MAX_PROJECTION_PATH_BYTES: usize = 4096;
// Keep the projection admission ceiling aligned with the shared source-text
// QRY page contract; the legacy search remains independently ranked.
const MAX_QUERY_RESULTS: usize = 100;

/// Immutable owner-selected extractor identity. `source_contract` must match
/// the current CMD domain contract; changing any extractor behavior requires
/// a new version or contract digest and a new consumer. Version 2 consumers
/// begin at sequence zero; ordered replay or explicit initial construction
/// must establish a complete source cut before joined output is readable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableProjectionProfile {
    pub id: String,
    pub version: u32,
    pub source_contract: Digest256,
}

impl DurableProjectionProfile {
    fn validate(&self) -> DurableResult<()> {
        validate_identifier(&self.id, MAX_PROFILE_FIELD_BYTES, "projection profile ID")?;
        if self.version == 0 {
            return Err(DurableError::Invalid("projection profile version"));
        }
        Ok(())
    }
}

/// One normalized document emitted from one exact source member. Version 1
/// accepts only the direct-source dependency equal to `source_path`; version 2
/// permits one explicitly selected companion source in its dependency set.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DurableProjectionDocument {
    pub document_id: String,
    pub title: String,
    pub body: String,
    pub aliases: Vec<String>,
    pub identity: String,
    pub document_type: String,
    pub dependency_paths: Vec<String>,
}

/// One exact historical source input selected by the projection worker.
/// Bytes are read from the committed STO locator only after the event-cut
/// history row and source path have been selected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableProjectionSource {
    pub source_path: String,
    pub revision: u64,
    pub exact_source_bytes: Vec<u8>,
}

/// Pure source-byte extractor. Implementations must not read ambient files,
/// databases, rights, or current projections; all derived inputs are explicit.
/// The default methods preserve the original single-source closure contract.
pub trait DurableProjectionExtractor {
    fn extract(
        &self,
        source_path: &str,
        revision: u64,
        exact_source_bytes: &[u8],
    ) -> DurableResult<Vec<DurableProjectionDocument>>;

    /// Select bounded additional root records when a committed source changes.
    /// Returned paths are candidates only; the worker resolves them at the
    /// event cut and supplies exact bytes explicitly.
    fn affected_source_paths(&self, _changed_path: &str) -> DurableResult<Vec<String>> {
        Ok(Vec::new())
    }

    /// Select exact companion source addresses required to derive this root.
    /// Missing or non-source companions are omitted from `selected_inputs`;
    /// extractors can then preserve a stale or source-only result.
    fn required_source_paths(
        &self,
        _root_path: &str,
        _revision: u64,
        _raw: &[u8],
    ) -> DurableResult<Vec<String>> {
        Ok(Vec::new())
    }

    /// Extract using exact selected companion bytes. Legacy implementations
    /// remain valid for roots with no companion inputs and fail closed otherwise.
    fn extract_with_sources(
        &self,
        source_path: &str,
        revision: u64,
        exact_source_bytes: &[u8],
        selected_inputs: &[DurableProjectionSource],
    ) -> DurableResult<Vec<DurableProjectionDocument>> {
        if !selected_inputs.is_empty() {
            return Err(DurableError::Refused(
                "projection extractor does not accept selected source inputs",
            ));
        }
        self.extract(source_path, revision, exact_source_bytes)
    }
}

impl<F> DurableProjectionExtractor for F
where
    F: Fn(&str, u64, &[u8]) -> DurableResult<Vec<DurableProjectionDocument>>,
{
    fn extract(
        &self,
        source_path: &str,
        revision: u64,
        exact_source_bytes: &[u8],
    ) -> DurableResult<Vec<DurableProjectionDocument>> {
        self(source_path, revision, exact_source_bytes)
    }
}

/// Resource ceilings for one bounded outbox replay call.
#[derive(Clone, Copy, Debug)]
pub struct DurableProjectionLimits {
    pub max_events: usize,
    /// Independent ceiling for committed event members, additional affected
    /// roots, and unique requested dependency addresses in one event.
    pub max_event_members: usize,
    pub max_event_documents: usize,
    pub max_event_bytes: u64,
    pub max_document_bytes: usize,
    pub max_dependencies: usize,
    pub max_aliases: usize,
    pub max_query_bytes: usize,
    pub max_results: usize,
    pub statement_timeout: Duration,
}

impl Default for DurableProjectionLimits {
    fn default() -> Self {
        Self {
            max_events: 8,
            max_event_members: 64,
            max_event_documents: 256,
            max_event_bytes: 16 * 1024 * 1024,
            max_document_bytes: 1024 * 1024,
            max_dependencies: 1,
            max_aliases: 64,
            max_query_bytes: 4096,
            max_results: 50,
            statement_timeout: Duration::from_secs(15),
        }
    }
}

impl DurableProjectionLimits {
    fn validate(self) -> DurableResult<Self> {
        if self.max_events == 0
            || self.max_events > 1024
            || self.max_event_members == 0
            || self.max_event_members > 4096
            || self.max_event_documents == 0
            || self.max_event_documents > 16_384
            || self.max_event_bytes == 0
            || self.max_event_bytes > i64::MAX as u64
            || self.max_document_bytes == 0
            || self.max_document_bytes as u64 > self.max_event_bytes
            || !matches!(self.max_dependencies, 1 | 2)
            || self.max_aliases > 1024
            || self.max_query_bytes == 0
            || self.max_query_bytes > 64 * 1024
            || self.max_results == 0
            || self.max_results > MAX_QUERY_RESULTS
            || self.statement_timeout.is_zero()
            || self.statement_timeout > MAX_PROJECTION_TIMEOUT
        {
            return Err(DurableError::Invalid("invalid durable projection limits"));
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct DurableProjectionHit {
    pub source_path: String,
    pub document_id: String,
    pub source_revision: u64,
    pub title: String,
    pub aliases: Vec<String>,
    pub identity: String,
    pub document_type: String,
    pub identity_exact: bool,
    pub rank: f32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum DurableProjectionStatus {
    Current {
        through_seq: u64,
    },
    Pending {
        through_seq: u64,
        head_seq: u64,
    },
    Stale {
        through_seq: Option<u64>,
        head_seq: u64,
        reason: &'static str,
    },
}

/// Queue-age classification for the first committed event not yet applied by
/// the selected projection consumer. A NULL persisted timestamp is explicitly
/// unknown history; it is never estimated from the sequence or observed time.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DurableProjectionQueueAgeStatus {
    Empty,
    Known,
    UnknownHistorical,
    FutureClock,
}

/// Bounded owner telemetry for one selected projection consumer. Sequence lag
/// is `head_seq - effective_cursor`; an uninitialized consumer has an
/// effective cursor of zero and a `None` `through_seq`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DurableProjectionQueueAge {
    pub status: DurableProjectionStatus,
    pub through_seq: Option<u64>,
    pub head_seq: u64,
    pub sequence_lag: u64,
    pub oldest_unprocessed_seq: Option<u64>,
    /// UTC microseconds since Unix epoch from PostgreSQL's persisted
    /// `timestamptz`; absent for empty or legacy rows without a timestamp.
    pub oldest_unprocessed_enqueued_at_unix_micros: Option<i64>,
    /// Whole elapsed milliseconds from the database server clock. `None`
    /// means the queue is empty, the row is legacy/unknown, or the server clock
    /// is behind the persisted enqueue timestamp; inspect `age_status`.
    pub oldest_unprocessed_age_millis: Option<u64>,
    pub oldest_unprocessed_age_status: DurableProjectionQueueAgeStatus,
}

#[derive(Clone, Debug, Serialize)]
pub enum DurableProjectionSearch {
    Current {
        hits: Vec<DurableProjectionHit>,
        through_seq: u64,
    },
    Pending {
        through_seq: u64,
        head_seq: u64,
    },
    Stale {
        through_seq: Option<u64>,
        head_seq: u64,
        reason: &'static str,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct DurableProjectionProgress {
    pub applied_events: usize,
    pub through_seq: u64,
    pub head_seq: u64,
    pub status: DurableProjectionStatus,
    pub work: DurableProjectionWork,
}

/// Logical work performed by this call. Source bytes count payload delivered
/// from STO, not device I/O or PostgreSQL page reads. Publication counters count
/// only transactions this worker applied; replayed events can still incur reads.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct DurableProjectionWork {
    pub examined_event_members: u64,
    pub exact_source_members_read: u64,
    pub exact_source_bytes_read: u64,
    pub custody_metadata_bytes_read: u64,
    pub segment_bytes_read: u64,
    pub committed_member_rows_examined: u64,
    pub examined_related_roots: u64,
    pub examined_dependency_paths: u64,
    pub extracted_documents: u64,
    pub published_source_paths: u64,
    pub published_documents: u64,
}

impl DurableProjectionWork {
    fn add(&mut self, other: &Self) -> DurableResult<()> {
        fn checked(left: &mut u64, right: u64) -> DurableResult<()> {
            *left = left
                .checked_add(right)
                .ok_or(DurableError::Refused("projection work counter overflow"))?;
            Ok(())
        }
        checked(
            &mut self.examined_event_members,
            other.examined_event_members,
        )?;
        checked(
            &mut self.exact_source_members_read,
            other.exact_source_members_read,
        )?;
        checked(
            &mut self.exact_source_bytes_read,
            other.exact_source_bytes_read,
        )?;
        checked(
            &mut self.custody_metadata_bytes_read,
            other.custody_metadata_bytes_read,
        )?;
        checked(&mut self.segment_bytes_read, other.segment_bytes_read)?;
        checked(
            &mut self.committed_member_rows_examined,
            other.committed_member_rows_examined,
        )?;
        checked(
            &mut self.examined_related_roots,
            other.examined_related_roots,
        )?;
        checked(
            &mut self.examined_dependency_paths,
            other.examined_dependency_paths,
        )?;
        checked(&mut self.extracted_documents, other.extracted_documents)?;
        checked(
            &mut self.published_source_paths,
            other.published_source_paths,
        )?;
        checked(&mut self.published_documents, other.published_documents)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct DomainSnapshot {
    // Extraction-to-publication guard only. Reader freshness deliberately does
    // not persist this audit counter because unrelated attempts also change it.
    audit_generation: i64,
    head_seq: i64,
    rights_version: i64,
    rights_allowed: bool,
    rule_version: i64,
    contract_digest: String,
    schema_profile_digest: Option<String>,
    source_generation: Option<i64>,
    source_complete: bool,
    maintenance_state: String,
}

impl DomainSnapshot {
    fn head_seq(&self) -> DurableResult<u64> {
        as_u64(self.head_seq)
    }

    fn require_live_projection(&self, profile: &DurableProjectionProfile) -> DurableResult<()> {
        if self.maintenance_state != "normal" {
            return Err(DurableError::Refused("physical maintenance is active"));
        }
        if !self.rights_allowed {
            return Err(DurableError::Refused("current rights revoked"));
        }
        if self.schema_profile_digest != Some(super::schema_profile_digest().to_hex()) {
            return Err(DurableError::Conflict("coordinator schema profile changed"));
        }
        if self.contract_digest != profile.source_contract.to_hex() {
            return Err(DurableError::Conflict("projection source contract changed"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct ConsumerState {
    cursor_seq: i64,
    profile_id: String,
    profile_version: String,
    source_contract_digest: String,
    observed_head_seq: Option<i64>,
    observed_rights_version: Option<i64>,
    observed_rights_allowed: Option<bool>,
    observed_rule_version: Option<i64>,
    observed_contract_digest: Option<String>,
    observed_schema_profile_digest: Option<String>,
    observed_source_generation: Option<i64>,
    observed_source_complete: Option<bool>,
    source_text_search_profile: Option<String>,
    source_text_search_profile_version: Option<i32>,
}

impl ConsumerState {
    fn from_row(row: &Row) -> Self {
        Self {
            cursor_seq: row.get("cursor_seq"),
            profile_id: row.get("profile_id"),
            profile_version: row.get("profile_version"),
            source_contract_digest: row.get("source_contract_digest"),
            observed_head_seq: row.get("observed_head_seq"),
            observed_rights_version: row.get("observed_rights_version"),
            observed_rights_allowed: row.get("observed_rights_allowed"),
            observed_rule_version: row.get("observed_rule_version"),
            observed_contract_digest: row.get("observed_contract_digest"),
            observed_schema_profile_digest: row.get("observed_schema_profile_digest"),
            observed_source_generation: row.get("observed_source_generation"),
            observed_source_complete: row.get("observed_source_complete"),
            source_text_search_profile: row.get("source_text_search_profile"),
            source_text_search_profile_version: row.get("source_text_search_profile_version"),
        }
    }

    fn matches_profile(&self, consumer_id: &str, profile: &DurableProjectionProfile) -> bool {
        self.profile_id == profile.id
            && self.profile_version == profile.version.to_string()
            && self.source_contract_digest == profile.source_contract.to_hex()
            && !consumer_id.is_empty()
    }

    fn observed_state_matches(&self, snapshot: &DomainSnapshot) -> bool {
        // The global audit fence also changes for unrelated attempts. This
        // private projection's reader freshness follows committed source and
        // rights state; direct unauthorized projection-table writes are outside
        // the trusted CMD writer boundary.
        self.observed_head_seq == Some(snapshot.head_seq)
            && self.observed_rights_version == Some(snapshot.rights_version)
            && self.observed_rights_allowed == Some(snapshot.rights_allowed)
            && self.observed_rule_version == Some(snapshot.rule_version)
            && self.observed_contract_digest.as_deref() == Some(snapshot.contract_digest.as_str())
            && self.observed_schema_profile_digest == snapshot.schema_profile_digest
            && self.observed_source_generation == snapshot.source_generation
            && self.observed_source_complete == Some(snapshot.source_complete)
    }
}

#[derive(Clone, Debug)]
struct ProjectionEvent {
    commit_seq: u64,
    event_kind: String,
}

#[derive(Clone, Debug)]
struct EventMember {
    source_path: String,
    revision: u64,
    content_length: u64,
    is_source: bool,
}

#[derive(Clone, Copy, Debug)]
struct ProjectionSourceVersion {
    revision: u64,
    commit_seq: u64,
    content_length: u64,
    is_source: bool,
}

#[derive(Clone, Debug)]
struct StagedDocument {
    source_path: String,
    source_revision: u64,
    document: DurableProjectionDocument,
}

#[derive(Clone, Debug, Default)]
struct StagedEvent {
    replaced_source_paths: Vec<String>,
    documents: Vec<StagedDocument>,
    work: DurableProjectionWork,
}

#[derive(Clone, Copy, Debug)]
enum PublishOutcome {
    Applied(u64),
    AlreadyApplied(u64),
}

enum ProjectionReadGate {
    Current {
        through_seq: u64,
        rights_version: u64,
        audit_generation: u64,
        source_text_search_profile: Option<String>,
        source_text_search_profile_version: Option<i32>,
    },
    Pending {
        through_seq: u64,
        head_seq: u64,
    },
    Stale {
        through_seq: Option<u64>,
        head_seq: u64,
        reason: &'static str,
    },
}

/// The exact source/projection cut held while a disclosure packet is being
/// rendered. The values are captured by the shared projection gate; callers
/// must not construct a second currentness model from raw SQL columns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProjectionDisclosureCut {
    pub(crate) through_seq: u64,
    pub(crate) rights_version: u64,
    pub(crate) audit_generation: u64,
}

impl ProjectionReadGate {
    fn status(self) -> DurableProjectionStatus {
        match self {
            Self::Current { through_seq, .. } => DurableProjectionStatus::Current { through_seq },
            Self::Pending {
                through_seq,
                head_seq,
            } => DurableProjectionStatus::Pending {
                through_seq,
                head_seq,
            },
            Self::Stale {
                through_seq,
                head_seq,
                reason,
            } => DurableProjectionStatus::Stale {
                through_seq,
                head_seq,
                reason,
            },
        }
    }
}

impl DurablePgCoordinator {
    /// Explicitly installs the separate projection schema and event seek index.
    /// It does not alter the source schema profile digest or source rows.
    pub fn init_projection_schema(&mut self) -> DurableResult<()> {
        self.client
            .batch_execute(include_str!("durable_projection.sql"))?;
        Ok(())
    }

    /// Apply at most `limits.max_events` ordered outbox events. Each compound
    /// source event replaces its affected paths and advances the durable cursor
    /// in one PostgreSQL transaction. Errors leave that event's cursor intact.
    /// Each event must fit the explicit member and byte limits; this initial
    /// reader profile supports bounded selected source events, not a paged
    /// full-corpus bootstrap.
    pub fn apply_projection_batch<E: DurableProjectionExtractor + ?Sized>(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        consumer_id: &str,
        profile: &DurableProjectionProfile,
        extractor: &E,
        limits: DurableProjectionLimits,
    ) -> DurableResult<DurableProjectionProgress> {
        let limits = limits.validate()?;
        validate_identifier(domain, MAX_DOMAIN_BYTES, "projection domain")?;
        validate_identifier(consumer_id, MAX_CONSUMER_ID_BYTES, "projection consumer ID")?;
        profile.validate()?;
        if store.custody_domain() != domain.as_bytes() {
            return Err(DurableError::Conflict("STO custody domain differs"));
        }
        let deadline = Instant::now() + limits.statement_timeout;
        ensure_consumer(
            &mut self.client,
            domain,
            consumer_id,
            profile,
            limits.statement_timeout,
        )?;
        let (initial_cursor, initial_snapshot, events) =
            read_frontier_and_events(&mut self.client, domain, consumer_id, profile, limits)?;
        initial_snapshot.require_live_projection(profile)?;
        let mut through_seq = initial_cursor;
        let mut applied_events = 0usize;
        let mut work = DurableProjectionWork::default();
        for event in events {
            if Instant::now() >= deadline {
                return Err(DurableError::Refused("projection batch deadline exceeded"));
            }
            if event.commit_seq <= through_seq {
                continue;
            }
            let expected = through_seq
                .checked_add(1)
                .ok_or(DurableError::Corrupt("projection cursor overflow"))?;
            if event.commit_seq != expected {
                return Err(DurableError::Corrupt(
                    "projection outbox sequence has a gap",
                ));
            }
            let staged = self.stage_event(
                store,
                domain,
                consumer_id,
                &event,
                profile,
                extractor,
                limits,
                deadline,
            )?;
            work.add(&staged.work)?;
            let publication_work = DurableProjectionWork {
                published_source_paths: staged.replaced_source_paths.len() as u64,
                published_documents: staged.documents.len() as u64,
                ..DurableProjectionWork::default()
            };
            match publish_event(
                &mut self.client,
                domain,
                consumer_id,
                profile,
                &event,
                &initial_snapshot,
                staged,
                limits,
            )? {
                PublishOutcome::Applied(seq) => {
                    through_seq = seq;
                    applied_events += 1;
                    work.add(&publication_work)?;
                }
                PublishOutcome::AlreadyApplied(seq) => {
                    through_seq = seq;
                }
            }
        }
        let status =
            self.projection_status(domain, consumer_id, profile, limits.statement_timeout)?;
        let (through_seq, head_seq) = match &status {
            DurableProjectionStatus::Current { through_seq } => (*through_seq, *through_seq),
            DurableProjectionStatus::Pending {
                through_seq,
                head_seq,
            }
            | DurableProjectionStatus::Stale {
                through_seq: Some(through_seq),
                head_seq,
                ..
            } => (*through_seq, *head_seq),
            DurableProjectionStatus::Stale {
                through_seq: None,
                head_seq,
                ..
            } => (through_seq, *head_seq),
        };
        Ok(DurableProjectionProgress {
            applied_events,
            through_seq,
            head_seq,
            status,
            work,
        })
    }

    /// Check whether the explicitly selected profile is current without a
    /// corpus scan. Local CMD rights are held through this check.
    pub fn projection_status(
        &mut self,
        domain: &str,
        consumer_id: &str,
        selected_profile: &DurableProjectionProfile,
        timeout: Duration,
    ) -> DurableResult<DurableProjectionStatus> {
        selected_profile.validate()?;
        validate_identifier(domain, MAX_DOMAIN_BYTES, "projection domain")?;
        validate_identifier(consumer_id, MAX_CONSUMER_ID_BYTES, "projection consumer ID")?;
        validate_timeout(timeout)?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        set_local_timeouts(&mut tx, timeout)?;
        let status = read_projection_gate(&mut tx, domain, consumer_id, selected_profile)?.status();
        tx.commit()?;
        Ok(status)
    }

    /// Read the oldest unprocessed outbox sequence and its server-persisted
    /// age for one selected consumer. The same current-rights, maintenance,
    /// schema, and profile gate as projection reads is held in this bounded
    /// read transaction. At most one outbox row is examined.
    pub fn projection_queue_age(
        &mut self,
        domain: &str,
        consumer_id: &str,
        selected_profile: &DurableProjectionProfile,
        timeout: Duration,
    ) -> DurableResult<DurableProjectionQueueAge> {
        selected_profile.validate()?;
        validate_identifier(domain, MAX_DOMAIN_BYTES, "projection domain")?;
        validate_identifier(consumer_id, MAX_CONSUMER_ID_BYTES, "projection consumer ID")?;
        validate_timeout(timeout)?;

        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()?;
        set_local_timeouts(&mut tx, timeout)?;
        let gate = read_projection_gate(&mut tx, domain, consumer_id, selected_profile)?;
        let (through_seq, head_seq, status) = match gate {
            ProjectionReadGate::Current { through_seq, .. } => (
                Some(through_seq),
                through_seq,
                DurableProjectionStatus::Current { through_seq },
            ),
            ProjectionReadGate::Pending {
                through_seq,
                head_seq,
            } => (
                Some(through_seq),
                head_seq,
                DurableProjectionStatus::Pending {
                    through_seq,
                    head_seq,
                },
            ),
            ProjectionReadGate::Stale {
                through_seq,
                head_seq,
                reason,
            } => (
                through_seq,
                head_seq,
                DurableProjectionStatus::Stale {
                    through_seq,
                    head_seq,
                    reason,
                },
            ),
        };
        let effective_cursor = through_seq.unwrap_or(0);
        let sequence_lag = head_seq
            .checked_sub(effective_cursor)
            .ok_or(DurableError::Corrupt(
                "projection cursor exceeds source head",
            ))?;
        let row = tx.query_opt(
            "WITH server_clock AS MATERIALIZED (SELECT clock_timestamp() AS now)
             SELECT o.commit_seq,
                    CASE WHEN o.enqueued_at IS NULL THEN NULL
                         ELSE floor(extract(epoch FROM o.enqueued_at) * 1000000)::bigint
                    END AS enqueued_at_unix_micros,
                    CASE WHEN o.enqueued_at IS NULL THEN NULL
                         ELSE floor(extract(epoch FROM (server_clock.now - o.enqueued_at)) * 1000)::bigint
                    END AS age_millis
             FROM cmd2_outbox o CROSS JOIN server_clock
             WHERE o.domain=$1 AND o.commit_seq>$2 AND o.commit_seq<=$3
             ORDER BY o.commit_seq LIMIT 1",
            &[&domain, &as_i64(effective_cursor)?, &as_i64(head_seq)?],
        )?;
        let (oldest_unprocessed_seq, enqueued_at_unix_micros, age_millis, age_status) =
            if let Some(row) = row {
                let seq: i64 = row.get("commit_seq");
                let seq = as_u64(seq)?;
                let expected_seq = effective_cursor
                    .checked_add(1)
                    .ok_or(DurableError::Corrupt("projection event sequence overflow"))?;
                if seq != expected_seq {
                    return Err(DurableError::Corrupt(
                        "projection outbox sequence has a gap",
                    ));
                }
                let enqueued_at_unix_micros: Option<i64> = row.get("enqueued_at_unix_micros");
                let age_millis: Option<i64> = row.get("age_millis");
                let (age_millis, age_status) = match (enqueued_at_unix_micros, age_millis) {
                    (None, None) => (None, DurableProjectionQueueAgeStatus::UnknownHistorical),
                    (Some(_), Some(age)) if age >= 0 => (
                        Some(u64::try_from(age).map_err(|_| {
                            DurableError::Corrupt("projection queue age is invalid")
                        })?),
                        DurableProjectionQueueAgeStatus::Known,
                    ),
                    (Some(_), Some(_)) => (None, DurableProjectionQueueAgeStatus::FutureClock),
                    _ => return Err(DurableError::Corrupt("projection queue timestamp differs")),
                };
                (Some(seq), enqueued_at_unix_micros, age_millis, age_status)
            } else {
                if sequence_lag != 0 {
                    return Err(DurableError::Corrupt(
                        "projection outbox is missing the next event",
                    ));
                }
                (None, None, None, DurableProjectionQueueAgeStatus::Empty)
            };
        tx.commit()?;
        Ok(DurableProjectionQueueAge {
            status,
            through_seq,
            head_seq,
            sequence_lag,
            oldest_unprocessed_seq,
            oldest_unprocessed_enqueued_at_unix_micros: enqueued_at_unix_micros,
            oldest_unprocessed_age_millis: age_millis,
            oldest_unprocessed_age_status: age_status,
        })
    }

    /// Search only an explicitly selected, current projection version. Exact
    /// identity equality sorts ahead of full-text rank; title and aliases have
    /// higher weights than body, followed by deterministic byte-order ties.
    pub fn search_projection(
        &mut self,
        domain: &str,
        consumer_id: &str,
        selected_profile: &DurableProjectionProfile,
        query: &str,
        limit: usize,
        timeout: Duration,
    ) -> DurableResult<DurableProjectionSearch> {
        self.search_projection_with_limits(
            domain,
            consumer_id,
            selected_profile,
            query,
            limit,
            DurableProjectionLimits {
                statement_timeout: timeout,
                ..DurableProjectionLimits::default()
            },
        )
    }

    /// Search with explicit query, result, and database timeout ceilings.
    pub fn search_projection_with_limits(
        &mut self,
        domain: &str,
        consumer_id: &str,
        selected_profile: &DurableProjectionProfile,
        query: &str,
        limit: usize,
        limits: DurableProjectionLimits,
    ) -> DurableResult<DurableProjectionSearch> {
        let limits = limits.validate()?;
        selected_profile.validate()?;
        validate_identifier(domain, MAX_DOMAIN_BYTES, "projection domain")?;
        validate_identifier(consumer_id, MAX_CONSUMER_ID_BYTES, "projection consumer ID")?;
        validate_query(query, selected_profile, limit, limits)?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        set_local_timeouts(&mut tx, limits.statement_timeout)?;
        let through_seq =
            match read_projection_gate(&mut tx, domain, consumer_id, selected_profile)? {
                ProjectionReadGate::Current { through_seq, .. } => through_seq,
                ProjectionReadGate::Pending {
                    through_seq,
                    head_seq,
                } => {
                    tx.commit()?;
                    return Ok(DurableProjectionSearch::Pending {
                        through_seq,
                        head_seq,
                    });
                }
                ProjectionReadGate::Stale {
                    through_seq,
                    head_seq,
                    reason,
                } => {
                    tx.commit()?;
                    return Ok(DurableProjectionSearch::Stale {
                        through_seq,
                        head_seq,
                        reason,
                    });
                }
            };
        let rows = tx.query(
            "SELECT source_path,document_id,source_revision,title,aliases,identity,
                    document_type,(identity COLLATE \"C\"=$3) AS identity_exact,
                    pg_catalog.ts_rank_cd(
                      search_vector,
                      pg_catalog.plainto_tsquery('pg_catalog.simple'::pg_catalog.regconfig,$3),
                      32
                    ) AS rank
             FROM cmd2_projection_document
             WHERE domain=$1 AND consumer_id=$2
               AND (identity COLLATE \"C\"=$3 OR search_vector @@
                    pg_catalog.plainto_tsquery('pg_catalog.simple'::pg_catalog.regconfig,$3))
             ORDER BY (identity COLLATE \"C\"=$3) DESC,
                      pg_catalog.ts_rank_cd(
                        search_vector,
                        pg_catalog.plainto_tsquery('pg_catalog.simple'::pg_catalog.regconfig,$3),
                        32
                      ) DESC,
                      identity COLLATE \"C\",source_path COLLATE \"C\",document_id COLLATE \"C\"
             LIMIT $4",
            &[
                &domain,
                &consumer_id,
                &query,
                &i64::try_from(limit)
                    .map_err(|_| DurableError::Invalid("projection result limit"))?,
            ],
        )?;
        let hits = rows
            .into_iter()
            .map(|row| {
                Ok(DurableProjectionHit {
                    source_path: row.get("source_path"),
                    document_id: row.get("document_id"),
                    source_revision: as_u64(row.get("source_revision"))?,
                    title: row.get("title"),
                    aliases: row.get("aliases"),
                    identity: row.get("identity"),
                    document_type: row.get("document_type"),
                    identity_exact: row.get("identity_exact"),
                    rank: row.get("rank"),
                })
            })
            .collect::<DurableResult<Vec<_>>>()?;
        tx.commit()?;
        Ok(DurableProjectionSearch::Current { hits, through_seq })
    }

    fn stage_event<E: DurableProjectionExtractor + ?Sized>(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        consumer_id: &str,
        event: &ProjectionEvent,
        profile: &DurableProjectionProfile,
        extractor: &E,
        limits: DurableProjectionLimits,
        deadline: Instant,
    ) -> DurableResult<StagedEvent> {
        let mut staged = StagedEvent::default();
        let members = event_members(&mut self.client, domain, event, limits)?;
        staged.work.examined_event_members = members.len() as u64;
        if event.event_kind != "command" {
            if !members.is_empty() {
                return Err(DurableError::Corrupt(
                    "non-command projection event has source history",
                ));
            }
            return Ok(staged);
        }

        let mut event_bytes = 0u64;
        // Cardinality ceilings are applied to each selected category below;
        // this union only ensures each unique address is byte-charged once.
        let mut path_budget = BTreeSet::new();
        let mut payload_budget = BTreeSet::new();
        let mut additional_root_budget = BTreeSet::new();
        let mut dependency_path_budget = BTreeSet::new();
        let mut changed_by_path = BTreeMap::new();
        let mut source_versions = BTreeMap::new();
        let mut replaced_paths = BTreeSet::new();
        for member in members {
            add_projection_bytes(
                &mut event_bytes,
                member
                    .content_length
                    .checked_add(member.source_path.len() as u64)
                    .ok_or(DurableError::Refused("projection event byte overflow"))?,
                limits,
            )?;
            path_budget.insert(member.source_path.clone());
            payload_budget.insert(member.source_path.clone());
            replaced_paths.insert(member.source_path.clone());
            source_versions.insert(
                member.source_path.clone(),
                Some(ProjectionSourceVersion {
                    revision: member.revision,
                    commit_seq: event.commit_seq,
                    content_length: member.content_length,
                    is_source: member.is_source,
                }),
            );
            changed_by_path.insert(member.source_path.clone(), member);
        }

        // Version 1 preserves its original single-member behavior exactly.
        // Version 2 adds roots selected by the extractor and by the durable
        // reverse dependency index, all bounded before any source payload read.
        let mut root_paths = BTreeSet::new();
        let mut related_roots = BTreeSet::new();
        for member in changed_by_path.values() {
            if member.is_source {
                root_paths.insert(member.source_path.clone());
            }
            if profile.version == 2 {
                validate_relative_path(&member.source_path)?;
                let extractor_roots = extractor.affected_source_paths(&member.source_path)?;
                if extractor_roots.len() > limits.max_event_members {
                    return Err(DurableError::Refused(
                        "projection extractor selected too many related roots",
                    ));
                }
                for path in extractor_roots {
                    validate_relative_path(&path)?;
                    if path != member.source_path {
                        related_roots.insert(path.clone());
                        if !changed_by_path.contains_key(&path) {
                            add_projection_budget_path(
                                &mut additional_root_budget,
                                &mut path_budget,
                                &path,
                                &mut event_bytes,
                                limits,
                                "projection event exceeds related path budget",
                            )?;
                        }
                        root_paths.insert(path);
                    }
                }
                for path in projection_reverse_dependents(
                    &mut self.client,
                    domain,
                    consumer_id,
                    &member.source_path,
                    limits,
                )? {
                    if path != member.source_path {
                        related_roots.insert(path.clone());
                        if !changed_by_path.contains_key(&path) {
                            add_projection_budget_path(
                                &mut additional_root_budget,
                                &mut path_budget,
                                &path,
                                &mut event_bytes,
                                limits,
                                "projection event exceeds related path budget",
                            )?;
                        }
                        root_paths.insert(path);
                    }
                }
            }
        }
        staged.work.examined_related_roots = u64::try_from(related_roots.len())
            .map_err(|_| DurableError::Refused("projection related-root count overflow"))?;
        for path in &root_paths {
            replaced_paths.insert(path.clone());
            if path_budget.insert(path.clone()) {
                add_projection_bytes(&mut event_bytes, path.len() as u64, limits)?;
            }
        }
        staged.replaced_source_paths = replaced_paths.into_iter().collect();

        if profile.version != 2 {
            for member in changed_by_path.values().filter(|member| member.is_source) {
                if Instant::now() >= deadline {
                    return Err(DurableError::Refused("projection event deadline exceeded"));
                }
                validate_relative_path(&member.source_path)?;
                let source = read_projection_source(
                    self,
                    store,
                    domain,
                    &member.source_path,
                    member.revision,
                    member.content_length,
                    event.commit_seq,
                    limits,
                    false,
                    deadline,
                    &mut event_bytes,
                    &mut staged.work,
                )?;
                let documents = extractor.extract(
                    &source.source_path,
                    source.revision,
                    &source.exact_source_bytes,
                )?;
                validate_extracted_documents(
                    &mut staged,
                    &source.source_path,
                    source.revision,
                    documents,
                    profile.version,
                    &BTreeSet::from([source.source_path.clone()]),
                    limits,
                    &mut event_bytes,
                )?;
            }
            return Ok(staged);
        }

        let mut loaded_sources = BTreeMap::new();
        for root_path in root_paths {
            if Instant::now() >= deadline {
                return Err(DurableError::Refused("projection event deadline exceeded"));
            }
            let root_version = projection_source_version_at_event(
                &mut self.client,
                domain,
                &root_path,
                event.commit_seq,
                limits,
                &mut source_versions,
            )?;
            let Some(root_version) = root_version.filter(|version| version.is_source) else {
                continue;
            };
            let root_source = match loaded_sources.get(&root_path).cloned() {
                Some(source) => source,
                None => {
                    if payload_budget.insert(root_path.clone()) {
                        add_projection_bytes(
                            &mut event_bytes,
                            root_version.content_length,
                            limits,
                        )?;
                    }
                    let source = read_projection_source(
                        self,
                        store,
                        domain,
                        &root_path,
                        root_version.revision,
                        root_version.content_length,
                        root_version.commit_seq,
                        limits,
                        true,
                        deadline,
                        &mut event_bytes,
                        &mut staged.work,
                    )?;
                    loaded_sources.insert(root_path.clone(), source.clone());
                    source
                }
            };

            let requested = extractor.required_source_paths(
                &root_path,
                root_source.revision,
                &root_source.exact_source_bytes,
            )?;
            if requested.len() > limits.max_event_members {
                return Err(DurableError::Refused(
                    "projection extractor selected too many dependency paths",
                ));
            }
            if profile.version != 2 && !requested.is_empty() {
                return Err(DurableError::Refused(
                    "version 1 projection cannot select companion sources",
                ));
            }
            let mut required_paths = Vec::new();
            let mut unique_required = BTreeSet::new();
            for path in requested {
                validate_relative_path(&path)?;
                if path == root_path || !unique_required.insert(path.clone()) {
                    continue;
                }
                add_projection_budget_path(
                    &mut dependency_path_budget,
                    &mut path_budget,
                    &path,
                    &mut event_bytes,
                    limits,
                    "projection event exceeds dependency path budget",
                )?;
                required_paths.push(path);
            }
            staged.work.examined_dependency_paths = staged
                .work
                .examined_dependency_paths
                .checked_add(required_paths.len() as u64)
                .ok_or(DurableError::Refused(
                    "projection dependency path count overflow",
                ))?;

            // Resolve every requested address at the event cut before reading
            // any companion bytes. An absent or non-source latest row stays
            // absent, allowing an extractor to remove stale joined output.
            let mut required_versions = Vec::with_capacity(required_paths.len());
            for path in &required_paths {
                let version = projection_source_version_at_event(
                    &mut self.client,
                    domain,
                    path,
                    event.commit_seq,
                    limits,
                    &mut source_versions,
                )?;
                required_versions.push((path.clone(), version));
            }

            let mut selected_inputs = Vec::new();
            for (path, version) in required_versions {
                let Some(version) = version.filter(|version| version.is_source) else {
                    continue;
                };
                let input = match loaded_sources.get(&path).cloned() {
                    Some(source) => source,
                    None => {
                        if payload_budget.insert(path.clone()) {
                            add_projection_bytes(&mut event_bytes, version.content_length, limits)?;
                        }
                        let source = read_projection_source(
                            self,
                            store,
                            domain,
                            &path,
                            version.revision,
                            version.content_length,
                            version.commit_seq,
                            limits,
                            true,
                            deadline,
                            &mut event_bytes,
                            &mut staged.work,
                        )?;
                        loaded_sources.insert(path.clone(), source.clone());
                        source
                    }
                };
                selected_inputs.push(input);
            }
            let mut selected_paths = BTreeSet::from([root_path.clone()]);
            selected_paths.extend(
                selected_inputs
                    .iter()
                    .map(|input| input.source_path.clone()),
            );
            let documents = extractor.extract_with_sources(
                &root_path,
                root_source.revision,
                &root_source.exact_source_bytes,
                &selected_inputs,
            )?;
            validate_extracted_documents(
                &mut staged,
                &root_path,
                root_source.revision,
                documents,
                profile.version,
                &selected_paths,
                limits,
                &mut event_bytes,
            )?;
        }
        Ok(staged)
    }
}

fn projection_source_version_at_event(
    client: &mut postgres::Client,
    domain: &str,
    source_path: &str,
    event_seq: u64,
    limits: DurableProjectionLimits,
    cache: &mut BTreeMap<String, Option<ProjectionSourceVersion>>,
) -> DurableResult<Option<ProjectionSourceVersion>> {
    if let Some(version) = cache.get(source_path) {
        return Ok(*version);
    }
    let mut tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()?;
    set_local_timeouts(&mut tx, limits.statement_timeout)?;
    let row = tx.query_opt(
        "SELECT h.revision,h.commit_seq,h.content_length,h.source_mode
         FROM cmd2_history h
         WHERE h.domain=$1 AND h.subject=$2 AND h.commit_seq<=$3
         ORDER BY h.commit_seq DESC LIMIT 1",
        &[&domain, &source_path, &as_i64(event_seq)?],
    )?;
    let version = row
        .map(|row| -> DurableResult<ProjectionSourceVersion> {
            Ok(ProjectionSourceVersion {
                revision: as_u64(row.get("revision"))?,
                commit_seq: as_u64(row.get("commit_seq"))?,
                content_length: as_u64(row.get("content_length"))?,
                is_source: row.get::<_, Option<i32>>("source_mode").is_some(),
            })
        })
        .transpose()?;
    tx.commit()?;
    cache.insert(source_path.to_owned(), version);
    Ok(version)
}

fn projection_reverse_dependents(
    client: &mut postgres::Client,
    domain: &str,
    consumer_id: &str,
    changed_path: &str,
    limits: DurableProjectionLimits,
) -> DurableResult<Vec<String>> {
    let max_rows = limits
        .max_event_members
        .checked_add(1)
        .and_then(|count| i64::try_from(count).ok())
        .ok_or(DurableError::Invalid("projection related-root limit"))?;
    let mut tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()?;
    set_local_timeouts(&mut tx, limits.statement_timeout)?;
    let rows = tx.query(
        "SELECT DISTINCT (left(source_path,4097) COLLATE \"C\") AS source_path,
                octet_length(source_path) AS path_bytes
         FROM cmd2_projection_dependency
         WHERE domain=$1 AND consumer_id=$2 AND dependency_kind='source'
           AND dependency_path COLLATE \"C\" = $3 COLLATE \"C\"
         ORDER BY 1 LIMIT $4",
        &[&domain, &consumer_id, &changed_path, &max_rows],
    )?;
    if rows.len() > limits.max_event_members {
        return Err(DurableError::Refused(
            "projection reverse dependency set exceeds path budget",
        ));
    }
    let mut paths = Vec::with_capacity(rows.len());
    for row in rows {
        let path: String = row.get("source_path");
        let path_bytes: i32 = row.get("path_bytes");
        if path_bytes < 0
            || path_bytes as usize > MAX_PROJECTION_PATH_BYTES
            || path_bytes as usize != path.len()
        {
            return Err(DurableError::Refused(
                "projection dependent source path exceeds byte budget",
            ));
        }
        validate_relative_path(&path)?;
        paths.push(path);
    }
    tx.commit()?;
    Ok(paths)
}

fn read_projection_source(
    coordinator: &mut DurablePgCoordinator,
    store: &SegmentStore,
    domain: &str,
    source_path: &str,
    revision: u64,
    content_length: u64,
    commit_seq: u64,
    limits: DurableProjectionLimits,
    count_physical_bytes_in_event_budget: bool,
    deadline: Instant,
    event_bytes: &mut u64,
    work: &mut DurableProjectionWork,
) -> DurableResult<DurableProjectionSource> {
    if Instant::now() >= deadline {
        return Err(DurableError::Refused("projection event deadline exceeded"));
    }
    let read_limit =
        if count_physical_bytes_in_event_budget {
            let prior_read_bytes = work
                .exact_source_bytes_read
                .checked_add(work.custody_metadata_bytes_read)
                .and_then(|bytes| bytes.checked_add(work.segment_bytes_read))
                .ok_or(DurableError::Refused(
                    "projection source read byte overflow",
                ))?;
            let remaining_read_bytes = limits.max_event_bytes.checked_sub(prior_read_bytes).ok_or(
                DurableError::Refused("projection event exceeds read byte budget"),
            )?;
            if content_length > remaining_read_bytes {
                return Err(DurableError::Refused(
                    "projection event exceeds selected source read budget",
                ));
            }
            remaining_read_bytes.max(1)
        } else {
            limits.max_event_bytes
        };
    let (history_row, exact_source_bytes, _, source_work) = coordinator
        .read_committed_source_selected(
            store,
            domain,
            source_path,
            Some(revision),
            read_limit,
            deadline,
            &std::sync::atomic::AtomicBool::new(false),
            |_| Ok(()),
        )?;
    if Instant::now() >= deadline {
        return Err(DurableError::Refused("projection event deadline exceeded"));
    }
    if exact_source_bytes.len() as u64 != content_length {
        return Err(DurableError::Corrupt(
            "projection source byte length differs",
        ));
    }
    if history_row.get::<_, String>("subject") != source_path
        || as_u64(history_row.get("revision"))? != revision
        || as_u64(history_row.get("commit_seq"))? != commit_seq
    {
        return Err(DurableError::Conflict(
            "projection source history is outside selected event cut",
        ));
    }
    add_projection_work_counter(
        &mut work.exact_source_members_read,
        1,
        "projection source read count overflow",
    )?;
    add_projection_work_counter(
        &mut work.exact_source_bytes_read,
        exact_source_bytes.len() as u64,
        "projection source byte count overflow",
    )?;
    add_projection_work_counter(
        &mut work.custody_metadata_bytes_read,
        source_work.custody_metadata_bytes_read,
        "projection custody metadata byte overflow",
    )?;
    add_projection_work_counter(
        &mut work.segment_bytes_read,
        source_work.segment_bytes_read,
        "projection segment read byte overflow",
    )?;
    add_projection_work_counter(
        &mut work.committed_member_rows_examined,
        source_work.committed_member_rows_examined,
        "projection committed member count overflow",
    )?;
    let actual_read_bytes = work
        .exact_source_bytes_read
        .checked_add(work.custody_metadata_bytes_read)
        .and_then(|bytes| bytes.checked_add(work.segment_bytes_read))
        .ok_or(DurableError::Refused(
            "projection source read byte overflow",
        ))?;
    if count_physical_bytes_in_event_budget && actual_read_bytes > limits.max_event_bytes {
        return Err(DurableError::Refused(
            "projection event exceeds selected source read budget",
        ));
    }
    let extra_bytes = source_work
        .custody_metadata_bytes_read
        .checked_add(source_work.segment_bytes_read)
        .ok_or(DurableError::Refused("projection read byte overflow"))?;
    if count_physical_bytes_in_event_budget {
        add_projection_bytes(event_bytes, extra_bytes, limits)?;
    }
    Ok(DurableProjectionSource {
        source_path: source_path.to_owned(),
        revision,
        exact_source_bytes,
    })
}

fn add_projection_bytes(
    total: &mut u64,
    addition: u64,
    limits: DurableProjectionLimits,
) -> DurableResult<()> {
    *total = total
        .checked_add(addition)
        .ok_or(DurableError::Refused("projection event byte overflow"))?;
    if *total > limits.max_event_bytes {
        return Err(DurableError::Refused(
            "projection event exceeds byte budget",
        ));
    }
    Ok(())
}

fn add_projection_budget_path(
    category_paths: &mut BTreeSet<String>,
    event_paths: &mut BTreeSet<String>,
    path: &str,
    event_bytes: &mut u64,
    limits: DurableProjectionLimits,
    exceeded_error: &'static str,
) -> DurableResult<()> {
    if category_paths.contains(path) {
        return Ok(());
    }
    if category_paths.len() >= limits.max_event_members {
        return Err(DurableError::Refused(exceeded_error));
    }
    category_paths.insert(path.to_owned());
    if event_paths.insert(path.to_owned()) {
        add_projection_bytes(event_bytes, path.len() as u64, limits)?;
    }
    Ok(())
}

fn add_projection_work_counter(
    counter: &mut u64,
    addition: u64,
    error: &'static str,
) -> DurableResult<()> {
    *counter = counter
        .checked_add(addition)
        .ok_or(DurableError::Refused(error))?;
    Ok(())
}

fn validate_extracted_documents(
    staged: &mut StagedEvent,
    source_path: &str,
    revision: u64,
    documents: Vec<DurableProjectionDocument>,
    profile_version: u32,
    selected_paths: &BTreeSet<String>,
    limits: DurableProjectionLimits,
    event_bytes: &mut u64,
) -> DurableResult<()> {
    let remaining_documents = limits
        .max_event_documents
        .saturating_sub(staged.documents.len());
    if documents.len() > remaining_documents {
        return Err(DurableError::Refused(
            "projection event has too many documents",
        ));
    }
    add_projection_work_counter(
        &mut staged.work.extracted_documents,
        documents.len() as u64,
        "projection extracted document count overflow",
    )?;
    for document in documents {
        validate_document(
            source_path,
            selected_paths,
            profile_version,
            document,
            staged,
            revision,
            limits,
            event_bytes,
        )?;
    }
    Ok(())
}

fn ensure_consumer(
    client: &mut postgres::Client,
    domain: &str,
    consumer_id: &str,
    profile: &DurableProjectionProfile,
    timeout: Duration,
) -> DurableResult<()> {
    let mut tx = client.transaction()?;
    set_local_timeouts(&mut tx, timeout)?;
    let profile_digest = profile.source_contract.to_hex();
    tx.execute(
        "INSERT INTO cmd2_projection_consumer(domain,consumer_id,profile_id,profile_version,
            source_contract_digest,source_text_search_profile,
            source_text_search_profile_version,cursor_seq)
         VALUES($1,$2,$3,$4,$5,$6,$7,0) ON CONFLICT(domain,consumer_id) DO NOTHING",
        &[
            &domain,
            &consumer_id,
            &profile.id,
            &profile.version.to_string(),
            &profile_digest,
            &durable_source_text_search::SOURCE_TEXT_SEARCH_PROFILE,
            &durable_source_text_search::SOURCE_TEXT_SEARCH_PROFILE_VERSION,
        ],
    )?;
    let row = tx.query_one(
        "SELECT profile_id,profile_version,source_contract_digest
         FROM cmd2_projection_consumer WHERE domain=$1 AND consumer_id=$2 FOR SHARE",
        &[&domain, &consumer_id],
    )?;
    if row.get::<_, String>(0) != profile.id
        || row.get::<_, String>(1) != profile.version.to_string()
        || row.get::<_, String>(2) != profile_digest
    {
        return Err(DurableError::Conflict(
            "projection consumer is bound to another profile",
        ));
    }
    tx.commit()?;
    Ok(())
}

fn read_projection_gate<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer_id: &str,
    selected_profile: &DurableProjectionProfile,
) -> DurableResult<ProjectionReadGate> {
    let snapshot = lock_domain_snapshot(tx, domain)?;
    if !snapshot.rights_allowed {
        return Err(DurableError::Refused("current rights revoked"));
    }
    if snapshot.maintenance_state != "normal" {
        return Err(DurableError::Refused("physical maintenance is active"));
    }
    let head_seq = snapshot.head_seq()?;
    let row = tx.query_opt(
        "SELECT * FROM cmd2_projection_consumer
         WHERE domain=$1 AND consumer_id=$2 FOR SHARE",
        &[&domain, &consumer_id],
    )?;
    let Some(row) = row else {
        return Ok(ProjectionReadGate::Stale {
            through_seq: None,
            head_seq,
            reason: "projection consumer is not initialized",
        });
    };
    let consumer = ConsumerState::from_row(&row);
    let through_seq = as_u64(consumer.cursor_seq)?;
    if through_seq > head_seq {
        return Err(DurableError::Corrupt(
            "projection cursor exceeds source head",
        ));
    }
    if !consumer.matches_profile(consumer_id, selected_profile)
        || selected_profile.source_contract.to_hex() != snapshot.contract_digest
        || snapshot.schema_profile_digest != Some(super::schema_profile_digest().to_hex())
    {
        return Ok(ProjectionReadGate::Stale {
            through_seq: Some(through_seq),
            head_seq,
            reason: "selected projection profile or source contract differs",
        });
    }
    if through_seq < head_seq {
        return Ok(ProjectionReadGate::Pending {
            through_seq,
            head_seq,
        });
    }
    if through_seq == 0 && head_seq == 0 && consumer.observed_head_seq.is_none() {
        return Ok(ProjectionReadGate::Current {
            through_seq,
            rights_version: as_u64(snapshot.rights_version)?,
            audit_generation: as_u64(snapshot.audit_generation)?,
            source_text_search_profile: consumer.source_text_search_profile,
            source_text_search_profile_version: consumer.source_text_search_profile_version,
        });
    }
    if !consumer.observed_state_matches(&snapshot) {
        return Ok(ProjectionReadGate::Stale {
            through_seq: Some(through_seq),
            head_seq,
            reason: "projection currentness fence differs",
        });
    }
    Ok(ProjectionReadGate::Current {
        through_seq,
        rights_version: as_u64(snapshot.rights_version)?,
        audit_generation: as_u64(snapshot.audit_generation)?,
        source_text_search_profile: consumer.source_text_search_profile,
        source_text_search_profile_version: consumer.source_text_search_profile_version,
    })
}

/// Recheck the shared currentness/rights/profile gate while the caller's
/// transaction remains open. This is the one owner path for disclosure
/// rechecks; it deliberately reuses `read_projection_gate` so rule, schema,
/// source-generation and observed-state fences cannot drift from projection
/// reads.
pub(crate) fn recheck_source_text_disclosure<C: GenericClient>(
    client: &mut C,
    domain: &str,
    consumer_id: &str,
    selected_profile: &DurableProjectionProfile,
    expected: ProjectionDisclosureCut,
) -> DurableResult<()> {
    let gate = read_projection_gate(client, domain, consumer_id, selected_profile)?;
    let ProjectionReadGate::Current {
        through_seq,
        rights_version,
        audit_generation,
        source_text_search_profile,
        source_text_search_profile_version,
    } = gate
    else {
        return Err(DurableError::Conflict(
            "source-text disclosure binding changed",
        ));
    };
    if through_seq != expected.through_seq
        || rights_version != expected.rights_version
        || audit_generation != expected.audit_generation
        || source_text_search_profile.as_deref()
            != Some(durable_source_text_search::SOURCE_TEXT_SEARCH_PROFILE)
        || source_text_search_profile_version
            != Some(durable_source_text_search::SOURCE_TEXT_SEARCH_PROFILE_VERSION)
    {
        return Err(DurableError::Conflict(
            "source-text disclosure binding changed",
        ));
    }
    Ok(())
}

fn read_frontier_and_events(
    client: &mut postgres::Client,
    domain: &str,
    consumer_id: &str,
    profile: &DurableProjectionProfile,
    limits: DurableProjectionLimits,
) -> DurableResult<(u64, DomainSnapshot, Vec<ProjectionEvent>)> {
    let mut tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()?;
    set_local_timeouts(&mut tx, limits.statement_timeout)?;
    let snapshot = read_domain_snapshot(&mut tx, domain)?;
    snapshot.require_live_projection(profile)?;
    let row = tx
        .query_opt(
            "SELECT * FROM cmd2_projection_consumer WHERE domain=$1 AND consumer_id=$2",
            &[&domain, &consumer_id],
        )?
        .ok_or(DurableError::Corrupt("projection consumer disappeared"))?;
    let consumer = ConsumerState::from_row(&row);
    if !consumer.matches_profile(consumer_id, profile) {
        return Err(DurableError::Conflict(
            "projection consumer profile differs",
        ));
    }
    durable_projection_bootstrap::require_no_active_bootstrap(&mut tx, domain, consumer_id)?;
    let cursor = as_u64(consumer.cursor_seq)?;
    let head = snapshot.head_seq()?;
    if cursor > head {
        return Err(DurableError::Corrupt(
            "projection cursor exceeds source head",
        ));
    }
    let rows = tx.query(
        "SELECT o.commit_seq,l.event_kind
         FROM cmd2_outbox o LEFT JOIN cmd2_log l USING(domain,commit_seq)
         WHERE o.domain=$1 AND o.commit_seq>$2 AND o.commit_seq<=$3
         ORDER BY o.commit_seq LIMIT $4",
        &[
            &domain,
            &as_i64(cursor)?,
            &as_i64(head)?,
            &i64::try_from(limits.max_events)
                .map_err(|_| DurableError::Invalid("projection event limit"))?,
        ],
    )?;
    let mut events = Vec::with_capacity(rows.len());
    let mut expected = cursor;
    for row in rows {
        let seq = as_u64(row.get("commit_seq"))?;
        expected = expected
            .checked_add(1)
            .ok_or(DurableError::Corrupt("projection event sequence overflow"))?;
        if seq != expected {
            return Err(DurableError::Corrupt(
                "projection outbox sequence has a gap",
            ));
        }
        let event_kind: Option<String> = row.get("event_kind");
        let event_kind =
            event_kind.ok_or(DurableError::Corrupt("outbox event lacks command log"))?;
        if !matches!(event_kind.as_str(), "command" | "rights" | "rule") {
            return Err(DurableError::Corrupt("projection event kind is invalid"));
        }
        events.push(ProjectionEvent {
            commit_seq: seq,
            event_kind,
        });
    }
    if cursor < head && events.is_empty() {
        return Err(DurableError::Corrupt(
            "projection outbox is missing the next event",
        ));
    }
    tx.commit()?;
    Ok((cursor, snapshot, events))
}

fn event_members(
    client: &mut postgres::Client,
    domain: &str,
    event: &ProjectionEvent,
    limits: DurableProjectionLimits,
) -> DurableResult<Vec<EventMember>> {
    let mut tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()?;
    set_local_timeouts(&mut tx, limits.statement_timeout)?;
    let event_row = tx
        .query_opt(
            "SELECT l.event_kind,l.command_id,o.event_id
             FROM cmd2_log l JOIN cmd2_outbox o USING(domain,commit_seq)
             WHERE l.domain=$1 AND l.commit_seq=$2",
            &[&domain, &as_i64(event.commit_seq)?],
        )?
        .ok_or(DurableError::Corrupt(
            "projection event lacks its committed log/outbox pair",
        ))?;
    if event_row.get::<_, String>("event_kind") != event.event_kind {
        return Err(DurableError::Corrupt(
            "projection event kind differs from committed log",
        ));
    }
    let expected_members_root = if event.event_kind == "command" {
        Some(verify_committed_command_membership(
            &mut tx, domain, event, limits, &event_row,
        )?)
    } else {
        if event.event_kind == "rights"
            && event_row.get::<_, String>("event_id")
                != format!("rights.revoke:{}", event.commit_seq)
        {
            return Err(DurableError::Corrupt(
                "projection rights outbox identity differs",
            ));
        }
        None
    };
    let max_rows = limits
        .max_event_members
        .min(super::MAX_MEMBERS)
        .checked_add(1)
        .and_then(|n| i64::try_from(n).ok())
        .ok_or(DurableError::Invalid("projection member limit"))?;
    let rows = tx.query(
        "SELECT member_slot,left(subject,4097) AS subject,
                octet_length(subject) AS subject_bytes,revision AS proposed_revision,
                sto_receipt_id,content_digest,content_length,source_mode,source_dependencies
         FROM cmd2_history WHERE domain=$1 AND commit_seq=$2
         ORDER BY member_slot LIMIT $3",
        &[&domain, &as_i64(event.commit_seq)?, &max_rows],
    )?;
    if rows.len() > limits.max_event_members {
        return Err(DurableError::Refused(
            "projection event has too many source members",
        ));
    }
    if rows.len() > super::MAX_MEMBERS {
        return Err(DurableError::Corrupt(
            "projection event exceeds committed member ceiling",
        ));
    }
    if event.event_kind != "command" && !rows.is_empty() {
        return Err(DurableError::Corrupt(
            "non-command event owns source history",
        ));
    }
    for row in &rows {
        let subject: String = row.get("subject");
        let subject_bytes: i32 = row.get("subject_bytes");
        if subject_bytes < 0
            || subject_bytes as usize > MAX_PROJECTION_PATH_BYTES
            || subject.len() > MAX_PROJECTION_PATH_BYTES
        {
            return Err(DurableError::Refused(
                "projection source path exceeds byte budget",
            ));
        }
    }
    if let Some(expected_root) = expected_members_root {
        let actual_root = member_rows_root(&rows);
        if actual_root != expected_root {
            return Err(DurableError::Corrupt(
                "committed event history membership differs from receipt",
            ));
        }
    }
    let mut members = Vec::with_capacity(rows.len());
    let mut total = 0u64;
    for row in rows {
        let source_path: String = row.get("subject");
        let revision = as_u64(row.get("proposed_revision"))?;
        let content_length = as_u64(row.get("content_length"))?;
        total = total
            .checked_add(content_length)
            .ok_or(DurableError::Refused("projection source byte overflow"))?;
        total = total
            .checked_add(source_path.len() as u64)
            .ok_or(DurableError::Refused("projection source byte overflow"))?;
        if total > limits.max_event_bytes {
            return Err(DurableError::Refused(
                "projection event exceeds byte budget",
            ));
        }
        let is_source: Option<i32> = row.get("source_mode");
        members.push(EventMember {
            source_path,
            revision,
            content_length,
            is_source: is_source.is_some(),
        });
    }
    tx.commit()?;
    Ok(members)
}

/// Verify one bounded command event against its committed receipt and complete
/// prepared membership before using its history rows to replace documents.
/// Both member reads are index-bounded by the profile and the coordinator's
/// existing per-command member ceiling; no domain-wide history scan is needed.
fn verify_committed_command_membership(
    tx: &mut Transaction<'_>,
    domain: &str,
    event: &ProjectionEvent,
    limits: DurableProjectionLimits,
    event_row: &Row,
) -> DurableResult<Digest256> {
    let command_id: String = event_row.get("command_id");
    let expected_event_id = format!("{domain}:{}", event.commit_seq);
    if event_row.get::<_, String>("event_id") != expected_event_id {
        return Err(DurableError::Corrupt(
            "projection command outbox identity differs",
        ));
    }

    let attempt = tx
        .query_opt(
            "SELECT * FROM cmd2_attempt WHERE domain=$1 AND command_id=$2",
            &[&domain, &command_id],
        )?
        .ok_or(DurableError::Corrupt(
            "projection command attempt is absent",
        ))?;
    let attempt_seq: Option<i64> = attempt.get("commit_seq");
    let event_seq = as_i64(event.commit_seq)?;
    if attempt.get::<_, String>("state") != "committed" || attempt_seq != Some(event_seq) {
        return Err(DurableError::Corrupt(
            "projection command attempt differs from committed event",
        ));
    }

    let prepare_id: Vec<u8> = attempt.get("prepare_id");
    let member_limit = limits.max_event_members.min(super::MAX_MEMBERS);
    let query_limit = member_limit
        .checked_add(1)
        .and_then(|count| i64::try_from(count).ok())
        .ok_or(DurableError::Invalid("projection member limit"))?;
    // Pre-admit the parent receipt verifier's complete member read. The
    // transaction is repeatable-read, so this count cannot grow between reads.
    let member_probe = tx.query(
        "SELECT member_slot FROM cmd2_member
         WHERE domain=$1 AND prepare_id=$2 ORDER BY member_slot LIMIT $3",
        &[&domain, &prepare_id, &query_limit],
    )?;
    if member_probe.len() > super::MAX_MEMBERS {
        return Err(DurableError::Corrupt(
            "committed command exceeds member ceiling",
        ));
    }
    if member_probe.len() > limits.max_event_members {
        return Err(DurableError::Refused(
            "projection event has too many source members",
        ));
    }
    if member_probe.is_empty() {
        return Err(DurableError::Corrupt("committed command has no members"));
    }
    let receipt = super::receipt_from_committed_attempt(tx, &attempt)?;
    if receipt.domain != domain || receipt.commit_seq != event.commit_seq {
        return Err(DurableError::Corrupt(
            "projection command receipt differs from event",
        ));
    }
    Ok(receipt.member_root)
}

fn member_rows_root(rows: &[Row]) -> Digest256 {
    let mut hasher = Digest256Hasher::new();
    super::part(&mut hasher, b"cmd2-member-root-v1");
    super::part(&mut hasher, &(rows.len() as u64).to_be_bytes());
    for row in rows {
        super::update_member_root(&mut hasher, row);
    }
    hasher.finalize()
}

fn validate_document(
    source_path: &str,
    selected_paths: &BTreeSet<String>,
    profile_version: u32,
    document: DurableProjectionDocument,
    staged: &mut StagedEvent,
    revision: u64,
    limits: DurableProjectionLimits,
    total_bytes: &mut u64,
) -> DurableResult<()> {
    if staged.documents.len() >= limits.max_event_documents {
        return Err(DurableError::Refused(
            "projection event has too many documents",
        ));
    }
    validate_identifier(
        &document.document_id,
        MAX_DOCUMENT_ID_BYTES,
        "projection document ID",
    )?;
    validate_identifier(
        &document.identity,
        MAX_SHORT_TEXT_BYTES,
        "projection document identity",
    )?;
    validate_identifier(
        &document.document_type,
        MAX_SHORT_TEXT_BYTES,
        "projection document type",
    )?;
    validate_text(&document.title, "projection title")?;
    validate_text(&document.body, "projection body")?;
    if document.aliases.len() > limits.max_aliases {
        return Err(DurableError::Refused(
            "projection document has too many aliases",
        ));
    }
    let dependency_limit = if profile_version == 2 {
        limits.max_dependencies.min(2)
    } else {
        1
    };
    if document.dependency_paths.is_empty()
        || document.dependency_paths.len() > dependency_limit
        || (profile_version != 2 && document.dependency_paths.len() != 1)
    {
        return Err(DurableError::Refused(
            "projection dependency set differs from profile",
        ));
    }
    let mut unique_dependencies = BTreeSet::new();
    for dependency in &document.dependency_paths {
        validate_relative_path(dependency)?;
        if !unique_dependencies.insert(dependency) || !selected_paths.contains(dependency) {
            return Err(DurableError::Refused(
                "projection dependency is duplicate or was not selected",
            ));
        }
    }
    if document.dependency_paths.first().map(String::as_str) != Some(source_path) {
        return Err(DurableError::Refused(
            "projection dependency root differs from source record",
        ));
    }
    for alias in &document.aliases {
        validate_text(alias, "projection alias")?;
    }
    let mut document_bytes = 0usize;
    for length in [
        document.document_id.len(),
        document.title.len(),
        document.body.len(),
        document.identity.len(),
        document.document_type.len(),
        source_path.len(),
    ] {
        document_bytes = document_bytes
            .checked_add(length)
            .ok_or(DurableError::Refused("projection document byte overflow"))?;
    }
    for value in &document.aliases {
        document_bytes = document_bytes
            .checked_add(value.len())
            .ok_or(DurableError::Refused("projection document byte overflow"))?;
    }
    document_bytes = document_bytes
        .checked_add(document.aliases.len().saturating_sub(1))
        .ok_or(DurableError::Refused("projection document byte overflow"))?;
    for dependency in document.dependency_paths.iter().skip(1) {
        document_bytes = document_bytes
            .checked_add(dependency.len())
            .ok_or(DurableError::Refused("projection document byte overflow"))?;
    }
    if document_bytes > limits.max_document_bytes {
        return Err(DurableError::Refused(
            "projection document exceeds byte budget",
        ));
    }
    *total_bytes = total_bytes
        .checked_add(document_bytes as u64)
        .ok_or(DurableError::Refused("projection event byte overflow"))?;
    if *total_bytes > limits.max_event_bytes {
        return Err(DurableError::Refused(
            "projection event exceeds byte budget",
        ));
    }
    if staged.documents.iter().any(|existing| {
        existing.source_path == source_path && existing.document.document_id == document.document_id
    }) {
        return Err(DurableError::Invalid(
            "duplicate projection document identity",
        ));
    }
    staged.documents.push(StagedDocument {
        source_path: source_path.to_owned(),
        source_revision: revision,
        document,
    });
    Ok(())
}

fn publish_event(
    client: &mut postgres::Client,
    domain: &str,
    consumer_id: &str,
    profile: &DurableProjectionProfile,
    event: &ProjectionEvent,
    expected_snapshot: &DomainSnapshot,
    staged: StagedEvent,
    limits: DurableProjectionLimits,
) -> DurableResult<PublishOutcome> {
    let mut tx = client
        .build_transaction()
        .isolation_level(IsolationLevel::ReadCommitted)
        .start()?;
    set_local_timeouts(&mut tx, limits.statement_timeout)?;
    let snapshot = lock_domain_snapshot(&mut tx, domain)?;
    snapshot.require_live_projection(profile)?;
    if &snapshot != expected_snapshot {
        return Err(DurableError::Conflict(
            "projection source fence changed during extraction",
        ));
    }
    let consumer_row = tx
        .query_opt(
            "SELECT * FROM cmd2_projection_consumer
         WHERE domain=$1 AND consumer_id=$2 FOR UPDATE",
            &[&domain, &consumer_id],
        )?
        .ok_or(DurableError::Corrupt("projection consumer disappeared"))?;
    let consumer = ConsumerState::from_row(&consumer_row);
    if !consumer.matches_profile(consumer_id, profile) {
        return Err(DurableError::Conflict(
            "projection consumer profile differs",
        ));
    }
    durable_projection_bootstrap::require_no_active_bootstrap(&mut tx, domain, consumer_id)?;
    let cursor = as_u64(consumer.cursor_seq)?;
    if cursor >= event.commit_seq {
        tx.commit()?;
        return Ok(PublishOutcome::AlreadyApplied(cursor));
    }
    if cursor.checked_add(1) != Some(event.commit_seq) {
        return Err(DurableError::Conflict("projection event cursor changed"));
    }

    let source_contract = profile.source_contract.to_hex();
    write_staged_documents(&mut tx, domain, consumer_id, &staged, limits)?;
    let seq_db = as_i64(event.commit_seq)?;
    let changed = tx.execute(
        "UPDATE cmd2_projection_consumer SET cursor_seq=$3,observed_head_seq=$4,
            observed_rights_version=$5,observed_rights_allowed=$6,observed_rule_version=$7,
            observed_contract_digest=$8,observed_schema_profile_digest=$9,
            observed_source_generation=$10,observed_source_complete=$11
         WHERE domain=$1 AND consumer_id=$2 AND cursor_seq=$12",
        &[
            &domain,
            &consumer_id,
            &seq_db,
            &snapshot.head_seq,
            &snapshot.rights_version,
            &snapshot.rights_allowed,
            &snapshot.rule_version,
            &source_contract,
            &snapshot.schema_profile_digest,
            &snapshot.source_generation,
            &snapshot.source_complete,
            &as_i64(cursor)?,
        ],
    )?;
    if changed != 1 {
        return Err(DurableError::Conflict(
            "projection watermark compare-and-set failed",
        ));
    }
    tx.commit()?;
    Ok(PublishOutcome::Applied(event.commit_seq))
}

fn write_staged_documents(
    tx: &mut Transaction<'_>,
    domain: &str,
    consumer_id: &str,
    staged: &StagedEvent,
    limits: DurableProjectionLimits,
) -> DurableResult<()> {
    for path in &staged.replaced_source_paths {
        tx.execute(
            "DELETE FROM cmd2_projection_document
             WHERE domain=$1 AND consumer_id=$2 AND source_path=$3",
            &[&domain, &consumer_id, path],
        )?;
    }
    for staged_document in &staged.documents {
        let document = &staged_document.document;
        let aliases_text = document.aliases.join(" ");
        let normalized = durable_source_text_search::normalize_document(document, limits)?;
        let normalized_aliases_text = normalized.aliases.join("\u{001f}");
        tx.execute(
            "INSERT INTO cmd2_projection_document(domain,consumer_id,source_path,document_id,
                source_revision,title,body,aliases,aliases_text,identity,document_type,
                source_text_identity_norm,source_text_title_norm,source_text_aliases_norm,
                source_text_body_norm,source_text_norm)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)",
            &[
                &domain,
                &consumer_id,
                &staged_document.source_path,
                &document.document_id,
                &as_i64(staged_document.source_revision)?,
                &document.title,
                &document.body,
                &document.aliases,
                &aliases_text,
                &document.identity,
                &document.document_type,
                &normalized.identity,
                &normalized.title,
                &normalized_aliases_text,
                &normalized.body,
                &normalized.all,
            ],
        )?;
        for dependency_path in &document.dependency_paths {
            tx.execute(
                "INSERT INTO cmd2_projection_dependency(domain,consumer_id,source_path,
                    document_id,dependency_kind,dependency_path)
                 VALUES($1,$2,$3,$4,'source',$5)",
                &[
                    &domain,
                    &consumer_id,
                    &staged_document.source_path,
                    &document.document_id,
                    dependency_path,
                ],
            )?;
        }
    }
    Ok(())
}

fn lock_domain_snapshot<C: GenericClient>(
    tx: &mut C,
    domain: &str,
) -> DurableResult<DomainSnapshot> {
    let audit = tx
        .query_opt(
            "SELECT generation,maintenance_state FROM cmd2_audit_fence WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?
        .ok_or(DurableError::Corrupt("domain audit fence absent"))?;
    let row = tx
        .query_opt(
            "SELECT head_seq,rights_version,rights_allowed,rule_version,contract_digest,
                schema_profile_digest,source_generation,source_complete
         FROM cmd2_domain WHERE domain=$1 FOR SHARE",
            &[&domain],
        )?
        .ok_or(DurableError::Invalid("projection domain absent"))?;
    Ok(DomainSnapshot {
        audit_generation: audit.get("generation"),
        head_seq: row.get("head_seq"),
        rights_version: row.get("rights_version"),
        rights_allowed: row.get("rights_allowed"),
        rule_version: row.get("rule_version"),
        contract_digest: row.get("contract_digest"),
        schema_profile_digest: row.get("schema_profile_digest"),
        source_generation: row.get("source_generation"),
        source_complete: row.get("source_complete"),
        maintenance_state: audit.get("maintenance_state"),
    })
}

fn read_domain_snapshot(tx: &mut Transaction<'_>, domain: &str) -> DurableResult<DomainSnapshot> {
    let row = tx
        .query_opt(
            "SELECT d.head_seq,d.rights_version,d.rights_allowed,d.rule_version,d.contract_digest,
                d.schema_profile_digest,d.source_generation,d.source_complete,
                f.generation AS audit_generation,f.maintenance_state
         FROM cmd2_domain d JOIN cmd2_audit_fence f USING(domain) WHERE d.domain=$1",
            &[&domain],
        )?
        .ok_or(DurableError::Invalid("projection domain absent"))?;
    Ok(DomainSnapshot {
        audit_generation: row.get("audit_generation"),
        head_seq: row.get("head_seq"),
        rights_version: row.get("rights_version"),
        rights_allowed: row.get("rights_allowed"),
        rule_version: row.get("rule_version"),
        contract_digest: row.get("contract_digest"),
        schema_profile_digest: row.get("schema_profile_digest"),
        source_generation: row.get("source_generation"),
        source_complete: row.get("source_complete"),
        maintenance_state: row.get("maintenance_state"),
    })
}

fn set_local_timeouts<C: GenericClient>(tx: &mut C, timeout: Duration) -> DurableResult<()> {
    validate_timeout(timeout)?;
    let statement_ms = timeout.as_millis().max(1).to_string();
    let lock_ms = timeout.as_millis().min(5000).max(1).to_string();
    tx.query_one(
        "SELECT set_config('statement_timeout',$1,true)",
        &[&format!("{statement_ms}ms")],
    )?;
    tx.query_one(
        "SELECT set_config('lock_timeout',$1,true)",
        &[&format!("{lock_ms}ms")],
    )?;
    Ok(())
}

fn validate_timeout(timeout: Duration) -> DurableResult<()> {
    if timeout.is_zero() || timeout > MAX_PROJECTION_TIMEOUT {
        Err(DurableError::Invalid(
            "projection timeout outside bounded range",
        ))
    } else {
        Ok(())
    }
}

fn validate_query(
    query: &str,
    profile: &DurableProjectionProfile,
    limit: usize,
    limits: DurableProjectionLimits,
) -> DurableResult<()> {
    profile.validate()?;
    if query.trim().is_empty()
        || query.len() > limits.max_query_bytes
        || query.contains('\0')
        || limit == 0
        || limit > limits.max_results
    {
        return Err(DurableError::Invalid(
            "projection search query or result limit",
        ));
    }
    Ok(())
}

fn validate_identifier(value: &str, max_bytes: usize, field: &'static str) -> DurableResult<()> {
    if value.is_empty() || value.len() > max_bytes || value.contains('\0') {
        Err(DurableError::Invalid(field))
    } else {
        Ok(())
    }
}

fn validate_text(value: &str, field: &'static str) -> DurableResult<()> {
    if value.contains('\0') {
        Err(DurableError::Invalid(field))
    } else {
        Ok(())
    }
}

fn validate_relative_path(path: &str) -> DurableResult<()> {
    if path.len() > MAX_PROJECTION_PATH_BYTES || RelativePath::parse(path).is_err() {
        Err(DurableError::Invalid("projection source path"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod projection_path_budget_tests {
    use super::*;
    use crate::source_projection_cli::SourceTextExtractorV2;

    fn event_member_paths(last_path: &str) -> BTreeSet<String> {
        (0..63)
            .map(|index| {
                format!("ToS/source-witnesses/agents/projection-budget/member-{index}.json")
            })
            .chain(std::iter::once(last_path.to_owned()))
            .collect()
    }

    fn member_path_bytes(paths: &BTreeSet<String>) -> u64 {
        paths.iter().map(|path| path.len() as u64).sum()
    }

    #[test]
    fn v2_64_member_events_allow_one_future_companion_and_prior_related_root() {
        let limits = DurableProjectionLimits::default();
        let extractor = SourceTextExtractorV2;
        let human_forms_path = "ToS/source-witnesses/agents/justus-lipsius/agent.human-forms.json";
        let metadata_paths = extractor
            .required_source_paths(human_forms_path, 1, b"{}")
            .unwrap();
        assert_eq!(metadata_paths.len(), 1);
        let metadata_path = metadata_paths[0].as_str();
        validate_relative_path(metadata_path).unwrap();

        // Sequence 3 changes 64 members; the sibling metadata is absent at
        // this event cut and therefore adds one bounded dependency address.
        let seq3_members = event_member_paths(human_forms_path);
        assert_eq!(seq3_members.len(), 64);
        assert!(!seq3_members.contains(metadata_path));
        let mut seq3_event_paths = seq3_members.clone();
        let mut seq3_dependency_paths = BTreeSet::new();
        let mut seq3_event_bytes = member_path_bytes(&seq3_members);
        add_projection_budget_path(
            &mut seq3_dependency_paths,
            &mut seq3_event_paths,
            metadata_path,
            &mut seq3_event_bytes,
            limits,
            "projection event exceeds dependency path budget",
        )
        .unwrap();
        assert_eq!(seq3_event_paths.len(), 65);
        assert_eq!(seq3_dependency_paths.len(), 1);

        // Sequence 4 changes 64 members including the metadata; its
        // previously-derived HumanForms root is one bounded additional root.
        let seq4_related_roots = extractor.affected_source_paths(metadata_path).unwrap();
        assert_eq!(seq4_related_roots, vec![human_forms_path.to_owned()]);
        let seq4_members = event_member_paths(metadata_path);
        assert_eq!(seq4_members.len(), 64);
        assert!(!seq4_members.contains(human_forms_path));
        let mut seq4_event_paths = seq4_members.clone();
        let mut seq4_root_paths = BTreeSet::new();
        let mut seq4_event_bytes = member_path_bytes(&seq4_members);
        add_projection_budget_path(
            &mut seq4_root_paths,
            &mut seq4_event_paths,
            human_forms_path,
            &mut seq4_event_bytes,
            limits,
            "projection event exceeds related path budget",
        )
        .unwrap();
        assert_eq!(seq4_event_paths.len(), 65);
        assert_eq!(seq4_root_paths.len(), 1);

        // The per-category ceiling remains real: the 65th unique address
        // still refuses for both related roots and dependency paths.
        let mut root_budget = BTreeSet::new();
        let mut root_event_paths = BTreeSet::new();
        let mut root_event_bytes = 0;
        for index in 0..limits.max_event_members {
            let path =
                format!("ToS/source-witnesses/agents/projection-budget/related-{index}.json");
            validate_relative_path(&path).unwrap();
            add_projection_budget_path(
                &mut root_budget,
                &mut root_event_paths,
                &path,
                &mut root_event_bytes,
                limits,
                "projection event exceeds related path budget",
            )
            .unwrap();
        }
        let over_budget_root = "ToS/source-witnesses/agents/projection-budget/related-64.json";
        assert!(matches!(
            add_projection_budget_path(
                &mut root_budget,
                &mut root_event_paths,
                over_budget_root,
                &mut root_event_bytes,
                limits,
                "projection event exceeds related path budget",
            ),
            Err(DurableError::Refused(
                "projection event exceeds related path budget"
            ))
        ));

        let mut dependency_budget = BTreeSet::new();
        let mut dependency_event_paths = BTreeSet::new();
        let mut dependency_event_bytes = 0;
        for index in 0..limits.max_event_members {
            let path =
                format!("ToS/source-witnesses/agents/projection-budget/dependency-{index}.json");
            validate_relative_path(&path).unwrap();
            add_projection_budget_path(
                &mut dependency_budget,
                &mut dependency_event_paths,
                &path,
                &mut dependency_event_bytes,
                limits,
                "projection event exceeds dependency path budget",
            )
            .unwrap();
        }
        let over_budget_dependency =
            "ToS/source-witnesses/agents/projection-budget/dependency-64.json";
        assert!(matches!(
            add_projection_budget_path(
                &mut dependency_budget,
                &mut dependency_event_paths,
                over_budget_dependency,
                &mut dependency_event_bytes,
                limits,
                "projection event exceeds dependency path budget",
            ),
            Err(DurableError::Refused(
                "projection event exceeds dependency path budget"
            ))
        ));
    }
}
