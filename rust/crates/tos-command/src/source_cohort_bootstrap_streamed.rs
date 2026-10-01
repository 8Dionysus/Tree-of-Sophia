//! Fresh, incomplete source bootstrap. Semantic completion belongs to the
//! shared streamed assessor; writing ORIGINAL bodies never grants completeness.
use super::*;
use tos_source_store::{StreamedCorpusCutReaderV1, StreamedSourceMemberV1};

const BATCH_BODY_BYTES: usize = 32 * 1024 * 1024;
const MEMBER_BYTES: u64 = 8 * 1024 * 1024;

/// Only this module can mint a seed, after genuine original EOF and the actual
/// durable commits. The finishing assessor consumes it and rechecks its fence.
pub(super) struct FreshSourceBootstrapV1 {
    observed: FreshSourceBootstrapObservationV1,
}

pub(super) struct FreshSourceBootstrapObservationV1 {
    pub domain: String,
    pub store_id: [u8; 16],
    pub revision: SourceRevision,
    pub membership: SourceMembershipV1,
    pub authored_members: u64,
    pub head: u64,
    pub audit: u64,
    pub contract: Digest256,
}

impl FreshSourceBootstrapV1 {
    pub(super) fn observation(&self) -> &FreshSourceBootstrapObservationV1 {
        &self.observed
    }
}

fn member_metadata(
    original: &StreamedCorpusCutReaderV1,
    member: &StreamedSourceMemberV1,
    limit: usize,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> DurableResult<SourceFileMetadata> {
    let dependencies = if original
        .has_dependency_index(member.revision, &member.path)
        .map_err(|_| DurableError::Refused("bootstrap dependency index"))?
    {
        let mut values = Vec::new();
        let mut after = None;
        let mut bytes = 0usize;
        while let Some(path) = original
            .dependency_after(member.revision, &member.path, after.as_ref())
            .map_err(|_| DurableError::Refused("bootstrap dependency cursor"))?
        {
            active(deadline, cancelled)?;
            bytes = bytes
                .checked_add(path.as_str().len())
                .and_then(|n| n.checked_add(std::mem::size_of::<String>()))
                .filter(|n| *n <= limit)
                .ok_or(DurableError::Refused("bootstrap member dependency bound"))?;
            values.push(path.as_str().to_owned());
            after = Some(path);
        }
        Some(values)
    } else {
        None
    };
    Ok(SourceFileMetadata {
        mode: member.mode,
        dependencies,
    })
}

impl DurablePgCoordinator {
    /// Bootstrap a fresh managed domain without materializing the whole source.
    /// ORIGINAL writes remain incomplete until the shared semantic assessor
    /// verifies their projections and selects a genuine post-write generation.
    pub fn bootstrap_source_cohort_streamed(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        original: &StreamedCorpusCutReaderV1,
        revision: SourceRevision,
        membership: SourceMembershipV1,
        context: &CommandContext,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        worker: &mut CutWorkerSchemaExecutor,
        contract: Digest256,
        max_rows: u64,
        max_source_bytes: u64,
        max_dependency_bytes_per_member: usize,
        schema_source_path: &str,
        workspace: &super::PrivateGenerationWorkspace,
        generation_profile: super::StreamedGenerationProfile,
        source_limits: super::source_cohort_streamed::StreamedColdSourceLimitsV1,
        item_limits: tos_validation::item_rules::ItemLimits,
        tree_limits: tos_segment_store::AuthenticatedTreeLimitsV1,
        body_work: &mut super::source_cohort_streamed::StreamedColdSourceWorkV1,
        tree_work: &mut tos_segment_store::AuthenticatedTreeWorkV1,
        deadline: Instant,
        cancelled: std::sync::Arc<AtomicBool>,
    ) -> DurableResult<crate::source_current_cut::ManagedCurrentSourceGeneration> {
        active(deadline, &cancelled)?;
        source_limits.index_limits()?;
        let generation_profile = generation_profile.validate(workspace)?;
        let seed = self.seed_fresh_source_bootstrap_streamed(
            store,
            domain,
            original,
            revision,
            membership,
            context,
            software,
            components,
            contract,
            max_rows,
            max_source_bytes,
            max_dependency_bytes_per_member,
            generation_profile.max_commit_seq,
            body_work,
            deadline,
            &cancelled,
        )?;
        self.complete_fresh_source_bootstrap_streamed(
            store,
            seed,
            original,
            context,
            software,
            components,
            worker,
            schema_source_path,
            workspace,
            generation_profile,
            source_limits,
            item_limits,
            tree_limits,
            body_work,
            tree_work,
            deadline,
            cancelled,
        )
    }

    /// Caller supplies an authenticated V1 cursor and explicit whole input
    /// ceilings. At most one existing command-sized batch is retained. No
    /// source_complete bit, owner index, predicate or projection is fabricated.
    fn seed_fresh_source_bootstrap_streamed(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        original: &StreamedCorpusCutReaderV1,
        revision: SourceRevision,
        membership: SourceMembershipV1,
        context: &CommandContext,
        software: &SoftwareCaptureReader,
        components: &SoftwareComponentSelectionV1,
        contract: Digest256,
        max_rows: u64,
        max_source_bytes: u64,
        max_dependency_bytes_per_member: usize,
        max_commit_seq: u64,
        body_work: &mut super::source_cohort_streamed::StreamedColdSourceWorkV1,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<FreshSourceBootstrapV1> {
        active(deadline, cancelled)?;
        // The authenticated reader checked these ceilings while building its
        // index. Refuse an incompatible profile before creating a domain;
        // do not rescan every member merely to repeat that proof.
        let input_limits = original.content_limits();
        let selected = original
            .revision_at(0)
            .map_err(|_| DurableError::Refused("bootstrap original revision"))?
            .ok_or(DurableError::Refused("bootstrap original absent"))?;
        if store.custody_domain() != domain.as_bytes()
            || context.base_revision != revision
            || selected.revision != revision
            || selected.membership != membership
            || max_commit_seq == 0
            || max_commit_seq > i64::MAX as u64
            || max_rows == 0
            || max_rows == u64::MAX
            || selected.member_count > max_rows
            || max_source_bytes == 0
            || max_source_bytes == u64::MAX
            || max_dependency_bytes_per_member == 0
            || max_dependency_bytes_per_member == usize::MAX
            || input_limits.max_total_bytes > max_source_bytes
            || input_limits.max_member_bytes > MEMBER_BYTES
        {
            return Err(DurableError::Refused(
                "bootstrap independent input selection",
            ));
        }
        context
            .check_selected_software_inputs(software, components, deadline, cancelled)
            .map_err(source_error)?;
        self.create_domain(domain, contract)?;
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        lock_audit_fence(&mut tx, domain)?;
        let row = tx.query_one(
            "SELECT * FROM cmd2_domain WHERE domain=$1 FOR UPDATE",
            &[&domain],
        )?;
        let occupied: bool = tx.query_one(
            "SELECT EXISTS(SELECT 1 FROM cmd2_attempt WHERE domain=$1 UNION ALL SELECT 1 FROM cmd2_current WHERE domain=$1 UNION ALL SELECT 1 FROM cmd2_history WHERE domain=$1 UNION ALL SELECT 1 FROM cmd2_source_index WHERE domain=$1 UNION ALL SELECT 1 FROM cmd2_predicate WHERE domain=$1)",
            &[&domain],
        )?.get(0);
        if occupied
            || row.get::<_, i64>("head_seq") != 0
            || row.get::<_, Option<String>>("source_revision").is_some()
            || row.get::<_, String>("contract_digest") != contract.to_hex()
            || row.get::<_, Option<String>>("schema_profile_digest")
                != Some(schema_profile_digest().to_hex())
            || row.get::<_, i64>("rule_version") != 0
            || row.get::<_, i64>("rights_version") != 0
            || !row.get::<_, bool>("rights_allowed")
        {
            return Err(DurableError::Refused(
                "bootstrap requires fresh authorized domain",
            ));
        }
        tx.execute("UPDATE cmd2_domain SET source_revision=$2,source_membership_digest=$3,source_membership_count=$4,source_epoch=1,source_generation=0,source_complete=false,source_definition_digest=$5 WHERE domain=$1",
            &[&domain, &revision.0.to_hex(), &membership.digest.to_hex(), &as_i64(membership.count)?, &definition().to_hex()])?;
        tx.commit()?;
        self.set_job_epoch(domain, "source-bootstrap", 1)?;

        let mut stream = original
            .stream_with_member_limit(revision, MEMBER_BYTES)
            .map_err(|_| DurableError::Refused("bootstrap original stream"))?;
        let mut files = Vec::new();
        let mut metadata = SourceMetadata::new();
        let mut batch_bytes = 0usize;
        let mut total_bytes = 0u64;
        let mut authored_members = 0u64;
        let mut batch = 0u64;
        let mut head = 0u64;
        while let Some(member) = stream
            .next_member(deadline, cancelled)
            .map_err(|_| DurableError::Refused("bootstrap original EOF/fixity"))?
        {
            ManagedSourceWorkV1::charge(&mut body_work.original_bodies_returned, 1)?;
            ManagedSourceWorkV1::charge(
                &mut body_work.original_body_bytes_returned,
                member.raw.len() as u64,
            )?;
            total_bytes = total_bytes
                .checked_add(member.size_bytes)
                .filter(|n| *n <= max_source_bytes)
                .ok_or(DurableError::Refused("bootstrap whole source byte bound"))?;
            if !member.path.as_str().starts_with("ToS/") {
                continue;
            }
            if files.len() == MAX_MEMBERS || batch_bytes + member.raw.len() > BATCH_BODY_BYTES {
                head = self.commit_streamed_bootstrap_batch(
                    store,
                    domain,
                    revision,
                    contract,
                    batch,
                    head,
                    &files,
                    &metadata,
                    max_commit_seq,
                    deadline,
                    cancelled,
                )?;
                batch = batch
                    .checked_add(1)
                    .ok_or(DurableError::Refused("bootstrap batch overflow"))?;
                files.clear();
                metadata.clear();
                batch_bytes = 0;
            }
            let value = member_metadata(
                original,
                &member,
                max_dependency_bytes_per_member,
                deadline,
                cancelled,
            )?;
            metadata.insert(member.path.as_str().to_owned(), value);
            batch_bytes += member.raw.len();
            files.push(SourceFile {
                path: member.path,
                raw: member.raw,
            });
            authored_members = authored_members
                .checked_add(1)
                .filter(|n| *n <= max_rows)
                .ok_or(DurableError::Refused("bootstrap authored row bound"))?;
        }
        if stream.coverage() != Some(membership) {
            return Err(DurableError::Refused("bootstrap incomplete original EOF"));
        }
        if !files.is_empty() {
            head = self.commit_streamed_bootstrap_batch(
                store,
                domain,
                revision,
                contract,
                batch,
                head,
                &files,
                &metadata,
                max_commit_seq,
                deadline,
                cancelled,
            )?;
        }
        let mut tx = self.client.transaction()?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='15s'")?;
        let audit = lock_audit_fence(&mut tx, domain)?;
        let row = tx.query_one(
            "SELECT * FROM cmd2_domain WHERE domain=$1 FOR UPDATE",
            &[&domain],
        )?;
        if as_u64(row.get("head_seq"))? != head
            || row.get::<_, bool>("source_complete")
            || !row.get::<_, bool>("rights_allowed")
            || row.get::<_, i64>("rights_version") != 0
            || row.get::<_, i64>("rule_version") != 0
            || row.get::<_, String>("contract_digest") != contract.to_hex()
            || row.get::<_, Option<String>>("source_revision") != Some(revision.0.to_hex())
            || row.get::<_, Option<String>>("source_membership_digest")
                != Some(membership.digest.to_hex())
            || row.get::<_, Option<i64>>("source_membership_count")
                != Some(as_i64(membership.count)?)
            || row.get::<_, Option<i64>>("source_epoch") != Some(1)
            || row.get::<_, Option<String>>("source_definition_digest")
                != Some(definition().to_hex())
            || row.get::<_, Option<String>>("schema_profile_digest")
                != Some(schema_profile_digest().to_hex())
        {
            return Err(DurableError::Conflict("bootstrap completion fence changed"));
        }
        active(deadline, cancelled)?;
        tx.commit()?;
        Ok(FreshSourceBootstrapV1 {
            observed: FreshSourceBootstrapObservationV1 {
                domain: domain.to_owned(),
                store_id: store.store_id(),
                revision,
                membership,
                authored_members,
                head,
                audit,
                contract,
            },
        })
    }

    fn commit_streamed_bootstrap_batch(
        &mut self,
        store: &SegmentStore,
        domain: &str,
        revision: SourceRevision,
        contract: Digest256,
        batch: u64,
        head: u64,
        files: &[SourceFile],
        metadata: &SourceMetadata,
        max_commit_seq: u64,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> DurableResult<u64> {
        active(deadline, cancelled)?;
        // Refuse before registering or sealing a batch that cannot commit.
        // The transactional check below remains authoritative against drift.
        if head >= max_commit_seq {
            return Err(DurableError::Refused("bootstrap commit sequence bound"));
        }
        let prepare = format!("source-bootstrap:{batch}").into_bytes();
        let identities = files
            .iter()
            .enumerate()
            .map(|(slot, f)| ShadowWriteIdentity {
                member_slot: slot as u32,
                subject: f.path.as_str(),
                expected_predecessor: None,
                proposed_revision: 1,
                exact_bytes: &f.raw,
            })
            .collect::<Vec<_>>();
        let fence = self.register_attempt(&RegisterShadowAttempt {
            domain,
            prepare_id: &prepare,
            command_id: std::str::from_utf8(&prepare).unwrap(),
            raw_request_digest: revision.0,
            delta_digest: durable_shadow_delta_prepared(&identities),
        })?;
        let members = seal_members(store, &prepare, fence, ORIGINAL, &identities)?;
        self.attach_ready_profile(
            store,
            domain,
            &prepare,
            fence,
            &members,
            ORIGINAL,
            None,
            Some(metadata),
        )?;
        let receipts = members
            .iter()
            .map(|m| m.receipt.clone())
            .collect::<Vec<_>>();
        let (receipt, _) = self.commit_durable_with_sequence_limit(
            store,
            &CommitShadowAttempt {
                domain,
                prepare_id: &prepare,
                attempt_fence: fence,
                receipts: &receipts,
                expected_contract_digest: contract,
                expected_rule_version: 0,
                expected_rights_version: 0,
                job_id: "source-bootstrap",
                job_fence: 1,
                full_base_seq: head,
            },
            CommitMode::Bootstrap,
            max_commit_seq,
        )?;
        active(deadline, cancelled)?;
        Ok(receipt.commit_seq)
    }
}
