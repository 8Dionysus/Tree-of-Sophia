//! Source-backed initial closure. Reconstruct every derived relation in one
//! repeatable-read cut; persisted progress is never an admission credential.
use super::*;

pub(super) struct Closure {
    pub rows: u64,
    pub bytes: u64,
    pub rights: u64,
    pub work: u64,
}

fn require_same_set(
    tx: &mut Transaction<'_>,
    domain: &str,
    consumer: &str,
    expected: &str,
    actual: &str,
) -> DurableResult<()> {
    // Both queries are fixed owner code, never caller-provided SQL.
    let sql = format!(
        "SELECT EXISTS (({expected} EXCEPT {actual}) UNION ALL ({actual} EXCEPT {expected}))"
    );
    if tx.query_one(&sql, &[&domain, &consumer])?.get::<_, bool>(0) {
        return Err(DurableError::Conflict(
            "navigation initial derived relation differs from Original",
        ));
    }
    Ok(())
}

pub(super) fn verify_derived(
    tx: &mut Transaction<'_>,
    domain: &str,
    consumer: &str,
    limits: DurableNavigationEventLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<Closure> {
    tx.batch_execute(
        "CREATE TEMP TABLE tos_initial_verify_contributor (
        domain text,consumer_id text,node_id text,owner_source_path text,owner_record_id text,
        PRIMARY KEY(domain,consumer_id,node_id,owner_source_path,owner_record_id)) ON COMMIT DROP;
        CREATE TEMP TABLE tos_initial_verify_dependency (
        owner_source_path text,owner_record_id text,dependency_path text,
        PRIMARY KEY(owner_source_path,owner_record_id,dependency_path)) ON COMMIT DROP",
    )?;
    let mut work = 0;
    let mut position = Position {
        phase: "node_facts".into(),
        ..Default::default()
    };
    while position.phase != "normalize_nodes" {
        let (next, bytes) = normalize::metadata_page_to(
            normalize::MetadataTarget::Verification,
            tx,
            domain,
            consumer,
            &position,
            limits,
            deadline,
            cancelled,
        )?;
        add(&mut work, bytes)?;
        position = next;
    }
    require_same_set(
        tx,
        domain,
        consumer,
        "SELECT node_id,owner_source_path,owner_record_id FROM pg_temp.tos_initial_verify_contributor WHERE domain=$1 AND consumer_id=$2",
        "SELECT node_id,owner_source_path,owner_record_id FROM cmd2_projection_navigation_initial_contributor WHERE domain=$1 AND consumer_id=$2",
    )?;
    let mut rows = 0;
    let mut bytes = 0;
    // Every expected key is checked against exact bytes and all derived fields.
    // Original node/edge identities are unique; repeated rights ordinals are
    // handled separately below. Exact cardinality excludes unexpected keys.
    while position.phase != "normalized" {
        let page =
            normalize::prepare_page(tx, domain, consumer, &position, limits, deadline, cancelled)?;
        add(&mut work, page.work)?;
        for expected in &page.rows {
            live(deadline, cancelled)?;
            let actual = tx
                .query_opt(
                    "SELECT active,visible,from_id,to_id,row_sha256,
                CASE WHEN octet_length(row_json)::bigint=$7 THEN row_json END
                FROM cmd2_projection_navigation_bootstrap_row WHERE domain=$1 AND consumer_id=$2
                AND row_kind=$3 AND row_id=$4 AND owner_source_path=$5 AND owner_record_id=$6",
                    &[
                        &domain,
                        &consumer,
                        &expected.kind.as_str(),
                        &expected.id,
                        &expected.owner_source_path,
                        &expected.owner_record_id,
                        &(expected.raw.len() as i64),
                    ],
                )?
                .ok_or(DurableError::Conflict(
                    "navigation initial normalized carrier missing",
                ))?;
            let raw = actual.get::<_, Option<String>>(5);
            if !actual.get::<_, bool>(0)
                || actual.get::<_, Option<bool>>(1) != expected.visible
                || actual.get::<_, Option<String>>(2) != expected.from_id
                || actual.get::<_, Option<String>>(3) != expected.to_id
                || parse_digest(actual.get(4))? != expected.sha256
                || raw.as_deref().map(str::as_bytes) != Some(expected.raw.as_slice())
            {
                return Err(DurableError::Conflict(
                    "navigation initial normalized carrier differs from Original",
                ));
            }
            add(&mut work, expected.raw.len() as u64)?;
            if expected.kind != DurableNavigationBootstrapRowKindV1::Right {
                add(&mut rows, 1)?;
                add(&mut bytes, expected.raw.len() as u64)?;
            }
        }
        for (path, id, dependency) in &page.dependencies {
            tx.execute("INSERT INTO pg_temp.tos_initial_verify_dependency VALUES($1,$2,$3) ON CONFLICT DO NOTHING",
                &[path, id, dependency])?;
        }
        position = page.next;
    }
    // Equal duplicate rights carriers normalize to the same key. The checks
    // above establish that key and bytes for every Original ordinal.
    let rights = as_u64(
        tx.query_one(
            "SELECT count(DISTINCT row_id) FROM cmd2_projection_navigation_initial_row
        WHERE domain=$1 AND consumer_id=$2 AND collection='rights'",
            &[&domain, &consumer],
        )?
        .get(0),
    )?;
    let stored_rights = tx.query_one("SELECT count(*),COALESCE(sum(octet_length(row_json)),0)::bigint
        FROM cmd2_projection_navigation_bootstrap_row WHERE domain=$1 AND consumer_id=$2 AND row_kind='right'", &[&domain, &consumer])?;
    if as_u64(stored_rights.get(0))? != rights {
        return Err(DurableError::Conflict(
            "navigation initial rights cardinality differs",
        ));
    }
    add(&mut rows, rights)?;
    add(&mut bytes, as_u64(stored_rights.get(1))?)?;
    if as_u64(
        tx.query_one(
            "SELECT count(*) FROM cmd2_projection_navigation_bootstrap_row
        WHERE domain=$1 AND consumer_id=$2",
            &[&domain, &consumer],
        )?
        .get(0),
    )? != rows
    {
        return Err(DurableError::Conflict(
            "navigation initial normalized cardinality differs",
        ));
    }
    require_same_set(
        tx,
        domain,
        consumer,
        "SELECT owner_source_path,owner_record_id,dependency_path FROM pg_temp.tos_initial_verify_dependency",
        "SELECT owner_source_path,owner_record_id,dependency_path FROM cmd2_projection_navigation_bootstrap_dependency WHERE domain=$1 AND consumer_id=$2",
    )?;
    require_same_set(
        tx,
        domain,
        consumer,
        "SELECT DISTINCT dependency_path FROM pg_temp.tos_initial_verify_dependency",
        "SELECT owner_source_path FROM cmd2_projection_navigation_bootstrap_owner WHERE domain=$1 AND consumer_id=$2",
    )?;
    tx.batch_execute(
        "DROP TABLE pg_temp.tos_initial_verify_contributor,pg_temp.tos_initial_verify_dependency",
    )?;
    Ok(Closure {
        rows,
        bytes,
        rights,
        work,
    })
}

/// Cursor carries bounded metadata only. Raw carriers are fetched one at a
/// time with an SQL length guard; even a maximum-sized row fits the page cap.
pub(super) fn root<P: ManagedSelectedProof>(
    tx: &mut Transaction<'_>,
    store: &SegmentStore,
    domain: &str,
    consumer: &str,
    selected: &ManagedAgentSelectedParent<P>,
    binding: &DurableNavigationCurrentIndexOwnerBindingV1,
    header: &[u8],
    limits: DurableNavigationEventLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<(Digest256, u64, u64, u64)> {
    let mut hash = NavigationBootstrapRootHasher::new(binding, header, limits.max_row_bytes)?;
    let mut work = 0;
    let mut sources = 0;
    let mut source_bytes = 0;
    tx.execute("DECLARE tos_initial_sources NO SCROLL CURSOR FOR
        SELECT owner_source_path,source_sha256,size_bytes FROM cmd2_projection_navigation_bootstrap_owner
        WHERE domain=$1 AND consumer_id=$2 ORDER BY owner_source_path COLLATE \"C\"", &[&domain, &consumer])?;
    loop {
        live(deadline, cancelled)?;
        let page = tx.query("FETCH 128 FROM tos_initial_sources", &[])?;
        if page.is_empty() {
            break;
        }
        for row in page {
            let path: String = row.get(0);
            validate_relative_path(&path)?;
            let relative = RelativePath::parse(&path)
                .map_err(|_| DurableError::Corrupt("initial owner path"))?;
            let member = crate::source_cohort::read_generation_source_metadata_in_tx(
                tx,
                store,
                selected.generation(),
                &relative,
                deadline,
                cancelled,
            )?
            .ok_or(DurableError::Conflict(
                "initial owner outside selected generation",
            ))?;
            let source = DurableNavigationBootstrapOwnerSourceV1 {
                source_path: path,
                sha256: parse_digest(row.get(1))?,
                size_bytes: as_u64(row.get(2))?,
            };
            if source.sha256 != member.sha256
                || source.size_bytes != member.size_bytes
                || source.size_bytes == 0
            {
                return Err(DurableError::Conflict(
                    "navigation initial source witness differs from selected placement",
                ));
            }
            add(&mut work, source.source_path.len() as u64 + 72)?;
            add(&mut sources, 1)?;
            add(&mut source_bytes, source.size_bytes)?;
            hash.source(&source)?;
        }
    }
    tx.batch_execute("CLOSE tos_initial_sources")?;
    add(
        &mut work,
        hash_rows(tx, domain, consumer, &mut hash, limits, deadline, cancelled)?,
    )?;
    Ok((hash.finish(), work, sources, source_bytes))
}

pub(super) fn hash_rows(
    tx: &mut Transaction<'_>,
    domain: &str,
    consumer: &str,
    hash: &mut NavigationBootstrapRootHasher,
    limits: DurableNavigationEventLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<u64> {
    let mut work = 0;
    for kind in [
        DurableNavigationBootstrapRowKindV1::Node,
        DurableNavigationBootstrapRowKindV1::Edge,
        DurableNavigationBootstrapRowKindV1::Right,
        DurableNavigationBootstrapRowKindV1::Diagnostic,
    ] {
        tx.execute("DECLARE tos_initial_rows NO SCROLL CURSOR FOR
            SELECT row_id,owner_source_path,owner_record_id,octet_length(row_json)::bigint
            FROM cmd2_projection_navigation_bootstrap_row WHERE domain=$1 AND consumer_id=$2 AND row_kind=$3
            ORDER BY row_id COLLATE \"C\",owner_source_path COLLATE \"C\",owner_record_id COLLATE \"C\"",
            &[&domain, &consumer, &kind.as_str()])?;
        loop {
            live(deadline, cancelled)?;
            let page = tx.query("FETCH 128 FROM tos_initial_rows", &[])?;
            if page.is_empty() {
                break;
            }
            for meta in page {
                live(deadline, cancelled)?;
                let id: String = meta.get(0);
                let path: String = meta.get(1);
                let owner: String = meta.get(2);
                let size: i64 = meta.get(3);
                if as_u64(size)? > limits.max_row_bytes as u64 {
                    return Err(DurableError::Refused("navigation initial root row size"));
                }
                let row = tx.query_one(
                    "SELECT visible,from_id,to_id,row_sha256,
                    CASE WHEN octet_length(row_json)::bigint=$7 THEN row_json END
                    FROM cmd2_projection_navigation_bootstrap_row WHERE domain=$1 AND consumer_id=$2
                    AND row_kind=$3 AND row_id=$4 AND owner_source_path=$5 AND owner_record_id=$6",
                    &[
                        &domain,
                        &consumer,
                        &kind.as_str(),
                        &id,
                        &path,
                        &owner,
                        &size,
                    ],
                )?;
                let raw = row
                    .get::<_, Option<String>>(4)
                    .ok_or(DurableError::Corrupt("initial root row changed"))?
                    .into_bytes();
                let sha256 = parse_digest(row.get(3))?;
                if Digest256::of_bytes(&raw) != sha256 {
                    return Err(DurableError::Corrupt("initial root row digest"));
                }
                add(&mut work, raw.len() as u64)?;
                hash.row(&DurableNavigationBaseRowReadV1 {
                    kind,
                    id,
                    owner_source_path: path,
                    owner_record_id: owner,
                    active: true,
                    source_refs: Vec::new(),
                    visible: row.get(0),
                    from_id: row.get(1),
                    to_id: row.get(2),
                    raw,
                    sha256,
                })?;
            }
        }
        tx.batch_execute("CLOSE tos_initial_rows")?;
    }
    Ok(work)
}

fn node_certificate(
    tx: &mut Transaction<'_>,
    domain: &str,
    consumer: &str,
    id: &str,
    root: Digest256,
    limits: DurableNavigationEventLimits,
) -> DurableResult<(Option<NavigationAdjacencyCertificate>, u64, usize, u64)> {
    let (facts, mut work) = normalize::node_facts(tx, domain, consumer, id)?
        .ok_or(DurableError::Corrupt("initial adjacency node absent"))?;
    if !facts.visible {
        return Ok((None, work, 0, 0));
    }
    let mut outgoing = BTreeMap::new();
    let mut raw_bytes = 0;
    let mut inspected = 0;
    let mut after = String::new();
    loop {
        let rows = tx.query(
            "SELECT row_id,row_sha256,octet_length(row_json)::bigint
            FROM cmd2_projection_navigation_bootstrap_row
            WHERE domain=$1 AND consumer_id=$2 AND row_kind='edge' AND active AND visible
              AND from_id=$3 AND row_id COLLATE \"C\">$4 COLLATE \"C\"
            ORDER BY row_id COLLATE \"C\" LIMIT 128",
            &[&domain, &consumer, &id, &after],
        )?;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            let edge_id: String = row.get(0);
            let digest = parse_digest(row.get(1))?;
            inspected += 1;
            add(&mut raw_bytes, as_u64(row.get(2))?)?;
            add(&mut work, edge_id.len() as u64 + 72)?;
            if inspected > limits.max_adjacency_rows
                || raw_bytes > limits.max_adjacency_bytes
                || work > limits.max_total_bytes
            {
                return Err(DurableError::Refused("initial per-node adjacency envelope"));
            }
            after = edge_id.clone();
            if outgoing
                .insert(edge_id, digest)
                .is_some_and(|old| old != digest)
            {
                return Err(DurableError::Corrupt(
                    "initial adjacency duplicate edge differs",
                ));
            }
        }
    }
    Ok((
        Some(make_navigation_adjacency_certificate(
            root,
            id.into(),
            &outgoing,
        )),
        work,
        inspected,
        raw_bytes,
    ))
}

pub(super) fn adjacency_page(
    tx: &mut Transaction<'_>,
    domain: &str,
    consumer: &str,
    position: &Position,
    root: Digest256,
    limits: DurableNavigationEventLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<(Position, u64)> {
    let mut next = position.clone();
    let mut work = 0;
    let mut edge_rows = 0;
    let mut edge_bytes = 0;
    let ids = tx.query("SELECT row_key FROM cmd2_projection_navigation_initial_row
        WHERE domain=$1 AND consumer_id=$2 AND collection='nodes' AND row_key COLLATE \"C\">$3 COLLATE \"C\"
        ORDER BY row_key COLLATE \"C\" LIMIT $4", &[&domain, &consumer, &position.after, &(PAGE_ROWS.min(limits.max_cohort_rows) as i64)])?;
    if ids.is_empty() {
        next.phase = "publish".into();
        next.after.clear();
    }
    for row in ids {
        live(deadline, cancelled)?;
        let id: String = row.get(0);
        let (certificate, bytes, rows, raw_bytes) =
            node_certificate(tx, domain, consumer, &id, root, limits)?;
        add(&mut work, bytes)?;
        if work > limits.max_total_bytes {
            return Err(DurableError::Refused(
                "initial adjacency transaction metadata bytes",
            ));
        }
        if edge_rows + rows > limits.max_adjacency_rows
            || edge_bytes + raw_bytes > limits.max_adjacency_bytes
        {
            break;
        }
        edge_rows += rows;
        edge_bytes += raw_bytes;
        if let Some(c) = certificate {
            tx.execute("INSERT INTO cmd2_projection_navigation_adjacency_certificate
                (domain,consumer_id,node_id,edge_count,edge_digest,certificate_root) VALUES($1,$2,$3,$4,$5,$6)",
                &[&domain, &consumer, &c.node_id, &as_i64(c.edge_count)?, &c.edge_digest.to_hex(), &c.certificate_root.to_hex()])?;
        }
        next.after = id;
    }
    add(&mut next.work_bytes, work)?;
    Ok((next, work))
}

pub(super) fn verify_adjacency(
    tx: &mut Transaction<'_>,
    domain: &str,
    consumer: &str,
    root: Digest256,
    limits: DurableNavigationEventLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<(Digest256, u64)> {
    let mut hash = Digest256Hasher::new();
    hash.update(b"tos-source-navigation-adjacency-root-v1");
    navigation_hash_part(&mut hash, root.to_hex().as_bytes());
    let mut after = String::new();
    let mut count = 0;
    let mut work = 0;
    loop {
        live(deadline, cancelled)?;
        let ids = tx.query("SELECT row_key FROM cmd2_projection_navigation_initial_row
            WHERE domain=$1 AND consumer_id=$2 AND collection='nodes' AND row_key COLLATE \"C\">$3 COLLATE \"C\"
            ORDER BY row_key COLLATE \"C\" LIMIT 128", &[&domain, &consumer, &after])?;
        if ids.is_empty() {
            break;
        }
        for row in ids {
            live(deadline, cancelled)?;
            let id: String = row.get(0);
            let (expected, bytes, _, _) =
                node_certificate(tx, domain, consumer, &id, root, limits)?;
            add(&mut work, bytes)?;
            if let Some(c) = expected {
                let row = tx.query_opt("SELECT edge_count,edge_digest,certificate_root FROM cmd2_projection_navigation_adjacency_certificate
                    WHERE domain=$1 AND consumer_id=$2 AND node_id=$3", &[&domain, &consumer, &id])?
                    .ok_or(DurableError::Conflict("initial adjacency certificate missing"))?;
                if as_u64(row.get(0))? != c.edge_count
                    || parse_digest(row.get(1))? != c.edge_digest
                    || parse_digest(row.get(2))? != c.certificate_root
                {
                    return Err(DurableError::Conflict(
                        "initial adjacency certificate differs",
                    ));
                }
                navigation_hash_part(&mut hash, c.node_id.as_bytes());
                navigation_hash_part(&mut hash, &c.edge_count.to_be_bytes());
                navigation_hash_part(&mut hash, c.edge_digest.as_bytes());
                navigation_hash_part(&mut hash, c.certificate_root.as_bytes());
                add(&mut count, 1)?;
            }
            after = id;
        }
    }
    if as_u64(tx.query_one("SELECT count(*) FROM cmd2_projection_navigation_adjacency_certificate WHERE domain=$1 AND consumer_id=$2",
        &[&domain, &consumer])?.get(0))? != count {
        return Err(DurableError::Conflict("initial adjacency cardinality differs"));
    }
    Ok((hash.finalize(), work))
}

pub(super) fn publish(
    tx: &mut Transaction<'_>,
    domain: &str,
    consumer: &str,
    profile: &DurableProjectionProfile,
    binding: &DurableNavigationCurrentIndexOwnerBindingV1,
    header: &[u8],
    root: Digest256,
    adjacency_root: Digest256,
    limits: DurableNavigationEventLimits,
) -> DurableResult<()> {
    let through = binding.managed_generation_commit_sequence;
    let revision = binding.selected_membership_root;
    let binding_json = serde_json::to_string(&navigation_owner_binding_json(binding))
        .map_err(|_| DurableError::Invalid("initial binding encoding"))?;
    for table in [
        "cmd2_projection_navigation_cohort",
        "cmd2_projection_navigation_coverage_event",
        "cmd2_projection_navigation_coverage_range",
    ] {
        if tx
            .query_opt(
                &format!("SELECT 1 FROM {table} WHERE domain=$1 AND consumer_id=$2 LIMIT 1"),
                &[&domain, &consumer],
            )?
            .is_some()
        {
            return Err(DurableError::Conflict(
                "initial consumer has incremental events",
            ));
        }
    }
    cohort_tree::initialize(tx, domain, consumer)?;
    let cohort_root = navigation_current_cohort_root_in_tx(tx, domain, consumer, limits)?;
    let event = format!("bootstrap:{}", root.to_hex());
    let checkpoint = navigation_verified_prefix_digest(
        domain,
        consumer,
        profile,
        through,
        root,
        &binding_json,
        through,
        root,
        cohort_root,
        adjacency_root,
        &event,
        revision,
        root,
        root,
    );
    if tx.execute("UPDATE cmd2_projection_navigation_bootstrap SET bootstrap_root=$3,owner_binding_json=$4,
        projected_right_count=$5,through_seq=$6,source_revision=$7,header_raw=$8,header_sha256=$9,
        node_count=$10,edge_count=$11,right_count=$12,diagnostic_count=0 WHERE domain=$1 AND consumer_id=$2",
        &[&domain, &consumer, &root.to_hex(), &binding_json, &as_i64(binding.navigation_projected_rights)?,
          &as_i64(through)?, &revision.to_hex(), &std::str::from_utf8(header).map_err(|_| DurableError::Corrupt("initial header UTF-8"))?,
          &Digest256::of_bytes(header).to_hex(), &as_i64(binding.navigation_nodes)?, &as_i64(binding.navigation_edges)?, &as_i64(binding.navigation_rights)?])? != 1 {
        return Err(DurableError::Conflict("initial bootstrap metadata absent"));
    }
    if tx.execute("UPDATE cmd2_projection_navigation_consumer SET cursor_seq=$3,source_revision=$4,last_event_id=$5,
        last_input_root=$6,last_output_root=$6,coverage_seq=$3,coverage_root=$6,
        adjacency_seq=$3,adjacency_root=$7,adjacency_ready=true,verified_prefix_version=$10,
        verified_prefix_seq=$3,verified_prefix_coverage_root=$6,verified_prefix_cohort_root=$8,verified_prefix_digest=$9
        WHERE domain=$1 AND consumer_id=$2 AND cursor_seq=0 AND coverage_seq=0 AND NOT adjacency_ready",
        &[&domain, &consumer, &as_i64(through)?, &revision.to_hex(), &event, &root.to_hex(),
          &adjacency_root.to_hex(), &cohort_root.to_hex(), &checkpoint.to_hex(), &VERIFIED_PREFIX_FORMAT_VERSION])? != 1 {
        return Err(DurableError::Conflict("initial publication consumer compare-and-set failed"));
    }
    Ok(())
}

/// A replay uses the existing verified prefix and exact selected binding. It
/// does not require staging that a prior successful invocation has retired.
pub(super) fn verify_published(
    tx: &mut Transaction<'_>,
    domain: &str,
    consumer: &str,
    profile: &DurableProjectionProfile,
    selected: &SelectedCapture,
    position: &Position,
    limits: DurableNavigationEventLimits,
) -> DurableResult<()> {
    let row = tx.query_one(
        "SELECT * FROM cmd2_projection_navigation_consumer
        WHERE domain=$1 AND consumer_id=$2 FOR UPDATE",
        &[&domain, &consumer],
    )?;
    let bootstrap = tx.query_one(
        "SELECT through_seq,bootstrap_root,
        CASE WHEN octet_length(owner_binding_json)<=1048576 THEN owner_binding_json END
        FROM cmd2_projection_navigation_bootstrap WHERE domain=$1 AND consumer_id=$2",
        &[&domain, &consumer],
    )?;
    let binding_json = bootstrap
        .get::<_, Option<String>>(2)
        .ok_or(DurableError::Corrupt("initial published binding size"))?;
    let mut binding = navigation_owner_binding_from_json(&selected.binding)?;
    binding.navigation_projected_rights = position.projected_rights;
    let expected_json = serde_json::to_string(&navigation_owner_binding_json(&binding))
        .map_err(|_| DurableError::Invalid("initial binding JSON"))?;
    let root = parse_digest(bootstrap.get(1))?;
    let through = as_u64(bootstrap.get(0))?;
    if binding_json != expected_json
        || position.bootstrap_root.as_deref() != Some(root.to_hex().as_str())
        || through != binding.managed_generation_commit_sequence
        || as_u64(row.get("cursor_seq"))? < through
    {
        return Err(DurableError::Conflict("initial published baseline differs"));
    }
    verify_navigation_verified_prefix_in_tx(
        tx,
        domain,
        consumer,
        profile,
        through,
        root,
        &binding_json,
        &row,
        limits,
    )
}

pub(super) fn cleanup_page(
    tx: &mut Transaction<'_>,
    domain: &str,
    consumer: &str,
    limits: DurableNavigationEventLimits,
) -> DurableResult<bool> {
    let cap = limits.max_cohort_rows.min(4096) as i64;
    let removed = tx.execute("WITH page AS (SELECT node_id,owner_source_path,owner_record_id
        FROM cmd2_projection_navigation_initial_contributor WHERE domain=$1 AND consumer_id=$2
        ORDER BY node_id COLLATE \"C\",owner_source_path COLLATE \"C\",owner_record_id COLLATE \"C\" LIMIT $3)
        DELETE FROM cmd2_projection_navigation_initial_contributor r USING page p
        WHERE r.domain=$1 AND r.consumer_id=$2 AND r.node_id=p.node_id
          AND r.owner_source_path=p.owner_source_path AND r.owner_record_id=p.owner_record_id", &[&domain, &consumer, &cap])?;
    if removed > 0 {
        return Ok(false);
    }
    let removed = tx.execute("WITH page AS (SELECT collection,row_key FROM cmd2_projection_navigation_initial_row
        WHERE domain=$1 AND consumer_id=$2 ORDER BY collection COLLATE \"C\",row_key COLLATE \"C\" LIMIT $3)
        DELETE FROM cmd2_projection_navigation_initial_row r USING page p WHERE r.domain=$1 AND r.consumer_id=$2
        AND r.collection=p.collection AND r.row_key=p.row_key", &[&domain, &consumer, &cap])?;
    Ok(removed == 0)
}
