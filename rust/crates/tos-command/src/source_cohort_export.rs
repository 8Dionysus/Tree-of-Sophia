//! Explicit initial/export delivery from an already assessed generation.
//! Only bounded metadata is retained; each body is released after its callback.
use super::*;

// Source delivery needs custody and file metadata. Inventory projections have
// their own verified catalogue visitor; including their bytea JSON encoding
// here can reject a valid source row before any exact bytes are exported.
const CURRENT_EXPORT_ROWS: &str = "SELECT domain,subject,revision,commit_seq,
    prepare_id,member_slot,profile_id,profile_version,content_digest,content_length,
    store_id,custody_domain_digest,custody_domain,pin_id,pin_fence,segment_digest,
    segment_size,frame_index,frame_header_offset,frame_digest,frame_length,
    sto_receipt_id,durability_class,source_mode,source_dependencies
    FROM cmd2_current WHERE domain=$1";
const INDEX_EXPORT_ROWS: &str = "SELECT kind,token,path,definition_digest
    FROM cmd2_source_index WHERE domain=$1 AND kind<>'path'";

pub(crate) struct SelectedSourceExportMetadata {
    pub metadata: BTreeMap<String, MemberMetadata>,
    pub dependencies: BTreeMap<String, Option<Vec<RelativePath>>>,
    pub indexes: Vec<(String, String, String)>,
    pub membership: SourceMembershipV1,
    pub source_bytes: u64,
}

impl DurablePgCoordinator {
    pub(crate) fn visit_selected_source_export(
        &mut self,
        store: &SegmentStore,
        generation: &crate::source_current_cut::ManagedCurrentSourceGeneration,
        limits: tos_source_store::CutReadLimits,
        max_metadata_bytes: u64,
        max_manifest_entries: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
        mut visit: impl FnMut(&MemberMetadata, &[u8]) -> DurableResult<()>,
    ) -> DurableResult<SelectedSourceExportMetadata> {
        active(deadline, cancelled)?;
        if !matches!(
            generation.selected(),
            SelectedSourceGeneration::Addressed(_)
        ) || limits.max_members == 0
            || limits.max_total_bytes == 0
            || limits.max_member_bytes == 0
            || max_metadata_bytes == 0
            || max_manifest_entries == 0
        {
            return Err(DurableError::Refused("selected source export envelope"));
        }
        let _custody = store.hold_audit_root()?;
        let mut tx = self
            .client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .start()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        held_generation_metadata(&mut tx, store, generation)?;
        let domain = generation.cohort().domain();
        let admitted = tx.query_one(
            &format!(
                "SELECT count(*),coalesce(sum(content_length),0)::bigint,
                    coalesce(max(content_length),0),
                    coalesce(sum(octet_length(row_to_json(c)::text)),0)::bigint,
                    coalesce(max(octet_length(row_to_json(c)::text)),0)
             FROM ({CURRENT_EXPORT_ROWS}) c"
            ),
            &[&domain],
        )?;
        let count = as_u64(admitted.get(0))?;
        let source_bytes = as_u64(admitted.get(1))?;
        let metadata_bytes = as_u64(admitted.get(3))?;
        if count != generation.member_count()
            || count > limits.max_members
            || source_bytes > limits.max_total_bytes
            || as_u64(admitted.get(2))? > limits.max_member_bytes
            || metadata_bytes > max_metadata_bytes
            || admitted.get::<_, i32>(4) > 1_048_576
        {
            return Err(DurableError::Refused(
                "selected source export exceeds admitted bounds",
            ));
        }
        let indexed = tx.query_one(
            &format!(
                "SELECT count(*),coalesce(sum(octet_length(row_to_json(i)::text)),0)::bigint
             FROM ({INDEX_EXPORT_ROWS}) i"
            ),
            &[&domain],
        )?;
        if as_u64(indexed.get(0))? > max_manifest_entries as u64
            || metadata_bytes
                .checked_add(as_u64(indexed.get(1))?)
                .is_none_or(|bytes| bytes > max_metadata_bytes)
        {
            return Err(DurableError::Refused(
                "selected source export metadata envelope",
            ));
        }
        let mut metadata = BTreeMap::new();
        let mut dependencies = BTreeMap::new();
        let mut after = String::new();
        let mut returned_bytes = 0u64;
        let mut manifest_entries = 0usize;
        let mut membership = Digest256Hasher::new();
        membership.update(b"tos-val-full-membership-v1\0");
        // Cache one bounded sealed attempt, not every source body or receipt.
        let mut previous_attempt = None;
        let mut receipts = Vec::new();
        loop {
            active(deadline, cancelled)?;
            let page = tx.query(
                &format!(
                    "SELECT c.*,a.attempt_fence AS source_attempt_fence,
                        a.state AS source_attempt_state,a.commit_seq AS source_attempt_seq
                 FROM ({CURRENT_EXPORT_ROWS}) c JOIN cmd2_attempt a USING(domain,prepare_id)
                 WHERE c.subject COLLATE \"C\">$2 COLLATE \"C\"
                 ORDER BY c.subject COLLATE \"C\" LIMIT 8"
                ),
                &[&domain, &after],
            )?;
            if page.is_empty() {
                break;
            }
            for row in page {
                active(deadline, cancelled)?;
                let name: String = row.get("subject");
                if name <= after
                    || metadata.len() as u64 >= count
                    || row.get::<_, String>("source_attempt_state") != "committed"
                    || row.get::<_, Option<i64>>("source_attempt_seq")
                        != Some(row.get("commit_seq"))
                {
                    return Err(DurableError::Corrupt(
                        "selected source export order/attempt differs",
                    ));
                }
                let path = RelativePath::parse(&name)
                    .map_err(|_| DurableError::Corrupt("selected export source path"))?;
                let selected = generation
                    .selected()
                    .lookup_current(domain, &path, deadline, cancelled)?
                    .ok_or(DurableError::Conflict("selected export member absent"))?;
                let member = selected_source_metadata(&row, &selected, domain, &path)?;
                let (_, claims) = expose_metadata(&row, &path)?;
                manifest_entries = manifest_entries
                    .checked_add(1)
                    .and_then(|n| n.checked_add(claims.as_ref().map_or(0, |refs| 1 + refs.len())))
                    .filter(|n| *n <= max_manifest_entries)
                    .ok_or(DurableError::Refused("selected export dependency envelope"))?;
                let attempt = (
                    row.get::<_, Vec<u8>>("prepare_id"),
                    as_u64(row.get("source_attempt_fence"))?,
                );
                if previous_attempt.as_ref() != Some(&attempt) {
                    receipts = match store.recover_attempt_fenced(&attempt.0, attempt.1, 0)? {
                        Some(AttemptRecovery::Sealed { receipts }) => receipts,
                        _ => {
                            return Err(DurableError::Corrupt("selected export intent not sealed"));
                        }
                    };
                    previous_attempt = Some(attempt);
                }
                let receipt = receipts
                    .iter()
                    .find(|r| r.receipt_id() == selected.placement.receipt_id())
                    .ok_or(DurableError::Corrupt("selected export receipt absent"))?;
                check_history_locator(&row, receipt, domain, &name, as_u64(row.get("revision"))?)?;
                if receipt.placement() != selected.placement {
                    return Err(DurableError::Corrupt("selected export placement differs"));
                }
                let mut raw = Vec::new();
                store.read_selected(receipt, limits.max_member_bytes, &mut raw)?;
                returned_bytes = returned_bytes
                    .checked_add(raw.len() as u64)
                    .filter(|n| *n <= limits.max_total_bytes)
                    .ok_or(DurableError::Refused("selected export byte budget"))?;
                if raw.len() as u64 != member.size_bytes
                    || Digest256::of_bytes(&raw) != member.sha256
                {
                    return Err(DurableError::Corrupt("selected export body fixity differs"));
                }
                tos_source_store::feed_source_membership_v1(
                    &mut membership,
                    &name,
                    member.size_bytes,
                    member.sha256,
                );
                visit(&member, &raw)?;
                after = name.clone();
                dependencies.insert(name.clone(), claims);
                metadata.insert(name, member);
            }
        }
        if metadata.len() as u64 != count || returned_bytes != source_bytes {
            return Err(DurableError::Corrupt(
                "selected export EOF coverage differs",
            ));
        }
        let mut indexes = Vec::new();
        let mut rows = tx.query_raw(
            &format!("{INDEX_EXPORT_ROWS} ORDER BY kind,token"),
            &[&domain],
        )?;
        while let Some(row) = rows.next()? {
            active(deadline, cancelled)?;
            let path: String = row.get(2);
            if !metadata.contains_key(&path)
                || row.get::<_, String>(3) != generation.agent_definition_digest().to_hex()
            {
                return Err(DurableError::Corrupt(
                    "selected export identity index differs",
                ));
            }
            manifest_entries = manifest_entries
                .checked_add(1)
                .filter(|n| *n <= max_manifest_entries)
                .ok_or(DurableError::Refused("selected export identity envelope"))?;
            indexes.push((row.get(0), row.get(1), path));
        }
        drop(rows);
        if indexes.len() as u64 != as_u64(indexed.get(0))? {
            return Err(DurableError::Corrupt(
                "selected export identity coverage differs",
            ));
        }
        held_generation_metadata(&mut tx, store, generation)?;
        active(deadline, cancelled)?;
        tx.commit()?;
        Ok(SelectedSourceExportMetadata {
            metadata,
            dependencies,
            indexes,
            source_bytes,
            membership: SourceMembershipV1 {
                count,
                digest: membership.finalize(),
            },
        })
    }
}
