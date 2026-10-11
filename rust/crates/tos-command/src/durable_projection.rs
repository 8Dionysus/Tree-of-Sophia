//! Bounded PostgreSQL-backed derived documents over exact committed source
//! history. This is a private projection mechanic, not source admission or an
//! external owner's authority.

use std::time::{Duration, Instant};

use postgres::{IsolationLevel, Row, Transaction};
use serde::Serialize;
use tos_foundation::{Digest256, Digest256Hasher, RelativePath};
use tos_segment_store::SegmentStore;

use super::{DurableError, DurablePgCoordinator, DurableResult, as_i64, as_u64};

const MAX_PROJECTION_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_DOMAIN_BYTES: usize = 1024;
const MAX_CONSUMER_ID_BYTES: usize = 256;
const MAX_PROFILE_FIELD_BYTES: usize = 128;
const MAX_DOCUMENT_ID_BYTES: usize = 256;
const MAX_SHORT_TEXT_BYTES: usize = 16 * 1024;
const MAX_QUERY_BYTES: usize = 4096;
const MAX_QUERY_RESULTS: usize = 50;

/// Immutable owner-selected extractor identity. `source_contract` must match
/// the current CMD domain contract; changing any extractor behavior requires
/// a new version or contract digest and a new consumer.
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
/// accepts only the direct-source dependency equal to `source_path`; enriched
/// joins need a separately declared extractor profile and dependency policy.
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

/// Pure source-byte extractor. Implementations must not read ambient files,
/// databases, rights, or current projections; all derived inputs are explicit.
pub trait DurableProjectionExtractor {
    fn extract(
        &self,
        source_path: &str,
        revision: u64,
        exact_source_bytes: &[u8],
    ) -> DurableResult<Vec<DurableProjectionDocument>>;
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
            || self.max_dependencies != 1
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

#[derive(Clone, Debug, Serialize)]
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DomainSnapshot {
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
}

#[derive(Clone, Copy, Debug)]
enum PublishOutcome {
    Applied(u64),
    AlreadyApplied(u64),
}

enum ProjectionReadGate {
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

impl ProjectionReadGate {
    fn status(self) -> DurableProjectionStatus {
        match self {
            Self::Current { through_seq } => DurableProjectionStatus::Current { through_seq },
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
    pub fn apply_projection_batch<E: DurableProjectionExtractor>(
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
            let staged = self.stage_event(store, domain, &event, extractor, limits, deadline)?;
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
                ProjectionReadGate::Current { through_seq } => through_seq,
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

    fn stage_event<E: DurableProjectionExtractor>(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        event: &ProjectionEvent,
        extractor: &E,
        limits: DurableProjectionLimits,
        deadline: Instant,
    ) -> DurableResult<StagedEvent> {
        let mut staged = StagedEvent::default();
        let members = event_members(&mut self.client, domain, event, limits)?;
        if event.event_kind != "command" {
            if !members.is_empty() {
                return Err(DurableError::Corrupt(
                    "non-command projection event has source history",
                ));
            }
            return Ok(staged);
        }
        let mut event_bytes = 0u64;
        for member in members {
            event_bytes = event_bytes
                .checked_add(member.content_length)
                .and_then(|bytes| bytes.checked_add(member.source_path.len() as u64))
                .ok_or(DurableError::Refused("projection event byte overflow"))?;
            if event_bytes > limits.max_event_bytes {
                return Err(DurableError::Refused(
                    "projection event exceeds byte budget",
                ));
            }
            // Any changed subject invalidates a same-named projected source path;
            // a non-source revision deliberately deletes an older source document.
            staged
                .replaced_source_paths
                .push(member.source_path.clone());
            if !member.is_source {
                continue;
            }
            if Instant::now() >= deadline {
                return Err(DurableError::Refused("projection event deadline exceeded"));
            }
            validate_relative_path(&member.source_path)?;
            let recovered =
                self.cold_recover_exact(store, domain, &member.source_path, member.revision)?;
            let bytes = self.warm_read_selected(store, &recovered, limits.max_event_bytes)?;
            if Instant::now() >= deadline {
                return Err(DurableError::Refused("projection event deadline exceeded"));
            }
            if bytes.len() as u64 != member.content_length {
                return Err(DurableError::Corrupt(
                    "projection source byte length differs",
                ));
            }
            let documents = extractor.extract(&member.source_path, member.revision, &bytes)?;
            if Instant::now() >= deadline {
                return Err(DurableError::Refused("projection event deadline exceeded"));
            }
            let remaining_documents = limits
                .max_event_documents
                .saturating_sub(staged.documents.len());
            if documents.len() > remaining_documents {
                return Err(DurableError::Refused(
                    "projection event has too many documents",
                ));
            }
            for document in documents {
                validate_document(
                    &member.source_path,
                    document,
                    &mut staged,
                    member.revision,
                    limits,
                    &mut event_bytes,
                )?;
            }
        }
        Ok(staged)
    }
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
            source_contract_digest,cursor_seq)
         VALUES($1,$2,$3,$4,$5,0) ON CONFLICT(domain,consumer_id) DO NOTHING",
        &[
            &domain,
            &consumer_id,
            &profile.id,
            &profile.version.to_string(),
            &profile_digest,
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

fn read_projection_gate(
    tx: &mut Transaction<'_>,
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
        return Ok(ProjectionReadGate::Current { through_seq });
    }
    if !consumer.observed_state_matches(&snapshot) {
        return Ok(ProjectionReadGate::Stale {
            through_seq: Some(through_seq),
            head_seq,
            reason: "projection currentness fence differs",
        });
    }
    Ok(ProjectionReadGate::Current { through_seq })
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
        if subject_bytes < 0 || subject_bytes > 4096 || subject.len() > 4096 {
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
    if document.dependency_paths.len() != limits.max_dependencies {
        return Err(DurableError::Refused(
            "projection dependency set differs from profile",
        ));
    }
    for dependency in &document.dependency_paths {
        validate_relative_path(dependency)?;
    }
    if document.dependency_paths.len() != 1
        || document.dependency_paths.first().map(String::as_str) != Some(source_path)
    {
        return Err(DurableError::Refused(
            "projection dependency is not the source record",
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
    let cursor = as_u64(consumer.cursor_seq)?;
    if cursor >= event.commit_seq {
        tx.commit()?;
        return Ok(PublishOutcome::AlreadyApplied(cursor));
    }
    if cursor.checked_add(1) != Some(event.commit_seq) {
        return Err(DurableError::Conflict("projection event cursor changed"));
    }

    let source_contract = profile.source_contract.to_hex();
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
        tx.execute(
            "INSERT INTO cmd2_projection_document(domain,consumer_id,source_path,document_id,
                source_revision,title,body,aliases,aliases_text,identity,document_type)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)",
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
    let seq_db = as_i64(event.commit_seq)?;
    let changed = tx.execute(
        "UPDATE cmd2_projection_consumer SET cursor_seq=$3,observed_head_seq=$4,
            observed_rights_version=$5,observed_rights_allowed=$6,observed_rule_version=$7,
            observed_contract_digest=$8,observed_schema_profile_digest=$9,
            observed_source_generation=$10,observed_source_complete=$11
         WHERE domain=$1 AND consumer_id=$2 AND cursor_seq=$13",
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

fn lock_domain_snapshot(tx: &mut Transaction<'_>, domain: &str) -> DurableResult<DomainSnapshot> {
    let audit = tx
        .query_opt(
            "SELECT maintenance_state FROM cmd2_audit_fence WHERE domain=$1 FOR SHARE",
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
                f.maintenance_state
         FROM cmd2_domain d JOIN cmd2_audit_fence f USING(domain) WHERE d.domain=$1",
            &[&domain],
        )?
        .ok_or(DurableError::Invalid("projection domain absent"))?;
    Ok(DomainSnapshot {
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

fn set_local_timeouts(tx: &mut Transaction<'_>, timeout: Duration) -> DurableResult<()> {
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
    if path.len() > 4096 || RelativePath::parse(path).is_err() {
        Err(DurableError::Invalid("projection source path"))
    } else {
        Ok(())
    }
}
