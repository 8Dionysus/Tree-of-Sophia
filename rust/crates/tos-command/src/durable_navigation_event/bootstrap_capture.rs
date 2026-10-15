//! Resumable capture for explicit initial navigation construction. Exact
//! Original carriers and each keyset position commit together. This is private
//! staging; it never changes the published consumer cursor or grants a read.
//!
//! The total selected model has no event-sized ceiling. Limits below apply to
//! a page, one carrier, one transaction and one invocation respectively.

use super::*;
use postgres::Transaction;
use serde::{Deserialize, Serialize};

const PAGE_ROWS: usize = 128;
const MEMBER_BOUND: u64 = 4096 + 5 + 192 + 8;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Position {
    phase: String,
    after: String,
    nodes: u64,
    edges: u64,
    rights: u64,
    raw_bytes: u64,
    work_bytes: u64,
}

impl Default for Position {
    fn default() -> Self {
        Self {
            phase: "nodes".into(),
            after: String::new(),
            nodes: 0,
            edges: 0,
            rights: 0,
            raw_bytes: 0,
            work_bytes: 0,
        }
    }
}

#[derive(Clone, Debug)]
struct CapturedRow {
    collection: String,
    key: String,
    ordinal: Option<i64>,
    id: String,
    raw: Vec<u8>,
    digest: Digest256,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct NavigationInitialCaptureProgress {
    pub captured: bool,
    pub verified: bool,
    pub phase: String,
    pub after: String,
    pub nodes: u64,
    pub edges: u64,
    pub rights: u64,
    pub raw_bytes: u64,
    pub total_work_bytes: u64,
    pub call_work_bytes: u64,
    pub committed_batches: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SelectedCapture {
    snapshot: DomainSnapshot,
    binding: Value,
    job_id: String,
    job_fence: u64,
    header_sha256: String,
}

fn position_json(position: &Position) -> DurableResult<String> {
    serde_json::to_string(position)
        .map_err(|_| DurableError::Invalid("navigation initial position encoding"))
}

pub(super) fn require_no_active_initial<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer: &str,
) -> DurableResult<()> {
    if !tx
        .query_one(
            "SELECT to_regclass('cmd2_projection_navigation_initial') IS NOT NULL",
            &[],
        )?
        .get::<_, bool>(0)
    {
        return Ok(());
    }
    if tx.query_opt(
        "SELECT 1 FROM cmd2_projection_navigation_initial
          WHERE domain=$1 AND consumer_id=$2 AND (position_json::jsonb->>'phase') IS DISTINCT FROM 'complete'",
        &[&domain, &consumer],
    )?.is_some() {
        return Err(DurableError::Conflict("navigation initial construction is unfinished"));
    }
    Ok(())
}

fn read_position(
    tx: &mut Transaction<'_>,
    domain: &str,
    consumer: &str,
    selection: &str,
) -> DurableResult<Position> {
    let row = tx.query_one(
        "SELECT CASE WHEN octet_length(selection_json)<=1048576 THEN selection_json END,
                CASE WHEN octet_length(position_json)<=8192 THEN position_json END
           FROM cmd2_projection_navigation_initial WHERE domain=$1 AND consumer_id=$2 FOR UPDATE",
        &[&domain, &consumer],
    )?;
    if row.get::<_, Option<String>>(0).as_deref() != Some(selection) {
        return Err(DurableError::Conflict(
            "navigation initial selected cut changed; use a new consumer",
        ));
    }
    serde_json::from_str(
        &row.get::<_, Option<String>>(1)
            .ok_or(DurableError::Corrupt("navigation initial position size"))?,
    )
    .map_err(|_| DurableError::Corrupt("navigation initial position encoding"))
}

fn begin_capture(
    tx: &mut Transaction<'_>,
    domain: &str,
    consumer: &str,
    profile: &DurableProjectionProfile,
    selection: &str,
    header: &[u8],
) -> DurableResult<Position> {
    if selection.len() > 1024 * 1024 || header.len() > 1024 * 1024 {
        return Err(DurableError::Refused(
            "navigation initial selection/header size",
        ));
    }
    tx.execute(
        "INSERT INTO cmd2_projection_navigation_consumer(domain,consumer_id,profile_id,profile_version,source_contract_digest)
         VALUES($1,$2,$3,$4,$5) ON CONFLICT(domain,consumer_id) DO NOTHING",
        &[&domain, &consumer, &profile.id, &profile.version.to_string(), &profile.source_contract.to_hex()],
    )?;
    let row = tx.query_one(
        "SELECT profile_id,profile_version,source_contract_digest,cursor_seq,coverage_seq,adjacency_ready
           FROM cmd2_projection_navigation_consumer WHERE domain=$1 AND consumer_id=$2 FOR UPDATE",
        &[&domain, &consumer],
    )?;
    if row.get::<_, String>(0) != profile.id
        || row.get::<_, String>(1) != profile.version.to_string()
        || row.get::<_, String>(2) != profile.source_contract.to_hex()
        || row.get::<_, i64>(3) != 0
        || row.get::<_, i64>(4) != 0
        || row.get::<_, bool>(5)
    {
        return Err(DurableError::Conflict(
            "navigation initial capture requires an unused matching consumer",
        ));
    }
    if tx
        .query_opt(
            "SELECT 1 FROM cmd2_projection_navigation_initial WHERE domain=$1 AND consumer_id=$2",
            &[&domain, &consumer],
        )?
        .is_none()
    {
        if tx.query_opt("SELECT 1 FROM cmd2_projection_navigation_bootstrap WHERE domain=$1 AND consumer_id=$2", &[&domain, &consumer])?.is_some()
            || tx.query_opt("SELECT 1 FROM cmd2_projection_navigation_cohort WHERE domain=$1 AND consumer_id=$2 LIMIT 1", &[&domain, &consumer])?.is_some()
        {
            return Err(DurableError::Conflict("navigation initial capture consumer already has derived rows"));
        }
        let header = std::str::from_utf8(header)
            .map_err(|_| DurableError::Corrupt("navigation initial header UTF-8"))?;
        tx.execute(
            "INSERT INTO cmd2_projection_navigation_initial(domain,consumer_id,selection_json,header_raw,position_json)
             VALUES($1,$2,$3,$4,$5)",
            &[&domain, &consumer, &selection, &header, &position_json(&Position::default())?],
        )?;
    }
    read_position(tx, domain, consumer, selection)
}

fn commit_page(
    tx: &mut Transaction<'_>,
    domain: &str,
    consumer: &str,
    selection: &str,
    previous: &Position,
    next: &Position,
    rows: &[CapturedRow],
    limits: DurableNavigationEventLimits,
) -> DurableResult<()> {
    if read_position(tx, domain, consumer, selection)? != *previous {
        return Err(DurableError::Conflict(
            "navigation initial capture position changed",
        ));
    }
    if rows.len() > PAGE_ROWS {
        return Err(DurableError::Refused(
            "navigation initial capture page rows",
        ));
    }
    let mut raw_bytes = 0u64;
    for row in rows {
        raw_bytes = raw_bytes
            .checked_add(row.raw.len() as u64)
            .filter(|n| *n <= limits.max_total_bytes)
            .ok_or(DurableError::Refused(
                "navigation initial capture page bytes",
            ))?;
        let max_id = if row.collection == "edges" {
            MAX_NAVIGATION_EDGE_ID_BYTES
        } else {
            MAX_NAVIGATION_RECORD_ID_BYTES
        };
        validate_identifier(&row.id, max_id, "navigation initial carrier ID")?;
        if row.raw.len() > limits.max_row_bytes || row.digest != Digest256::of_bytes(&row.raw) {
            return Err(DurableError::Corrupt(
                "navigation initial exact carrier size or digest",
            ));
        }
        let raw = std::str::from_utf8(&row.raw)
            .map_err(|_| DurableError::Corrupt("navigation initial carrier UTF-8"))?;
        // An unexpected duplicate is a conflict, never a silent overwrite.
        tx.execute(
            "INSERT INTO cmd2_projection_navigation_initial_row(
                domain,consumer_id,collection,row_key,row_id,ordinal,row_json,row_sha256)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8)",
            &[
                &domain,
                &consumer,
                &row.collection,
                &row.key,
                &row.id,
                &row.ordinal,
                &raw,
                &row.digest.to_hex(),
            ],
        )?;
    }
    tx.execute(
        "UPDATE cmd2_projection_navigation_initial SET position_json=$3 WHERE domain=$1 AND consumer_id=$2",
        &[&domain, &consumer, &position_json(next)?],
    )?;
    Ok(())
}

fn live(deadline: Instant, cancelled: &AtomicBool) -> DurableResult<()> {
    if Instant::now() >= deadline || cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(DurableError::Refused(
            "navigation initial capture deadline or cancellation; committed pages retained",
        ));
    }
    Ok(())
}

fn add(total: &mut u64, bytes: u64) -> DurableResult<()> {
    *total = total.checked_add(bytes).ok_or(DurableError::Refused(
        "navigation initial work counter overflow",
    ))?;
    Ok(())
}

/// Keyset metadata first, then exact-sized carriers. A changed/oversized value
/// becomes SQL NULL before allocating it in the client. No OFFSET, total COUNT
/// or global raw carrier vector is needed by later normalization/seal phases.
fn read_captured_page<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer: &str,
    collection: &str,
    after: &str,
    limits: DurableNavigationEventLimits,
) -> DurableResult<Vec<CapturedRow>> {
    let cap = limits.max_total_bytes.min(4 * 1024 * 1024);
    let meta = tx.query(
        "SELECT row_key,octet_length(row_json)::bigint FROM cmd2_projection_navigation_initial_row
          WHERE domain=$1 AND consumer_id=$2 AND collection=$3 AND row_key COLLATE \"C\">$4 COLLATE \"C\"
          ORDER BY row_key COLLATE \"C\" LIMIT $5",
        &[&domain, &consumer, &collection, &after, &(PAGE_ROWS as i64)],
    )?;
    let mut keys = Vec::<String>::new();
    let mut sizes = Vec::<i64>::new();
    let mut bytes = 0u64;
    for row in meta {
        let key: String = row.get(0);
        let size = as_u64(row.get(1))?;
        if key.len() > MAX_NAVIGATION_EDGE_ID_BYTES
            || size > limits.max_row_bytes as u64
            || size > cap
        {
            return Err(DurableError::Refused(
                "navigation initial persisted carrier size",
            ));
        }
        let next = bytes.checked_add(size).ok_or(DurableError::Refused(
            "navigation initial page byte overflow",
        ))?;
        if next > cap {
            break;
        }
        bytes = next;
        keys.push(key);
        sizes.push(as_i64(size)?);
    }
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    let stored = tx.query(
        "SELECT r.row_key,r.row_id,r.ordinal,r.row_sha256,
                CASE WHEN octet_length(r.row_json)::bigint=s.size THEN r.row_json END
           FROM unnest($4::text[],$5::bigint[]) s(key,size)
           JOIN cmd2_projection_navigation_initial_row r
             ON r.domain=$1 AND r.consumer_id=$2 AND r.collection=$3 AND r.row_key=s.key
          ORDER BY r.row_key COLLATE \"C\"",
        &[&domain, &consumer, &collection, &keys, &sizes],
    )?;
    if stored.len() != keys.len() {
        return Err(DurableError::Conflict(
            "navigation initial captured page changed",
        ));
    }
    let mut result = Vec::with_capacity(stored.len());
    for (index, row) in stored.into_iter().enumerate() {
        let key: String = row.get(0);
        let raw = row
            .get::<_, Option<String>>(4)
            .ok_or(DurableError::Corrupt(
                "navigation initial captured row length changed",
            ))?
            .into_bytes();
        let digest = parse_digest(row.get(3))?;
        if key != keys[index] || Digest256::of_bytes(&raw) != digest {
            return Err(DurableError::Corrupt(
                "navigation initial captured row digest differs",
            ));
        }
        result.push(CapturedRow {
            collection: collection.into(),
            key,
            id: row.get(1),
            ordinal: row.get(2),
            raw,
            digest,
        });
    }
    Ok(result)
}

/// Explicit full initial closure, streamed once after all capture pages exist.
/// Interruption restarts only this seal pass; captured pages stay committed.
fn verify_captured_roots<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer: &str,
    receipt: &NavigationOriginalReceipt,
    limits: DurableNavigationEventLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<u64> {
    let mut work_bytes = 0u64;
    for (collection, expected_count, expected_root) in [
        ("nodes", receipt.nodes, &receipt.node_input_root_sha256),
        ("edges", receipt.edges, &receipt.edge_input_root_sha256),
        ("rights", receipt.rights, &receipt.rights_root_sha256),
    ] {
        let mut after = String::new();
        let mut count = 0u64;
        let mut hash = Digest256Hasher::new();
        let mut rights_hash = tos_compiler::NavigationOriginalRightsHasher::default();
        loop {
            live(deadline, cancelled)?;
            let rows = read_captured_page(tx, domain, consumer, collection, &after, limits)?;
            if rows.is_empty() {
                break;
            }
            for row in rows {
                add(&mut work_bytes, row.raw.len() as u64)?;
                if collection == "rights" {
                    if row.ordinal != Some(as_i64(count)?) || row.key != format!("{count:020}") {
                        return Err(DurableError::Corrupt(
                            "navigation initial rights ordinal differs",
                        ));
                    }
                    rights_hash
                        .push(as_i64(count)?, &row.raw)
                        .map_err(|_| DurableError::Corrupt("navigation initial rights framing"))?;
                } else {
                    if row.ordinal.is_some() || row.id != row.key {
                        return Err(DurableError::Corrupt(
                            "navigation initial carrier identity differs",
                        ));
                    }
                    navigation_hash_part(&mut hash, row.id.as_bytes());
                    hash.update(row.digest.as_bytes());
                }
                add(&mut count, 1)?;
                if count > expected_count {
                    return Err(DurableError::Conflict(
                        "navigation initial closure count exceeds receipt",
                    ));
                }
                after = row.key;
            }
        }
        let root = if collection == "rights" {
            rights_hash.finish()
        } else {
            hash.finalize().to_hex()
        };
        if count != expected_count || root != *expected_root {
            return Err(DurableError::Conflict(
                "navigation initial complete Original closure differs",
            ));
        }
    }
    Ok(work_bytes)
}

impl DurablePgCoordinator {
    /// The caller holds the real managed source/rights/audit/job/model lease
    /// for the whole invocation. Every sibling transaction rechecks that exact
    /// held cut without trying to upgrade its domain lock. A later invocation
    /// reopens the same protected selection and resumes its durable position.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn capture_navigation_initial_v1<P: ManagedSelectedProof>(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        consumer: &str,
        profile: &DurableProjectionProfile,
        selected: &ManagedAgentSelectedParent<P>,
        model: &VerifiedKnowledgeModel<'_>,
        contract: Digest256,
        rule_version: u64,
        rights_version: u64,
        job_id: &str,
        job_fence: u64,
        limits: DurableNavigationEventLimits,
        max_batches: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<NavigationInitialCaptureProgress> {
        let limits = limits.validate()?;
        if !(1..=256).contains(&max_batches)
            || limits.max_total_bytes < MEMBER_BOUND + 3 * limits.max_row_bytes as u64
        {
            return Err(DurableError::Invalid(
                "navigation initial capture page/call limits",
            ));
        }
        validate_identifier(
            domain,
            super::super::MAX_DOMAIN_BYTES,
            "navigation initial domain",
        )?;
        validate_identifier(
            consumer,
            super::super::MAX_CONSUMER_ID_BYTES,
            "navigation initial consumer",
        )?;
        validate_identifier(job_id, 256, "navigation initial job")?;
        profile.validate()?;
        if store.custody_domain() != domain.as_bytes() || contract != profile.source_contract {
            return Err(DurableError::Conflict(
                "navigation initial source domain or contract differs",
            ));
        }
        live(deadline, cancelled)?;
        model
            .check_pin()
            .map_err(|_| DurableError::Corrupt("navigation initial model pin"))?;
        let receipt = model
            .navigation_original_receipt()
            .map_err(|_| DurableError::Corrupt("navigation initial Original receipt"))?;
        if receipt != selected.navigation_original_receipt() {
            return Err(DurableError::Conflict(
                "navigation initial Original receipt differs from selection",
            ));
        }
        let mut binding = navigation_owner_binding(domain, selected, profile)?;
        let header_page = model
            .navigation_original_page_under_caller_budget(
                None,
                1,
                limits.max_row_bytes,
                limits.max_row_bytes as u64,
            )
            .map_err(|_| DurableError::Corrupt("navigation initial header"))?;
        if header_page.rows.len() != 1 || header_page.rows[0].0 != -1 {
            return Err(DurableError::Corrupt("navigation initial header row"));
        }
        let header = &header_page.rows[0].1;
        if Digest256::of_bytes(header).to_hex() != receipt.header_sha256 {
            return Err(DurableError::Conflict(
                "navigation initial header digest differs",
            ));
        }
        binding.navigation_header = strict_json_value(header, limits.max_row_bytes)?;
        let counts = &binding.navigation_header["counts"];
        if counts["nodes"].as_u64() != Some(receipt.nodes)
            || counts["edges"].as_u64() != Some(receipt.edges)
            || counts["rights"].as_u64() != Some(receipt.rights)
        {
            return Err(DurableError::Conflict(
                "navigation initial header counts differ",
            ));
        }
        let mut tx = self.client.transaction()?;
        set_local_timeouts(&mut tx, limits.statement_timeout)?;
        let snapshot = read_domain_snapshot(&mut tx, domain)?;
        snapshot.require_live_projection(profile)?;
        if snapshot.head_seq()? != selected.generation().commit_seq()
            || snapshot.audit_generation != as_i64(selected.generation().audit_generation())?
            || snapshot.rule_version != as_i64(rule_version)?
            || snapshot.rights_version != as_i64(rights_version)?
        {
            return Err(DurableError::Conflict(
                "navigation initial source/rights/audit fence differs",
            ));
        }
        let selected_cut = SelectedCapture {
            snapshot,
            binding: navigation_owner_binding_json(&binding),
            job_id: job_id.into(),
            job_fence,
            header_sha256: receipt.header_sha256.clone(),
        };
        check_fences(&mut tx, domain, profile, &selected_cut)?;
        let selection = serde_json::to_string(&selected_cut)
            .map_err(|_| DurableError::Invalid("navigation initial selection encoding"))?;
        let mut position = begin_capture(&mut tx, domain, consumer, profile, &selection, header)?;
        tx.commit()?;
        let mut call_work_bytes = header.len() as u64;
        let mut committed_batches = 0;
        while matches!(position.phase.as_str(), "nodes" | "edges" | "rights")
            && committed_batches < max_batches
        {
            live(deadline, cancelled)?;
            let (next, rows, work) =
                capture_page(model, &receipt, &position, limits, deadline, cancelled)?;
            model
                .check_pin()
                .map_err(|_| DurableError::Corrupt("navigation initial model pin changed"))?;
            live(deadline, cancelled)?;
            let mut tx = self.client.transaction()?;
            set_local_timeouts(&mut tx, limits.statement_timeout)?;
            check_fences(&mut tx, domain, profile, &selected_cut)?;
            commit_page(
                &mut tx, domain, consumer, &selection, &position, &next, &rows, limits,
            )?;
            tx.commit()?;
            position = next;
            committed_batches += 1;
            add(&mut call_work_bytes, work)?;
        }
        if position.phase == "captured" && committed_batches < max_batches {
            live(deadline, cancelled)?;
            let mut tx = self.client.transaction()?;
            set_local_timeouts(&mut tx, limits.statement_timeout)?;
            check_fences(&mut tx, domain, profile, &selected_cut)?;
            if read_position(&mut tx, domain, consumer, &selection)? != position {
                return Err(DurableError::Conflict(
                    "navigation initial closure position changed",
                ));
            }
            let work = verify_captured_roots(
                &mut tx, domain, consumer, &receipt, limits, deadline, cancelled,
            )?;
            let mut next = position.clone();
            next.phase = "verified".into();
            add(&mut next.work_bytes, work)?;
            live(deadline, cancelled)?;
            commit_page(
                &mut tx,
                domain,
                consumer,
                &selection,
                &position,
                &next,
                &[],
                limits,
            )?;
            tx.commit()?;
            position = next;
            add(&mut call_work_bytes, work)?;
            committed_batches += 1;
        }
        Ok(NavigationInitialCaptureProgress {
            captured: matches!(position.phase.as_str(), "captured" | "verified"),
            verified: position.phase == "verified",
            phase: position.phase,
            after: position.after,
            nodes: position.nodes,
            edges: position.edges,
            rights: position.rights,
            raw_bytes: position.raw_bytes,
            total_work_bytes: position.work_bytes,
            call_work_bytes,
            committed_batches,
        })
    }
}

fn check_fences(
    tx: &mut Transaction<'_>,
    domain: &str,
    profile: &DurableProjectionProfile,
    selected: &SelectedCapture,
) -> DurableResult<()> {
    let snapshot = read_domain_snapshot(tx, domain)?;
    snapshot.require_live_projection(profile)?;
    if snapshot != selected.snapshot {
        return Err(DurableError::Conflict(
            "navigation initial held source fence changed",
        ));
    }
    let row = tx.query_one(
        "SELECT source_membership_digest,source_membership_count,selected_generation_digest,source_projection_digest
           FROM cmd2_domain WHERE domain=$1", &[&domain],
    )?;
    let b = &selected.binding;
    if row.get::<_, Option<String>>(0).as_deref() != b["source_membership_root"].as_str()
        || row.get::<_, Option<i64>>(1).map(as_u64).transpose()?
            != b["source_membership_count"].as_u64()
        || row.get::<_, Option<String>>(2).as_deref() != b["managed_generation_digest"].as_str()
        || row.get::<_, Option<String>>(3).as_deref() != b["source_projection_digest"].as_str()
        || !snapshot.source_complete
        || snapshot.source_generation != Some(snapshot.head_seq)
    {
        return Err(DurableError::Conflict(
            "navigation initial managed source currentness differs",
        ));
    }
    let job = tx.query_opt(
        "SELECT fence_epoch FROM cmd2_job WHERE domain=$1 AND job_id=$2",
        &[&domain, &selected.job_id],
    )?;
    if job.map(|row| row.get::<_, i64>(0)) != Some(as_i64(selected.job_fence)?) {
        return Err(DurableError::Conflict(
            "navigation initial job fence changed",
        ));
    }
    Ok(())
}

fn capture_page(
    model: &VerifiedKnowledgeModel<'_>,
    receipt: &NavigationOriginalReceipt,
    previous: &Position,
    limits: DurableNavigationEventLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<(Position, Vec<CapturedRow>, u64)> {
    let mut next = previous.clone();
    let mut rows = Vec::new();
    let mut work = 0u64;
    match previous.phase.as_str() {
        "nodes" | "edges" => {
            let collection = previous.phase.as_str();
            let page_rows = PAGE_ROWS.min(
                ((limits.max_total_bytes - 3 * limits.max_row_bytes as u64) / MEMBER_BOUND).max(1)
                    as usize,
            );
            let page = model
                .navigation_original_members_under_caller_budget(
                    collection,
                    (!previous.after.is_empty()).then_some(previous.after.as_str()),
                    page_rows,
                    MEMBER_BOUND * page_rows as u64,
                )
                .map_err(|_| {
                    DurableError::Corrupt("navigation initial Original membership page")
                })?;
            add(&mut work, page.decoded_bytes)?;
            let expected = if collection == "nodes" {
                receipt.nodes
            } else {
                receipt.edges
            };
            if page.rows.is_empty() {
                let observed = if collection == "nodes" {
                    next.nodes
                } else {
                    next.edges
                };
                if observed != expected {
                    return Err(DurableError::Conflict(
                        "navigation initial Original count differs",
                    ));
                }
                next.phase = if collection == "nodes" {
                    "edges"
                } else {
                    "rights"
                }
                .into();
                next.after.clear();
            }
            for member in page.rows {
                live(deadline, cancelled)?;
                // Keep enough room for one worst-case exact point read. The
                // unused metadata suffix is read again after the last committed
                // ID; none of its carrier bytes have been materialized.
                if !rows.is_empty()
                    && limits.max_total_bytes.saturating_sub(work)
                        < 3 * limits.max_row_bytes as u64 + MEMBER_BOUND
                {
                    break;
                }
                if member.collection != collection || member.id <= next.after {
                    return Err(DurableError::Corrupt(
                        "navigation initial Original member order",
                    ));
                }
                let original =
                    tos_compiler::managed_agent_producer::managed_navigation_original_member_bytes(
                        model,
                        collection,
                        &member.id,
                        tos_compiler::NavigationOriginalLimits {
                            max_rows: limits.max_current_rows as u64,
                            max_row_bytes: limits.max_row_bytes,
                            max_total_bytes: limits.max_total_bytes.saturating_sub(work),
                        },
                    )
                    .map_err(|_| DurableError::Corrupt("navigation initial exact Original member"))?
                    .ok_or(DurableError::Corrupt(
                        "navigation initial Original member absent",
                    ))?;
                add(&mut work, original.work_bytes)?;
                if work > limits.max_total_bytes {
                    return Err(DurableError::Refused("navigation initial page work budget"));
                }
                let count = if collection == "nodes" {
                    &mut next.nodes
                } else {
                    &mut next.edges
                };
                add(count, 1)?;
                if *count > expected {
                    return Err(DurableError::Conflict(
                        "navigation initial Original count differs",
                    ));
                }
                add(&mut next.raw_bytes, original.raw.len() as u64)?;
                next.after = member.id.clone();
                rows.push(CapturedRow {
                    collection: collection.into(),
                    key: member.id.clone(),
                    ordinal: None,
                    id: member.id,
                    digest: Digest256::of_bytes(&original.raw),
                    raw: original.raw,
                });
            }
        }
        "rights" => {
            let after = if previous.rights == 0 {
                -1
            } else {
                as_i64(previous.rights - 1)?
            };
            let page_bytes = limits.max_total_bytes.min(64 * 1024 * 1024);
            let page_rows = PAGE_ROWS.min((page_bytes / limits.max_row_bytes as u64) as usize);
            let page = model
                .navigation_original_page_under_caller_budget(
                    Some(after),
                    page_rows,
                    limits.max_row_bytes,
                    page_bytes,
                )
                .map_err(|_| DurableError::Corrupt("navigation initial Original rights page"))?;
            if page.rows.is_empty() {
                if next.rights != receipt.rights {
                    return Err(DurableError::Conflict(
                        "navigation initial Original rights count differs",
                    ));
                }
                next.phase = "captured".into();
                next.after.clear();
            }
            for (ordinal, raw) in page.rows {
                live(deadline, cancelled)?;
                if ordinal != as_i64(next.rights)? || next.rights >= receipt.rights {
                    return Err(DurableError::Conflict(
                        "navigation initial Original rights ordinal differs",
                    ));
                }
                let value = strict_json_value(&raw, limits.max_row_bytes)?;
                let id = value
                    .get("rights_id")
                    .and_then(Value::as_str)
                    .ok_or(DurableError::Corrupt("navigation initial rights identity"))?
                    .to_owned();
                add(&mut work, raw.len() as u64)?;
                add(&mut next.raw_bytes, raw.len() as u64)?;
                add(&mut next.rights, 1)?;
                let key = format!("{ordinal:020}");
                next.after = key.clone();
                rows.push(CapturedRow {
                    collection: "rights".into(),
                    key,
                    id,
                    ordinal: Some(ordinal),
                    digest: Digest256::of_bytes(&raw),
                    raw,
                });
            }
        }
        _ => return Err(DurableError::Corrupt("navigation initial capture phase")),
    }
    add(&mut next.work_bytes, work)?;
    Ok((next, rows, work))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_initial_rights_stream_keeps_existing_framing_and_ordinals() {
        let mut hash = tos_compiler::NavigationOriginalRightsHasher::default();
        assert!(hash.push(1, b"{}").is_err());
        hash.push(0, b"{}").unwrap();
        hash.push(1, b"{}").unwrap();
        assert_eq!(
            hash.finish(),
            "9783345b17029d37edf3aeed983d0c51477caf861b622efc1ded3932601ef508"
        );
    }

    #[cfg(feature = "postgres-lab")]
    #[test]
    #[ignore = "requires explicit isolated TOS_CMD_POSTGRES_URL"]
    fn navigation_initial_capture_retains_committed_pages_and_refuses_partial_closure() {
        let url = std::env::var("TOS_CMD_POSTGRES_URL").expect("explicit private PG lab URL");
        let mut client = postgres::Client::connect(&url, postgres::NoTls).unwrap();
        client.batch_execute(
            "CREATE TEMP TABLE cmd2_projection_navigation_consumer(
               domain text,consumer_id text,profile_id text,profile_version text,source_contract_digest text,
               cursor_seq bigint DEFAULT 0,coverage_seq bigint DEFAULT 0,adjacency_ready boolean DEFAULT false,
               PRIMARY KEY(domain,consumer_id));
             CREATE TEMP TABLE cmd2_projection_navigation_bootstrap(domain text,consumer_id text);
             CREATE TEMP TABLE cmd2_projection_navigation_cohort(domain text,consumer_id text);
             CREATE TEMP TABLE cmd2_projection_navigation_initial(
               domain text,consumer_id text,selection_json text,header_raw text,position_json text,
               PRIMARY KEY(domain,consumer_id));
             CREATE TEMP TABLE cmd2_projection_navigation_initial_row(
               domain text,consumer_id text,collection text,row_key text,row_id text,ordinal bigint,
               row_json text,row_sha256 text,PRIMARY KEY(domain,consumer_id,collection,row_key));
             CREATE INDEX ON cmd2_projection_navigation_initial_row(domain,consumer_id,collection,row_key COLLATE \"C\");"
        ).unwrap();
        let profile = DurableProjectionProfile {
            id: "test".into(),
            version: 2,
            source_contract: Digest256::of_bytes(b"contract"),
        };
        let limits = DurableNavigationEventLimits {
            max_current_rows: 128,
            max_row_bytes: 64 * 1024,
            max_total_bytes: 1024 * 1024,
            max_adjacency_bytes: 1024 * 1024,
            ..Default::default()
        };
        let mut tx = client.transaction().unwrap();
        let initial = begin_capture(
            &mut tx,
            "lab",
            "initial",
            &profile,
            "selected",
            "{}".as_bytes(),
        )
        .unwrap();
        tx.commit().unwrap();
        let make_row = |n: usize| {
            let id = format!("node:{n:06}");
            let raw =
                serde_json::to_vec(&json!({"node_id":id,"payload":"x".repeat(7000)})).unwrap();
            CapturedRow {
                collection: "nodes".into(),
                key: id.clone(),
                id,
                ordinal: None,
                digest: Digest256::of_bytes(&raw),
                raw,
            }
        };
        let mut first = initial.clone();
        let first_rows = (0..128).map(make_row).collect::<Vec<_>>();
        first.after = first_rows.last().unwrap().key.clone();
        first.nodes = 128;
        first.raw_bytes = first_rows.iter().map(|r| r.raw.len() as u64).sum();
        let mut tx = client.transaction().unwrap();
        commit_page(
            &mut tx,
            "lab",
            "initial",
            "selected",
            &initial,
            &first,
            &first_rows,
            limits,
        )
        .unwrap();
        tx.commit().unwrap();
        // Simulated lost response: a fresh transaction resumes the committed
        // key; replay of the old position cannot insert the page twice.
        let mut tx = client.transaction().unwrap();
        assert_eq!(
            begin_capture(&mut tx, "lab", "initial", &profile, "selected", b"{}").unwrap(),
            first
        );
        assert!(
            commit_page(
                &mut tx,
                "lab",
                "initial",
                "selected",
                &initial,
                &first,
                &first_rows,
                limits
            )
            .is_err()
        );
        assert!(read_position(&mut tx, "lab", "initial", "different-selection").is_err());
        assert!(require_no_active_initial(&mut tx, "lab", "initial").is_err());
        tx.rollback().unwrap();
        let second_rows = (128..256).map(make_row).collect::<Vec<_>>();
        let mut second = first.clone();
        second.after = second_rows.last().unwrap().key.clone();
        second.nodes = 256;
        second.raw_bytes += second_rows.iter().map(|r| r.raw.len() as u64).sum::<u64>();
        let mut tx = client.transaction().unwrap();
        commit_page(
            &mut tx,
            "lab",
            "initial",
            "selected",
            &first,
            &second,
            &second_rows,
            limits,
        )
        .unwrap();
        tx.rollback().unwrap();
        let mut tx = client.transaction().unwrap();
        assert_eq!(
            read_position(&mut tx, "lab", "initial", "selected").unwrap(),
            first
        );
        assert_eq!(
            tx.query_one(
                "SELECT count(*) FROM cmd2_projection_navigation_initial_row",
                &[]
            )
            .unwrap()
            .get::<_, i64>(0),
            128
        );
        commit_page(
            &mut tx,
            "lab",
            "initial",
            "selected",
            &first,
            &second,
            &second_rows,
            limits,
        )
        .unwrap();
        tx.commit().unwrap();
        let last_rows = (256..300).map(make_row).collect::<Vec<_>>();
        let mut last = second.clone();
        last.after = last_rows.last().unwrap().key.clone();
        last.nodes = 300;
        last.raw_bytes += last_rows.iter().map(|r| r.raw.len() as u64).sum::<u64>();
        let mut tx = client.transaction().unwrap();
        commit_page(
            &mut tx, "lab", "initial", "selected", &second, &last, &last_rows, limits,
        )
        .unwrap();
        tx.commit().unwrap();
        assert!(
            last.nodes > limits.max_current_rows as u64 && last.raw_bytes > limits.max_total_bytes
        );
        let mut root = Digest256Hasher::new();
        for n in 0..300 {
            let row = make_row(n);
            navigation_hash_part(&mut root, row.id.as_bytes());
            root.update(row.digest.as_bytes());
        }
        let receipt = NavigationOriginalReceipt {
            profile: tos_compiler::NAVIGATION_ORIGINAL_PROFILE.into(),
            descriptor_sha256: String::new(),
            source_cut: String::new(),
            membership_root: String::new(),
            source_graph: String::new(),
            nodes: 300,
            edges: 0,
            rights: 0,
            node_input_root_sha256: root.finalize().to_hex(),
            edge_input_root_sha256: Digest256::of_bytes(b"").to_hex(),
            header_sha256: Digest256::of_bytes(b"{}").to_hex(),
            rights_root_sha256: tos_compiler::navigation_original_rights_root(&[]),
            component_root_sha256: String::new(),
            total_bytes: 0,
            member_index_root_sha256: String::new(),
            member_index_bytes: 0,
        };
        let cancelled = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(20);
        assert_eq!(
            verify_captured_roots(
                &mut client,
                "lab",
                "initial",
                &receipt,
                limits,
                deadline,
                &cancelled
            )
            .unwrap(),
            last.raw_bytes
        );
        assert!(
            !client
                .query_one(
                    "SELECT adjacency_ready FROM cmd2_projection_navigation_consumer",
                    &[]
                )
                .unwrap()
                .get::<_, bool>(0)
        );
        // Tampered raw bytes and a missing selected member fail independently.
        let mut tx = client.transaction().unwrap();
        tx.execute("UPDATE cmd2_projection_navigation_initial_row SET row_json='{}' WHERE row_key='node:000000'",&[]).unwrap();
        assert!(
            verify_captured_roots(
                &mut tx, "lab", "initial", &receipt, limits, deadline, &cancelled
            )
            .is_err()
        );
        tx.rollback().unwrap();
        let mut tx = client.transaction().unwrap();
        tx.execute(
            "DELETE FROM cmd2_projection_navigation_initial_row WHERE row_key='node:000001'",
            &[],
        )
        .unwrap();
        assert!(
            verify_captured_roots(
                &mut tx, "lab", "initial", &receipt, limits, deadline, &cancelled
            )
            .is_err()
        );
        tx.rollback().unwrap();
        let mut tx = client.transaction().unwrap();
        tx.execute("UPDATE cmd2_projection_navigation_initial_row SET row_json=repeat('x',65537) WHERE row_key='node:000000'",&[]).unwrap();
        assert!(matches!(
            read_captured_page(&mut tx, "lab", "initial", "nodes", "", limits),
            Err(DurableError::Refused(_))
        ));
        tx.rollback().unwrap();
        println!(
            "navigation initial persisted capture: 300 exact rows, {} bytes; per-page ceiling {} bytes; rollback/resume and raw/membership tamper refused",
            last.raw_bytes, limits.max_total_bytes
        );
    }
}
