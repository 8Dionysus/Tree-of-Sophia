//! Initial current-cut text projection, committed one source at a time. This
//! is independent of original compound event sizes and never publishes a prefix.
use super::*;

#[derive(Clone, Copy, Debug)]
pub struct DurableProjectionBootstrapLimits {
    pub max_sources: usize,
    /// Per source plus selected companions, custody, segment reads and output.
    pub source: DurableProjectionLimits,
    /// Aggregate charged bytes for this invocation, separate from per-source work.
    pub max_call_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DurableProjectionBootstrapProgress {
    pub completed: bool,
    pub through_seq: u64,
    pub selected_head_seq: u64,
    pub processed_sources: u64,
    pub applied_sources: usize,
    pub after_path: String,
    pub charged_work_bytes: u64,
    pub work: DurableProjectionWork,
}

pub(super) fn require_no_active_bootstrap<C: GenericClient>(
    tx: &mut C,
    domain: &str,
    consumer: &str,
) -> DurableResult<()> {
    // Existing projection consumers remain usable before an explicit derived
    // schema init adds this optional initial-build table. Without the table
    // there cannot be an unfinished build; no implicit DDL is performed here.
    if !tx
        .query_one(
            "SELECT to_regclass('cmd2_projection_text_bootstrap') IS NOT NULL",
            &[],
        )?
        .get::<_, bool>(0)
    {
        return Ok(());
    }
    if tx.query_opt(
        "SELECT 1 FROM cmd2_projection_text_bootstrap WHERE domain=$1 AND consumer_id=$2 AND NOT completed",
        &[&domain, &consumer],
    )?.is_some() {
        return Err(DurableError::Conflict("initial text bootstrap is unfinished"));
    }
    Ok(())
}

fn checked_state(
    tx: &mut Transaction<'_>,
    domain: &str,
    consumer: &str,
    profile: &DurableProjectionProfile,
) -> DurableResult<(DomainSnapshot, String, i64, bool)> {
    let snapshot = lock_domain_snapshot(tx, domain)?;
    snapshot.require_live_projection(profile)?;
    if snapshot.head_seq == 0 {
        return Err(DurableError::Refused(
            "initial text bootstrap requires committed sources",
        ));
    }
    let row = tx.query_one(
        "SELECT * FROM cmd2_projection_consumer WHERE domain=$1 AND consumer_id=$2 FOR UPDATE",
        &[&domain, &consumer],
    )?;
    let state = ConsumerState::from_row(&row);
    if !state.matches_profile(consumer, profile) {
        return Err(DurableError::Conflict(
            "projection consumer profile differs",
        ));
    }
    let prior = tx.query_opt(
        "SELECT snapshot_json,after_path,processed_sources,completed FROM cmd2_projection_text_bootstrap WHERE domain=$1 AND consumer_id=$2",
        &[&domain, &consumer],
    )?;
    match prior {
        Some(row) => {
            let raw: String = row.get(0);
            let expected: DomainSnapshot = serde_json::from_str(&raw)
                .map_err(|_| DurableError::Corrupt("initial text bootstrap fence is invalid"))?;
            let completed: bool = row.get(3);
            if expected != snapshot
                || state.cursor_seq != if completed { snapshot.head_seq } else { 0 }
            {
                return Err(DurableError::Conflict(
                    "initial text bootstrap cut changed; select a new consumer",
                ));
            }
            Ok((snapshot, row.get(1), row.get(2), completed))
        }
        None => {
            if state.cursor_seq != 0 || tx.query_opt(
                "SELECT 1 FROM cmd2_projection_document WHERE domain=$1 AND consumer_id=$2 LIMIT 1",
                &[&domain, &consumer],
            )?.is_some() || state.source_text_search_profile.as_deref() != Some(durable_source_text_search::SOURCE_TEXT_SEARCH_PROFILE)
                || state.source_text_search_profile_version != Some(durable_source_text_search::SOURCE_TEXT_SEARCH_PROFILE_VERSION) {
                return Err(DurableError::Conflict("initial text bootstrap requires an unused current-profile consumer"));
            }
            let raw = serde_json::to_string(&snapshot)
                .map_err(|_| DurableError::Invalid("initial text bootstrap fence serialization"))?;
            tx.execute(
                "INSERT INTO cmd2_projection_text_bootstrap(domain,consumer_id,snapshot_json) VALUES($1,$2,$3)",
                &[&domain, &consumer, &raw],
            )?;
            Ok((snapshot, String::new(), 0, false))
        }
    }
}

impl DurablePgCoordinator {
    /// Resume initial projection into an unused consumer. Every source commits
    /// documents and its keyset position atomically, but reads remain Pending
    /// until EOF atomically publishes the captured head. A source/rights/schema
    /// fence change refuses continuation; no completed prefix is discarded.
    pub fn bootstrap_projection_sources<E: DurableProjectionExtractor + ?Sized>(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        consumer: &str,
        profile: &DurableProjectionProfile,
        extractor: &E,
        limits: DurableProjectionBootstrapLimits,
    ) -> DurableResult<DurableProjectionBootstrapProgress> {
        let source_limits = limits.source.validate()?;
        if !(1..=256).contains(&limits.max_sources)
            || limits.max_call_bytes < source_limits.max_event_bytes
            || limits.max_call_bytes > i64::MAX as u64
        {
            return Err(DurableError::Invalid("initial text bootstrap limits"));
        }
        validate_identifier(domain, MAX_DOMAIN_BYTES, "projection domain")?;
        validate_identifier(consumer, MAX_CONSUMER_ID_BYTES, "projection consumer ID")?;
        profile.validate()?;
        if store.custody_domain() != domain.as_bytes() {
            return Err(DurableError::Conflict("STO custody domain differs"));
        }
        let deadline = Instant::now() + source_limits.statement_timeout;
        ensure_consumer(
            &mut self.client,
            domain,
            consumer,
            profile,
            source_limits.statement_timeout,
        )?;
        let mut tx = self.client.transaction()?;
        set_local_timeouts(&mut tx, source_limits.statement_timeout)?;
        let (snapshot, mut after_path, mut processed, mut completed) =
            checked_state(&mut tx, domain, consumer, profile)?;
        tx.commit()?;
        let mut work = DurableProjectionWork::default();
        let mut applied_sources = 0;
        let mut charged_work_bytes = 0u64;
        while !completed && applied_sources < limits.max_sources {
            if Instant::now() >= deadline {
                break;
            }
            let remaining = limits.max_call_bytes.saturating_sub(charged_work_bytes);
            if remaining < source_limits.max_event_bytes {
                break;
            }
            // Fetch exactly one current address, using the existing C-order
            // subject index. No payload, OFFSET or whole-corpus map is retained.
            let mut tx = self.client.transaction()?;
            set_local_timeouts(&mut tx, source_limits.statement_timeout)?;
            let now = lock_domain_snapshot(&mut tx, domain)?;
            if now != snapshot {
                return Err(DurableError::Conflict(
                    "initial text bootstrap source fence changed",
                ));
            }
            let row = tx.query_opt(
                "SELECT left(subject,4097) AS subject,octet_length(subject) AS path_bytes,
                    revision,commit_seq,content_length,source_mode
                 FROM cmd2_current WHERE domain=$1 AND subject COLLATE \"C\">$2 COLLATE \"C\"
                 ORDER BY subject COLLATE \"C\" LIMIT 1",
                &[&domain, &after_path],
            )?;
            tx.commit()?;
            let mut staged = StagedEvent::default();
            let mut charged = 0;
            let path = if let Some(row) = row {
                let path: String = row.get("subject");
                if row.get::<_, i32>("path_bytes") as usize != path.len() {
                    return Err(DurableError::Refused(
                        "initial source path exceeds byte limit",
                    ));
                }
                validate_relative_path(&path)?;
                if row.get::<_, Option<i32>>("source_mode").is_some() {
                    let version = ProjectionSourceVersion {
                        revision: as_u64(row.get("revision"))?,
                        commit_seq: as_u64(row.get("commit_seq"))?,
                        content_length: as_u64(row.get("content_length"))?,
                        is_source: true,
                    };
                    staged = self.stage_bootstrap_source(
                        store,
                        domain,
                        &path,
                        version,
                        snapshot.head_seq()?,
                        profile,
                        extractor,
                        source_limits,
                        deadline,
                        &mut charged,
                    )?;
                }
                Some(path)
            } else {
                None
            };
            if Instant::now() >= deadline {
                return Err(DurableError::Refused(
                    "initial text bootstrap deadline exceeded; committed progress retained",
                ));
            }
            let mut tx = self.client.transaction()?;
            set_local_timeouts(&mut tx, source_limits.statement_timeout)?;
            let (now, actual_after, actual_processed, already_complete) =
                checked_state(&mut tx, domain, consumer, profile)?;
            if now != snapshot
                || actual_after != after_path
                || actual_processed != processed
                || already_complete
            {
                return Err(DurableError::Conflict(
                    "initial text bootstrap position changed",
                ));
            }
            if let Some(path) = path {
                write_staged_documents(&mut tx, domain, consumer, &staged, source_limits)?;
                processed = processed
                    .checked_add(1)
                    .ok_or(DurableError::Refused("initial text source count overflow"))?;
                tx.execute(
                    "UPDATE cmd2_projection_text_bootstrap SET after_path=$3,processed_sources=$4 WHERE domain=$1 AND consumer_id=$2",
                    &[&domain, &consumer, &path, &processed],
                )?;
                tx.commit()?;
                staged.work.published_source_paths = staged.replaced_source_paths.len() as u64;
                staged.work.published_documents = staged.documents.len() as u64;
                work.add(&staged.work)?;
                after_path = path;
                applied_sources += 1;
                charged_work_bytes = charged_work_bytes
                    .checked_add(charged)
                    .ok_or(DurableError::Refused("initial text work count overflow"))?;
            } else {
                tx.execute(
                    "UPDATE cmd2_projection_consumer SET cursor_seq=$3,observed_head_seq=$3,
                        observed_rights_version=$4,observed_rights_allowed=$5,observed_rule_version=$6,
                        observed_contract_digest=$7,observed_schema_profile_digest=$8,
                        observed_source_generation=$9,observed_source_complete=$10
                     WHERE domain=$1 AND consumer_id=$2 AND cursor_seq=0",
                    &[&domain, &consumer, &snapshot.head_seq, &snapshot.rights_version,
                      &snapshot.rights_allowed, &snapshot.rule_version, &snapshot.contract_digest,
                      &snapshot.schema_profile_digest, &snapshot.source_generation, &snapshot.source_complete],
                )?;
                tx.execute(
                    "UPDATE cmd2_projection_text_bootstrap SET completed=true WHERE domain=$1 AND consumer_id=$2",
                    &[&domain, &consumer],
                )?;
                tx.commit()?;
                completed = true;
            }
        }
        Ok(DurableProjectionBootstrapProgress {
            completed,
            through_seq: if completed { snapshot.head_seq()? } else { 0 },
            selected_head_seq: snapshot.head_seq()?,
            processed_sources: as_u64(processed)?,
            applied_sources,
            after_path,
            charged_work_bytes,
            work,
        })
    }

    fn stage_bootstrap_source<E: DurableProjectionExtractor + ?Sized>(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        path: &str,
        version: ProjectionSourceVersion,
        head: u64,
        profile: &DurableProjectionProfile,
        extractor: &E,
        limits: DurableProjectionLimits,
        deadline: Instant,
        charged: &mut u64,
    ) -> DurableResult<StagedEvent> {
        let mut staged = StagedEvent::default();
        add_projection_bytes(
            charged,
            version
                .content_length
                .checked_add(path.len() as u64)
                .ok_or(DurableError::Refused("initial source byte overflow"))?,
            limits,
        )?;
        let root = read_projection_source(
            self,
            store,
            domain,
            path,
            version.revision,
            version.content_length,
            version.commit_seq,
            limits,
            true,
            deadline,
            charged,
            &mut staged.work,
        )?;
        let requested =
            extractor.required_source_paths(path, version.revision, &root.exact_source_bytes)?;
        if requested.len() > limits.max_dependencies.saturating_sub(1)
            || (profile.version != 2 && !requested.is_empty())
        {
            return Err(DurableError::Refused(
                "initial source companion limit exceeded",
            ));
        }
        let mut selected = BTreeSet::from([path.to_owned()]);
        let mut inputs = Vec::new();
        let mut versions = BTreeMap::new();
        for dependency in requested {
            validate_relative_path(&dependency)?;
            if selected.contains(&dependency) {
                continue;
            }
            let version = projection_source_version_at_event(
                &mut self.client,
                domain,
                &dependency,
                head,
                limits,
                &mut versions,
            )?;
            let Some(version) = version.filter(|v| v.is_source) else {
                continue;
            };
            add_projection_bytes(
                charged,
                version
                    .content_length
                    .checked_add(dependency.len() as u64)
                    .ok_or(DurableError::Refused("initial companion byte overflow"))?,
                limits,
            )?;
            inputs.push(read_projection_source(
                self,
                store,
                domain,
                &dependency,
                version.revision,
                version.content_length,
                version.commit_seq,
                limits,
                true,
                deadline,
                charged,
                &mut staged.work,
            )?);
            selected.insert(dependency);
        }
        staged.work.examined_event_members = 1;
        staged.work.examined_dependency_paths = inputs.len() as u64;
        let documents = extractor.extract_with_sources(
            path,
            root.revision,
            &root.exact_source_bytes,
            &inputs,
        )?;
        validate_extracted_documents(
            &mut staged,
            path,
            root.revision,
            documents,
            profile.version,
            &selected,
            limits,
            charged,
        )?;
        staged.replaced_source_paths.push(path.to_owned());
        Ok(staged)
    }
}
