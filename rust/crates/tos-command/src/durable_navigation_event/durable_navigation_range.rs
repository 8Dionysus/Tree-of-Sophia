//! Exact bounded outbox-range facts for coalesced navigation publication.
//!
//! This module deliberately keeps the committed event range separate from a
//! terminal source context. A range is never represented as a synthetic
//! command whose member list is the union of several historical events.

use std::collections::{BTreeMap, BTreeSet};

use postgres::IsolationLevel;
use serde_json::{Value, json};
use tos_foundation::{CanonicalProfile, Digest256, JsonLimits, canonical_raw_bytes_v1};
use tos_segment_store::SegmentStore;

use super::{
    DurableError, DurableNavigationEventAffectedOwnerKindV1, DurableNavigationEventAffectedOwnerV1,
    DurableNavigationEventContextV1, DurableNavigationEventLimits, DurablePgCoordinator,
    DurableProjectionLimits, DurableProjectionProfile, DurableResult, EventWitness,
    MAX_NAVIGATION_EVENT_ID_BYTES, MAX_NAVIGATION_EVENT_STEPS_PER_CALL,
    MAX_NAVIGATION_RECORD_ID_BYTES, as_i64, as_u64, canonical_value,
    navigation_verified_prefix_digest, parse_digest, read_domain_snapshot,
    read_navigation_event_cursor, read_navigation_event_head, resolve_event_witness,
    selected_version_at_cut, set_local_timeouts, validate_identifier, validate_relative_path,
    verify_navigation_consumer, verify_navigation_verified_prefix_in_tx,
};

/// One exact source-history member committed by a range event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableNavigationEventRangeMemberV1 {
    pub source_path: String,
    pub revision: u64,
    pub is_source: bool,
    pub size_bytes: u64,
    pub sha256: Digest256,
}

/// One committed CMD event and its complete exact history membership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableNavigationEventRangeEventV1 {
    pub sequence: u64,
    pub event_id: String,
    pub event_kind: String,
    /// Exact command identity used by Claim creation/revision evidence. The
    /// event's durable outbox ID is a separate `domain:sequence` identity.
    pub command_id: Option<String>,
    pub member_root: Digest256,
    pub members: Vec<DurableNavigationEventRangeMemberV1>,
}

/// Typed reason why a complete range cannot be offered to a producer yet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableNavigationEventRangePendingReasonV1 {
    EventBudgetExceeded,
    MissingEventProof,
}

/// Exact `cursor_before + 1 ..= through_sequence` proof. `manifest_raw` is
/// canonical JSON over the public facts excluding `coverage_root`; its SHA-256
/// is the range root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableNavigationEventRangeCoverageV1 {
    cursor_before: u64,
    through_sequence: u64,
    held_source_head_sequence: u64,
    terminal_event_id: String,
    events: Vec<DurableNavigationEventRangeEventV1>,
    coverage_root: Digest256,
    manifest_raw: Vec<u8>,
}

impl DurableNavigationEventRangeCoverageV1 {
    pub fn cursor_before(&self) -> u64 {
        self.cursor_before
    }

    pub fn through_sequence(&self) -> u64 {
        self.through_sequence
    }

    /// Current source head held while selecting and publishing this historical
    /// prefix. The event range root remains bound only to its exact interval.
    pub fn held_source_head_sequence(&self) -> u64 {
        self.held_source_head_sequence
    }

    pub fn terminal_event_id(&self) -> &str {
        &self.terminal_event_id
    }

    pub fn events(&self) -> &[DurableNavigationEventRangeEventV1] {
        &self.events
    }

    pub fn coverage_root(&self) -> Digest256 {
        self.coverage_root
    }

    pub fn manifest_raw(&self) -> &[u8] {
        &self.manifest_raw
    }

    pub fn changed_source_paths(&self) -> Vec<String> {
        self.events
            .iter()
            .flat_map(|event| event.members.iter())
            .filter(|member| member.is_source)
            .map(|member| member.source_path.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn event(&self, sequence: u64) -> Option<&DurableNavigationEventRangeEventV1> {
        self.events.iter().find(|event| event.sequence == sequence)
    }

    pub fn member(
        &self,
        sequence: u64,
        source_path: &str,
    ) -> Option<&DurableNavigationEventRangeMemberV1> {
        self.event(sequence)?
            .members
            .iter()
            .find(|member| member.source_path == source_path)
    }

    pub(crate) fn verify(&self, limits: DurableNavigationEventLimits) -> DurableResult<()> {
        let (raw, root) = range_manifest(
            self.cursor_before,
            self.through_sequence,
            &self.terminal_event_id,
            &self.events,
            limits.max_total_bytes,
        )?;
        if self.cursor_before >= self.through_sequence
            || self.through_sequence > self.held_source_head_sequence
            || raw != self.manifest_raw
            || root != self.coverage_root
        {
            return Err(DurableError::Conflict(
                "navigation range coverage root differs from its exact events",
            ));
        }
        Ok(())
    }
}

/// Atomically replace the complete terminal owner set for one exact event
/// range and advance the consumer cursor in the caller's held owner
/// transaction. The range and component manifests are persisted together so
/// readers never need to infer intermediate projection states.
#[allow(clippy::too_many_arguments)]
pub(crate) fn publish_navigation_event_range_cohort_set_v1_in_tx(
    tx: &mut postgres::Transaction<'_>,
    store: &SegmentStore,
    domain: &str,
    consumer_id: &str,
    profile: &DurableProjectionProfile,
    coverage: &DurableNavigationEventRangeCoverageV1,
    terminal_context: &DurableNavigationEventContextV1,
    record_set: &tos_compiler::source_navigation_record_range::NavigationRecordEventRangeCohortV1,
    claim_set: &tos_compiler::source_navigation_claim_event::NavigationClaimEventRangeOwnerSetV1,
    claim_closure: Option<&crate::source_cohort::revision::SourceRevisionClaimNavigationClosure>,
    event_source_revision: Digest256,
    expected_audit_generation: u64,
    contract: Digest256,
    rule_version: u64,
    rights_version: u64,
    job_id: &str,
    job_fence: u64,
    limits: DurableNavigationEventLimits,
) -> DurableResult<DurableNavigationEventRangeApplyV1> {
    let limits = limits.validate()?;
    validate_identifier(
        domain,
        super::super::MAX_DOMAIN_BYTES,
        "navigation range domain",
    )?;
    validate_identifier(
        consumer_id,
        super::super::MAX_CONSUMER_ID_BYTES,
        "navigation range consumer ID",
    )?;
    validate_identifier(job_id, 256, "navigation range job ID")?;
    profile.validate()?;
    if store.custody_domain() != domain.as_bytes() {
        return Err(DurableError::Conflict("STO custody domain differs"));
    }
    coverage.verify(limits)?;
    record_set
        .verify_integrity()
        .map_err(|_| DurableError::Conflict("navigation Record range integrity differs"))?;
    claim_set
        .verify_integrity()
        .map_err(|_| DurableError::Conflict("navigation Claim range integrity differs"))?;
    let through = coverage.through_sequence();
    let cursor_before = coverage.cursor_before();
    let terminal_event_id = coverage.terminal_event_id();
    if through == 0
        || cursor_before >= through
        || terminal_context.event_sequence != through
        || terminal_context.source_head_sequence != coverage.held_source_head_sequence()
        || terminal_context.event_id != terminal_event_id
        || terminal_context.consumer_cursor.sequence != cursor_before
        || record_set.through_sequence() != through
        || record_set.terminal_event_id() != terminal_event_id
        || record_set.coverage_root() != coverage.coverage_root()
        || claim_set.manifest().coverage().through_sequence() != through
        || claim_set.manifest().coverage().terminal_event_id() != terminal_event_id
        || claim_set.manifest().coverage().coverage_root() != coverage.coverage_root()
        || record_set.claim_range_root() != Some(claim_set.manifest().root())
    {
        return Err(DurableError::Conflict(
            "navigation range compiler sets differ from durable coverage",
        ));
    }
    if terminal_context.event_kind
        != coverage
            .event(through)
            .map(|event| event.event_kind.as_str())
            .unwrap_or_default()
    {
        return Err(DurableError::Conflict(
            "navigation range terminal event kind differs",
        ));
    }
    verify_range_terminal_members(coverage, terminal_context)?;

    let record_raw = record_set
        .canonical_manifest_bytes()
        .map_err(|_| DurableError::Conflict("navigation Record range manifest differs"))?;
    let claim_raw = claim_set
        .manifest()
        .canonical_manifest_bytes()
        .map_err(|_| DurableError::Conflict("navigation Claim range manifest differs"))?;
    if Digest256::of_bytes(&record_raw) != record_set.root()
        || Digest256::of_bytes(&claim_raw) != claim_set.manifest().root()
        || record_raw.len() > 64 * 1024 * 1024
        || claim_raw.len() > 64 * 1024 * 1024
    {
        return Err(DurableError::Refused(
            "navigation range component manifest budget or root",
        ));
    }

    let mut owner_rows = Vec::<(
        super::NavigationEventOwnerV2,
        super::PersistedRows,
        Option<u64>,
        Option<Digest256>,
    )>::new();
    let mut owner_keys = BTreeSet::new();
    for row in record_set.rows() {
        let cohort = row.cohort();
        cohort
            .budget_cost()
            .map_err(|_| DurableError::Conflict("navigation Record range row integrity differs"))?;
        if cohort.event_id() != terminal_event_id || cohort.event_sequence() != through {
            return Err(DurableError::Conflict(
                "navigation Record range row is not at the real terminal event",
            ));
        }
        verify_range_dependencies(cohort.dependencies(), terminal_context, limits)?;
        let byte_count = range_dependency_bytes(cohort.dependencies(), limits)?;
        let rows = super::persist_navigation_projection_rows(
            cohort.dependencies(),
            &cohort.projection().nodes,
            &cohort.projection().edges,
            &cohort.projection().diagnostics,
            cohort.node_visibility(),
            cohort.edge_visibility(),
            &[],
            byte_count,
            limits,
        )?;
        let owner = range_record_owner(cohort);
        if !owner_keys.insert((
            owner.source_path.as_str().to_owned(),
            owner.record_id.clone(),
        )) {
            return Err(DurableError::Conflict("duplicate navigation range owner"));
        }
        owner_rows.push((owner, rows, None, None));
    }
    for cohort in claim_set.cohorts() {
        let integrity = cohort
            .verify_integrity()
            .map_err(|_| DurableError::Conflict("navigation Claim range row integrity differs"))?;
        let terminal = &integrity.terminal;
        if cohort.terminal_event_id() != terminal_event_id
            || cohort.through_sequence() != through
            || cohort.cursor_before() != cursor_before
            || cohort.coverage_root() != coverage.coverage_root()
            || cohort.range_manifest_root() != claim_set.manifest().root()
        {
            return Err(DurableError::Conflict(
                "navigation Claim range row is not bound to the exact range",
            ));
        }
        verify_range_dependencies(cohort.dependencies(), terminal_context, limits)?;
        let byte_count = range_dependency_bytes(cohort.dependencies(), limits)?;
        let rows = super::persist_navigation_projection_rows(
            cohort.dependencies(),
            &cohort.projection().nodes,
            &cohort.projection().edges,
            &cohort.projection().diagnostics,
            terminal.node_visibility,
            terminal.edge_visibility,
            terminal.external_endpoint_ids,
            byte_count,
            limits,
        )?;
        let owner = range_claim_owner(cohort, &integrity);
        if !owner_keys.insert((
            owner.source_path.as_str().to_owned(),
            owner.record_id.clone(),
        )) {
            return Err(DurableError::Conflict("duplicate navigation range owner"));
        }
        owner_rows.push((
            owner,
            rows,
            Some(terminal.source_line),
            Some(cohort.trace_sha256()),
        ));
    }
    owner_rows.sort_by(|left, right| {
        (left.0.source_path.as_str(), left.0.record_id.as_str())
            .cmp(&(right.0.source_path.as_str(), right.0.record_id.as_str()))
    });
    if owner_rows.len() > limits.max_current_cohorts {
        return Err(DurableError::Refused("navigation range owner count budget"));
    }
    let record_component = super::strict_json_value(&record_raw, 64 * 1024 * 1024)?;
    let claim_component = super::strict_json_value(&claim_raw, 64 * 1024 * 1024)?;
    let (terminal_manifest_raw, terminal_cohort_root) = range_terminal_manifest(
        coverage,
        record_set.root(),
        &record_component,
        claim_set.manifest().root(),
        &claim_component,
        &owner_rows,
        limits.max_total_bytes as usize,
    )?;
    let terminal_output_root = terminal_cohort_root;
    let mut dependency_hasher = super::Digest256Hasher::new();
    dependency_hasher.update(b"tos-source-navigation-event-range-dependency-roots-v1");
    super::navigation_hash_part(
        &mut dependency_hasher,
        record_set.root().to_hex().as_bytes(),
    );
    super::navigation_hash_part(
        &mut dependency_hasher,
        claim_set.manifest().root().to_hex().as_bytes(),
    );
    let dependency_root = dependency_hasher.finalize();

    let tx_isolation = tx.query_one("SHOW transaction_isolation", &[])?;
    let isolation: String = tx_isolation
        .try_get(0)
        .map_err(|_| DurableError::Corrupt("navigation range transaction isolation response"))?;
    if isolation != "repeatable read" {
        return Err(DurableError::Conflict(
            "navigation event range requires the held repeatable-read owner transaction",
        ));
    }
    super::set_local_timeouts(tx, limits.statement_timeout)?;
    let snapshot = super::read_domain_snapshot(tx, domain)?;
    snapshot.require_live_projection(profile)?;
    if snapshot.head_seq()? != coverage.held_source_head_sequence()
        || snapshot.audit_generation != super::as_i64(expected_audit_generation)?
        || snapshot.rights_version != super::as_i64(rights_version)?
        || snapshot.rule_version != super::as_i64(rule_version)?
        || snapshot.contract_digest != contract.to_hex()
        || !snapshot.rights_allowed
    {
        return Err(DurableError::Conflict(
            "navigation range held source/rights/rule fence differs",
        ));
    }
    verify_range_empty_claim_evidence(
        coverage,
        record_set,
        claim_set,
        claim_closure,
        tx,
        domain,
        consumer_id,
        store.store_id(),
        snapshot.source_generation,
        expected_audit_generation,
        rights_version,
        rule_version,
        contract,
        job_id,
        job_fence,
        limits,
    )?;
    let job = tx.query_opt(
        "SELECT fence_epoch FROM cmd2_job WHERE domain=$1 AND job_id=$2",
        &[&domain, &job_id],
    )?;
    if job.map(|row| row.get::<_, i64>(0)) != Some(super::as_i64(job_fence)?) {
        return Err(DurableError::Refused(
            "navigation range held job fence differs",
        ));
    }
    let profile_version = profile.version.to_string();
    let source_contract = profile.source_contract.to_hex();
    tx.execute(
        "INSERT INTO cmd2_projection_navigation_consumer(
            domain,consumer_id,profile_id,profile_version,source_contract_digest)
         VALUES($1,$2,$3,$4,$5) ON CONFLICT(domain,consumer_id) DO NOTHING",
        &[
            &domain,
            &consumer_id,
            &profile.id,
            &profile_version,
            &source_contract,
        ],
    )?;
    let consumer = tx.query_one(
        "SELECT profile_id,profile_version,source_contract_digest,cursor_seq,
                source_revision,last_event_id,last_input_root,last_output_root,
                coverage_seq,coverage_root,adjacency_seq,adjacency_root,adjacency_ready,
                verified_prefix_version,verified_prefix_seq,verified_prefix_coverage_root,
                verified_prefix_cohort_root,verified_prefix_digest
           FROM cmd2_projection_navigation_consumer
          WHERE domain=$1 AND consumer_id=$2 FOR UPDATE",
        &[&domain, &consumer_id],
    )?;
    bootstrap_capture::require_no_active_initial(tx, domain, consumer_id)?;
    if consumer.get::<_, String>("profile_id") != profile.id
        || consumer.get::<_, String>("profile_version") != profile_version
        || consumer.get::<_, String>("source_contract_digest") != source_contract
    {
        return Err(DurableError::Conflict(
            "navigation range consumer profile differs",
        ));
    }
    let cursor = as_u64(consumer.get("cursor_seq"))?;
    if cursor == through {
        let saved = tx.query_opt(
            "SELECT cursor_before,range_root,terminal_cohort_root,terminal_cohort_manifest
               FROM cmd2_projection_navigation_coverage_range
              WHERE domain=$1 AND consumer_id=$2 AND through_seq=$3",
            &[&domain, &consumer_id, &super::as_i64(through)?],
        )?;
        let replayed = saved.is_some_and(|row| {
            as_u64(row.get::<_, i64>("cursor_before")).ok() == Some(cursor_before)
                && row.get::<_, String>("range_root").trim_end()
                    == coverage.coverage_root().to_hex()
                && row.get::<_, String>("terminal_cohort_root").trim_end()
                    == terminal_cohort_root.to_hex()
                && row.get::<_, Vec<u8>>("terminal_cohort_manifest") == terminal_manifest_raw
        });
        if replayed
            && consumer
                .get::<_, Option<String>>("last_event_id")
                .as_deref()
                == Some(terminal_event_id)
            && consumer
                .get::<_, Option<String>>("last_input_root")
                .as_deref()
                == Some(coverage.coverage_root().to_hex().as_str())
            && consumer
                .get::<_, Option<String>>("last_output_root")
                .as_deref()
                == Some(terminal_output_root.to_hex().as_str())
        {
            let bootstrap = tx.query_opt(
                "SELECT through_seq,bootstrap_root,owner_binding_json
                   FROM cmd2_projection_navigation_bootstrap
                  WHERE domain=$1 AND consumer_id=$2 FOR UPDATE",
                &[&domain, &consumer_id],
            )?;
            let bootstrap = bootstrap.ok_or(DurableError::Conflict(
                "navigation range replay requires a full bootstrap",
            ))?;
            super::verify_navigation_verified_prefix_in_tx(
                tx,
                domain,
                consumer_id,
                profile,
                super::as_u64(bootstrap.get("through_seq"))?,
                super::parse_digest(bootstrap.get("bootstrap_root"))?,
                &bootstrap.get::<_, String>("owner_binding_json"),
                &consumer,
                limits,
            )?;
            return Ok(DurableNavigationEventRangeApplyV1 {
                cursor_before,
                through_sequence: through,
                terminal_event_id: terminal_event_id.to_owned(),
                coverage_root: coverage.coverage_root(),
                terminal_cohort_root,
                replacement_owner_count: owner_rows.len(),
                replayed: true,
            });
        }
        return Err(DurableError::Conflict(
            "navigation range cursor already owns a different terminal set",
        ));
    }
    if cursor != cursor_before {
        return Err(DurableError::Conflict(
            "navigation range cursor is not the predecessor",
        ));
    }
    let bootstrap = tx
        .query_opt(
            "SELECT through_seq,bootstrap_root,owner_binding_json,header_raw
           FROM cmd2_projection_navigation_bootstrap
          WHERE domain=$1 AND consumer_id=$2 FOR UPDATE",
            &[&domain, &consumer_id],
        )?
        .ok_or(DurableError::Conflict(
            "navigation event range requires a full bootstrap",
        ))?;
    let bootstrap_sequence = as_u64(bootstrap.get("through_seq"))?;
    let bootstrap_root = parse_digest(bootstrap.get("bootstrap_root"))?;
    let previous_coverage_root = consumer
        .get::<_, Option<String>>("coverage_root")
        .map(parse_digest)
        .transpose()?
        .ok_or(DurableError::Conflict(
            "navigation range predecessor coverage root absent",
        ))?;
    let previous_adjacency_root = consumer
        .get::<_, Option<String>>("adjacency_root")
        .map(parse_digest)
        .transpose()?
        .ok_or(DurableError::Conflict(
            "navigation range predecessor adjacency root absent",
        ))?;
    if bootstrap_sequence > cursor
        || as_u64(consumer.get("coverage_seq"))? != cursor
        || as_u64(consumer.get("adjacency_seq"))? != cursor
        || !consumer.get::<_, bool>("adjacency_ready")
    {
        return Err(DurableError::Conflict(
            "navigation range predecessor lacks complete coverage",
        ));
    }
    let owner_binding_json: String = bootstrap.get("owner_binding_json");
    verify_navigation_verified_prefix_in_tx(
        tx,
        domain,
        consumer_id,
        profile,
        bootstrap_sequence,
        bootstrap_root,
        &owner_binding_json,
        &consumer,
        limits,
    )?;

    let claim_stream_paths = claim_set
        .manifest()
        .terminal_streams()
        .iter()
        .map(|stream| stream.source_path().to_owned())
        .collect::<Vec<_>>();
    let held_claim_predecessors = read_navigation_claim_stream_predecessors(
        tx,
        domain,
        consumer_id,
        &claim_stream_paths,
        limits,
    )?;
    if held_claim_predecessors != claim_set.manifest().stream_predecessors() {
        return Err(DurableError::Conflict(
            "navigation Claim stream predecessors differ from held publication",
        ));
    }

    // Reverse projection dependencies describe the predecessor publication.
    // Terminal owners also include new Records and stream-complete Claims;
    // their current source causes were verified above against the held closure.
    let mut predecessor_owners = BTreeSet::new();
    for prior in record_set.prior_owners() {
        predecessor_owners.insert((
            prior.source_path().as_str().to_owned(),
            prior.record_id().to_owned(),
        ));
    }
    for prior in claim_set.manifest().prior_owners() {
        predecessor_owners.insert((prior.claim_stream_path.clone(), prior.claim_id.clone()));
    }
    let mut target_owners = owner_keys.clone();
    target_owners.extend(predecessor_owners.iter().cloned());
    let cohort_checkpoint =
        super::cohort_tree::verify_owners(tx, domain, consumer_id, &target_owners, limits)?;
    let changed_paths = coverage.changed_source_paths();
    verify_navigation_range_reverse_dependency_closure(
        tx,
        domain,
        consumer_id,
        &predecessor_owners,
        &changed_paths,
        limits,
    )?;
    if !target_owners.is_empty() {
        let replacement_rows = owner_rows
            .iter()
            .map(|entry| entry.1.clone())
            .collect::<Vec<_>>();
        super::verify_navigation_batch_final_budget(
            tx,
            domain,
            consumer_id,
            &target_owners,
            &replacement_rows,
            bootstrap.get::<_, String>("header_raw").len() as u64,
            limits,
        )?;
    }
    for (path, id) in &target_owners {
        verify_navigation_range_predecessor_root(
            tx,
            domain,
            consumer_id,
            path,
            id,
            owner_rows
                .iter()
                .find(|entry| entry.0.source_path.as_str() == path && entry.0.record_id == *id)
                .and_then(|entry| entry.0.predecessor_cohort_root)
                .or_else(|| {
                    record_set
                        .prior_owners()
                        .iter()
                        .find(|prior| {
                            prior.source_path().as_str() == path && prior.record_id() == id
                        })
                        .map(|prior| prior.predecessor_cohort_root())
                })
                .or_else(|| {
                    claim_set
                        .manifest()
                        .stream_predecessors()
                        .iter()
                        .find(|prior| prior.claim_stream_path == *path && prior.claim_id == *id)
                        .map(|prior| match prior.basis {
                            tos_compiler::source_navigation_claim_event::NavigationClaimEventRangePriorOwnerBasisV1::EventCohort {
                                cohort_root,
                            } => cohort_root,
                            tos_compiler::source_navigation_claim_event::NavigationClaimEventRangePriorOwnerBasisV1::Bootstrap {
                                owner_root,
                            } => owner_root,
                        })
                }),
            bootstrap_root,
            limits,
        )?;
    }

    let replacement_rows = owner_rows.iter().map(|entry| &entry.1).collect::<Vec<_>>();
    let (adjacency_updates, adjacency_root) = if target_owners.is_empty() {
        (BTreeMap::new(), previous_adjacency_root)
    } else {
        super::prepare_navigation_adjacency_update_batch(
            tx,
            domain,
            consumer_id,
            &target_owners,
            bootstrap_root,
            previous_adjacency_root,
            through,
            &replacement_rows,
            limits,
        )?
    };
    for (owner, rows, _, _) in &owner_rows {
        super::replace_navigation_event_cohort_rows(
            tx,
            domain,
            consumer_id,
            owner,
            rows,
            through,
            terminal_cohort_root,
        )?;
    }
    let replacement_keys = owner_rows
        .iter()
        .map(|entry| {
            (
                entry.0.source_path.as_str().to_owned(),
                entry.0.record_id.clone(),
            )
        })
        .collect::<BTreeSet<_>>();
    let retire_keys = target_owners
        .difference(&replacement_keys)
        .cloned()
        .collect::<Vec<_>>();
    for (path, id) in retire_keys {
        retire_navigation_range_owner(tx, domain, consumer_id, &path, &id)?;
    }
    let coverage_root = navigation_range_coverage_root_next(
        previous_coverage_root,
        cursor_before,
        through,
        coverage.held_source_head_sequence(),
        terminal_event_id,
        coverage.coverage_root(),
        terminal_cohort_root,
        event_source_revision,
        dependency_root,
        terminal_output_root,
    );
    tx.execute(
        "INSERT INTO cmd2_projection_navigation_coverage_range(
            domain,consumer_id,cursor_before,through_seq,held_source_head_seq,terminal_event_id,range_root,
            range_manifest,source_revision,input_root,dependency_root,output_root,
            previous_coverage_root,coverage_root,terminal_cohort_count,
            terminal_cohort_root,terminal_cohort_manifest)
         VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)",
        &[
            &domain,
            &consumer_id,
            &super::as_i64(cursor_before)?,
            &super::as_i64(through)?,
            &super::as_i64(coverage.held_source_head_sequence())?,
            &terminal_event_id,
            &coverage.coverage_root().to_hex(),
            &coverage.manifest_raw,
            &event_source_revision.to_hex(),
            &coverage.coverage_root().to_hex(),
            &dependency_root.to_hex(),
            &terminal_output_root.to_hex(),
            &previous_coverage_root.to_hex(),
            &coverage_root.to_hex(),
            &i32::try_from(owner_rows.len())
                .map_err(|_| DurableError::Refused("navigation range owner count"))?,
            &terminal_cohort_root.to_hex(),
            &terminal_manifest_raw,
        ],
    )?;
    let affected_ids = adjacency_updates.keys().cloned().collect::<Vec<_>>();
    if !affected_ids.is_empty() {
        tx.execute(
            "DELETE FROM cmd2_projection_navigation_adjacency_certificate
              WHERE domain=$1 AND consumer_id=$2 AND node_id=ANY($3)",
            &[&domain, &consumer_id, &affected_ids],
        )?;
    }
    for certificate in adjacency_updates.values().flatten() {
        tx.execute(
            "INSERT INTO cmd2_projection_navigation_adjacency_certificate(
                domain,consumer_id,node_id,edge_count,edge_digest,certificate_root)
             VALUES($1,$2,$3,$4,$5,$6)",
            &[
                &domain,
                &consumer_id,
                &certificate.node_id,
                &super::as_i64(certificate.edge_count)?,
                &certificate.edge_digest.to_hex(),
                &certificate.certificate_root.to_hex(),
            ],
        )?;
    }
    let cohort_root = super::cohort_tree::update_owners(
        tx,
        domain,
        consumer_id,
        &target_owners,
        &cohort_checkpoint,
        limits,
    )?;
    let owner_binding_json: String = bootstrap.get("owner_binding_json");
    let checkpoint_digest = navigation_verified_prefix_digest(
        domain,
        consumer_id,
        profile,
        bootstrap_sequence,
        bootstrap_root,
        &owner_binding_json,
        through,
        coverage_root,
        cohort_root,
        adjacency_root,
        terminal_event_id,
        event_source_revision,
        coverage.coverage_root(),
        terminal_output_root,
    );
    let changed = tx.execute(
        "UPDATE cmd2_projection_navigation_consumer
            SET cursor_seq=$3,source_revision=$4,last_event_id=$5,
                last_input_root=$6,last_output_root=$7,coverage_seq=$3,
                coverage_root=$8,adjacency_seq=$3,adjacency_root=$9,adjacency_ready=true,
                verified_prefix_version=$13,
                verified_prefix_seq=$3,verified_prefix_coverage_root=$8,
                verified_prefix_cohort_root=$10,verified_prefix_digest=$11
          WHERE domain=$1 AND consumer_id=$2 AND cursor_seq=$12",
        &[
            &domain,
            &consumer_id,
            &super::as_i64(through)?,
            &event_source_revision.to_hex(),
            &terminal_event_id,
            &coverage.coverage_root().to_hex(),
            &terminal_output_root.to_hex(),
            &coverage_root.to_hex(),
            &adjacency_root.to_hex(),
            &cohort_root.to_hex(),
            &checkpoint_digest.to_hex(),
            &super::as_i64(cursor_before)?,
            &super::VERIFIED_PREFIX_FORMAT_VERSION,
        ],
    )?;
    if changed != 1 {
        return Err(DurableError::Conflict(
            "navigation range cursor compare-and-set failed",
        ));
    }
    Ok(DurableNavigationEventRangeApplyV1 {
        cursor_before,
        through_sequence: through,
        terminal_event_id: terminal_event_id.to_owned(),
        coverage_root: coverage.coverage_root(),
        terminal_cohort_root,
        replacement_owner_count: owner_rows.len(),
        replayed: false,
    })
}

impl DurablePgCoordinator {
    pub fn read_navigation_event_range_claim_stream_predecessors_v1(
        &mut self,
        domain: &str,
        consumer_id: &str,
        profile: &DurableProjectionProfile,
        coverage: &DurableNavigationEventRangeCoverageV1,
        stream_paths: &[String],
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<Vec<tos_compiler::source_navigation_claim_event::NavigationClaimEventRangeStreamPredecessorV1>>{
        let limits = limits.validate()?;
        coverage.verify(limits)?;
        let cursor =
            read_navigation_event_cursor(&mut self.client, domain, consumer_id, profile, limits)?;
        let head = read_navigation_event_head(&mut self.client, domain, profile, limits)?;
        if cursor.sequence != coverage.cursor_before()
            || head != coverage.held_source_head_sequence()
        {
            return Err(DurableError::Conflict(
                "navigation Claim predecessor source cut changed",
            ));
        }
        read_navigation_claim_stream_predecessors(
            &mut self.client,
            domain,
            consumer_id,
            stream_paths,
            limits,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn publish_navigation_event_range_cohort_set_v1_in_tx(
        tx: &mut postgres::Transaction<'_>,
        store: &SegmentStore,
        domain: &str,
        consumer_id: &str,
        profile: &DurableProjectionProfile,
        coverage: &DurableNavigationEventRangeCoverageV1,
        terminal_context: &DurableNavigationEventContextV1,
        record_set: &tos_compiler::source_navigation_record_range::NavigationRecordEventRangeCohortV1,
        claim_set: &tos_compiler::source_navigation_claim_event::NavigationClaimEventRangeOwnerSetV1,
        claim_closure: Option<
            &crate::source_cohort::revision::SourceRevisionClaimNavigationClosure,
        >,
        event_source_revision: Digest256,
        expected_audit_generation: u64,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<DurableNavigationEventRangeApplyV1> {
        publish_navigation_event_range_cohort_set_v1_in_tx(
            tx,
            store,
            domain,
            consumer_id,
            profile,
            coverage,
            terminal_context,
            record_set,
            claim_set,
            claim_closure,
            event_source_revision,
            expected_audit_generation,
            contract,
            rule_version,
            rights_version,
            job_id,
            job_fence,
            limits,
        )
    }
}

fn verify_range_empty_claim_evidence(
    coverage: &DurableNavigationEventRangeCoverageV1,
    record_set: &tos_compiler::source_navigation_record_range::NavigationRecordEventRangeCohortV1,
    claim_set: &tos_compiler::source_navigation_claim_event::NavigationClaimEventRangeOwnerSetV1,
    held_closure: Option<&crate::source_cohort::revision::SourceRevisionClaimNavigationClosure>,
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    consumer_id: &str,
    store_id: [u8; 16],
    source_generation: Option<i64>,
    expected_audit_generation: u64,
    rights_version: u64,
    rule_version: u64,
    contract: Digest256,
    job_id: &str,
    job_fence: u64,
    limits: DurableNavigationEventLimits,
) -> DurableResult<()> {
    use tos_compiler::source_navigation_claim_event::NavigationClaimEventRangeEmptyOwnerEvidenceV1;

    let evidence = claim_set.manifest().empty_owner_evidence();
    if !claim_set.cohorts().is_empty() {
        if evidence.is_some() {
            return Err(DurableError::Conflict(
                "nonempty Claim range unexpectedly has empty-owner evidence",
            ));
        }
        let closure = held_closure.ok_or(DurableError::Conflict(
            "nonempty Claim range lacks its held reverse closure",
        ))?;
        verify_range_claim_closure_binding(
            coverage,
            closure,
            domain,
            store_id,
            source_generation,
            expected_audit_generation,
            rights_version,
            rule_version,
            contract,
            job_id,
            job_fence,
        )?;
        verify_range_claim_reverse_closure(
            tx,
            domain,
            consumer_id,
            coverage,
            record_set,
            claim_set,
            closure,
            limits,
        )?;
        return Ok(());
    }
    match evidence {
        Some(NavigationClaimEventRangeEmptyOwnerEvidenceV1::RightsRuleOnly) => {
            if coverage.events().is_empty()
                || coverage.events().iter().any(|event| {
                    !matches!(event.event_kind.as_str(), "rights" | "rule")
                        || !event.members.is_empty()
                })
            {
                return Err(DurableError::Conflict(
                    "empty Claim rights/rule range contains source work",
                ));
            }
        }
        Some(NavigationClaimEventRangeEmptyOwnerEvidenceV1::SourceClosure {
            closure_root,
            lookup_keys,
        }) => {
            let closure = held_closure.ok_or(DurableError::Conflict(
                "empty Claim source range lacks its held reverse closure",
            ))?;
            verify_range_claim_closure_binding(
                coverage,
                closure,
                domain,
                store_id,
                source_generation,
                expected_audit_generation,
                rights_version,
                rule_version,
                contract,
                job_id,
                job_fence,
            )?;
            let actual_keys = lookup_keys
                .iter()
                .map(|lookup| (lookup.kind.as_str(), lookup.key.as_str()))
                .collect::<Vec<_>>();
            if closure.closure_root() != *closure_root
                || closure
                    .lookups()
                    .values()
                    .any(|observation| !observation.rows.is_empty())
            {
                return Err(DurableError::Conflict(
                    "empty Claim source range differs from the held reverse closure",
                ));
            }
            verify_range_claim_reverse_closure(
                tx,
                domain,
                consumer_id,
                coverage,
                record_set,
                claim_set,
                closure,
                limits,
            )?;
            let expected_keys = closure
                .lookups()
                .keys()
                .map(|(kind, key)| (kind.as_str(), key.as_str()))
                .collect::<Vec<_>>();
            if expected_keys != actual_keys {
                return Err(DurableError::Conflict(
                    "empty Claim range lookup keys differ from the held source closure",
                ));
            }
        }
        Some(NavigationClaimEventRangeEmptyOwnerEvidenceV1::NoProjectableToSMembers) => {
            if coverage.events().is_empty()
                || coverage.events().iter().any(|event| {
                    !matches!(event.event_kind.as_str(), "command" | "rights" | "rule")
                        || event
                            .members
                            .iter()
                            .any(|member| member.source_path.starts_with("ToS/"))
                })
            {
                return Err(DurableError::Conflict(
                    "empty Claim no-ToS range contains projectable ToS source work",
                ));
            }
        }
        None => {
            return Err(DurableError::Conflict(
                "empty Claim range lacks typed zero-owner evidence",
            ));
        }
    }
    Ok(())
}

fn verify_range_member_digest(
    coverage: &DurableNavigationEventRangeCoverageV1,
    event_sequence: u64,
    path: &str,
    digest: Digest256,
    expected_is_source: Option<bool>,
) -> DurableResult<()> {
    validate_relative_path(path)?;
    let member = coverage
        .member(event_sequence, path)
        .ok_or(DurableError::Conflict(
            "navigation mutation identity path is absent from exact event members",
        ))?;
    if member.sha256 != digest
        || expected_is_source.is_some_and(|expected| member.is_source != expected)
    {
        return Err(DurableError::Conflict(
            "navigation mutation identity digest differs from exact event member",
        ));
    }
    Ok(())
}

fn verify_optional_range_member_digest(
    coverage: &DurableNavigationEventRangeCoverageV1,
    event_sequence: u64,
    path: Option<&str>,
    digest: Option<Digest256>,
    expected_is_source: bool,
) -> DurableResult<bool> {
    match (path, digest) {
        (Some(path), Some(digest)) => {
            verify_range_member_digest(
                coverage,
                event_sequence,
                path,
                digest,
                Some(expected_is_source),
            )?;
            Ok(true)
        }
        (None, None) => Ok(false),
        _ => Err(DurableError::Conflict(
            "navigation mutation identity path/digest pair is incomplete",
        )),
    }
}

fn verify_record_range_mutation_identity(
    coverage: &DurableNavigationEventRangeCoverageV1,
    record_id: &str,
    cause: &tos_compiler::source_navigation_record_range::NavigationRecordEventRangeCauseRefV1,
) -> DurableResult<()> {
    use tos_compiler::source_navigation_record_range::NavigationRecordEventRangeCauseKindV1 as CauseKind;

    let event = coverage
        .event(cause.event_sequence())
        .ok_or(DurableError::Conflict(
            "Record mutation cause event is absent from exact range",
        ))?;
    if event.event_kind != "command"
        || event.event_id != cause.event_id()
        || event.command_id.as_deref() != cause.command_id()
        || cause.command_id().is_none()
    {
        return Err(DurableError::Conflict(
            "Record mutation cause command identity differs from exact range",
        ));
    }

    match cause.kind() {
        CauseKind::DirectCreate | CauseKind::RecordRevision => {
            let identity = cause.mutation_identity().ok_or(DurableError::Conflict(
                "direct Record cause lacks typed mutation identity",
            ))?;
            let source_path = identity.source_path().as_str();
            if identity.record_id() != record_id
                || source_path != cause.path().as_str()
                || identity.source_sha256() != cause.sha256()
            {
                return Err(DurableError::Conflict(
                    "direct Record identity differs from its owner cause",
                ));
            }
            verify_range_member_digest(
                coverage,
                cause.event_sequence(),
                source_path,
                identity.source_sha256(),
                Some(true),
            )?;

            let has_history = verify_optional_range_member_digest(
                coverage,
                cause.event_sequence(),
                identity.history_path().map(|path| path.as_str()),
                identity.history_sha256(),
                true,
            )?;
            let has_receipt = verify_optional_range_member_digest(
                coverage,
                cause.event_sequence(),
                identity.receipt_path().map(|path| path.as_str()),
                identity.receipt_sha256(),
                false,
            )?;
            let has_provenance = verify_optional_range_member_digest(
                coverage,
                cause.event_sequence(),
                identity.provenance_event_path().map(|path| path.as_str()),
                identity.provenance_event_sha256(),
                true,
            )?;
            let has_forms = verify_optional_range_member_digest(
                coverage,
                cause.event_sequence(),
                identity.origin_forms_path().map(|path| path.as_str()),
                identity.origin_forms_sha256(),
                true,
            )?;

            let identity_shape_is_valid = match cause.kind() {
                CauseKind::DirectCreate => !has_history && has_receipt && has_provenance,
                CauseKind::RecordRevision => {
                    has_history && !has_receipt && !has_provenance && !has_forms
                }
                _ => false,
            };
            if !identity_shape_is_valid {
                return Err(DurableError::Conflict(
                    "direct Record mutation sidecar identity shape differs",
                ));
            }
        }
        _ if cause.mutation_identity().is_some() => {
            return Err(DurableError::Conflict(
                "non-mutation Record cause carries a mutation identity",
            ));
        }
        _ => {}
    }
    Ok(())
}

fn verify_claim_range_mutation_identity(
    coverage: &DurableNavigationEventRangeCoverageV1,
    mutation: &tos_compiler::source_navigation_claim_event::NavigationClaimEventRangeMutationV1,
    byte_limit: usize,
) -> DurableResult<()> {
    let event = coverage
        .event(mutation.event_sequence())
        .ok_or(DurableError::Conflict(
            "Claim mutation event is absent from exact range",
        ))?;
    let stream_path = mutation.claim_stream_member_path().as_str();
    let receipt_path = mutation.receipt_path().as_str();
    if event.event_kind != "command" || event.event_id != mutation.event_id() {
        return Err(DurableError::Conflict(
            "Claim mutation command identity differs from exact range",
        ));
    }
    verify_range_member_digest(
        coverage,
        mutation.event_sequence(),
        stream_path,
        mutation.claim_stream_sha256(),
        Some(true),
    )?;
    verify_range_member_digest(
        coverage,
        mutation.event_sequence(),
        receipt_path,
        mutation.receipt_sha256(),
        Some(true),
    )?;

    let home = stream_path
        .strip_suffix("source-claims.jsonl")
        .filter(|home| home.ends_with('/'))
        .ok_or(DurableError::Conflict(
            "Claim mutation stream has no canonical owner home",
        ))?;
    let is_revision = mutation.revision_history_path().is_some();
    let expected_receipt_path = if is_revision {
        format!("{home}claim-revision-history.json")
    } else {
        format!("{home}source-create-receipt.json")
    };
    let receipt_member = coverage
        .member(mutation.event_sequence(), receipt_path)
        .ok_or(DurableError::Conflict(
            "Claim mutation receipt is absent from exact event members",
        ))?;
    let revision_receipt_sha256 = if is_revision {
        let raw = mutation
            .revision_history_raw()
            .ok_or(DurableError::Conflict(
                "Claim revision history bytes are absent",
            ))?;
        let history: Value = serde_json::from_slice(raw)
            .map_err(|_| DurableError::Corrupt("Claim revision history JSON is invalid"))?;
        let receipt = history
            .get("receipts")
            .and_then(Value::as_array)
            .and_then(|receipts| receipts.last())
            .ok_or(DurableError::Conflict(
                "Claim revision receipt is absent from history",
            ))?;
        Some(source_record_digest(receipt, byte_limit)?)
    } else {
        None
    };
    if receipt_path != expected_receipt_path
        || mutation.mutation_claim_ids().is_empty()
        || mutation
            .mutation_claim_ids()
            .iter()
            .any(|id| !id.starts_with("tos.claim."))
        || if is_revision {
            mutation.revision_history_path().map(|path| path.as_str()) != Some(receipt_path)
                || mutation.revision_history_sha256() != Some(mutation.receipt_sha256())
                || mutation
                    .revision_history_raw()
                    .is_none_or(|raw| Digest256::of_bytes(raw) != mutation.receipt_sha256())
                || mutation.revision_receipt_sha256() != revision_receipt_sha256
                || !receipt_member.is_source
                || mutation.mutation_claim_ids().len() != 1
        } else {
            mutation.revision_history_sha256().is_some()
                || mutation.revision_history_raw().is_some()
                || mutation.revision_receipt_sha256().is_some()
                || !receipt_member.is_source
        }
    {
        return Err(DurableError::Conflict(
            "Claim mutation stream/receipt identity differs from exact event",
        ));
    }
    Ok(())
}

fn source_record_digest(value: &Value, byte_limit: usize) -> DurableResult<Digest256> {
    let raw = serde_json::to_vec(value)
        .map_err(|_| DurableError::Invalid("navigation Claim revision receipt JSON"))?;
    let limits = JsonLimits::new(byte_limit, 96, 1_000_000, 4096)
        .map_err(|_| DurableError::Refused("navigation Claim revision receipt limits"))?;
    let canonical = canonical_raw_bytes_v1(&raw, CanonicalProfile::SourceRecordDigestV1, limits)
        .map_err(|_| DurableError::Refused("navigation Claim revision receipt digest"))?;
    Ok(Digest256::of_bytes(&canonical))
}

/// Rebuild the exact reverse-lookup input set from the held predecessor
/// projection and compiler-sealed Record/Claim mutation proofs. The producer's
/// lookup map is not accepted as authority: source paths and IDs are matched
/// against the exact event interval, predecessor owner rows, and direct
/// mutation causes before any Claim lookup result is trusted.
fn verify_range_claim_reverse_closure(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    consumer_id: &str,
    coverage: &DurableNavigationEventRangeCoverageV1,
    record_set: &tos_compiler::source_navigation_record_range::NavigationRecordEventRangeCohortV1,
    claim_set: &tos_compiler::source_navigation_claim_event::NavigationClaimEventRangeOwnerSetV1,
    closure: &crate::source_cohort::revision::SourceRevisionClaimNavigationClosure,
    limits: DurableNavigationEventLimits,
) -> DurableResult<()> {
    use crate::source_claim_dependency_index::ClaimNavigationReferenceKind as LookupKind;
    use tos_compiler::source_navigation_claim_event::NavigationClaimEventRangePriorOwnerBasisV1 as ClaimPriorBasis;
    use tos_compiler::source_navigation_record_range::NavigationRecordEventRangeCauseKindV1 as RecordCauseKind;

    let changed_source_paths = coverage.changed_source_paths();
    let changed_tos_paths = changed_source_paths
        .iter()
        .filter(|path| path.starts_with("ToS/"))
        .cloned()
        .collect::<BTreeSet<_>>();
    let row_limit = i64::try_from(limits.max_current_rows.saturating_add(1))
        .map_err(|_| DurableError::Refused("navigation Claim source-ID owner count"))?;

    let mut predecessor_ids_by_path = BTreeMap::<String, BTreeSet<String>>::new();
    if !changed_source_paths.is_empty() {
        let rows = tx.query(
            "SELECT cause_path,owner_source_path,record_id,predecessor_kind,predecessor_cohort_root
               FROM (
               SELECT c.source_path AS cause_path,c.source_path AS owner_source_path,
                      c.record_id,'event'::text AS predecessor_kind,
                      c.output_root AS predecessor_cohort_root
                 FROM cmd2_projection_navigation_cohort c
                WHERE c.domain=$1 AND c.consumer_id=$2 AND c.source_path=ANY($3)
                  AND c.basis_kind IN (
                    'record-mutation','dependency-refresh','dependency-refresh-set',
                    'claim-revision-refresh','range-reconciliation'
                  )
               UNION
               SELECT d.dependency_path AS cause_path,c.source_path AS owner_source_path,
                      c.record_id,'event'::text AS predecessor_kind,
                      c.output_root AS predecessor_cohort_root
                 FROM cmd2_projection_navigation_dependency d
                 JOIN cmd2_projection_navigation_cohort c
                   ON c.domain=d.domain AND c.consumer_id=d.consumer_id
                  AND c.source_path=d.source_path AND c.record_id=d.record_id
                WHERE d.domain=$1 AND d.consumer_id=$2 AND d.dependency_path=ANY($3)
                  AND c.basis_kind IN (
                    'record-mutation','dependency-refresh','dependency-refresh-set',
                    'claim-revision-refresh','range-reconciliation'
                  )
               UNION
               SELECT b.owner_source_path AS cause_path,
                      b.owner_source_path AS owner_source_path,
                      b.owner_record_id AS record_id,'bootstrap'::text AS predecessor_kind,
                      NULL::text AS predecessor_cohort_root
                 FROM cmd2_projection_navigation_bootstrap_row b
                WHERE b.domain=$1 AND b.consumer_id=$2 AND b.active
                  AND b.owner_source_path=ANY($3) AND b.owner_record_id NOT LIKE 'tos.claim.%'
                  AND NOT EXISTS (
                    SELECT 1 FROM cmd2_projection_navigation_cohort c
                     WHERE c.domain=b.domain AND c.consumer_id=b.consumer_id
                       AND c.source_path=b.owner_source_path AND c.record_id=b.owner_record_id
                  )
               UNION
               SELECT d.dependency_path AS cause_path,
                      b.owner_source_path AS owner_source_path,
                      b.owner_record_id AS record_id,'bootstrap'::text AS predecessor_kind,
                      NULL::text AS predecessor_cohort_root
                 FROM cmd2_projection_navigation_bootstrap_dependency d
                 JOIN cmd2_projection_navigation_bootstrap_row b
                   ON b.domain=d.domain AND b.consumer_id=d.consumer_id
                  AND b.owner_source_path=d.owner_source_path AND b.owner_record_id=d.owner_record_id
                WHERE d.domain=$1 AND d.consumer_id=$2 AND b.active
                  AND d.dependency_path=ANY($3)
                  AND b.owner_record_id NOT LIKE 'tos.claim.%'
                  AND NOT EXISTS (
                    SELECT 1 FROM cmd2_projection_navigation_cohort c
                     WHERE c.domain=b.domain AND c.consumer_id=b.consumer_id
                       AND c.source_path=b.owner_source_path AND c.record_id=b.owner_record_id
                  )
             ) prior_ids
             ORDER BY cause_path COLLATE \"C\",owner_source_path COLLATE \"C\",
                      record_id COLLATE \"C\",predecessor_kind COLLATE \"C\"
             LIMIT $4",
            &[&domain, &consumer_id, &changed_source_paths, &row_limit],
        )?;
        if rows.len() > limits.max_current_rows {
            return Err(DurableError::Refused(
                "navigation Claim source-ID owner count budget",
            ));
        }
        let mut expected_record_prior = BTreeSet::new();
        let mut predecessor_roots_by_owner = BTreeMap::<(String, String), Digest256>::new();
        for row in rows {
            let path: String = row.get("cause_path");
            let owner_path: String = row.get("owner_source_path");
            let id: String = row.get("record_id");
            let predecessor_kind: String = row.get("predecessor_kind");
            validate_relative_path(&owner_path)?;
            validate_relative_path(&path)?;
            validate_identifier(
                &id,
                MAX_NAVIGATION_RECORD_ID_BYTES,
                "navigation prior source ID",
            )?;
            let predecessor_root = match predecessor_kind.as_str() {
                "event" => parse_digest(
                    row.get::<_, Option<String>>("predecessor_cohort_root")
                        .ok_or(DurableError::Corrupt(
                            "navigation prior Record cohort root is absent",
                        ))?,
                )?,
                "bootstrap" => navigation_bootstrap_owner_root(
                    tx,
                    domain,
                    consumer_id,
                    &owner_path,
                    &id,
                    limits,
                )?,
                _ => return Err(DurableError::Corrupt("navigation prior Record basis")),
            };
            let owner_key = (owner_path.clone(), id.clone());
            if predecessor_roots_by_owner
                .insert(owner_key, predecessor_root)
                .is_some_and(|previous| previous != predecessor_root)
            {
                return Err(DurableError::Conflict(
                    "navigation prior Record owner has inconsistent predecessor roots",
                ));
            }
            expected_record_prior.insert((path.clone(), owner_path, id.clone(), predecessor_root));
            predecessor_ids_by_path.entry(path).or_default().insert(id);
        }
        let observed_record_prior = record_set
            .prior_owners()
            .iter()
            .map(|prior| {
                (
                    prior.cause_path().as_str().to_owned(),
                    prior.source_path().as_str().to_owned(),
                    prior.record_id().to_owned(),
                    prior.predecessor_cohort_root(),
                )
            })
            .collect::<BTreeSet<_>>();
        if observed_record_prior != expected_record_prior {
            return Err(DurableError::Conflict(
                "Record range predecessor cause paths differ from held projection",
            ));
        }
    }

    if !changed_source_paths.is_empty() {
        let rows = tx.query(
            "SELECT cause_path,claim_stream_path,claim_id,predecessor_kind,
                    predecessor_cohort_root
               FROM (
                 SELECT c.source_path AS cause_path,c.source_path AS claim_stream_path,
                        c.record_id AS claim_id,'event'::text AS predecessor_kind,
                        c.output_root AS predecessor_cohort_root
                   FROM cmd2_projection_navigation_cohort c
                  WHERE c.domain=$1 AND c.consumer_id=$2
                    AND c.source_path=ANY($3)
                    AND c.basis_kind IN (
                      'claim-mutation','claim-dependency-refresh','claim-revision-refresh','claim-range-reconciliation'
                    )
                 UNION
                 SELECT d.dependency_path AS cause_path,c.source_path AS claim_stream_path,
                        c.record_id AS claim_id,'event'::text AS predecessor_kind,
                        c.output_root AS predecessor_cohort_root
                   FROM cmd2_projection_navigation_dependency d
                   JOIN cmd2_projection_navigation_cohort c
                     ON c.domain=d.domain AND c.consumer_id=d.consumer_id
                    AND c.source_path=d.source_path AND c.record_id=d.record_id
                  WHERE d.domain=$1 AND d.consumer_id=$2
                    AND d.dependency_path=ANY($3)
                    AND c.basis_kind IN (
                      'claim-mutation','claim-dependency-refresh','claim-revision-refresh','claim-range-reconciliation'
                    )
                 UNION
                 SELECT b.owner_source_path AS cause_path,
                        b.owner_source_path AS claim_stream_path,
                        b.owner_record_id AS claim_id,'bootstrap'::text AS predecessor_kind,
                        NULL::text AS predecessor_cohort_root
                   FROM cmd2_projection_navigation_bootstrap_row b
                  WHERE b.domain=$1 AND b.consumer_id=$2 AND b.active
                    AND b.owner_source_path=ANY($3)
                    AND b.owner_record_id LIKE 'tos.claim.%'
                    AND NOT EXISTS (
                      SELECT 1 FROM cmd2_projection_navigation_cohort c
                       WHERE c.domain=b.domain AND c.consumer_id=b.consumer_id
                         AND c.source_path=b.owner_source_path
                         AND c.record_id=b.owner_record_id
                    )
                 UNION
                 SELECT d.dependency_path AS cause_path,
                        b.owner_source_path AS claim_stream_path,
                        b.owner_record_id AS claim_id,'bootstrap'::text AS predecessor_kind,
                        NULL::text AS predecessor_cohort_root
                   FROM cmd2_projection_navigation_bootstrap_dependency d
                   JOIN cmd2_projection_navigation_bootstrap_row b
                     ON b.domain=d.domain AND b.consumer_id=d.consumer_id
                    AND b.owner_source_path=d.owner_source_path
                    AND b.owner_record_id=d.owner_record_id
                  WHERE d.domain=$1 AND d.consumer_id=$2 AND b.active
                    AND d.dependency_path=ANY($3)
                    AND b.owner_record_id LIKE 'tos.claim.%'
                    AND NOT EXISTS (
                      SELECT 1 FROM cmd2_projection_navigation_cohort c
                       WHERE c.domain=b.domain AND c.consumer_id=b.consumer_id
                         AND c.source_path=b.owner_source_path
                         AND c.record_id=b.owner_record_id
                    )
               ) prior
              ORDER BY cause_path COLLATE \"C\",claim_stream_path COLLATE \"C\",
                       claim_id COLLATE \"C\",predecessor_kind COLLATE \"C\"
              LIMIT $4",
            &[&domain, &consumer_id, &changed_source_paths, &row_limit],
        )?;
        if rows.len() > limits.max_current_rows {
            return Err(DurableError::Refused(
                "navigation range prior Claim cause count budget",
            ));
        }
        let mut expected_claim_prior = BTreeSet::new();
        let mut claim_prior_roots_by_owner =
            BTreeMap::<(String, String), (String, Digest256)>::new();
        for row in rows {
            let cause_path: String = row.get("cause_path");
            let claim_stream_path: String = row.get("claim_stream_path");
            let claim_id: String = row.get("claim_id");
            let predecessor_kind: String = row.get("predecessor_kind");
            validate_relative_path(&cause_path)?;
            validate_relative_path(&claim_stream_path)?;
            validate_identifier(
                &claim_id,
                MAX_NAVIGATION_RECORD_ID_BYTES,
                "navigation prior Claim ID",
            )?;
            let predecessor_root = match predecessor_kind.as_str() {
                "event" => parse_digest(
                    row.get::<_, Option<String>>("predecessor_cohort_root")
                        .ok_or(DurableError::Corrupt(
                            "navigation prior Claim cohort root is absent",
                        ))?,
                )?,
                "bootstrap" => navigation_bootstrap_owner_root(
                    tx,
                    domain,
                    consumer_id,
                    &claim_stream_path,
                    &claim_id,
                    limits,
                )?,
                _ => return Err(DurableError::Corrupt("navigation prior Claim basis")),
            };
            let owner_key = (claim_stream_path.clone(), claim_id.clone());
            let prior_basis = (predecessor_kind.clone(), predecessor_root);
            if claim_prior_roots_by_owner
                .insert(owner_key, prior_basis.clone())
                .is_some_and(|previous| previous != prior_basis)
            {
                return Err(DurableError::Conflict(
                    "navigation prior Claim owner has inconsistent predecessor roots",
                ));
            }
            expected_claim_prior.insert((
                cause_path,
                claim_stream_path,
                claim_id,
                predecessor_kind,
                predecessor_root,
            ));
        }
        let observed_claim_prior = claim_set
            .manifest()
            .prior_owners()
            .iter()
            .map(|prior| {
                let (kind, root) = match prior.basis {
                    ClaimPriorBasis::EventCohort { cohort_root } => ("event", cohort_root),
                    ClaimPriorBasis::Bootstrap { owner_root } => ("bootstrap", owner_root),
                };
                (
                    prior.cause_path.clone(),
                    prior.claim_stream_path.clone(),
                    prior.claim_id.clone(),
                    kind.to_owned(),
                    root,
                )
            })
            .collect::<BTreeSet<_>>();
        if observed_claim_prior != expected_claim_prior {
            return Err(DurableError::Conflict(
                "Claim range predecessor cause paths differ from held projection",
            ));
        }
    }

    let mut direct_record_ids_by_path = BTreeMap::<String, BTreeSet<String>>::new();
    let mut direct_record_ids = BTreeSet::<String>::new();
    for row in record_set.rows() {
        for cause in row.causes() {
            verify_record_range_mutation_identity(coverage, row.record_id(), cause)?;
            if !matches!(
                cause.kind(),
                RecordCauseKind::DirectCreate | RecordCauseKind::RecordRevision
            ) {
                continue;
            }
            let path = cause.path().as_str().to_owned();
            let id = row.record_id().to_owned();
            validate_relative_path(&path)?;
            validate_identifier(
                &id,
                MAX_NAVIGATION_RECORD_ID_BYTES,
                "navigation direct source ID",
            )?;
            let identity_path = cause
                .mutation_identity()
                .ok_or(DurableError::Conflict(
                    "direct Record cause lacks a mutation identity path",
                ))?
                .source_path()
                .as_str()
                .to_owned();
            direct_record_ids_by_path
                .entry(identity_path)
                .or_default()
                .insert(id.clone());
            direct_record_ids.insert(id);
        }
    }

    for mutation in claim_set.manifest().mutations() {
        verify_claim_range_mutation_identity(coverage, mutation, limits.max_row_bytes)?;
    }

    let mut expected_keys = BTreeSet::new();
    for path in &changed_tos_paths {
        expected_keys.insert((LookupKind::SourcePath, path.clone()));
    }
    let mut changed_ids = direct_record_ids;
    for ids in predecessor_ids_by_path.values() {
        changed_ids.extend(ids.iter().cloned());
    }
    for mutation in claim_set.manifest().mutations() {
        changed_ids.extend(mutation.mutation_claim_ids().iter().cloned());
    }
    for id in changed_ids {
        validate_identifier(
            &id,
            MAX_NAVIGATION_RECORD_ID_BYTES,
            "navigation changed source ID",
        )?;
        expected_keys.insert((LookupKind::TosId, id));
    }
    let actual_keys = closure.lookups().keys().cloned().collect::<BTreeSet<_>>();
    if actual_keys != expected_keys {
        return Err(DurableError::Conflict(
            "navigation Claim lookup keys differ from held source and predecessor proof",
        ));
    }

    let expected_causes = coverage
        .events()
        .iter()
        .filter(|event| event.event_kind == "command")
        .flat_map(|event| {
            event.members.iter().filter_map(move |member| {
                (member.is_source
                    && member.source_path.starts_with("ToS/")
                    && !member.source_path.ends_with("/source-claims.jsonl")
                    && !member.source_path.ends_with("/source-create-receipt.json")
                    && !member.source_path.ends_with("/claim-revision-history.json"))
                .then_some((
                    event.sequence,
                    event.event_id.clone(),
                    member.source_path.clone(),
                    member.sha256,
                ))
            })
        })
        .collect::<BTreeSet<_>>();
    let observed_causes = claim_set
        .manifest()
        .dependency_causes()
        .iter()
        .map(|cause| {
            (
                cause.event_sequence(),
                cause.event_id().to_owned(),
                cause.changed_dependency_path().as_str().to_owned(),
                cause.changed_dependency_sha256(),
            )
        })
        .collect::<BTreeSet<_>>();
    if expected_causes != observed_causes {
        return Err(DurableError::Conflict(
            "navigation Claim dependency causes differ from exact changed ToS members",
        ));
    }

    let mut predecessor_claim_ids_by_path = BTreeMap::<String, BTreeSet<String>>::new();
    for owner in claim_set.manifest().prior_owners() {
        predecessor_claim_ids_by_path
            .entry(owner.cause_path.clone())
            .or_default()
            .insert(owner.claim_id.clone());
    }
    for cause in claim_set.manifest().dependency_causes() {
        let path = cause.changed_dependency_path().as_str();
        let source_observation = closure
            .lookups()
            .get(&(LookupKind::SourcePath, path.to_owned()))
            .ok_or(DurableError::Conflict(
                "navigation Claim dependency lacks its exact source-path lookup",
            ))?;
        let mut current_ids = source_observation
            .rows
            .iter()
            .map(|row| row.claim_id.clone())
            .collect::<BTreeSet<_>>();
        if let Some(ids) = predecessor_ids_by_path.get(path) {
            for id in ids {
                let observation = closure
                    .lookups()
                    .get(&(LookupKind::TosId, id.clone()))
                    .ok_or(DurableError::Conflict(
                        "navigation Claim dependency lacks a predecessor ID lookup",
                    ))?;
                current_ids.extend(observation.rows.iter().map(|row| row.claim_id.clone()));
            }
        }
        if let Some(ids) = direct_record_ids_by_path.get(path) {
            for id in ids {
                let observation = closure
                    .lookups()
                    .get(&(LookupKind::TosId, id.clone()))
                    .ok_or(DurableError::Conflict(
                        "navigation Claim dependency lacks a direct mutation ID lookup",
                    ))?;
                current_ids.extend(observation.rows.iter().map(|row| row.claim_id.clone()));
            }
        }
        let predecessor_ids = predecessor_claim_ids_by_path
            .get(path)
            .cloned()
            .unwrap_or_default();
        let current_ids = current_ids.into_iter().collect::<Vec<_>>();
        let predecessor_ids = predecessor_ids.into_iter().collect::<Vec<_>>();
        if current_ids.as_slice() != cause.current_claim_ids()
            || predecessor_ids.as_slice() != cause.predecessor_claim_ids()
        {
            return Err(DurableError::Conflict(
                "navigation Claim dependency reverse owners differ from exact range closure",
            ));
        }
    }
    Ok(())
}

fn verify_range_claim_closure_binding(
    coverage: &DurableNavigationEventRangeCoverageV1,
    closure: &crate::source_cohort::revision::SourceRevisionClaimNavigationClosure,
    domain: &str,
    store_id: [u8; 16],
    source_generation: Option<i64>,
    expected_audit_generation: u64,
    rights_version: u64,
    rule_version: u64,
    contract: Digest256,
    job_id: &str,
    job_fence: u64,
) -> DurableResult<()> {
    let identity = closure.selection_identity();
    let expected_generation =
        source_generation
            .map(as_u64)
            .transpose()?
            .ok_or(DurableError::Conflict(
                "Claim source range lacks a held source generation",
            ))?;
    if closure.domain() != domain
        || identity.head_seq != coverage.held_source_head_sequence()
        || closure.as_of_sequence() != coverage.through_sequence()
        || closure.baseline_sequence() > coverage.through_sequence()
        || closure.held_source_head_sequence() != coverage.held_source_head_sequence()
        || identity.store_id != store_id
        || identity.source_generation != expected_generation
        || identity.audit_generation != expected_audit_generation
        || identity.rights_version != rights_version
        || identity.rule_version != rule_version
        || identity.contract != contract
        || identity.job_id_digest != Digest256::of_bytes(job_id.as_bytes())
        || identity.job_fence != job_fence
    {
        return Err(DurableError::Conflict(
            "navigation Claim range closure identity differs from held source cut",
        ));
    }
    Ok(())
}

fn verify_range_terminal_members(
    coverage: &DurableNavigationEventRangeCoverageV1,
    context: &DurableNavigationEventContextV1,
) -> DurableResult<()> {
    let terminal = coverage
        .event(coverage.through_sequence())
        .ok_or(DurableError::Conflict(
            "navigation range terminal event proof absent",
        ))?;
    let empty_non_source_context = context.members.is_empty()
        && context.selected_sources.is_empty()
        && terminal.members.iter().all(|member| !member.is_source);
    if empty_non_source_context {
        return Ok(());
    }
    if context.members.len() != terminal.members.len() {
        return Err(DurableError::Conflict(
            "navigation terminal context member count differs from range proof",
        ));
    }
    for expected in &terminal.members {
        let observed = context
            .members
            .iter()
            .find(|member| member.source_path == expected.source_path)
            .ok_or(DurableError::Conflict(
                "navigation terminal context member is absent",
            ))?;
        if observed.revision != Some(expected.revision)
            || observed.commit_sequence != Some(coverage.through_sequence())
            || observed.is_source != expected.is_source
            || observed.content_length != Some(expected.size_bytes)
            || observed.content_digest != Some(expected.sha256)
        {
            return Err(DurableError::Conflict(
                "navigation terminal context member differs from exact range proof",
            ));
        }
    }
    Ok(())
}

fn verify_range_dependencies(
    dependencies: &[tos_compiler::source_navigation_event::NavigationDependencyV1],
    context: &DurableNavigationEventContextV1,
    limits: DurableNavigationEventLimits,
) -> DurableResult<()> {
    let selected = context
        .selected_sources
        .iter()
        .map(|source| (source.source_path.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    for dependency in dependencies {
        let source =
            selected
                .get(dependency.path.as_str())
                .copied()
                .ok_or(DurableError::Conflict(
                    "navigation range owner dependency is outside the held current cut",
                ))?;
        match &dependency.state {
            tos_compiler::source_navigation_event::NavigationDependencyStateV1::Present {
                sha256,
                size_bytes,
            } => {
                let raw = source
                    .exact_source_bytes
                    .as_deref()
                    .ok_or(DurableError::Conflict(
                        "navigation range present dependency lacks exact bytes",
                    ))?;
                let limit = if dependency.role
                    == tos_compiler::source_navigation_event::NavigationDependencyRoleV1::Event
                {
                    limits.max_total_bytes
                } else {
                    limits.max_row_bytes as u64
                };
                if !source.is_source
                    || source.content_digest != Some(*sha256)
                    || source.content_length != Some(*size_bytes)
                    || raw.len() as u64 != *size_bytes
                    || Digest256::of_bytes(raw) != *sha256
                    || *size_bytes > limit
                {
                    return Err(DurableError::Conflict(
                        "navigation range dependency differs from exact current source bytes",
                    ));
                }
            }
            tos_compiler::source_navigation_event::NavigationDependencyStateV1::Absent {
                ..
            } if source.is_source => {
                return Err(DurableError::Conflict(
                    "navigation range absent dependency exists in current source cut",
                ));
            }
            tos_compiler::source_navigation_event::NavigationDependencyStateV1::Absent {
                ..
            } => {}
        }
    }
    Ok(())
}

fn range_dependency_bytes(
    dependencies: &[tos_compiler::source_navigation_event::NavigationDependencyV1],
    limits: DurableNavigationEventLimits,
) -> DurableResult<u64> {
    let source_path = dependencies
        .iter()
        .find(|dependency| {
            dependency.role
                == tos_compiler::source_navigation_event::NavigationDependencyRoleV1::Record
        })
        .map(|dependency| dependency.path.as_str())
        .ok_or(DurableError::Invalid(
            "navigation range owner source dependency absent",
        ))?;
    super::validate_navigation_dependency_envelope(dependencies, source_path, limits)
}

fn range_record_owner(
    cohort: &tos_compiler::source_navigation_event::NavigationRecordEventCohortV1,
) -> super::NavigationEventOwnerV2 {
    super::NavigationEventOwnerV2 {
        event_id: cohort.event_id().to_owned(),
        event_sequence: cohort.event_sequence(),
        source_event_id: cohort.source_event_id().to_owned(),
        source_event_version: cohort.source_event_version(),
        predecessor_source_revision: cohort.predecessor_source_revision(),
        source_revision: cohort.source_revision(),
        basis_kind: "range-reconciliation".to_owned(),
        basis_dependency_path: None,
        basis_dependency_sha256: None,
        basis_dependency_causes_json: None,
        basis_dependency_causes_root: None,
        basis_history_path: None,
        basis_history_sha256: None,
        basis_claim_id: None,
        basis_revision_proof_root: None,
        claim_owner_inventory_root: None,
        claim_stream_rows_scanned: None,
        claim_event_basis_root: None,
        claim_navigation_manifest_root: None,
        source_path: cohort.source_path().clone(),
        record_id: cohort.record_id().to_owned(),
        predecessor_cohort_root: cohort.predecessor_cohort_root(),
        dependency_root: cohort.dependency_root(),
        input_root: cohort.input_root(),
        output_root: cohort.output_root(),
        dependencies: cohort.dependencies().to_vec(),
    }
}

fn range_claim_owner(
    cohort: &tos_compiler::source_navigation_claim_event::NavigationClaimEventRangeOwnerCohortV1,
    integrity: &tos_compiler::source_navigation_claim_event::NavigationClaimEventRangeOwnerIntegrityV1<'_>,
) -> super::NavigationEventOwnerV2 {
    let terminal = &integrity.terminal;
    super::NavigationEventOwnerV2 {
        event_id: cohort.terminal_event_id().to_owned(),
        event_sequence: cohort.through_sequence(),
        source_event_id: terminal.source_event_id.to_owned(),
        source_event_version: terminal.source_event_version,
        predecessor_source_revision: terminal.predecessor_source_revision,
        source_revision: terminal.source_revision,
        basis_kind: "claim-range-reconciliation".to_owned(),
        basis_dependency_path: None,
        basis_dependency_sha256: None,
        basis_dependency_causes_json: None,
        basis_dependency_causes_root: None,
        basis_history_path: None,
        basis_history_sha256: None,
        basis_claim_id: None,
        basis_revision_proof_root: None,
        claim_owner_inventory_root: Some(terminal.owner_inventory_root),
        claim_stream_rows_scanned: Some(cohort.claim_stream_rows_scanned()),
        claim_event_basis_root: Some(terminal.event_basis_root),
        claim_navigation_manifest_root: Some(cohort.range_manifest_root()),
        source_path: cohort.source_path().clone(),
        record_id: cohort.record_id().to_owned(),
        predecessor_cohort_root: cohort.predecessor_cohort_root(),
        dependency_root: terminal.dependency_root,
        input_root: terminal.input_root,
        output_root: terminal.output_root,
        dependencies: cohort.dependencies().to_vec(),
    }
}

fn range_terminal_manifest(
    coverage: &DurableNavigationEventRangeCoverageV1,
    record_root: Digest256,
    record_manifest: &Value,
    claim_root: Digest256,
    claim_manifest: &Value,
    owners: &[(
        super::NavigationEventOwnerV2,
        super::PersistedRows,
        Option<u64>,
        Option<Digest256>,
    )],
    byte_limit: usize,
) -> DurableResult<(Vec<u8>, Digest256)> {
    let cohorts = owners
        .iter()
        .map(|(owner, _, source_line, trace)| {
            json!({
                "source_path": owner.source_path.as_str(),
                "record_id": owner.record_id,
                "owner_kind": if owner.basis_kind.starts_with("claim-") { "claim" } else { "record" },
                "basis_kind": owner.basis_kind,
                "source_event_id": owner.source_event_id,
                "source_event_version": owner.source_event_version,
                "predecessor_source_revision": owner.predecessor_source_revision.to_hex(),
                "source_revision": owner.source_revision.to_hex(),
                "predecessor_cohort_root": owner.predecessor_cohort_root.map(Digest256::to_hex),
                "input_root": owner.input_root.to_hex(),
                "dependency_root": owner.dependency_root.to_hex(),
                "output_root": owner.output_root.to_hex(),
                "claim_owner_inventory_root": owner.claim_owner_inventory_root.map(Digest256::to_hex),
                "claim_stream_rows_scanned": owner.claim_stream_rows_scanned,
                "claim_event_basis_root": owner.claim_event_basis_root.map(Digest256::to_hex),
                "claim_navigation_manifest_root": owner.claim_navigation_manifest_root.map(Digest256::to_hex),
                "claim_source_line": source_line,
                "claim_trace_sha256": trace.map(Digest256::to_hex),
            })
        })
        .collect::<Vec<_>>();
    let raw = super::canonical_value(
        &json!({
            "schema": "tos_source_navigation_event_range_terminal_cohort_set_v1",
            "cursor_before": coverage.cursor_before(),
            "through_sequence": coverage.through_sequence(),
            "held_source_head_sequence": coverage.held_source_head_sequence(),
            "terminal_event_id": coverage.terminal_event_id(),
            "coverage_root": coverage.coverage_root().to_hex(),
            "coverage_manifest": String::from_utf8_lossy(coverage.manifest_raw()).as_ref(),
            "record_range_root": record_root.to_hex(),
            "record_range_manifest": record_manifest,
            "claim_range_root": claim_root.to_hex(),
            "claim_range_manifest": claim_manifest,
            "cohorts": cohorts,
        }),
        byte_limit,
    )?;
    let root = Digest256::of_bytes(&raw);
    Ok((raw, root))
}

/// Re-open the persisted range terminal set and re-derive its canonical roots
/// before the global reader accepts terminal owner metadata. The compiler
/// manifests remain the authority for their own Record/Claim members; this
/// outer verifier binds both component roots to the exact event range and the
/// durable terminal owner rows.
#[allow(clippy::too_many_arguments)]
pub(super) fn verify_navigation_event_range_terminal_manifest(
    raw: &[u8],
    stored_root: Digest256,
    stored_count: u32,
    cursor_before: u64,
    through_sequence: u64,
    held_source_head_sequence: u64,
    terminal_event_id: &str,
    range_root: Digest256,
    range_manifest_raw: &[u8],
    input_root: Digest256,
    dependency_root: Digest256,
    output_root: Digest256,
    byte_limit: usize,
) -> DurableResult<super::NavigationEventManifestV2> {
    if raw.is_empty()
        || raw.len() > byte_limit
        || range_manifest_raw.is_empty()
        || range_manifest_raw.len() > byte_limit
        || Digest256::of_bytes(raw) != stored_root
        || Digest256::of_bytes(range_manifest_raw) != range_root
        || input_root != range_root
        || output_root != stored_root
        || cursor_before >= through_sequence
        || through_sequence > held_source_head_sequence
    {
        return Err(DurableError::Corrupt(
            "stored navigation range roots or bounds differ",
        ));
    }
    let value = super::strict_json_value(raw, byte_limit)?;
    if value.as_object().is_none_or(|object| object.len() != 12)
        || value.get("schema").and_then(Value::as_str)
            != Some("tos_source_navigation_event_range_terminal_cohort_set_v1")
        || value.get("cursor_before").and_then(Value::as_u64) != Some(cursor_before)
        || value.get("through_sequence").and_then(Value::as_u64) != Some(through_sequence)
        || value
            .get("held_source_head_sequence")
            .and_then(Value::as_u64)
            != Some(held_source_head_sequence)
        || value.get("terminal_event_id").and_then(Value::as_str) != Some(terminal_event_id)
        || value.get("coverage_root").and_then(Value::as_str) != Some(range_root.to_hex().as_str())
        || value.get("coverage_manifest").and_then(Value::as_str)
            != std::str::from_utf8(range_manifest_raw).ok()
        || super::canonical_value(&value, byte_limit)? != raw
    {
        return Err(DurableError::Corrupt(
            "stored navigation range terminal manifest is not canonical",
        ));
    }

    let range_value = super::strict_json_value(range_manifest_raw, byte_limit)?;
    if super::canonical_value(&range_value, byte_limit)? != range_manifest_raw {
        return Err(DurableError::Corrupt(
            "stored navigation range coverage manifest is not canonical",
        ));
    }
    verify_stored_range_coverage(
        &range_value,
        cursor_before,
        through_sequence,
        terminal_event_id,
    )?;

    let record_root = json_digest_field(&value, "record_range_root")?;
    let claim_root = json_digest_field(&value, "claim_range_root")?;
    let record_manifest = value
        .get("record_range_manifest")
        .ok_or(DurableError::Corrupt(
            "stored navigation Record range manifest absent",
        ))?;
    let claim_manifest = value
        .get("claim_range_manifest")
        .ok_or(DurableError::Corrupt(
            "stored navigation Claim range manifest absent",
        ))?;
    if Digest256::of_bytes(&super::canonical_value(record_manifest, byte_limit)?) != record_root
        || Digest256::of_bytes(&super::canonical_value(claim_manifest, byte_limit)?) != claim_root
    {
        return Err(DurableError::Corrupt(
            "stored navigation range component root differs",
        ));
    }
    verify_component_range_binding(
        record_manifest,
        "tos_source_navigation_record_event_range_cohort_v1",
        cursor_before,
        through_sequence,
        terminal_event_id,
        range_root,
    )?;
    verify_component_range_binding(
        claim_manifest,
        "tos_source_navigation_claim_event_range_manifest_v1",
        cursor_before,
        through_sequence,
        terminal_event_id,
        range_root,
    )?;
    if record_manifest
        .get("claim_range_root")
        .and_then(Value::as_str)
        != Some(claim_root.to_hex().as_str())
    {
        return Err(DurableError::Corrupt(
            "stored Record range manifest Claim binding differs",
        ));
    }

    let record_keys = record_manifest
        .get("rows")
        .and_then(Value::as_array)
        .ok_or(DurableError::Corrupt("stored Record range rows absent"))?
        .iter()
        .map(range_owner_key)
        .collect::<DurableResult<Vec<_>>>()?;
    if record_keys.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(DurableError::Corrupt(
            "stored Record range owner keys are not canonical",
        ));
    }

    let claim_inventory_root = json_digest_field(claim_manifest, "terminal_inventory_root")?;
    let terminal_owner_ids = claim_manifest
        .get("terminal_owner_ids")
        .and_then(Value::as_array)
        .ok_or(DurableError::Corrupt("stored Claim range owner IDs absent"))?
        .iter()
        .map(|value| {
            let id = value
                .as_str()
                .filter(|id| id.starts_with("tos.claim."))
                .ok_or(DurableError::Corrupt("stored Claim range owner ID invalid"))?;
            validate_identifier(
                id,
                MAX_NAVIGATION_RECORD_ID_BYTES,
                "stored Claim range owner ID",
            )?;
            Ok(id.to_owned())
        })
        .collect::<DurableResult<Vec<_>>>()?;
    if terminal_owner_ids.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(DurableError::Corrupt(
            "stored Claim range owner IDs are not canonical",
        ));
    }
    verify_empty_claim_manifest_evidence(
        claim_manifest,
        &range_value,
        terminal_owner_ids.is_empty(),
    )?;
    let terminal_streams_value = claim_manifest
        .get("terminal_streams")
        .and_then(Value::as_array)
        .ok_or(DurableError::Corrupt("stored Claim range streams absent"))?;
    let mut terminal_streams = Vec::with_capacity(terminal_streams_value.len());
    let mut claim_keys = Vec::new();
    let mut prior_stream_path: Option<String> = None;
    for stream in terminal_streams_value {
        if stream.as_object().is_none_or(|object| object.len() != 4) {
            return Err(DurableError::Corrupt("stored Claim range stream shape"));
        }
        let source_path =
            stream
                .get("source_path")
                .and_then(Value::as_str)
                .ok_or(DurableError::Corrupt(
                    "stored Claim range stream path absent",
                ))?;
        validate_relative_path(source_path)
            .map_err(|_| DurableError::Corrupt("stored Claim range stream path invalid"))?;
        if prior_stream_path
            .as_deref()
            .is_some_and(|previous| previous >= source_path)
        {
            return Err(DurableError::Corrupt(
                "stored Claim range streams are not canonical",
            ));
        }
        prior_stream_path = Some(source_path.to_owned());
        let owner_claim_ids = stream
            .get("owner_ids")
            .and_then(Value::as_array)
            .ok_or(DurableError::Corrupt(
                "stored Claim stream owner IDs absent",
            ))?
            .iter()
            .map(|value| {
                let id = value
                    .as_str()
                    .filter(|id| id.starts_with("tos.claim."))
                    .ok_or(DurableError::Corrupt(
                        "stored Claim stream owner ID invalid",
                    ))?;
                validate_identifier(
                    id,
                    MAX_NAVIGATION_RECORD_ID_BYTES,
                    "stored Claim stream owner ID",
                )?;
                Ok(id.to_owned())
            })
            .collect::<DurableResult<Vec<_>>>()?;
        if owner_claim_ids.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(DurableError::Corrupt(
                "stored Claim stream owner IDs are not canonical",
            ));
        }
        for id in &owner_claim_ids {
            claim_keys.push((source_path.to_owned(), id.clone()));
        }
        terminal_streams.push(super::NavigationEventManifestClaimStreamV1 {
            source_path: source_path.to_owned(),
            stream_sha256: json_digest_field(stream, "stream_sha256")?,
            rows_scanned: stream.get("rows_scanned").and_then(Value::as_u64).ok_or(
                DurableError::Corrupt("stored Claim stream row count absent"),
            )?,
            owner_claim_ids,
        });
    }
    claim_keys.sort();
    if claim_keys.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(DurableError::Corrupt(
            "stored Claim range owner keys are not canonical",
        ));
    }
    let mut expected_claim_ids = claim_keys
        .iter()
        .map(|(_, id)| id.clone())
        .collect::<Vec<_>>();
    expected_claim_ids.sort();
    expected_claim_ids.dedup();
    if expected_claim_ids != terminal_owner_ids {
        return Err(DurableError::Corrupt(
            "stored Claim range streams differ from terminal owner IDs",
        ));
    }
    let stream_predecessors = read_stored_claim_stream_predecessors(claim_manifest, &claim_keys)?;

    let cohorts_value =
        value
            .get("cohorts")
            .and_then(Value::as_array)
            .ok_or(DurableError::Corrupt(
                "stored navigation range cohorts absent",
            ))?;
    if cohorts_value.len() != stored_count as usize {
        return Err(DurableError::Corrupt(
            "stored navigation range cohort count differs",
        ));
    }
    let mut cohorts = BTreeMap::new();
    let mut previous_key: Option<(String, String)> = None;
    let mut actual_record_keys = Vec::new();
    let mut actual_claim_keys = Vec::new();
    for cohort in cohorts_value {
        if cohort.as_object().is_none_or(|object| object.len() != 18) {
            return Err(DurableError::Corrupt(
                "stored navigation range cohort shape",
            ));
        }
        let source_path = cohort
            .get("source_path")
            .and_then(Value::as_str)
            .ok_or(DurableError::Corrupt("stored range owner path absent"))?;
        let record_id = cohort
            .get("record_id")
            .and_then(Value::as_str)
            .ok_or(DurableError::Corrupt("stored range owner ID absent"))?;
        validate_relative_path(source_path)
            .map_err(|_| DurableError::Corrupt("stored range owner path invalid"))?;
        validate_identifier(
            record_id,
            MAX_NAVIGATION_RECORD_ID_BYTES,
            "stored range owner ID",
        )?;
        let owner_kind = cohort
            .get("owner_kind")
            .and_then(Value::as_str)
            .ok_or(DurableError::Corrupt("stored range owner kind absent"))?;
        let basis_kind = cohort
            .get("basis_kind")
            .and_then(Value::as_str)
            .ok_or(DurableError::Corrupt("stored range basis kind absent"))?;
        if (owner_kind, basis_kind) != ("record", "range-reconciliation")
            && (owner_kind, basis_kind) != ("claim", "claim-range-reconciliation")
        {
            return Err(DurableError::Corrupt("stored range owner basis differs"));
        }
        let key = (source_path.to_owned(), record_id.to_owned());
        if previous_key
            .as_ref()
            .is_some_and(|previous| previous >= &key)
        {
            return Err(DurableError::Corrupt(
                "stored navigation range cohorts are not canonical",
            ));
        }
        previous_key = Some(key.clone());
        if owner_kind == "record" {
            actual_record_keys.push(key.clone());
        } else {
            actual_claim_keys.push(key.clone());
        }
        let claim_owner_inventory_root =
            optional_digest_field(cohort, "claim_owner_inventory_root")?;
        let claim_stream_rows_scanned = cohort
            .get("claim_stream_rows_scanned")
            .and_then(Value::as_u64);
        let claim_event_basis_root = optional_digest_field(cohort, "claim_event_basis_root")?;
        let claim_navigation_manifest_root =
            optional_digest_field(cohort, "claim_navigation_manifest_root")?;
        let claim_source_line = cohort.get("claim_source_line").and_then(Value::as_u64);
        let claim_trace_sha256 = optional_digest_field(cohort, "claim_trace_sha256")?;
        if owner_kind == "claim" {
            if claim_owner_inventory_root != Some(claim_inventory_root)
                || claim_stream_rows_scanned.is_none()
                || claim_event_basis_root.is_none()
                || claim_navigation_manifest_root != Some(claim_root)
                || claim_source_line.is_none()
                || claim_trace_sha256.is_none()
                || claim_keys.binary_search(&key).is_err()
            {
                return Err(DurableError::Corrupt(
                    "stored Claim range owner binding differs",
                ));
            }
        } else if claim_owner_inventory_root.is_some()
            || claim_stream_rows_scanned.is_some()
            || claim_event_basis_root.is_some()
            || claim_navigation_manifest_root.is_some()
            || claim_source_line.is_some()
            || claim_trace_sha256.is_some()
        {
            return Err(DurableError::Corrupt(
                "stored Record range owner has Claim fields",
            ));
        }
        let predecessor_source_revision = json_digest_field(cohort, "predecessor_source_revision")?;
        let source_event_id = cohort
            .get("source_event_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or(DurableError::Corrupt(
                "stored range source event ID is absent",
            ))?
            .to_owned();
        let source_event_version = cohort
            .get("source_event_version")
            .and_then(Value::as_u64)
            .filter(|version| *version != 0)
            .ok_or(DurableError::Corrupt(
                "stored range source event version is absent",
            ))?;
        let predecessor_cohort_root = optional_digest_field(cohort, "predecessor_cohort_root")?;
        if owner_kind == "claim"
            && let Some(predecessors) = &stream_predecessors
            && predecessors.get(&key).copied().flatten() != predecessor_cohort_root
        {
            return Err(DurableError::Corrupt(
                "stored Claim cohort predecessor differs from stream inventory",
            ));
        }
        let manifest_cohort = super::NavigationEventManifestCohortV2 {
            source_path: source_path.to_owned(),
            record_id: record_id.to_owned(),
            owner_kind: owner_kind.to_owned(),
            basis_kind: basis_kind.to_owned(),
            basis_dependency_path: None,
            basis_dependency_sha256: None,
            basis_dependency_causes: Vec::new(),
            basis_dependency_causes_root: None,
            basis_history_path: None,
            basis_history_sha256: None,
            basis_claim_id: None,
            basis_revision_proof_root: None,
            claim_owner_inventory_root,
            claim_stream_rows_scanned,
            claim_event_basis_root,
            claim_navigation_manifest_root,
            claim_source_line,
            claim_trace_sha256,
            source_event_id: Some(source_event_id),
            source_event_version: Some(source_event_version),
            predecessor_source_revision: Some(predecessor_source_revision),
            predecessor_cohort_root,
            source_revision: json_digest_field(cohort, "source_revision")?,
            input_root: json_digest_field(cohort, "input_root")?,
            dependency_root: json_digest_field(cohort, "dependency_root")?,
            output_root: json_digest_field(cohort, "output_root")?,
        };
        if cohorts.insert(key, manifest_cohort).is_some() {
            return Err(DurableError::Corrupt(
                "stored navigation range repeats an owner",
            ));
        }
    }
    if actual_record_keys != record_keys || actual_claim_keys != claim_keys {
        return Err(DurableError::Corrupt(
            "stored range owner rows differ from component manifests",
        ));
    }
    let expected_dependency_root = navigation_range_dependency_root(record_root, claim_root);
    if expected_dependency_root != dependency_root {
        return Err(DurableError::Corrupt(
            "stored navigation range dependency root differs",
        ));
    }
    Ok(super::NavigationEventManifestV2 {
        cohorts,
        claim_set: None,
        claim_refresh_set: None,
        claim_empty_closure: None,
        claim_range_set: Some(super::NavigationEventManifestClaimRangeSetV1 {
            range_manifest_root: claim_root,
            owner_inventory_root: claim_inventory_root,
            terminal_owner_ids,
            terminal_streams,
        }),
    })
}

fn verify_component_range_binding(
    value: &Value,
    schema: &str,
    cursor_before: u64,
    through_sequence: u64,
    terminal_event_id: &str,
    range_root: Digest256,
) -> DurableResult<()> {
    let expected_fields = match schema {
        "tos_source_navigation_record_event_range_cohort_v1" => 11,
        "tos_source_navigation_claim_event_range_manifest_v1" => {
            if value.get("stream_predecessors").is_some() {
                14
            } else {
                13
            }
        }
        _ => {
            return Err(DurableError::Corrupt(
                "stored navigation range component schema",
            ));
        }
    };
    if value
        .as_object()
        .is_none_or(|object| object.len() != expected_fields)
        || value.get("schema").and_then(Value::as_str) != Some(schema)
        || value.get("cursor_before").and_then(Value::as_u64) != Some(cursor_before)
        || value.get("through_sequence").and_then(Value::as_u64) != Some(through_sequence)
        || value.get("terminal_event_id").and_then(Value::as_str) != Some(terminal_event_id)
        || value.get("coverage_root").and_then(Value::as_str) != Some(range_root.to_hex().as_str())
    {
        return Err(DurableError::Corrupt(
            "stored navigation range component binding differs",
        ));
    }
    Ok(())
}

/// V1 manifests written before the complete stream inventory remain readable.
/// The additive field is accepted only as the exact, canonical typed owner set;
/// arbitrary extra fields do not change the accepted component shape.
fn read_stored_claim_stream_predecessors(
    manifest: &Value,
    claim_keys: &[(String, String)],
) -> DurableResult<Option<BTreeMap<(String, String), Option<Digest256>>>> {
    let Some(value) = manifest.get("stream_predecessors") else {
        return Ok(None);
    };
    let rows = value.as_array().ok_or(DurableError::Corrupt(
        "stored Claim stream predecessor shape",
    ))?;
    if rows.len() > claim_keys.len() {
        return Err(DurableError::Corrupt(
            "stored Claim stream predecessor count",
        ));
    }
    let mut predecessors = BTreeMap::new();
    let mut bases = BTreeMap::new();
    let mut previous = None;
    for row in rows {
        let object =
            row.as_object()
                .filter(|object| object.len() == 3)
                .ok_or(DurableError::Corrupt(
                    "stored Claim stream predecessor row shape",
                ))?;
        let key = (
            object
                .get("claim_stream_path")
                .and_then(Value::as_str)
                .ok_or(DurableError::Corrupt("stored Claim predecessor path"))?
                .to_owned(),
            object
                .get("claim_id")
                .and_then(Value::as_str)
                .ok_or(DurableError::Corrupt("stored Claim predecessor ID"))?
                .to_owned(),
        );
        if previous.as_ref().is_some_and(|prior| prior >= &key)
            || claim_keys.binary_search(&key).is_err()
        {
            return Err(DurableError::Corrupt(
                "stored Claim stream predecessor order or membership",
            ));
        }
        previous = Some(key.clone());
        let basis = object
            .get("basis")
            .ok_or(DurableError::Corrupt("stored Claim predecessor basis"))?;
        if basis.as_object().is_none_or(|object| object.len() != 2) {
            return Err(DurableError::Corrupt(
                "stored Claim predecessor basis shape",
            ));
        }
        let root = json_digest_field(basis, "root")?;
        if root == Digest256::of_bytes(&[]) {
            return Err(DurableError::Corrupt(
                "stored Claim predecessor root is empty",
            ));
        }
        let event_root = match basis.get("kind").and_then(Value::as_str) {
            Some("event-cohort") => Some(root),
            Some("bootstrap") => None,
            _ => return Err(DurableError::Corrupt("stored Claim predecessor basis kind")),
        };
        bases.insert(key.clone(), basis);
        predecessors.insert(key, event_root);
    }
    let causal = manifest
        .get("prior_owners")
        .and_then(Value::as_array)
        .ok_or(DurableError::Corrupt(
            "stored Claim causal predecessors absent",
        ))?;
    for row in causal {
        let key = (
            row.get("claim_stream_path")
                .and_then(Value::as_str)
                .ok_or(DurableError::Corrupt(
                    "stored Claim causal predecessor path",
                ))?
                .to_owned(),
            row.get("claim_id")
                .and_then(Value::as_str)
                .ok_or(DurableError::Corrupt("stored Claim causal predecessor ID"))?
                .to_owned(),
        );
        if bases.get(&key).copied() != row.get("basis") {
            return Err(DurableError::Corrupt(
                "stored Claim causal predecessor differs from stream inventory",
            ));
        }
    }
    Ok(Some(predecessors))
}

fn verify_empty_claim_manifest_evidence(
    claim_manifest: &Value,
    range_manifest: &Value,
    empty: bool,
) -> DurableResult<()> {
    let evidence = claim_manifest
        .get("empty_owner_evidence")
        .ok_or(DurableError::Corrupt(
            "stored Claim range empty-owner evidence absent",
        ))?;
    if !empty {
        if !evidence.is_null() {
            return Err(DurableError::Corrupt(
                "stored nonempty Claim range has empty-owner evidence",
            ));
        }
        return Ok(());
    }
    let evidence = evidence.as_object().ok_or(DurableError::Corrupt(
        "stored empty Claim range evidence shape",
    ))?;
    match evidence.get("kind").and_then(Value::as_str) {
        Some("rights-rule-only") => {
            if evidence.len() != 1
                || range_manifest
                    .get("events")
                    .and_then(Value::as_array)
                    .is_none_or(|events| {
                        events.is_empty()
                            || events.iter().any(|event| {
                                !matches!(
                                    event.get("event_kind").and_then(Value::as_str),
                                    Some("rights" | "rule")
                                ) || event
                                    .get("members")
                                    .and_then(Value::as_array)
                                    .is_none_or(|members| !members.is_empty())
                            })
                    })
            {
                return Err(DurableError::Corrupt(
                    "stored Claim rights/rule empty set has source events",
                ));
            }
        }
        Some("source-closure") => {
            if evidence.len() != 3 {
                return Err(DurableError::Corrupt(
                    "stored Claim source-closure evidence shape",
                ));
            }
            let closure_root = json_digest_field(&Value::Object(evidence.clone()), "closure_root")?;
            if closure_root == Digest256::of_bytes(&[]) {
                return Err(DurableError::Corrupt(
                    "stored Claim source-closure root is empty",
                ));
            }
            let lookup_keys = evidence
                .get("lookup_keys")
                .and_then(Value::as_array)
                .ok_or(DurableError::Corrupt(
                    "stored Claim source-closure lookups absent",
                ))?;
            if lookup_keys.is_empty() {
                return Err(DurableError::Corrupt(
                    "stored Claim source-closure lookups are empty",
                ));
            }
            let mut prior_lookup: Option<(String, String)> = None;
            let mut source_path_lookups = BTreeSet::new();
            for lookup in lookup_keys {
                if lookup.as_object().is_none_or(|object| object.len() != 2) {
                    return Err(DurableError::Corrupt(
                        "stored Claim source-closure lookup shape",
                    ));
                }
                let kind = lookup
                    .get("kind")
                    .and_then(Value::as_str)
                    .filter(|kind| matches!(*kind, "source-path" | "tos-id"))
                    .ok_or(DurableError::Corrupt(
                        "stored Claim source-closure lookup kind",
                    ))?;
                let key = lookup
                    .get("key")
                    .and_then(Value::as_str)
                    .filter(|key| !key.is_empty())
                    .ok_or(DurableError::Corrupt(
                        "stored Claim source-closure lookup key",
                    ))?;
                match kind {
                    "source-path" => {
                        validate_relative_path(key).map_err(|_| {
                            DurableError::Corrupt("stored Claim source-closure path lookup invalid")
                        })?;
                        if !key.starts_with("ToS/") || !source_path_lookups.insert(key.to_owned()) {
                            return Err(DurableError::Corrupt(
                                "stored Claim source-closure path lookup is not canonical",
                            ));
                        }
                    }
                    "tos-id" => validate_identifier(
                        key,
                        MAX_NAVIGATION_RECORD_ID_BYTES,
                        "stored Claim source-closure ID lookup",
                    )?,
                    _ => unreachable!(),
                }
                let key = (kind.to_owned(), key.to_owned());
                if prior_lookup
                    .as_ref()
                    .is_some_and(|previous| previous >= &key)
                {
                    return Err(DurableError::Corrupt(
                        "stored Claim source-closure lookups are not canonical",
                    ));
                }
                prior_lookup = Some(key);
            }
            let events = range_manifest
                .get("events")
                .and_then(Value::as_array)
                .ok_or(DurableError::Corrupt(
                    "stored Claim source-closure events absent",
                ))?;
            let has_command = events
                .iter()
                .any(|event| event.get("event_kind").and_then(Value::as_str) == Some("command"));
            let expected_paths = events
                .iter()
                .flat_map(|event| {
                    event
                        .get("members")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                })
                .filter(|member| {
                    member.get("is_source").and_then(Value::as_bool) == Some(true)
                        && member
                            .get("source_path")
                            .and_then(Value::as_str)
                            .is_some_and(|path| path.starts_with("ToS/"))
                })
                .filter_map(|member| {
                    member
                        .get("source_path")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .collect::<BTreeSet<_>>();
            if !has_command || expected_paths.is_empty() || expected_paths != source_path_lookups {
                return Err(DurableError::Corrupt(
                    "stored Claim source-closure paths differ from command range",
                ));
            }
        }
        _ => {
            return Err(DurableError::Corrupt(
                "stored empty Claim range evidence kind",
            ));
        }
    }
    Ok(())
}

fn verify_stored_range_coverage(
    value: &Value,
    cursor_before: u64,
    through_sequence: u64,
    terminal_event_id: &str,
) -> DurableResult<()> {
    if value.as_object().is_none_or(|object| object.len() != 5)
        || value.get("schema_version").and_then(Value::as_str)
            != Some("tos_source_navigation_event_range_coverage_v1")
        || value.get("cursor_before").and_then(Value::as_u64) != Some(cursor_before)
        || value.get("through_sequence").and_then(Value::as_u64) != Some(through_sequence)
        || value.get("terminal_event_id").and_then(Value::as_str) != Some(terminal_event_id)
    {
        return Err(DurableError::Corrupt(
            "stored navigation range coverage header",
        ));
    }
    let events = value
        .get("events")
        .and_then(Value::as_array)
        .ok_or(DurableError::Corrupt(
            "stored navigation range events absent",
        ))?;
    if events.len() as u64 != through_sequence - cursor_before || events.is_empty() {
        return Err(DurableError::Corrupt("stored navigation range event count"));
    }
    let mut total_members = 0usize;
    for (offset, event) in events.iter().enumerate() {
        if event.as_object().is_none_or(|object| object.len() != 6) {
            return Err(DurableError::Corrupt("stored navigation range event shape"));
        }
        let expected_sequence =
            cursor_before
                .checked_add(offset as u64 + 1)
                .ok_or(DurableError::Corrupt(
                    "stored navigation range sequence overflow",
                ))?;
        let sequence =
            event
                .get("sequence")
                .and_then(Value::as_u64)
                .ok_or(DurableError::Corrupt(
                    "stored navigation range event sequence",
                ))?;
        let event_id = event
            .get("event_id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or(DurableError::Corrupt("stored navigation range event ID"))?;
        validate_identifier(
            event_id,
            MAX_NAVIGATION_EVENT_ID_BYTES,
            "stored navigation range event ID",
        )?;
        let event_kind = event
            .get("event_kind")
            .and_then(Value::as_str)
            .ok_or(DurableError::Corrupt("stored navigation range event kind"))?;
        let command_id = event.get("command_id").and_then(Value::as_str);
        if sequence != expected_sequence
            || !matches!(event_kind, "command" | "rights" | "rule")
            || (event_kind == "command" && command_id.is_none_or(str::is_empty))
            || (event_kind != "command" && !event.get("command_id").is_some_and(Value::is_null))
        {
            return Err(DurableError::Corrupt(
                "stored navigation range event sequence or kind",
            ));
        }
        if let Some(command_id) = command_id {
            validate_identifier(
                command_id,
                MAX_NAVIGATION_EVENT_ID_BYTES,
                "stored navigation range command ID",
            )?;
        }
        let _member_root = json_digest_field(event, "member_root")?;
        let members =
            event
                .get("members")
                .and_then(Value::as_array)
                .ok_or(DurableError::Corrupt(
                    "stored navigation range members absent",
                ))?;
        if event_kind != "command" && !members.is_empty() {
            return Err(DurableError::Corrupt(
                "stored navigation non-command event has members",
            ));
        }
        total_members = total_members
            .checked_add(members.len())
            .ok_or(DurableError::Refused(
                "stored navigation range member count",
            ))?;
        if total_members > 65_536 {
            return Err(DurableError::Refused(
                "stored navigation range member count budget",
            ));
        }
        let mut previous_path: Option<String> = None;
        for member in members {
            if member.as_object().is_none_or(|object| object.len() != 5) {
                return Err(DurableError::Corrupt(
                    "stored navigation range member shape",
                ));
            }
            let path = member
                .get("source_path")
                .and_then(Value::as_str)
                .ok_or(DurableError::Corrupt("stored navigation range member path"))?;
            validate_relative_path(path)
                .map_err(|_| DurableError::Corrupt("stored navigation range member path"))?;
            if previous_path
                .as_deref()
                .is_some_and(|previous| previous >= path)
            {
                return Err(DurableError::Corrupt(
                    "stored navigation range members are not canonical",
                ));
            }
            previous_path = Some(path.to_owned());
            if member.get("revision").and_then(Value::as_u64).is_none()
                || member.get("is_source").and_then(Value::as_bool).is_none()
                || member.get("size_bytes").and_then(Value::as_u64).is_none()
            {
                return Err(DurableError::Corrupt(
                    "stored navigation range member facts are incomplete",
                ));
            }
            let _ = json_digest_field(member, "sha256")?;
        }
    }
    if events
        .last()
        .and_then(|event| event.get("event_id"))
        .and_then(Value::as_str)
        != Some(terminal_event_id)
    {
        return Err(DurableError::Corrupt(
            "stored navigation range terminal event differs",
        ));
    }
    Ok(())
}

fn range_owner_key(value: &Value) -> DurableResult<(String, String)> {
    let source_path = value
        .get("source_path")
        .and_then(Value::as_str)
        .ok_or(DurableError::Corrupt("stored component owner path absent"))?;
    let record_id = value
        .get("record_id")
        .and_then(Value::as_str)
        .ok_or(DurableError::Corrupt("stored component owner ID absent"))?;
    validate_relative_path(source_path)
        .map_err(|_| DurableError::Corrupt("stored component owner path invalid"))?;
    validate_identifier(
        record_id,
        MAX_NAVIGATION_RECORD_ID_BYTES,
        "stored component owner ID",
    )?;
    Ok((source_path.to_owned(), record_id.to_owned()))
}

fn json_digest_field(value: &Value, key: &str) -> DurableResult<Digest256> {
    value
        .get(key)
        .and_then(Value::as_str)
        .and_then(|hex| Digest256::from_hex(hex).ok())
        .ok_or(DurableError::Corrupt(
            "stored navigation range digest field",
        ))
}

fn optional_digest_field(value: &Value, key: &str) -> DurableResult<Option<Digest256>> {
    match value.get(key) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(hex)) => Digest256::from_hex(hex)
            .map(Some)
            .map_err(|_| DurableError::Corrupt("stored navigation range optional digest")),
        _ => Err(DurableError::Corrupt(
            "stored navigation range optional digest field",
        )),
    }
}

fn navigation_range_dependency_root(record_root: Digest256, claim_root: Digest256) -> Digest256 {
    let mut hasher = super::Digest256Hasher::new();
    hasher.update(b"tos-source-navigation-event-range-dependency-roots-v1");
    super::navigation_hash_part(&mut hasher, record_root.to_hex().as_bytes());
    super::navigation_hash_part(&mut hasher, claim_root.to_hex().as_bytes());
    hasher.finalize()
}

pub(super) fn navigation_range_coverage_root_next(
    previous_root: Digest256,
    cursor_before: u64,
    through_sequence: u64,
    held_source_head_sequence: u64,
    terminal_event_id: &str,
    range_root: Digest256,
    terminal_cohort_root: Digest256,
    source_revision: Digest256,
    dependency_root: Digest256,
    output_root: Digest256,
) -> Digest256 {
    let mut hash = super::Digest256Hasher::new();
    hash.update(b"tos-source-navigation-coverage-range-v2");
    super::navigation_hash_part(&mut hash, previous_root.to_hex().as_bytes());
    super::navigation_hash_part(&mut hash, &cursor_before.to_be_bytes());
    super::navigation_hash_part(&mut hash, &through_sequence.to_be_bytes());
    super::navigation_hash_part(&mut hash, &held_source_head_sequence.to_be_bytes());
    super::navigation_hash_part(&mut hash, terminal_event_id.as_bytes());
    super::navigation_hash_part(&mut hash, range_root.to_hex().as_bytes());
    super::navigation_hash_part(&mut hash, terminal_cohort_root.to_hex().as_bytes());
    super::navigation_hash_part(&mut hash, source_revision.to_hex().as_bytes());
    super::navigation_hash_part(&mut hash, dependency_root.to_hex().as_bytes());
    super::navigation_hash_part(&mut hash, output_root.to_hex().as_bytes());
    hash.finalize()
}

fn verify_navigation_range_reverse_dependency_closure(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    consumer_id: &str,
    expected_predecessor_owners: &BTreeSet<(String, String)>,
    changed_paths: &[String],
    limits: DurableNavigationEventLimits,
) -> DurableResult<()> {
    if changed_paths.is_empty() {
        if expected_predecessor_owners.is_empty() {
            return Ok(());
        }
        return Err(DurableError::Conflict(
            "navigation range owners have no changed source cause",
        ));
    }
    let owner_limit = i64::try_from(limits.max_current_rows.saturating_add(1))
        .map_err(|_| DurableError::Refused("navigation range reverse-owner count"))?;
    let owners = tx.query(
        "SELECT owner_source_path,owner_record_id FROM (
           SELECT DISTINCT owner_source_path,owner_record_id
             FROM cmd2_projection_navigation_bootstrap_row
            WHERE domain=$1 AND consumer_id=$2 AND active
              AND owner_source_path=ANY($3)
           UNION
           SELECT DISTINCT b.owner_source_path,b.owner_record_id
             FROM cmd2_projection_navigation_bootstrap_dependency b
            WHERE b.domain=$1 AND b.consumer_id=$2 AND b.dependency_path=ANY($3)
              AND EXISTS(SELECT 1 FROM cmd2_projection_navigation_bootstrap_row r
                WHERE r.domain=b.domain AND r.consumer_id=b.consumer_id AND r.active
                  AND r.owner_source_path=b.owner_source_path AND r.owner_record_id=b.owner_record_id)
           UNION
           SELECT DISTINCT source_path,record_id
             FROM cmd2_projection_navigation_cohort
            WHERE domain=$1 AND consumer_id=$2 AND source_path=ANY($3)
           UNION
           SELECT DISTINCT source_path,record_id
             FROM cmd2_projection_navigation_dependency
            WHERE domain=$1 AND consumer_id=$2 AND dependency_path=ANY($3)
         ) affected_owners
         ORDER BY owner_source_path COLLATE \"C\",owner_record_id COLLATE \"C\"
         LIMIT $4",
        &[&domain, &consumer_id, &changed_paths, &owner_limit],
    )?;
    if owners.len() > limits.max_current_rows {
        return Err(DurableError::Refused(
            "navigation range reverse-owner count budget",
        ));
    }
    let observed = owners
        .into_iter()
        .map(|row| {
            (
                row.get::<_, String>("owner_source_path"),
                row.get::<_, String>("owner_record_id"),
            )
        })
        .collect::<BTreeSet<_>>();
    if observed != *expected_predecessor_owners {
        return Err(DurableError::Conflict(
            "navigation range does not cover the complete reverse-owner closure",
        ));
    }
    Ok(())
}

fn verify_navigation_range_predecessor_root(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    consumer_id: &str,
    source_path: &str,
    record_id: &str,
    expected_root: Option<Digest256>,
    _bootstrap_root: Digest256,
    limits: DurableNavigationEventLimits,
) -> DurableResult<()> {
    let event_root = tx.query_opt(
        "SELECT output_root FROM cmd2_projection_navigation_cohort
          WHERE domain=$1 AND consumer_id=$2 AND source_path=$3 AND record_id=$4 FOR UPDATE",
        &[&domain, &consumer_id, &source_path, &record_id],
    )?;
    let bootstrap_exists: bool = tx
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM cmd2_projection_navigation_bootstrap_row
              WHERE domain=$1 AND consumer_id=$2 AND active
                AND owner_source_path=$3 AND owner_record_id=$4)",
            &[&domain, &consumer_id, &source_path, &record_id],
        )?
        .get(0);
    let observed = if let Some(row) = event_root {
        Some(parse_digest(row.get("output_root"))?)
    } else if bootstrap_exists {
        Some(navigation_bootstrap_owner_root(
            tx,
            domain,
            consumer_id,
            source_path,
            record_id,
            limits,
        )?)
    } else {
        None
    };
    if observed != expected_root {
        return Err(DurableError::Conflict(
            "navigation range predecessor owner root differs",
        ));
    }
    Ok(())
}

fn retire_navigation_range_owner(
    tx: &mut postgres::Transaction<'_>,
    domain: &str,
    consumer_id: &str,
    source_path: &str,
    record_id: &str,
) -> DurableResult<()> {
    tx.execute(
        "UPDATE cmd2_projection_navigation_bootstrap_row
            SET active=false
          WHERE domain=$1 AND consumer_id=$2 AND owner_source_path=$3 AND owner_record_id=$4",
        &[&domain, &consumer_id, &source_path, &record_id],
    )?;
    tx.execute(
        "DELETE FROM cmd2_projection_navigation_cohort
          WHERE domain=$1 AND consumer_id=$2 AND source_path=$3 AND record_id=$4",
        &[&domain, &consumer_id, &source_path, &record_id],
    )?;
    Ok(())
}

/// Predecessor Claim cohort evidence, keyed to the exact path that caused its
/// prior reverse dependency. `cause_path == claim_stream_path` identifies a
/// direct Claim-stream mutation; otherwise it names a prior dependency row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableNavigationEventRangePriorClaimOwnerV1 {
    pub cause_path: String,
    pub claim_stream_path: String,
    pub claim_id: String,
    pub predecessor_basis: DurableNavigationEventRangePriorOwnerBasisV1,
}

/// Predecessor Record cohort evidence, keyed to the exact changed path that
/// caused its prior reverse dependency. A bootstrap owner root binds its full
/// active row and dependency set because bootstrap has no event-cohort root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableNavigationEventRangePriorRecordOwnerV1 {
    pub cause_path: String,
    pub source_path: String,
    pub record_id: String,
    pub predecessor_basis: DurableNavigationEventRangePriorOwnerBasisV1,
}

/// The predecessor surface in which a Claim owner was last projected. A
/// bootstrap owner has no event-cohort root, so its root is derived from its
/// complete active bootstrap rows and dependency paths instead.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableNavigationEventRangePriorOwnerBasisV1 {
    EventCohort { cohort_root: Digest256 },
    Bootstrap { owner_root: Digest256 },
}

fn navigation_bootstrap_owner_root<C: postgres::GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer_id: &str,
    source_path: &str,
    owner_id: &str,
    limits: DurableNavigationEventLimits,
) -> DurableResult<Digest256> {
    let row_limit = i64::try_from(limits.max_current_rows.saturating_add(1))
        .map_err(|_| DurableError::Refused("navigation bootstrap owner rows"))?;
    let rows = tx.query(
        "SELECT row_kind,row_id,visible,row_json,row_sha256
           FROM cmd2_projection_navigation_bootstrap_row
          WHERE domain=$1 AND consumer_id=$2 AND active
            AND owner_source_path=$3 AND owner_record_id=$4
          ORDER BY row_kind COLLATE \"C\",row_id COLLATE \"C\" LIMIT $5",
        &[&domain, &consumer_id, &source_path, &owner_id, &row_limit],
    )?;
    if rows.len() > limits.max_current_rows {
        return Err(DurableError::Refused(
            "navigation bootstrap owner row count budget",
        ));
    }
    let mut row_facts = Vec::with_capacity(rows.len());
    let mut previous_row: Option<(String, String)> = None;
    let mut charged_bytes = 0u64;
    for row in rows {
        let row_kind: String = row.get("row_kind");
        if !matches!(row_kind.as_str(), "node" | "edge" | "right" | "diagnostic") {
            return Err(DurableError::Corrupt("navigation bootstrap owner row kind"));
        }
        let row_id: String = row.get("row_id");
        validate_identifier(
            &row_id,
            MAX_NAVIGATION_RECORD_ID_BYTES,
            "navigation bootstrap owner row ID",
        )?;
        let key = (row_kind.clone(), row_id.clone());
        if previous_row
            .as_ref()
            .is_some_and(|previous| previous >= &key)
        {
            return Err(DurableError::Corrupt(
                "navigation bootstrap owner rows are not uniquely ordered",
            ));
        }
        previous_row = Some(key);
        let visible: Option<bool> = row.get("visible");
        if matches!(row_kind.as_str(), "node" | "edge") != visible.is_some() {
            return Err(DurableError::Corrupt(
                "navigation bootstrap owner row visibility differs",
            ));
        }
        let row_raw: String = row.get("row_json");
        let row_raw = row_raw.into_bytes();
        let row_sha256 = parse_digest(row.get("row_sha256"))?;
        if row_raw.is_empty()
            || row_raw.len() > limits.max_row_bytes
            || Digest256::of_bytes(&row_raw) != row_sha256
        {
            return Err(DurableError::Corrupt(
                "navigation bootstrap owner row digest differs",
            ));
        }
        charged_bytes =
            charged_bytes
                .checked_add(row_raw.len() as u64)
                .ok_or(DurableError::Refused(
                    "navigation bootstrap owner byte count",
                ))?;
        row_facts.push(json!({
            "row_kind":row_kind,
            "row_id":row_id,
            "visible":visible,
            "row_sha256":row_sha256.to_hex(),
        }));
    }

    let dependency_limit = i64::try_from(limits.max_dependencies.saturating_add(1))
        .map_err(|_| DurableError::Refused("navigation bootstrap owner dependencies"))?;
    let dependencies = tx.query(
        "SELECT dependency_path
           FROM cmd2_projection_navigation_bootstrap_dependency
          WHERE domain=$1 AND consumer_id=$2
            AND owner_source_path=$3 AND owner_record_id=$4
          ORDER BY dependency_path COLLATE \"C\" LIMIT $5",
        &[
            &domain,
            &consumer_id,
            &source_path,
            &owner_id,
            &dependency_limit,
        ],
    )?;
    if dependencies.len() > limits.max_dependencies {
        return Err(DurableError::Refused(
            "navigation bootstrap owner dependency count budget",
        ));
    }
    let mut dependency_paths = Vec::with_capacity(dependencies.len());
    let mut previous_path: Option<String> = None;
    for row in dependencies {
        let path: String = row.get("dependency_path");
        validate_relative_path(&path)?;
        if previous_path
            .as_ref()
            .is_some_and(|previous| previous >= &path)
        {
            return Err(DurableError::Corrupt(
                "navigation bootstrap owner dependencies are not uniquely ordered",
            ));
        }
        charged_bytes =
            charged_bytes
                .checked_add(path.len() as u64)
                .ok_or(DurableError::Refused(
                    "navigation bootstrap owner byte count",
                ))?;
        previous_path = Some(path.clone());
        dependency_paths.push(path);
    }
    if charged_bytes > limits.max_total_bytes {
        return Err(DurableError::Refused(
            "navigation bootstrap owner root byte budget",
        ));
    }
    let raw = canonical_value(
        &json!({
            "schema":"tos_source_navigation_bootstrap_owner_v1",
            "source_path":source_path,
            "record_id":owner_id,
            "rows":row_facts,
            "dependencies":dependency_paths,
        }),
        limits.max_total_bytes as usize,
    )?;
    Ok(Digest256::of_bytes(&raw))
}

/// Complete published owner inventory for the selected Claim streams. A
/// reverse lookup returns causal owners only; replacing a whole stream also
/// needs the roots of its unchanged companions. Re-read this same bounded
/// inventory in the publication transaction before accepting the manifest.
fn read_navigation_claim_stream_predecessors<C: postgres::GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer_id: &str,
    stream_paths: &[String],
    limits: DurableNavigationEventLimits,
) -> DurableResult<
    Vec<tos_compiler::source_navigation_claim_event::NavigationClaimEventRangeStreamPredecessorV1>,
> {
    use tos_compiler::source_navigation_claim_event::{
        NavigationClaimEventRangePriorOwnerBasisV1 as Basis,
        NavigationClaimEventRangeStreamPredecessorV1 as Predecessor,
    };
    if stream_paths.is_empty() {
        return Ok(Vec::new());
    }
    if stream_paths.len() > limits.max_dependencies
        || stream_paths.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(DurableError::Invalid(
            "navigation Claim predecessor stream set",
        ));
    }
    for path in stream_paths {
        validate_relative_path(path)?;
        if !path.ends_with("/source-claims.jsonl") {
            return Err(DurableError::Invalid(
                "navigation Claim predecessor stream path",
            ));
        }
    }
    let cap = i64::try_from(limits.max_current_cohorts.saturating_add(1))
        .map_err(|_| DurableError::Refused("navigation Claim stream predecessor count"))?;
    let rows = tx.query(
        "SELECT source_path,record_id,output_root FROM (
           SELECT c.source_path,c.record_id,c.output_root
             FROM cmd2_projection_navigation_cohort c
            WHERE c.domain=$1 AND c.consumer_id=$2 AND c.source_path=ANY($3)
              AND c.basis_kind IN ('claim-mutation','claim-dependency-refresh',
                                  'claim-revision-refresh','claim-range-reconciliation')
           UNION
           SELECT b.owner_source_path,b.owner_record_id,NULL::text
             FROM cmd2_projection_navigation_bootstrap_row b
            WHERE b.domain=$1 AND b.consumer_id=$2 AND b.active
              AND b.owner_source_path=ANY($3) AND b.owner_record_id LIKE 'tos.claim.%'
              AND NOT EXISTS (
                SELECT 1 FROM cmd2_projection_navigation_cohort c
                 WHERE c.domain=b.domain AND c.consumer_id=b.consumer_id
                   AND c.source_path=b.owner_source_path AND c.record_id=b.owner_record_id)
         ) owners ORDER BY source_path COLLATE \"C\",record_id COLLATE \"C\" LIMIT $4",
        &[&domain, &consumer_id, &stream_paths, &cap],
    )?;
    if rows.len() > limits.max_current_cohorts {
        return Err(DurableError::Refused(
            "navigation Claim stream predecessor count",
        ));
    }
    let mut result = Vec::with_capacity(rows.len());
    let mut previous = None;
    for row in rows {
        let path: String = row.get("source_path");
        let id: String = row.get("record_id");
        validate_relative_path(&path)?;
        validate_identifier(
            &id,
            MAX_NAVIGATION_RECORD_ID_BYTES,
            "navigation Claim predecessor ID",
        )?;
        let key = (path.clone(), id.clone());
        if !id.starts_with("tos.claim.") || previous.as_ref().is_some_and(|prior| prior >= &key) {
            return Err(DurableError::Corrupt(
                "navigation Claim stream predecessor identity",
            ));
        }
        previous = Some(key);
        let basis = match row.get::<_, Option<String>>("output_root") {
            Some(root) => Basis::EventCohort {
                cohort_root: parse_digest(root)?,
            },
            None => Basis::Bootstrap {
                owner_root: navigation_bootstrap_owner_root(
                    tx,
                    domain,
                    consumer_id,
                    &path,
                    &id,
                    limits,
                )?,
            },
        };
        result.push(Predecessor {
            claim_stream_path: path,
            claim_id: id,
            basis,
        });
    }
    Ok(result)
}

/// One direct Claim mutation at its exact historical CMD event cut.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableNavigationEventRangeClaimMutationV1 {
    pub event_sequence: u64,
    pub event_id: String,
    pub claim_stream_member_path: String,
    pub claim_stream_sha256: Digest256,
    pub claim_stream_raw: Vec<u8>,
    pub receipt_path: String,
    pub receipt_sha256: Digest256,
    pub receipt_raw: Vec<u8>,
    pub mutation_claim_ids: Vec<String>,
    pub revision_history_path: Option<String>,
    pub revision_history_sha256: Option<Digest256>,
    pub revision_history_raw: Option<Vec<u8>>,
}

/// One event-specific dependency transition with both predecessor and held
/// current reverse Claim owners. This preserves old-only references so stale
/// edges can be retracted after a Claim changes its target in the range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableNavigationEventRangeDependencyCauseV1 {
    pub event_sequence: u64,
    pub event_id: String,
    pub changed_dependency_path: String,
    pub changed_dependency_sha256: Digest256,
    pub predecessor_claim_ids: Vec<String>,
    pub current_claim_ids: Vec<String>,
    pub predecessor_owner_roots: Vec<DurableNavigationEventRangePriorClaimOwnerV1>,
}

/// A complete range seed. The terminal source context is selected separately
/// after the producer derives its union dependency closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DurableNavigationEventRangeSeedV1 {
    UpToDate {
        cursor_sequence: u64,
        source_head_sequence: u64,
    },
    Pending {
        cursor_sequence: u64,
        source_head_sequence: u64,
        reason: DurableNavigationEventRangePendingReasonV1,
    },
    Ready(DurableNavigationEventRangeCoverageV1),
}

/// Result of an atomic range advance. The cursor is moved only after the
/// complete ordered event/member proof and terminal replacement set have been
/// retained in the range coverage table.
#[derive(Clone, Debug)]
pub struct DurableNavigationEventRangeApplyV1 {
    pub cursor_before: u64,
    pub through_sequence: u64,
    pub terminal_event_id: String,
    pub coverage_root: Digest256,
    pub terminal_cohort_root: Digest256,
    pub replacement_owner_count: usize,
    pub replayed: bool,
}

impl DurablePgCoordinator {
    /// Read a bounded exact prefix from the consumer cursor under one held source
    /// head. The event range and the held current source head are separate facts.
    pub fn read_navigation_event_range_seed_v1(
        &mut self,
        domain: &str,
        consumer_id: &str,
        profile: &DurableProjectionProfile,
        max_events: usize,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<DurableNavigationEventRangeSeedV1> {
        let limits = limits.validate()?;
        validate_identifier(
            domain,
            super::super::MAX_DOMAIN_BYTES,
            "navigation range domain",
        )?;
        validate_identifier(
            consumer_id,
            super::super::MAX_CONSUMER_ID_BYTES,
            "navigation range consumer ID",
        )?;
        profile.validate()?;
        if max_events == 0 || max_events > MAX_NAVIGATION_EVENT_STEPS_PER_CALL {
            return Err(DurableError::Invalid("navigation event range event budget"));
        }

        let cursor =
            read_navigation_event_cursor(&mut self.client, domain, consumer_id, profile, limits)?;
        let source_head = read_navigation_event_head(&mut self.client, domain, profile, limits)?;
        if cursor.sequence > source_head {
            return Err(DurableError::Corrupt(
                "navigation event cursor exceeds source head",
            ));
        }
        if cursor.sequence == source_head {
            return Ok(DurableNavigationEventRangeSeedV1::UpToDate {
                cursor_sequence: cursor.sequence,
                source_head_sequence: source_head,
            });
        }

        let range_len = source_head
            .checked_sub(cursor.sequence)
            .ok_or(DurableError::Corrupt("navigation range sequence underflow"))?;
        let requested_len = range_len.min(max_events as u64);
        let requested_through = cursor
            .sequence
            .checked_add(requested_len)
            .ok_or(DurableError::Corrupt("navigation range sequence overflow"))?;

        let mut events = Vec::with_capacity(requested_len as usize);
        let mut total_members = 0usize;
        let mut total_member_bytes = 0u64;
        for sequence in (cursor.sequence + 1)..=requested_through {
            let event_row = self.client.query_opt(
                "SELECT l.event_kind,l.command_id
                   FROM cmd2_log l JOIN cmd2_outbox o USING(domain,commit_seq)
                  WHERE l.domain=$1 AND l.commit_seq=$2",
                &[&domain, &as_i64(sequence)?],
            )?;
            let Some(event_row) = event_row else {
                return Ok(DurableNavigationEventRangeSeedV1::Pending {
                    cursor_sequence: cursor.sequence,
                    source_head_sequence: source_head,
                    reason: DurableNavigationEventRangePendingReasonV1::MissingEventProof,
                });
            };
            let event_kind: String = event_row.get("event_kind");
            let command_id: Option<String> = event_row.get("command_id");
            if event_kind == "command" {
                let Some(command_id) = command_id.as_deref() else {
                    return Ok(DurableNavigationEventRangeSeedV1::Pending {
                        cursor_sequence: cursor.sequence,
                        source_head_sequence: source_head,
                        reason: DurableNavigationEventRangePendingReasonV1::MissingEventProof,
                    });
                };
                let has_receipt: bool = self
                    .client
                    .query_one(
                        "SELECT EXISTS(
                           SELECT 1 FROM cmd2_attempt a
                           JOIN cmd2_receipt r USING(domain,command_id)
                          WHERE a.domain=$1 AND a.command_id=$2 AND a.state='committed'
                            AND a.commit_seq=$3 AND r.commit_seq=$3
                         )",
                        &[&domain, &command_id, &as_i64(sequence)?],
                    )?
                    .get(0);
                if !has_receipt {
                    return Ok(DurableNavigationEventRangeSeedV1::Pending {
                        cursor_sequence: cursor.sequence,
                        source_head_sequence: source_head,
                        reason: DurableNavigationEventRangePendingReasonV1::MissingEventProof,
                    });
                }
            } else if command_id.is_some() {
                return Err(DurableError::Corrupt(
                    "navigation non-command event carries a command identity",
                ));
            }
            let witness =
                resolve_event_witness(&mut self.client, domain, sequence, profile, limits)?;
            if witness.source_head_sequence != source_head || witness.event.commit_seq != sequence {
                return Err(DurableError::Conflict(
                    "navigation event range moved from its held source head",
                ));
            }
            let event = exact_range_event(&mut self.client, domain, command_id, &witness, limits)?;
            let next_member_count =
                total_members
                    .checked_add(event.members.len())
                    .ok_or(DurableError::Refused(
                        "navigation event range member count overflow",
                    ))?;
            for member in &event.members {
                total_member_bytes = total_member_bytes
                    .checked_add(member.source_path.len() as u64)
                    .and_then(|bytes| bytes.checked_add(member.size_bytes))
                    .ok_or(DurableError::Refused(
                        "navigation event range member byte count overflow",
                    ))?;
            }
            if next_member_count > limits.max_current_rows
                || total_member_bytes > limits.max_total_bytes
            {
                if events.is_empty() {
                    return Ok(DurableNavigationEventRangeSeedV1::Pending {
                        cursor_sequence: cursor.sequence,
                        source_head_sequence: source_head,
                        reason: DurableNavigationEventRangePendingReasonV1::EventBudgetExceeded,
                    });
                }
                break;
            }
            total_members = next_member_count;
            events.push(event);
        }
        if events.is_empty() {
            return Ok(DurableNavigationEventRangeSeedV1::Pending {
                cursor_sequence: cursor.sequence,
                source_head_sequence: source_head,
                reason: DurableNavigationEventRangePendingReasonV1::EventBudgetExceeded,
            });
        }

        let terminal_sequence = events
            .last()
            .ok_or(DurableError::Corrupt("navigation event range is empty"))?
            .sequence;
        let terminal_event_id = events
            .last()
            .ok_or(DurableError::Corrupt("navigation event range is empty"))?
            .event_id
            .clone();
        let (manifest_raw, coverage_root) = match range_manifest(
            cursor.sequence,
            terminal_sequence,
            &terminal_event_id,
            &events,
            limits.max_total_bytes,
        ) {
            Ok(manifest) => manifest,
            Err(DurableError::Refused(_)) => {
                return Ok(DurableNavigationEventRangeSeedV1::Pending {
                    cursor_sequence: cursor.sequence,
                    source_head_sequence: source_head,
                    reason: DurableNavigationEventRangePendingReasonV1::EventBudgetExceeded,
                });
            }
            Err(error) => return Err(error),
        };
        let latest_cursor =
            read_navigation_event_cursor(&mut self.client, domain, consumer_id, profile, limits)?;
        let latest_head = read_navigation_event_head(&mut self.client, domain, profile, limits)?;
        if latest_cursor.sequence != cursor.sequence || latest_head != source_head {
            return Err(DurableError::Conflict(
                "navigation event range cursor or source head changed during selection",
            ));
        }
        Ok(DurableNavigationEventRangeSeedV1::Ready(
            DurableNavigationEventRangeCoverageV1 {
                cursor_before: cursor.sequence,
                through_sequence: terminal_sequence,
                held_source_head_sequence: source_head,
                terminal_event_id,
                events,
                coverage_root,
                manifest_raw,
            },
        ))
    }

    /// Select one exact source member at its historical event cut. Unlike the
    /// event-context reader this reads only the requested member, which keeps
    /// multi-event Claim receipts inside the same bounded byte budget.
    pub fn read_navigation_event_range_member_source_v1(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        profile: &DurableProjectionProfile,
        coverage: &DurableNavigationEventRangeCoverageV1,
        event_sequence: u64,
        source_path: &str,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<Vec<u8>> {
        let limits = limits.validate()?;
        coverage.verify(limits)?;
        validate_relative_path(source_path)?;
        if store.custody_domain() != domain.as_bytes() {
            return Err(DurableError::Conflict("STO custody domain differs"));
        }
        let event = coverage
            .event(event_sequence)
            .ok_or(DurableError::Conflict(
                "navigation historical source event is outside the range",
            ))?;
        let member = coverage
            .member(event_sequence, source_path)
            .ok_or(DurableError::Conflict(
                "navigation historical source is not an exact event member",
            ))?;
        if !member.is_source {
            return Err(DurableError::Conflict(
                "navigation historical member is not source content",
            ));
        }
        let witness =
            resolve_event_witness(&mut self.client, domain, event_sequence, profile, limits)?;
        if witness.event_id != event.event_id || witness.event.event_kind != event.event_kind {
            return Err(DurableError::Conflict(
                "navigation historical event differs from range coverage",
            ));
        }
        let selected =
            selected_version_at_cut(&mut self.client, domain, source_path, event_sequence)?.ok_or(
                DurableError::Conflict("navigation historical member has no selected version"),
            )?;
        if selected.revision != member.revision
            || selected.commit_sequence != event_sequence
            || selected.content_length != member.size_bytes
            || selected.content_digest != member.sha256
            || !selected.is_source
        {
            return Err(DurableError::Conflict(
                "navigation historical member differs from exact event bytes",
            ));
        }
        if selected.content_length == 0 || selected.content_length > limits.max_row_bytes as u64 {
            return Err(DurableError::Refused(
                "navigation historical member byte budget",
            ));
        }
        let projection_limits = DurableProjectionLimits {
            max_event_bytes: limits.max_total_bytes,
            max_document_bytes: limits.max_row_bytes,
            statement_timeout: limits.statement_timeout,
            ..DurableProjectionLimits::default()
        };
        let mut event_bytes = 0;
        let mut source_work = super::super::DurableProjectionWork::default();
        let source = super::super::read_projection_source(
            self,
            store,
            domain,
            source_path,
            selected.revision,
            selected.content_length,
            selected.commit_sequence,
            projection_limits,
            false,
            std::time::Instant::now() + limits.statement_timeout,
            &mut event_bytes,
            &mut source_work,
        )?;
        if Digest256::of_bytes(&source.exact_source_bytes) != member.sha256
            || source.exact_source_bytes.len() as u64 != member.size_bytes
        {
            return Err(DurableError::Corrupt(
                "navigation historical STO bytes differ from range member digest",
            ));
        }
        Ok(source.exact_source_bytes)
    }

    /// Resolve the held terminal source cut after the caller has derived its
    /// complete dependency path set. This context is terminal-only and is
    /// intentionally not augmented with the historical range's member union.
    pub fn read_navigation_event_range_current_context_v1(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        consumer_id: &str,
        source_cut_anchor_path: &str,
        requested_paths: &[String],
        profile: &DurableProjectionProfile,
        coverage: &DurableNavigationEventRangeCoverageV1,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<DurableNavigationEventContextV1> {
        let limits = limits.validate()?;
        coverage.verify(limits)?;
        validate_identifier(
            domain,
            super::super::MAX_DOMAIN_BYTES,
            "navigation range domain",
        )?;
        validate_identifier(
            consumer_id,
            super::super::MAX_CONSUMER_ID_BYTES,
            "navigation range consumer ID",
        )?;
        profile.validate()?;
        if store.custody_domain() != domain.as_bytes() {
            return Err(DurableError::Conflict("STO custody domain differs"));
        }
        if !source_cut_anchor_path.is_empty() {
            validate_relative_path(source_cut_anchor_path)?;
            if !requested_paths
                .iter()
                .any(|path| path == source_cut_anchor_path)
            {
                return Err(DurableError::Invalid(
                    "navigation source-cut anchor is not selected",
                ));
            }
        }
        if requested_paths.is_empty() && source_cut_anchor_path.is_empty() {
            let cursor = read_navigation_event_cursor(
                &mut self.client,
                domain,
                consumer_id,
                profile,
                limits,
            )?;
            let source_head_sequence =
                read_navigation_event_head(&mut self.client, domain, profile, limits)?;
            let witness = resolve_event_witness(
                &mut self.client,
                domain,
                coverage.through_sequence,
                profile,
                limits,
            )?;
            let terminal =
                coverage
                    .event(coverage.through_sequence)
                    .ok_or(DurableError::Conflict(
                        "navigation terminal event is absent from the exact range",
                    ))?;
            let exact = exact_range_event(
                &mut self.client,
                domain,
                terminal.command_id.clone(),
                &witness,
                limits,
            )?;
            if cursor.sequence != coverage.cursor_before
                || source_head_sequence != coverage.held_source_head_sequence
                || witness.source_head_sequence != coverage.held_source_head_sequence
                || witness.event_id != coverage.terminal_event_id
                || exact != *terminal
                || terminal.members.iter().any(|member| member.is_source)
            {
                return Err(DurableError::Conflict(
                    "navigation terminal no-source cut differs from its exact range",
                ));
            }
            return Ok(DurableNavigationEventContextV1 {
                event_id: witness.event_id,
                event_sequence: coverage.through_sequence,
                event_kind: witness.event.event_kind,
                source_head_sequence,
                consumer_cursor: cursor,
                members: Vec::new(),
                selected_sources: Vec::new(),
            });
        }
        if !requested_paths.is_empty() && source_cut_anchor_path.is_empty() {
            return Err(DurableError::Invalid(
                "navigation source-cut dependencies lack a selection anchor",
            ));
        }
        // `source_cut_anchor_path` is selected at `through_sequence` as the
        // configured large-source path. It is not asserted to be a member of
        // the terminal event; `read_navigation_event_sources_v1` resolves that
        // event's real member set independently from its exact witness.
        let context = self.read_navigation_event_sources_v1(
            store,
            domain,
            consumer_id,
            coverage.through_sequence,
            source_cut_anchor_path,
            requested_paths,
            profile,
            limits,
        )?;
        if context.event_id != coverage.terminal_event_id
            || context.event_sequence != coverage.through_sequence
            || context.source_head_sequence != coverage.held_source_head_sequence
            || context.consumer_cursor.sequence != coverage.cursor_before
        {
            return Err(DurableError::Conflict(
                "navigation terminal source cut differs from exact range head",
            ));
        }
        Ok(context)
    }

    /// Return all prior Claim owners connected to changed paths either as
    /// their own changed Claim stream or through a stored Claim dependency.
    /// Rows are predecessor projection evidence, not claims about terminal
    /// source truth; the producer binds current reverse observations separately.
    pub fn read_navigation_event_range_prior_claim_owners_v1(
        &mut self,
        domain: &str,
        consumer_id: &str,
        coverage: &DurableNavigationEventRangeCoverageV1,
        changed_source_paths: &[String],
        profile: &DurableProjectionProfile,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<Vec<DurableNavigationEventRangePriorClaimOwnerV1>> {
        let limits = limits.validate()?;
        coverage.verify(limits)?;
        validate_identifier(
            domain,
            super::super::MAX_DOMAIN_BYTES,
            "navigation range domain",
        )?;
        validate_identifier(
            consumer_id,
            super::super::MAX_CONSUMER_ID_BYTES,
            "navigation range consumer ID",
        )?;
        profile.validate()?;
        if changed_source_paths.is_empty() || changed_source_paths.len() > limits.max_dependencies {
            return Err(DurableError::Invalid(
                "navigation range prior Claim cause path set",
            ));
        }
        let mut changed = BTreeSet::new();
        for path in changed_source_paths {
            validate_relative_path(path)?;
            if !changed.insert(path.as_str()) {
                return Err(DurableError::Invalid(
                    "duplicate navigation range prior Claim cause path",
                ));
            }
            if !coverage.events.iter().any(|event| {
                event
                    .members
                    .iter()
                    .any(|member| member.source_path == *path && member.is_source)
            }) {
                return Err(DurableError::Conflict(
                    "navigation range prior Claim cause is absent from exact events",
                ));
            }
        }
        let changed_paths = changed.into_iter().map(str::to_owned).collect::<Vec<_>>();
        let max_owners = limits.max_current_cohorts.min(limits.max_current_rows);
        let row_limit = i64::try_from(max_owners.saturating_add(1))
            .map_err(|_| DurableError::Refused("navigation range prior Claim count"))?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        set_local_timeouts(&mut tx, limits.statement_timeout)?;
        let snapshot = read_domain_snapshot(&mut tx, domain)?;
        snapshot.require_live_projection(profile)?;
        if snapshot.head_seq()? != coverage.held_source_head_sequence {
            return Err(DurableError::Conflict(
                "navigation range predecessor lookup source head changed",
            ));
        }
        verify_navigation_consumer(&mut tx, domain, consumer_id, profile)?;
        let consumer = tx.query_opt(
            "SELECT cursor_seq,coverage_seq,coverage_root,adjacency_seq,adjacency_root,adjacency_ready
               FROM cmd2_projection_navigation_consumer
              WHERE domain=$1 AND consumer_id=$2",
            &[&domain, &consumer_id],
        )?;
        let Some(consumer) = consumer else {
            return Err(DurableError::Conflict(
                "navigation range predecessor consumer is absent",
            ));
        };
        let cursor = as_u64(consumer.get("cursor_seq"))?;
        let coverage_sequence = as_u64(consumer.get("coverage_seq"))?;
        let adjacency_sequence = as_u64(consumer.get("adjacency_seq"))?;
        if cursor != coverage.cursor_before
            || coverage_sequence != cursor
            || adjacency_sequence != cursor
            || !consumer.get::<_, bool>("adjacency_ready")
            || consumer.get::<_, Option<String>>("coverage_root").is_none()
            || consumer
                .get::<_, Option<String>>("adjacency_root")
                .is_none()
        {
            return Err(DurableError::Conflict(
                "navigation range predecessor projection is not fully covered",
            ));
        }
        let rows = tx.query(
            "SELECT cause_path,claim_stream_path,claim_id,predecessor_kind,
                    predecessor_cohort_root
               FROM (
                 SELECT c.source_path AS cause_path,c.source_path AS claim_stream_path,
                        c.record_id AS claim_id,'event'::text AS predecessor_kind,
                        c.output_root AS predecessor_cohort_root
                   FROM cmd2_projection_navigation_cohort c
                  WHERE c.domain=$1 AND c.consumer_id=$2
                    AND c.source_path=ANY($3)
                    AND c.basis_kind IN ('claim-mutation','claim-dependency-refresh','claim-revision-refresh','claim-range-reconciliation')
                 UNION
                 SELECT d.dependency_path AS cause_path,c.source_path AS claim_stream_path,
                        c.record_id AS claim_id,'event'::text AS predecessor_kind,
                        c.output_root AS predecessor_cohort_root
                   FROM cmd2_projection_navigation_dependency d
                   JOIN cmd2_projection_navigation_cohort c
                     ON c.domain=d.domain AND c.consumer_id=d.consumer_id
                    AND c.source_path=d.source_path AND c.record_id=d.record_id
                  WHERE d.domain=$1 AND d.consumer_id=$2
                    AND d.dependency_path=ANY($3)
                    AND c.basis_kind IN ('claim-mutation','claim-dependency-refresh','claim-revision-refresh','claim-range-reconciliation')
                 UNION
                 SELECT b.owner_source_path AS cause_path,b.owner_source_path AS claim_stream_path,
                        b.owner_record_id AS claim_id,'bootstrap'::text AS predecessor_kind,
                        NULL::text AS predecessor_cohort_root
                   FROM cmd2_projection_navigation_bootstrap_row b
                  WHERE b.domain=$1 AND b.consumer_id=$2 AND b.active
                    AND b.owner_source_path=ANY($3)
                    AND b.owner_record_id LIKE 'tos.claim.%'
                    AND NOT EXISTS (
                      SELECT 1 FROM cmd2_projection_navigation_cohort c
                       WHERE c.domain=b.domain AND c.consumer_id=b.consumer_id
                         AND c.source_path=b.owner_source_path
                         AND c.record_id=b.owner_record_id
                    )
                 UNION
                 SELECT d.dependency_path AS cause_path,b.owner_source_path AS claim_stream_path,
                        b.owner_record_id AS claim_id,'bootstrap'::text AS predecessor_kind,
                        NULL::text AS predecessor_cohort_root
                   FROM cmd2_projection_navigation_bootstrap_dependency d
                   JOIN cmd2_projection_navigation_bootstrap_row b
                     ON b.domain=d.domain AND b.consumer_id=d.consumer_id
                    AND b.owner_source_path=d.owner_source_path
                    AND b.owner_record_id=d.owner_record_id
                  WHERE d.domain=$1 AND d.consumer_id=$2 AND b.active
                    AND d.dependency_path=ANY($3)
                    AND b.owner_record_id LIKE 'tos.claim.%'
                    AND NOT EXISTS (
                      SELECT 1 FROM cmd2_projection_navigation_cohort c
                       WHERE c.domain=b.domain AND c.consumer_id=b.consumer_id
                         AND c.source_path=b.owner_source_path
                         AND c.record_id=b.owner_record_id
                    )
               ) prior
              ORDER BY cause_path COLLATE \"C\",claim_stream_path COLLATE \"C\",
                       claim_id COLLATE \"C\",predecessor_kind COLLATE \"C\"
              LIMIT $4",
            &[&domain, &consumer_id, &changed_paths, &row_limit],
        )?;
        if rows.len() > max_owners {
            return Err(DurableError::Refused(
                "navigation range prior Claim owner count budget",
            ));
        }
        let mut owners = Vec::with_capacity(rows.len());
        let mut previous: Option<(String, String, String)> = None;
        let mut bootstrap_roots = BTreeMap::new();
        for row in rows {
            let cause_path: String = row.get("cause_path");
            let claim_stream_path: String = row.get("claim_stream_path");
            let claim_id: String = row.get("claim_id");
            let predecessor_kind: String = row.get("predecessor_kind");
            validate_relative_path(&cause_path)?;
            validate_relative_path(&claim_stream_path)?;
            validate_identifier(
                &claim_id,
                MAX_NAVIGATION_RECORD_ID_BYTES,
                "navigation prior Claim ID",
            )?;
            let key = (
                cause_path.clone(),
                claim_stream_path.clone(),
                claim_id.clone(),
            );
            if previous.as_ref().is_some_and(|previous| previous >= &key) {
                return Err(DurableError::Corrupt(
                    "navigation range prior Claim owners are not uniquely ordered",
                ));
            }
            previous = Some(key);
            let predecessor_basis = match predecessor_kind.as_str() {
                "event" => DurableNavigationEventRangePriorOwnerBasisV1::EventCohort {
                    cohort_root: parse_digest(
                        row.get::<_, Option<String>>("predecessor_cohort_root")
                            .ok_or(DurableError::Corrupt(
                                "navigation prior event cohort root is absent",
                            ))?,
                    )?,
                },
                "bootstrap" => {
                    let owner_key = (claim_stream_path.clone(), claim_id.clone());
                    let owner_root = if let Some(root) = bootstrap_roots.get(&owner_key) {
                        *root
                    } else {
                        let root = navigation_bootstrap_owner_root(
                            &mut tx,
                            domain,
                            consumer_id,
                            &claim_stream_path,
                            &claim_id,
                            limits,
                        )?;
                        bootstrap_roots.insert(owner_key, root);
                        root
                    };
                    DurableNavigationEventRangePriorOwnerBasisV1::Bootstrap { owner_root }
                }
                _ => {
                    return Err(DurableError::Corrupt("navigation prior Claim owner basis"));
                }
            };
            owners.push(DurableNavigationEventRangePriorClaimOwnerV1 {
                cause_path,
                claim_stream_path,
                claim_id,
                predecessor_basis,
            });
        }
        tx.commit()?;
        Ok(owners)
    }

    /// Return exact predecessor Record owners connected to changed paths.
    /// Event cohorts carry their output root; active bootstrap owners use a
    /// root over all current owner rows and dependency paths.
    pub fn read_navigation_event_range_prior_record_owners_v1(
        &mut self,
        domain: &str,
        consumer_id: &str,
        coverage: &DurableNavigationEventRangeCoverageV1,
        changed_source_paths: &[String],
        profile: &DurableProjectionProfile,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<Vec<DurableNavigationEventRangePriorRecordOwnerV1>> {
        let limits = limits.validate()?;
        coverage.verify(limits)?;
        validate_identifier(
            domain,
            super::super::MAX_DOMAIN_BYTES,
            "navigation range domain",
        )?;
        validate_identifier(
            consumer_id,
            super::super::MAX_CONSUMER_ID_BYTES,
            "navigation range consumer ID",
        )?;
        profile.validate()?;
        if changed_source_paths.is_empty() || changed_source_paths.len() > limits.max_dependencies {
            return Err(DurableError::Invalid(
                "navigation range prior Record cause path set",
            ));
        }
        let mut changed = BTreeSet::new();
        for path in changed_source_paths {
            validate_relative_path(path)?;
            if !changed.insert(path.as_str()) {
                return Err(DurableError::Invalid(
                    "duplicate navigation range prior Record cause path",
                ));
            }
            if !coverage.events.iter().any(|event| {
                event
                    .members
                    .iter()
                    .any(|member| member.source_path == *path && member.is_source)
            }) {
                return Err(DurableError::Conflict(
                    "navigation range prior Record cause is absent from exact events",
                ));
            }
        }
        let changed_paths = changed.into_iter().map(str::to_owned).collect::<Vec<_>>();
        let max_owners = limits.max_current_cohorts.min(limits.max_current_rows);
        let row_limit = i64::try_from(max_owners.saturating_add(1))
            .map_err(|_| DurableError::Refused("navigation range prior Record count"))?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        set_local_timeouts(&mut tx, limits.statement_timeout)?;
        let snapshot = read_domain_snapshot(&mut tx, domain)?;
        snapshot.require_live_projection(profile)?;
        if snapshot.head_seq()? != coverage.held_source_head_sequence {
            return Err(DurableError::Conflict(
                "navigation range predecessor lookup source head changed",
            ));
        }
        verify_navigation_consumer(&mut tx, domain, consumer_id, profile)?;
        let consumer = tx.query_opt(
            "SELECT cursor_seq,coverage_seq,coverage_root,adjacency_seq,adjacency_root,adjacency_ready
               FROM cmd2_projection_navigation_consumer
              WHERE domain=$1 AND consumer_id=$2",
            &[&domain, &consumer_id],
        )?.ok_or(DurableError::Conflict(
            "navigation range predecessor consumer is absent",
        ))?;
        let cursor = as_u64(consumer.get("cursor_seq"))?;
        if cursor != coverage.cursor_before
            || as_u64(consumer.get("coverage_seq"))? != cursor
            || as_u64(consumer.get("adjacency_seq"))? != cursor
            || !consumer.get::<_, bool>("adjacency_ready")
            || consumer.get::<_, Option<String>>("coverage_root").is_none()
            || consumer
                .get::<_, Option<String>>("adjacency_root")
                .is_none()
        {
            return Err(DurableError::Conflict(
                "navigation range predecessor projection is not fully covered",
            ));
        }
        let rows = tx.query(
            "SELECT cause_path,source_path,record_id,predecessor_kind,predecessor_cohort_root
               FROM (
                 SELECT c.source_path AS cause_path,c.source_path,c.record_id,
                        'event'::text AS predecessor_kind,c.output_root AS predecessor_cohort_root
                  FROM cmd2_projection_navigation_cohort c
                  WHERE c.domain=$1 AND c.consumer_id=$2 AND c.source_path=ANY($3)
                    AND c.basis_kind IN (
                      'record-mutation','dependency-refresh','dependency-refresh-set',
                      'claim-revision-refresh','range-reconciliation'
                    )
                 UNION
                 SELECT d.dependency_path AS cause_path,c.source_path,c.record_id,
                        'event'::text AS predecessor_kind,c.output_root AS predecessor_cohort_root
                   FROM cmd2_projection_navigation_dependency d
                   JOIN cmd2_projection_navigation_cohort c
                     ON c.domain=d.domain AND c.consumer_id=d.consumer_id
                    AND c.source_path=d.source_path AND c.record_id=d.record_id
                  WHERE d.domain=$1 AND d.consumer_id=$2 AND d.dependency_path=ANY($3)
                    AND c.basis_kind IN (
                      'record-mutation','dependency-refresh','dependency-refresh-set',
                      'claim-revision-refresh','range-reconciliation'
                    )
                 UNION
                 SELECT DISTINCT b.owner_source_path AS cause_path,b.owner_source_path AS source_path,
                        b.owner_record_id AS record_id,'bootstrap'::text AS predecessor_kind,
                        NULL::text AS predecessor_cohort_root
                   FROM cmd2_projection_navigation_bootstrap_row b
                  WHERE b.domain=$1 AND b.consumer_id=$2 AND b.active
                    AND b.owner_source_path=ANY($3) AND b.owner_record_id NOT LIKE 'tos.claim.%'
                    AND NOT EXISTS (
                      SELECT 1 FROM cmd2_projection_navigation_cohort c
                       WHERE c.domain=b.domain AND c.consumer_id=b.consumer_id
                         AND c.source_path=b.owner_source_path AND c.record_id=b.owner_record_id
                    )
                 UNION
                 SELECT DISTINCT d.dependency_path AS cause_path,b.owner_source_path AS source_path,
                        b.owner_record_id AS record_id,'bootstrap'::text AS predecessor_kind,
                        NULL::text AS predecessor_cohort_root
                   FROM cmd2_projection_navigation_bootstrap_dependency d
                   JOIN cmd2_projection_navigation_bootstrap_row b
                     ON b.domain=d.domain AND b.consumer_id=d.consumer_id
                    AND b.owner_source_path=d.owner_source_path AND b.owner_record_id=d.owner_record_id
                  WHERE d.domain=$1 AND d.consumer_id=$2 AND b.active AND d.dependency_path=ANY($3)
                    AND b.owner_record_id NOT LIKE 'tos.claim.%'
                    AND NOT EXISTS (
                      SELECT 1 FROM cmd2_projection_navigation_cohort c
                       WHERE c.domain=b.domain AND c.consumer_id=b.consumer_id
                         AND c.source_path=b.owner_source_path AND c.record_id=b.owner_record_id
                    )
               ) prior
              ORDER BY cause_path COLLATE \"C\",source_path COLLATE \"C\",
                       record_id COLLATE \"C\",predecessor_kind COLLATE \"C\"
              LIMIT $4",
            &[&domain, &consumer_id, &changed_paths, &row_limit],
        )?;
        if rows.len() > max_owners {
            return Err(DurableError::Refused(
                "navigation range prior Record owner count budget",
            ));
        }
        let mut owners = Vec::with_capacity(rows.len());
        let mut previous = None;
        let mut bootstrap_roots = BTreeMap::new();
        for row in rows {
            let cause_path: String = row.get("cause_path");
            let source_path: String = row.get("source_path");
            let record_id: String = row.get("record_id");
            let predecessor_kind: String = row.get("predecessor_kind");
            validate_relative_path(&cause_path)?;
            validate_relative_path(&source_path)?;
            validate_identifier(
                &record_id,
                MAX_NAVIGATION_RECORD_ID_BYTES,
                "navigation prior Record ID",
            )?;
            let key = (cause_path.clone(), source_path.clone(), record_id.clone());
            if previous.as_ref().is_some_and(|previous| previous >= &key) {
                return Err(DurableError::Corrupt(
                    "navigation range prior Record owners are not uniquely ordered",
                ));
            }
            previous = Some(key);
            let predecessor_basis = match predecessor_kind.as_str() {
                "event" => DurableNavigationEventRangePriorOwnerBasisV1::EventCohort {
                    cohort_root: parse_digest(
                        row.get::<_, Option<String>>("predecessor_cohort_root")
                            .ok_or(DurableError::Corrupt(
                                "navigation prior Record event cohort root is absent",
                            ))?,
                    )?,
                },
                "bootstrap" => {
                    let owner_key = (source_path.clone(), record_id.clone());
                    let owner_root = if let Some(root) = bootstrap_roots.get(&owner_key) {
                        *root
                    } else {
                        let root = navigation_bootstrap_owner_root(
                            &mut tx,
                            domain,
                            consumer_id,
                            &source_path,
                            &record_id,
                            limits,
                        )?;
                        bootstrap_roots.insert(owner_key, root);
                        root
                    };
                    DurableNavigationEventRangePriorOwnerBasisV1::Bootstrap { owner_root }
                }
                _ => return Err(DurableError::Corrupt("navigation prior Record owner basis")),
            };
            owners.push(DurableNavigationEventRangePriorRecordOwnerV1 {
                cause_path,
                source_path,
                record_id,
                predecessor_basis,
            });
        }
        tx.commit()?;
        Ok(owners)
    }

    /// Resolve the union of persisted Record and Claim owners affected by any
    /// exact source path in a complete event range.
    pub fn read_navigation_event_range_affected_owners_v1(
        &mut self,
        domain: &str,
        consumer_id: &str,
        coverage: &DurableNavigationEventRangeCoverageV1,
        changed_source_paths: &[String],
        profile: &DurableProjectionProfile,
        limits: DurableNavigationEventLimits,
    ) -> DurableResult<Vec<DurableNavigationEventAffectedOwnerV1>> {
        let limits = limits.validate()?;
        coverage.verify(limits)?;
        validate_identifier(
            domain,
            super::super::MAX_DOMAIN_BYTES,
            "navigation range domain",
        )?;
        validate_identifier(
            consumer_id,
            super::super::MAX_CONSUMER_ID_BYTES,
            "navigation range consumer ID",
        )?;
        profile.validate()?;
        if changed_source_paths.is_empty() || changed_source_paths.len() > limits.max_dependencies {
            return Err(DurableError::Invalid(
                "navigation range affected-owner source paths",
            ));
        }
        let mut changed = BTreeSet::new();
        for path in changed_source_paths {
            validate_relative_path(path)?;
            if !changed.insert(path.as_str()) {
                return Err(DurableError::Invalid(
                    "duplicate navigation range affected-owner path",
                ));
            }
            if !coverage.events.iter().any(|event| {
                event
                    .members
                    .iter()
                    .any(|member| member.source_path == *path && member.is_source)
            }) {
                return Err(DurableError::Conflict(
                    "navigation range affected path is absent from exact events",
                ));
            }
        }
        let changed_paths = changed.into_iter().map(str::to_owned).collect::<Vec<_>>();
        let max_owners = limits.max_current_cohorts.min(limits.max_current_rows);
        let owner_limit = i64::try_from(max_owners.saturating_add(1))
            .map_err(|_| DurableError::Refused("navigation range reverse-owner count"))?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        set_local_timeouts(&mut tx, limits.statement_timeout)?;
        let snapshot = read_domain_snapshot(&mut tx, domain)?;
        snapshot.require_live_projection(profile)?;
        if snapshot.head_seq()? != coverage.held_source_head_sequence {
            return Err(DurableError::Conflict(
                "navigation range affected-owner cut is no longer current",
            ));
        }
        verify_navigation_consumer(&mut tx, domain, consumer_id, profile)?;
        let cursor_sequence = tx
            .query_opt(
                "SELECT cursor_seq FROM cmd2_projection_navigation_consumer
                  WHERE domain=$1 AND consumer_id=$2",
                &[&domain, &consumer_id],
            )?
            .map(|row| as_u64(row.get("cursor_seq")))
            .transpose()?
            .ok_or(DurableError::Conflict(
                "navigation range affected-owner consumer is absent",
            ))?;
        if cursor_sequence != coverage.cursor_before {
            return Err(DurableError::Conflict(
                "navigation range affected-owner cursor changed",
            ));
        }
        let rows = tx.query(
            "SELECT owner_source_path,owner_record_id,
                    CASE WHEN owner_record_id LIKE 'tos.claim.%' OR EXISTS(
                      SELECT 1 FROM cmd2_projection_navigation_cohort c
                       WHERE c.domain=$1 AND c.consumer_id=$2
                         AND c.source_path=affected.owner_source_path
                         AND c.record_id=affected.owner_record_id
                         AND c.basis_kind IN ('claim-mutation','claim-dependency-refresh','claim-revision-refresh','claim-range-reconciliation')
                    ) THEN 'claim' ELSE 'record' END AS owner_kind
             FROM (
               SELECT DISTINCT owner_source_path,owner_record_id
                 FROM cmd2_projection_navigation_bootstrap_row
                WHERE domain=$1 AND consumer_id=$2 AND active
                  AND owner_source_path=ANY($3)
               UNION
               SELECT DISTINCT b.owner_source_path,b.owner_record_id
                 FROM cmd2_projection_navigation_bootstrap_dependency b
                WHERE b.domain=$1 AND b.consumer_id=$2 AND b.dependency_path=ANY($3)
                  AND EXISTS(
                    SELECT 1 FROM cmd2_projection_navigation_bootstrap_row r
                     WHERE r.domain=b.domain AND r.consumer_id=b.consumer_id AND r.active
                       AND r.owner_source_path=b.owner_source_path
                       AND r.owner_record_id=b.owner_record_id
                  )
               UNION
               SELECT DISTINCT source_path,record_id
                 FROM cmd2_projection_navigation_cohort
                WHERE domain=$1 AND consumer_id=$2 AND source_path=ANY($3)
               UNION
               SELECT DISTINCT source_path,record_id
                 FROM cmd2_projection_navigation_dependency
                WHERE domain=$1 AND consumer_id=$2 AND dependency_path=ANY($3)
             ) affected
             ORDER BY owner_source_path COLLATE \"C\",owner_record_id COLLATE \"C\"
             LIMIT $4",
            &[&domain, &consumer_id, &changed_paths, &owner_limit],
        )?;
        if rows.len() > max_owners {
            return Err(DurableError::Refused(
                "navigation range affected-owner count budget",
            ));
        }
        let mut owners = Vec::with_capacity(rows.len());
        let mut previous_key = None;
        for row in rows {
            let source_path: String = row.get("owner_source_path");
            let record_id: String = row.get("owner_record_id");
            validate_relative_path(&source_path)?;
            validate_identifier(
                &record_id,
                MAX_NAVIGATION_RECORD_ID_BYTES,
                "navigation range affected owner ID",
            )?;
            let key = (source_path.clone(), record_id.clone());
            if previous_key
                .as_ref()
                .is_some_and(|previous| previous >= &key)
            {
                return Err(DurableError::Corrupt(
                    "navigation range affected owners are not uniquely ordered",
                ));
            }
            previous_key = Some(key);
            let owner_kind: String = row.get("owner_kind");
            let kind = match owner_kind.as_str() {
                "record" => DurableNavigationEventAffectedOwnerKindV1::Record,
                "claim" => DurableNavigationEventAffectedOwnerKindV1::Claim,
                _ => {
                    return Err(DurableError::Corrupt(
                        "navigation range affected owner kind",
                    ));
                }
            };
            owners.push(DurableNavigationEventAffectedOwnerV1 {
                source_path,
                record_id,
                kind,
            });
        }
        let unsupported_rights: bool = tx
            .query_one(
                "SELECT EXISTS(
                   SELECT 1 FROM cmd2_projection_navigation_bootstrap_row b
                    WHERE b.domain=$1 AND b.consumer_id=$2 AND b.row_kind='right'
                      AND EXISTS(
                        SELECT 1 FROM unnest($3::text[],$4::text[]) affected(path,id)
                         WHERE affected.path=b.owner_source_path
                           AND affected.id=b.owner_record_id
                      )
                 )",
                &[
                    &domain,
                    &consumer_id,
                    &owners
                        .iter()
                        .map(|owner| owner.source_path.clone())
                        .collect::<Vec<_>>(),
                    &owners
                        .iter()
                        .map(|owner| owner.record_id.clone())
                        .collect::<Vec<_>>(),
                ],
            )?
            .get(0);
        if unsupported_rights {
            return Err(DurableError::Conflict(
                "navigation range affected-owner set contains unsupported rights rows",
            ));
        }
        tx.commit()?;
        Ok(owners)
    }
}

fn exact_range_event(
    client: &mut postgres::Client,
    domain: &str,
    command_id: Option<String>,
    witness: &EventWitness,
    limits: DurableNavigationEventLimits,
) -> DurableResult<DurableNavigationEventRangeEventV1> {
    let max_rows = 65i64;
    let root_row = client.query_opt(
        "SELECT members_root FROM cmd2_log WHERE domain=$1 AND commit_seq=$2",
        &[&domain, &as_i64(witness.event.commit_seq)?],
    )?;
    let Some(root_row) = root_row else {
        return Err(DurableError::Corrupt(
            "navigation range event member commitment is absent",
        ));
    };
    let member_root = parse_digest(root_row.get("members_root"))?;
    let rows = client.query(
        "SELECT member_slot,left(subject,4097) AS subject,
                octet_length(subject) AS subject_bytes,revision AS proposed_revision,
                sto_receipt_id,content_digest,content_length,source_mode,source_dependencies
         FROM cmd2_history WHERE domain=$1 AND commit_seq=$2
         ORDER BY member_slot LIMIT $3",
        &[&domain, &as_i64(witness.event.commit_seq)?, &max_rows],
    )?;
    if rows.len() > 64 {
        return Err(DurableError::Refused(
            "navigation range event member count budget",
        ));
    }
    let empty_members_root = Digest256::of_bytes(b"");
    if witness.event.event_kind == "command" {
        if command_id.as_deref().is_none_or(str::is_empty) {
            return Err(DurableError::Corrupt(
                "navigation command event identity is absent",
            ));
        }
        if super::super::member_rows_root(&rows) != member_root {
            return Err(DurableError::Corrupt(
                "navigation range member rows differ from log commitment",
            ));
        }
    } else if command_id.is_some()
        || !rows.is_empty()
        || member_root != empty_members_root
        || !witness.members.is_empty()
    {
        return Err(DurableError::Corrupt(
            "navigation range rights or rule event has source members",
        ));
    }
    if rows.len() != witness.members.len() {
        return Err(DurableError::Corrupt(
            "navigation range witness member count differs from history",
        ));
    }
    let mut members = Vec::with_capacity(rows.len());
    let mut previous_path: Option<String> = None;
    for (row, witness_member) in rows.iter().zip(&witness.members) {
        let source_path: String = row.get("subject");
        let subject_bytes: i32 = row.get("subject_bytes");
        let revision = as_u64(row.get("proposed_revision"))?;
        let size_bytes = as_u64(row.get("content_length"))?;
        let is_source: Option<i32> = row.get("source_mode");
        let digest = parse_digest(row.get("content_digest"))?;
        validate_relative_path(&source_path)?;
        if subject_bytes < 0
            || subject_bytes as usize > 4096
            || source_path.len() > 4096
            || previous_path
                .as_ref()
                .is_some_and(|previous| previous >= &source_path)
            || witness_member.source_path != source_path
            || witness_member.revision != revision
            || witness_member.content_length != size_bytes
            || witness_member.is_source != is_source.is_some()
        {
            return Err(DurableError::Corrupt(
                "navigation range member differs from committed history",
            ));
        }
        previous_path = Some(source_path.clone());
        members.push(DurableNavigationEventRangeMemberV1 {
            source_path,
            revision,
            is_source: is_source.is_some(),
            size_bytes,
            sha256: digest,
        });
    }
    Ok(DurableNavigationEventRangeEventV1 {
        sequence: witness.event.commit_seq,
        event_id: witness.event_id.clone(),
        event_kind: witness.event.event_kind.clone(),
        command_id,
        member_root,
        members,
    })
}

fn range_manifest(
    cursor_before: u64,
    through_sequence: u64,
    terminal_event_id: &str,
    events: &[DurableNavigationEventRangeEventV1],
    byte_limit: u64,
) -> DurableResult<(Vec<u8>, Digest256)> {
    if cursor_before >= through_sequence
        || events.is_empty()
        || events.len() as u64 != through_sequence - cursor_before
        || events.first().map(|event| event.sequence) != Some(cursor_before + 1)
        || events.last().map(|event| event.sequence) != Some(through_sequence)
        || events.last().map(|event| event.event_id.as_str()) != Some(terminal_event_id)
    {
        return Err(DurableError::Invalid(
            "navigation range coverage is not complete and contiguous",
        ));
    }
    let mut previous_sequence = cursor_before;
    let mut total_members = 0usize;
    for event in events {
        validate_identifier(
            &event.event_id,
            MAX_NAVIGATION_EVENT_ID_BYTES,
            "navigation range event ID",
        )?;
        if event.sequence != previous_sequence + 1
            || !matches!(event.event_kind.as_str(), "command" | "rights" | "rule")
            || (event.event_kind == "command"
                && event.command_id.as_deref().is_none_or(str::is_empty))
            || (event.event_kind != "command" && event.command_id.is_some())
        {
            return Err(DurableError::Invalid(
                "navigation range event order or kind",
            ));
        }
        previous_sequence = event.sequence;
        total_members =
            total_members
                .checked_add(event.members.len())
                .ok_or(DurableError::Refused(
                    "navigation range member count overflow",
                ))?;
        let mut previous_path = None;
        for member in &event.members {
            validate_relative_path(&member.source_path)?;
            if previous_path.is_some_and(|previous: &str| previous >= member.source_path.as_str()) {
                return Err(DurableError::Invalid(
                    "navigation range member paths are not uniquely ordered",
                ));
            }
            previous_path = Some(member.source_path.as_str());
        }
        if event.event_kind != "command" && !event.members.is_empty() {
            return Err(DurableError::Invalid(
                "navigation range no-source event has members",
            ));
        }
    }
    if total_members > 65_536 {
        return Err(DurableError::Refused(
            "navigation range aggregate member count budget",
        ));
    }
    let value = range_manifest_value(cursor_before, through_sequence, terminal_event_id, events);
    let raw = canonical_value(&value, usize::try_from(byte_limit).unwrap_or(usize::MAX))?;
    if raw.len() as u64 > byte_limit {
        return Err(DurableError::Refused(
            "navigation range canonical coverage byte budget",
        ));
    }
    let root = Digest256::of_bytes(&raw);
    Ok((raw, root))
}

fn range_manifest_value(
    cursor_before: u64,
    through_sequence: u64,
    terminal_event_id: &str,
    events: &[DurableNavigationEventRangeEventV1],
) -> Value {
    json!({
        "schema_version": "tos_source_navigation_event_range_coverage_v1",
        "cursor_before": cursor_before,
        "through_sequence": through_sequence,
        "terminal_event_id": terminal_event_id,
        "events": events.iter().map(|event| json!({
            "sequence": event.sequence,
            "event_id": event.event_id.as_str(),
            "event_kind": event.event_kind.as_str(),
            "command_id": event.command_id.as_deref(),
            "member_root": event.member_root.to_hex(),
            "members": event.members.iter().map(|member| json!({
                "source_path": member.source_path.as_str(),
                "revision": member.revision,
                "is_source": member.is_source,
                "size_bytes": member.size_bytes,
                "sha256": member.sha256.to_hex(),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod range_manifest_tests {
    use super::*;

    #[test]
    fn claim_stream_predecessor_extension_preserves_legacy_and_rejects_unbound_rows() {
        let root = Digest256::of_bytes(b"published");
        let path = "ToS/source-witnesses/relations/source-claims.jsonl";
        let keys = vec![
            (path.to_owned(), "tos.claim.a".to_owned()),
            (path.to_owned(), "tos.claim.b".to_owned()),
        ];
        let mut manifest = json!({
            "schema":"tos_source_navigation_claim_event_range_manifest_v1",
            "cursor_before":6,"through_sequence":7,"terminal_event_id":"event-7",
            "coverage_root":root.to_hex(),"mutations":[],"dependency_causes":[],
            "prior_owners":[],"terminal_inventory_root":root.to_hex(),
            "empty_owner_evidence":null,"terminal_streams":[],"terminal_owner_ids":[],"removed_owners":[],
        });
        let verify_shape = |value: &Value| {
            verify_component_range_binding(
                value,
                "tos_source_navigation_claim_event_range_manifest_v1",
                6,
                7,
                "event-7",
                root,
            )
        };
        assert!(verify_shape(&manifest).is_ok());
        assert!(
            read_stored_claim_stream_predecessors(&manifest, &keys)
                .unwrap()
                .is_none()
        );
        let bootstrap = json!({"kind":"bootstrap","root":root.to_hex()});
        let event = json!({"kind":"event-cohort","root":root.to_hex()});
        manifest["stream_predecessors"] = json!([
            {"claim_stream_path":path,"claim_id":"tos.claim.a","basis":bootstrap},
            {"claim_stream_path":path,"claim_id":"tos.claim.b","basis":event},
        ]);
        manifest["prior_owners"] = json!([{"cause_path":"ToS/works/work.json", "claim_stream_path":path,"claim_id":"tos.claim.a","basis":bootstrap}]);
        assert!(verify_shape(&manifest).is_ok());
        let predecessors = read_stored_claim_stream_predecessors(&manifest, &keys)
            .unwrap()
            .unwrap();
        assert_eq!(predecessors[&keys[0]], None);
        assert_eq!(predecessors[&keys[1]], Some(root));
        manifest["prior_owners"][0]["basis"] = event;
        assert!(read_stored_claim_stream_predecessors(&manifest, &keys).is_err());
        manifest["prior_owners"] = json!([]);
        manifest["stream_predecessors"][1]["claim_id"] = json!("tos.claim.foreign");
        assert!(read_stored_claim_stream_predecessors(&manifest, &keys).is_err());
        manifest["stream_predecessors"] = Value::Null;
        assert!(read_stored_claim_stream_predecessors(&manifest, &keys).is_err());
        manifest
            .as_object_mut()
            .unwrap()
            .remove("stream_predecessors");
        manifest["unknown_extension"] = json!([]);
        assert!(verify_shape(&manifest).is_err());
    }
}
