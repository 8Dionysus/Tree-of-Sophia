//! Bounded normalization of the retained initial Original. These tables remain
//! private until the source-backed closure and publication phase succeeds.
use super::*;
use crate::source_navigation_bootstrap::{
    bind_historical_record_owner, claim_contribution, decode_original_carrier, maker_identity,
};
use std::cell::{Cell, RefCell};

type Owner = (String, String);

// Fixed private tables only. Verification rebuilds the contribution relation
// without trusting a persisted phase, counter, or derived metadata row.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MetadataTarget {
    Staged,
    Verification,
}
impl MetadataTarget {
    fn table(self) -> &'static str {
        match self {
            Self::Staged => "cmd2_projection_navigation_initial_contributor",
            Self::Verification => "pg_temp.tos_initial_verify_contributor",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct NodeFacts {
    pub(super) visible: bool,
    // Only the current record node may supply a historical carrier's owner.
    current_owner: Option<String>,
}

pub(super) fn derive_node_facts(
    captured: &CapturedRow,
    limits: DurableNavigationEventLimits,
) -> DurableResult<NodeFacts> {
    let (row, _) = decode_original_carrier(
        DurableNavigationBootstrapRowKindV1::Node,
        captured.id.clone(),
        captured.raw.clone(),
        limits,
        |_| Ok(None),
        |_| Ok(None),
    )?;
    Ok(NodeFacts {
        visible: row
            .visible
            .ok_or(DurableError::Corrupt("node visibility"))?,
        current_owner: (row.id == row.owner_record_id).then_some(row.owner_source_path),
    })
}

pub(super) fn node_facts<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer: &str,
    id: &str,
) -> DurableResult<Option<(NodeFacts, u64)>> {
    validate_identifier(
        id,
        MAX_NAVIGATION_RECORD_ID_BYTES,
        "navigation initial node lookup",
    )?;
    let Some(row) = tx.query_opt(
        "SELECT CASE WHEN octet_length(node_facts_json)<=8192 THEN node_facts_json END
           FROM cmd2_projection_navigation_initial_row
          WHERE domain=$1 AND consumer_id=$2 AND collection='nodes' AND row_key=$3",
        &[&domain, &consumer, &id],
    )?
    else {
        return Ok(None);
    };
    let raw = row
        .get::<_, Option<String>>(0)
        .ok_or(DurableError::Corrupt(
            "navigation initial node facts absent or oversized",
        ))?;
    let facts: NodeFacts = serde_json::from_str(&raw)
        .map_err(|_| DurableError::Corrupt("navigation initial node facts encoding"))?;
    if let Some(path) = &facts.current_owner {
        validate_relative_path(path)?;
    }
    Ok(Some((facts, raw.len() as u64)))
}

pub(super) fn decode<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer: &str,
    captured: &CapturedRow,
    limits: DurableNavigationEventLimits,
) -> DurableResult<(DurableNavigationBootstrapRowV1, u64)> {
    let kind = match captured.collection.as_str() {
        "nodes" => DurableNavigationBootstrapRowKindV1::Node,
        "edges" => DurableNavigationBootstrapRowKindV1::Edge,
        "rights" => DurableNavigationBootstrapRowKindV1::Right,
        _ => return Err(DurableError::Corrupt("navigation initial collection")),
    };
    let tx = RefCell::new(tx);
    let work = Cell::new(0u64);
    let lookup = |id: &str| -> DurableResult<Option<NodeFacts>> {
        let result = node_facts(&mut **tx.borrow_mut(), domain, consumer, id)?;
        if let Some((_, bytes)) = &result {
            let mut total = work.get();
            add(&mut total, *bytes)?;
            work.set(total);
        }
        Ok(result.map(|(facts, _)| facts))
    };
    let (mut row, _) = decode_original_carrier(
        kind,
        captured.id.clone(),
        captured.raw.clone(),
        limits,
        |id| Ok(lookup(id)?.map(|f| f.visible)),
        |id| Ok(lookup(id)?.and_then(|f| f.current_owner)),
    )?;
    bind_historical_record_owner(&mut row, limits, |id| {
        Ok(lookup(id)?.and_then(|f| f.current_owner))
    })?;
    Ok((row, work.get()))
}

fn contributors<C: GenericClient>(
    target: MetadataTarget,
    tx: &mut C,
    domain: &str,
    consumer: &str,
    id: &str,
    limits: DurableNavigationEventLimits,
) -> DurableResult<(Vec<Owner>, u64)> {
    let rows = tx.query(
        &format!(
            "SELECT CASE WHEN octet_length(owner_source_path)<=4096 THEN owner_source_path END,
                CASE WHEN octet_length(owner_record_id)<=256 THEN owner_record_id END
           FROM {}
          WHERE domain=$1 AND consumer_id=$2 AND node_id=$3
          ORDER BY owner_source_path COLLATE \"C\",owner_record_id COLLATE \"C\" LIMIT $4",
            target.table()
        ),
        &[
            &domain,
            &consumer,
            &id,
            &((limits.max_dependencies + 1) as i64),
        ],
    )?;
    if rows.len() > limits.max_dependencies {
        return Err(DurableError::Refused(
            "navigation initial per-carrier contributor limit",
        ));
    }
    let mut work = 0;
    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        let path = row
            .get::<_, Option<String>>(0)
            .ok_or(DurableError::Corrupt(
                "navigation initial contributor path size",
            ))?;
        let id = row
            .get::<_, Option<String>>(1)
            .ok_or(DurableError::Corrupt(
                "navigation initial contributor identity size",
            ))?;
        validate_relative_path(&path)?;
        validate_identifier(
            &id,
            MAX_NAVIGATION_RECORD_ID_BYTES,
            "navigation initial contributor ID",
        )?;
        add(&mut work, (path.len() + id.len()) as u64)?;
        result.push((path, id));
    }
    Ok((result, work))
}

fn insert_contributor<C: GenericClient>(
    target: MetadataTarget,
    tx: &mut C,
    domain: &str,
    consumer: &str,
    node: &str,
    owner: &Owner,
) -> DurableResult<()> {
    tx.execute(
        &format!(
            "INSERT INTO {}
         (domain,consumer_id,node_id,owner_source_path,owner_record_id) VALUES($1,$2,$3,$4,$5)
         ON CONFLICT(domain,consumer_id,node_id,owner_source_path,owner_record_id) DO NOTHING",
            target.table()
        ),
        &[&domain, &consumer, &node, &owner.0, &owner.1],
    )?;
    Ok(())
}

fn page_limits(limits: DurableNavigationEventLimits) -> DurableNavigationEventLimits {
    DurableNavigationEventLimits {
        max_total_bytes: (limits.max_total_bytes / 4)
            .min(4 * 1024 * 1024)
            .max(limits.max_row_bytes as u64),
        ..limits
    }
}

/// Node metadata precedes Claim edges; maker links are processed once in the
/// same bytewise node order as the existing in-memory rule. A page and its
/// position are committed by the caller in the same transaction.
pub(super) fn metadata_page(
    tx: &mut Transaction<'_>,
    domain: &str,
    consumer: &str,
    position: &Position,
    limits: DurableNavigationEventLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<(Position, u64)> {
    metadata_page_to(
        MetadataTarget::Staged,
        tx,
        domain,
        consumer,
        position,
        limits,
        deadline,
        cancelled,
    )
}

pub(super) fn metadata_page_to(
    target: MetadataTarget,
    tx: &mut Transaction<'_>,
    domain: &str,
    consumer: &str,
    position: &Position,
    limits: DurableNavigationEventLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<(Position, u64)> {
    let (collection, following) = match position.phase.as_str() {
        "node_facts" => ("nodes", "claim_contributors"),
        "claim_contributors" => ("edges", "maker_contributors"),
        "maker_contributors" => ("nodes", "normalize_nodes"),
        _ => return Err(DurableError::Corrupt("navigation initial metadata phase")),
    };
    let rows = read_captured_page(
        tx,
        domain,
        consumer,
        collection,
        &position.after,
        page_limits(limits),
    )?;
    let mut next = position.clone();
    let mut work = rows.iter().map(|row| row.raw.len() as u64).sum::<u64>();
    let mut writes = 0usize;
    if rows.is_empty() {
        next.phase = following.into();
        next.after.clear();
    }
    for (index, captured) in rows.iter().enumerate() {
        live(deadline, cancelled)?;
        match position.phase.as_str() {
            "node_facts" => {
                let facts = derive_node_facts(captured, limits)?;
                let raw = serde_json::to_string(&facts)
                    .map_err(|_| DurableError::Invalid("navigation initial node facts JSON"))?;
                if raw.len() > 8192 {
                    return Err(DurableError::Refused("navigation initial node facts size"));
                }
                add(&mut work, raw.len() as u64)?;
                if target == MetadataTarget::Verification {
                    let stored = node_facts(tx, domain, consumer, &captured.id)?.ok_or(
                        DurableError::Corrupt("navigation initial node facts missing"),
                    )?;
                    add(&mut work, stored.1)?;
                    if stored.0 != facts {
                        return Err(DurableError::Conflict(
                            "navigation initial node facts differ from Original",
                        ));
                    }
                } else if tx.execute(
                    "UPDATE cmd2_projection_navigation_initial_row SET node_facts_json=$4
                      WHERE domain=$1 AND consumer_id=$2 AND collection='nodes' AND row_key=$3
                        AND node_facts_json IS NULL",
                    &[&domain, &consumer, &captured.key, &raw],
                )? != 1
                {
                    return Err(DurableError::Conflict(
                        "navigation initial node facts already present",
                    ));
                }
            }
            "claim_contributors" => {
                let (row, bytes) = decode(tx, domain, consumer, captured, limits)?;
                add(&mut work, bytes)?;
                if let Some((owner, targets)) = claim_contribution(&row, limits)? {
                    if writes + targets.len() > limits.max_cohort_rows && index > 0 {
                        break;
                    }
                    if targets.len() > limits.max_cohort_rows {
                        return Err(DurableError::Refused(
                            "navigation initial contributor transaction rows",
                        ));
                    }
                    for target in targets {
                        let (_, bytes) = node_facts(tx, domain, consumer, &target)?.ok_or(
                            DurableError::Corrupt("navigation Claim contribution carrier absent"),
                        )?;
                        add(&mut work, bytes)?;
                        insert_contributor(target, tx, domain, consumer, &target, &owner)?;
                        writes += 1;
                    }
                }
            }
            "maker_contributors" => {
                // No historical rebinding is needed to read this exact property.
                let (row, _) = decode_original_carrier(
                    DurableNavigationBootstrapRowKindV1::Node,
                    captured.id.clone(),
                    captured.raw.clone(),
                    limits,
                    |_| Ok(None),
                    |_| Ok(None),
                )?;
                if let Some(identity) = maker_identity(&row, limits)?.filter(|id| {
                    validate_identifier(id, MAX_NAVIGATION_RECORD_ID_BYTES, "maker identity")
                        .is_ok()
                }) {
                    if let Some((_, bytes)) = node_facts(tx, domain, consumer, &identity)? {
                        add(&mut work, bytes)?;
                        let (owners, bytes) =
                            contributors(target, tx, domain, consumer, &row.id, limits)?;
                        add(&mut work, bytes)?;
                        if writes + owners.len() > limits.max_cohort_rows && index > 0 {
                            break;
                        }
                        if owners.len() > limits.max_cohort_rows {
                            return Err(DurableError::Refused(
                                "navigation initial maker transaction rows",
                            ));
                        }
                        for owner in owners {
                            insert_contributor(target, tx, domain, consumer, &identity, &owner)?;
                            writes += 1;
                        }
                    }
                }
            }
            _ => unreachable!(),
        }
        if work > limits.max_total_bytes {
            return Err(DurableError::Refused(
                "navigation initial metadata page bytes",
            ));
        }
        next.after = captured.key.clone();
    }
    add(&mut next.work_bytes, work)?;
    Ok((next, work))
}

pub(super) struct NormalizedPage {
    pub next: Position,
    pub rows: Vec<DurableNavigationBaseRowReadV1>,
    pub dependencies: BTreeSet<(String, String, String)>,
    pub paths: BTreeSet<String>,
    pub work: u64,
}

/// Read a bounded source page and derive final carriers with the shared owner
/// rules. A per-carrier fanout must fit one transaction; the total tree is not
/// charged against that transaction's envelope.
pub(super) fn prepare_page<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer: &str,
    position: &Position,
    limits: DurableNavigationEventLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<NormalizedPage> {
    let (collection, following) = match position.phase.as_str() {
        "normalize_nodes" => ("nodes", "normalize_edges"),
        "normalize_edges" => ("edges", "normalize_rights"),
        "normalize_rights" => ("rights", "normalized"),
        _ => {
            return Err(DurableError::Corrupt(
                "navigation initial normalization phase",
            ));
        }
    };
    let captured = read_captured_page(
        tx,
        domain,
        consumer,
        collection,
        &position.after,
        page_limits(limits),
    )?;
    let mut result = NormalizedPage {
        next: position.clone(),
        rows: Vec::new(),
        dependencies: BTreeSet::new(),
        paths: BTreeSet::new(),
        work: captured.iter().map(|row| row.raw.len() as u64).sum(),
    };
    if captured.is_empty() {
        result.next.phase = following.into();
        result.next.after.clear();
    }
    let mut output_bytes = 0u64;
    for original in captured {
        live(deadline, cancelled)?;
        let (row, bytes) = decode(tx, domain, consumer, &original, limits)?;
        add(&mut result.work, bytes)?;
        let owners = match row.kind {
            DurableNavigationBootstrapRowKindV1::Node => {
                let (owners, bytes) = contributors(
                    MetadataTarget::Staged,
                    tx,
                    domain,
                    consumer,
                    &row.id,
                    limits,
                )?;
                add(&mut result.work, bytes)?;
                owners
            }
            DurableNavigationBootstrapRowKindV1::Edge => claim_contribution(&row, limits)?
                .map(|(owner, _)| vec![owner])
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let owners = if owners.is_empty() {
            vec![(row.owner_source_path.clone(), row.owner_record_id.clone())]
        } else {
            owners
        };
        let expanded_bytes = (row.raw.len() as u64)
            .checked_mul(owners.len() as u64)
            .ok_or(DurableError::Refused(
                "navigation initial fanout bytes overflow",
            ))?;
        if owners.len() > limits.max_cohort_rows || expanded_bytes > limits.max_total_bytes / 2 {
            return Err(DurableError::Refused(
                "navigation initial single-carrier transaction envelope",
            ));
        }
        if result.rows.len() + owners.len() > limits.max_cohort_rows
            || output_bytes + expanded_bytes > limits.max_total_bytes / 2
        {
            break;
        }
        let mut local_work = 0u64;
        let mut local_rows = Vec::with_capacity(owners.len());
        let mut local_deps = BTreeSet::new();
        let mut local_paths = BTreeSet::new();
        for (path, id) in owners {
            let mut contribution = row.clone();
            contribution.owner_source_path = path;
            contribution.owner_record_id = id;
            let (base, dependencies) = prepare_navigation_bootstrap_row(contribution, limits)?;
            // Charge every encoded SQL field, including reverse dependencies
            // and source-witness metadata, before retaining the next fanout.
            add(&mut local_work, base.raw.len() as u64)?;
            add(
                &mut local_work,
                (base.kind.as_str().len()
                    + base.id.len()
                    + base.owner_source_path.len()
                    + base.owner_record_id.len()
                    + base.from_id.as_ref().map_or(0, String::len)
                    + base.to_id.as_ref().map_or(0, String::len)
                    + 65) as u64,
            )?;
            for path in dependencies {
                if local_paths.insert(path.clone()) {
                    add(&mut local_work, (path.len() + 72) as u64)?;
                }
                let size = base.owner_source_path.len() + base.owner_record_id.len() + path.len();
                if local_deps.insert((
                    base.owner_source_path.clone(),
                    base.owner_record_id.clone(),
                    path,
                )) {
                    add(&mut local_work, size as u64)?;
                }
                if local_work > limits.max_total_bytes / 2 {
                    return Err(DurableError::Refused(
                        "navigation initial carrier and dependency transaction bytes",
                    ));
                }
            }
            local_rows.push(base);
        }
        if result.paths.union(&local_paths).count() > limits.max_dependencies {
            if result.rows.is_empty() {
                return Err(DurableError::Refused(
                    "navigation initial source page paths",
                ));
            }
            break;
        }
        if output_bytes
            .checked_add(local_work)
            .is_none_or(|bytes| bytes > limits.max_total_bytes / 2)
        {
            break;
        }
        output_bytes += local_work;
        result.rows.extend(local_rows);
        result.dependencies.extend(local_deps);
        result.paths.extend(local_paths);
        result.next.after = original.key;
    }
    add(&mut result.work, output_bytes)?;
    if result.work > limits.max_total_bytes {
        return Err(DurableError::Refused(
            "navigation initial normalized page bytes",
        ));
    }
    add(&mut result.next.work_bytes, result.work)?;
    Ok(result)
}

pub(super) fn begin_rows(
    tx: &mut Transaction<'_>,
    domain: &str,
    consumer: &str,
    selected: &SelectedCapture,
    header: &[u8],
) -> DurableResult<()> {
    let binding = navigation_owner_binding_from_json(&selected.binding)?;
    tx.execute(
        "INSERT INTO cmd2_projection_navigation_bootstrap(domain,consumer_id,through_seq,source_revision,
          bootstrap_root,owner_binding_json,header_raw,header_sha256,node_count,edge_count,right_count,
          projected_right_count,diagnostic_count) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,0,0)",
        &[&domain, &consumer, &as_i64(binding.managed_generation_commit_sequence)?,
          &binding.selected_membership_root.to_hex(), &"0".repeat(64),
          &serde_json::to_string(&selected.binding).map_err(|_| DurableError::Invalid("initial binding JSON"))?,
          &std::str::from_utf8(header).map_err(|_| DurableError::Corrupt("initial header UTF-8"))?,
          &selected.header_sha256, &as_i64(binding.navigation_nodes)?, &as_i64(binding.navigation_edges)?,
          &as_i64(binding.navigation_rights)?],
    )?;
    Ok(())
}

pub(super) fn persist_page(
    tx: &mut Transaction<'_>,
    domain: &str,
    consumer: &str,
    page: &mut NormalizedPage,
    owners: &[DurableNavigationBootstrapOwnerSourceV1],
    limits: DurableNavigationEventLimits,
) -> DurableResult<()> {
    for owner in owners {
        tx.execute(
            "INSERT INTO cmd2_projection_navigation_bootstrap_owner(domain,consumer_id,owner_source_path,source_sha256,size_bytes)
             VALUES($1,$2,$3,$4,$5) ON CONFLICT(domain,consumer_id,owner_source_path) DO NOTHING",
            &[&domain, &consumer, &owner.source_path, &owner.sha256.to_hex(), &as_i64(owner.size_bytes)?],
        )?;
        let stored = tx.query_one(
            "SELECT source_sha256,size_bytes FROM cmd2_projection_navigation_bootstrap_owner
              WHERE domain=$1 AND consumer_id=$2 AND owner_source_path=$3",
            &[&domain, &consumer, &owner.source_path],
        )?;
        if parse_digest(stored.get(0))? != owner.sha256
            || as_u64(stored.get(1))? != owner.size_bytes
        {
            return Err(DurableError::Conflict(
                "navigation initial source witness changed",
            ));
        }
    }
    for row in &page.rows {
        if row.kind == DurableNavigationBootstrapRowKindV1::Right {
            let prior = tx.query(
                "SELECT row_sha256,CASE WHEN octet_length(row_json)::bigint<=$4 THEN row_json END
                 FROM cmd2_projection_navigation_bootstrap_row WHERE domain=$1 AND consumer_id=$2
                   AND row_kind='right' AND row_id=$3 LIMIT 2",
                &[&domain, &consumer, &row.id, &(limits.max_row_bytes as i64)],
            )?;
            if prior.len() > 1
                || prior.first().is_some_and(|prior| {
                    prior.get::<_, String>(0).trim_end() != row.sha256.to_hex()
                        || prior
                            .get::<_, Option<String>>(1)
                            .as_deref()
                            .map(str::as_bytes)
                            != Some(row.raw.as_slice())
                })
            {
                return Err(DurableError::Conflict(
                    "navigation initial duplicate rights identity differs",
                ));
            }
        }
        let inserted = tx.execute(
            "INSERT INTO cmd2_projection_navigation_bootstrap_row(domain,consumer_id,row_kind,row_id,
                owner_source_path,owner_record_id,visible,from_id,to_id,row_json,row_sha256)
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
             ON CONFLICT(domain,consumer_id,row_kind,row_id,owner_source_path,owner_record_id) DO NOTHING",
            &[&domain, &consumer, &row.kind.as_str(), &row.id, &row.owner_source_path, &row.owner_record_id,
              &row.visible, &row.from_id, &row.to_id, &std::str::from_utf8(&row.raw)
                .map_err(|_| DurableError::Corrupt("navigation initial row UTF-8"))?, &row.sha256.to_hex()],
        )?;
        if inserted == 0 {
            // Only identical duplicate Original rights ordinals collapse. Every
            // other duplicate is a position/ownership defect, never a retry.
            if row.kind != DurableNavigationBootstrapRowKindV1::Right {
                return Err(DurableError::Conflict(
                    "navigation initial normalized row already exists",
                ));
            }
            let stored = tx.query_one(
                "SELECT row_sha256,CASE WHEN octet_length(row_json)::bigint<=$6 THEN row_json END
                 FROM cmd2_projection_navigation_bootstrap_row WHERE domain=$1 AND consumer_id=$2
                  AND row_kind='right' AND row_id=$3 AND owner_source_path=$4 AND owner_record_id=$5",
                &[&domain, &consumer, &row.id, &row.owner_source_path, &row.owner_record_id, &(limits.max_row_bytes as i64)],
            )?;
            if parse_digest(stored.get(0))? != row.sha256
                || stored
                    .get::<_, Option<String>>(1)
                    .as_deref()
                    .map(str::as_bytes)
                    != Some(row.raw.as_slice())
            {
                return Err(DurableError::Conflict(
                    "navigation initial duplicate rights bytes differ",
                ));
            }
        } else {
            add(&mut page.next.normalized_rows, 1)?;
            add(&mut page.next.normalized_bytes, row.raw.len() as u64)?;
            if row.kind == DurableNavigationBootstrapRowKindV1::Right {
                add(&mut page.next.projected_rights, 1)?;
            }
        }
    }
    let mut changed_owners = BTreeSet::new();
    for (path, id, dependency) in &page.dependencies {
        tx.execute(
            "INSERT INTO cmd2_projection_navigation_bootstrap_dependency(domain,consumer_id,
                owner_source_path,owner_record_id,dependency_path) VALUES($1,$2,$3,$4,$5)
             ON CONFLICT(domain,consumer_id,owner_source_path,owner_record_id,dependency_path) DO NOTHING",
            &[&domain, &consumer, path, id, dependency],
        )?;
        changed_owners.insert((path, id));
    }
    for (path, id) in changed_owners {
        let count = tx.query(
            "SELECT 1 FROM cmd2_projection_navigation_bootstrap_dependency
              WHERE domain=$1 AND consumer_id=$2 AND owner_source_path=$3 AND owner_record_id=$4 LIMIT $5",
            &[&domain, &consumer, path, id, &((limits.max_dependencies + 1) as i64)],
        )?.len();
        if count > limits.max_dependencies {
            return Err(DurableError::Refused(
                "navigation initial owner dependency limit",
            ));
        }
    }
    Ok(())
}

#[cfg(all(test, feature = "postgres-lab"))]
#[path = "bootstrap_normalize_pg_tests.rs"]
mod tests;
